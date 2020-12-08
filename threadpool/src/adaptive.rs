#![allow(clippy::clippy::mutex_atomic)]
#![allow(dead_code)]
use std::{
    collections::{HashSet, VecDeque},
    sync::{atomic, atomic::AtomicBool, atomic::AtomicUsize, Arc, Condvar, Mutex},
    thread,
    time::Duration,
};

use log::{debug, info};
use scaling_adapter::ScalingAdapter;

use crate::{get_pid, Job, Threadpool};

#[derive(Eq, PartialEq)]
enum State {
    Stopping,
    Active,
}

#[derive(Copy, Clone)]
enum ScaleCommand {
    Terminate,
    Clone,
}

enum WorkItem {
    Execute(Job),
    ScaleCommand(ScaleCommand),
}

pub struct AdaptiveThreadpool {
    work_queue: Mutex<VecDeque<WorkItem>>,
    workers: Mutex<HashSet<i32>>,
    busy_workers_count: Mutex<usize>,
    // used to signal blocked wait handler
    all_workers_idle: Condvar,
    // used to signal blocked destroy handler
    all_workers_exited: Condvar,
    // used to signal blocked (on empty work_queue) workers
    work_queue_non_empty: Condvar,
    is_stopping: AtomicBool,
    scaling_adapter: Mutex<ScalingAdapter>,
    next_worker_id: AtomicUsize,
}

impl Threadpool for AdaptiveThreadpool {
    fn submit_job(&self, job: Job) {
        let mut work_queue = self.work_queue.lock().unwrap();
        let was_empty = work_queue.is_empty();
        work_queue.push_back(WorkItem::Execute(job));
        // in case queue was empty, all workers may be blocked, wake up one
        if was_empty {
            self.work_queue_non_empty.notify_one();
        }
    }

    fn wait_completion(&self) {
        debug!("wait for completion");
        let mut busy_count = self.busy_workers_count.lock().unwrap();
        while *busy_count > 0 {
            busy_count = self.all_workers_idle.wait(busy_count).unwrap();
        }
    }

    fn destroy(&self) {
        debug!("destroy");
        self.is_stopping.store(true, atomic::Ordering::Relaxed);
        // workers are either a. completing last job b. blocked on empty queue
        // wake up one blocked worker, so it can exit and notify others
        self.work_queue_non_empty.notify_one();
        let mut workers = self.workers.lock().unwrap();
        while workers.len() > 0 {
            debug!("some workers still active, wait on exit condition variable");
            workers = self.all_workers_exited.wait(workers).unwrap();
        }
    }
}

fn worker_loop(threadpool: Arc<AdaptiveThreadpool>) {
    let worker_pid = get_pid();
    debug!("worker startup, pid: {}", worker_pid);
    {
        let mut workers = threadpool.workers.lock().unwrap();
        workers.insert(worker_pid);
    }
    {
        let mut adapter = threadpool.scaling_adapter.lock().unwrap();
        if !adapter.add_tracee(worker_pid) {
            panic!("worker {} could not add itself as tracee", worker_pid);
        }
    }
    loop {
        threadpool.adapt_size();
        // lock queue, get item, unlock queue
        let mut work_queue_guard = threadpool.work_queue.lock().unwrap();
        let queue_size = work_queue_guard.len();
        let mut work_item = work_queue_guard.pop_front();
        drop(work_queue_guard);
        info!("_I_QSIZE: {}", queue_size);
        while work_item.is_none() && !threadpool.is_stopping() {
            // relock queue
            let work_queue_guard = threadpool.work_queue.lock().unwrap();
            // release queue and block on signal, reaquire on return
            let (mut work_queue_reaquired, wait_result) = threadpool
                .work_queue_non_empty
                .wait_timeout(work_queue_guard, Duration::from_millis(1000))
                .unwrap();
            // get next work item and release queue
            work_item = work_queue_reaquired.pop_front();
            drop(work_queue_reaquired);
            if wait_result.timed_out() {
                threadpool.adapt_size();
            }
        }
        if threadpool.is_stopping() {
            threadpool.workers.lock().unwrap().remove(&worker_pid);
            break;
        }
        let work_item = work_item.unwrap();
        match work_item {
            WorkItem::Execute(job) => {
                let mut busy_count = threadpool.busy_workers_count.lock().unwrap();
                *busy_count += 1;
                drop(busy_count);
                job.execute();

                let mut busy_count = threadpool.busy_workers_count.lock().unwrap();
                *busy_count -= 1;
                if !threadpool.is_stopping()
                    && *busy_count == 0
                    && threadpool.work_queue.lock().unwrap().is_empty()
                {
                    threadpool.all_workers_idle.notify_one();
                }
            }
            WorkItem::ScaleCommand(ScaleCommand::Clone) => {
                debug!("clone command: spawning new worker");
                threadpool.clone().spawn_worker();
            }
            WorkItem::ScaleCommand(ScaleCommand::Terminate) => {
                let mut workers = threadpool.workers.lock().unwrap();
                let amount_workers = workers.len();
                // only terminate self if not the last worker
                if amount_workers > 1 {
                    debug!("terminate command: worker {}", worker_pid);
                    workers.remove(&worker_pid);
                    break;
                }
            }
        }
    }
    debug!("worker terminating, pid: {}", worker_pid);
    if threadpool.workers.lock().unwrap().len() == 0 {
        threadpool.all_workers_exited.notify_all();
    }
}

impl AdaptiveThreadpool {
    pub fn new(scaling_adapter: ScalingAdapter) -> Arc<Self> {
        let thread_pool = Arc::new(AdaptiveThreadpool {
            work_queue: Mutex::new(VecDeque::new()),
            workers: Mutex::new(HashSet::new()),
            busy_workers_count: Mutex::new(0),
            all_workers_idle: Condvar::new(),
            all_workers_exited: Condvar::new(),
            work_queue_non_empty: Condvar::new(),
            is_stopping: AtomicBool::new(false),
            scaling_adapter: Mutex::new(scaling_adapter),
            next_worker_id: AtomicUsize::new(0),
        });
        thread_pool.clone().spawn_worker();
        thread_pool
    }

    fn adapt_size(&self) {
        let mut to_scale = self.scaling_adapter.lock().unwrap().get_scaling_advice();
        debug!("got scaling advice: {}", to_scale);
        let current_size = self.workers.lock().unwrap().len() as i32;
        //
        if current_size + to_scale < 1 {
            to_scale = -(current_size - 1);
        }
        let (scale_command, n) = match to_scale.cmp(&0) {
            std::cmp::Ordering::Greater => (ScaleCommand::Clone, to_scale),
            std::cmp::Ordering::Less => (ScaleCommand::Terminate, -to_scale),
            std::cmp::Ordering::Equal => {
                // scale_command is garbage here
                (ScaleCommand::Clone, 0)
            }
        };
        if n > 0 {
            let mut work_queue = self.work_queue.lock().unwrap();
            for _ in 0..n {
                work_queue.push_front(WorkItem::ScaleCommand(scale_command));
            }
        }
    }

    fn spawn_worker(self: Arc<Self>) {
        let worker_id = self.next_worker_id.fetch_add(1, atomic::Ordering::Relaxed);
        let name = format!("worker-{}", worker_id);
        let builder = thread::Builder::new().name(name.to_owned());
        let _handle: thread::JoinHandle<()> = builder
            .spawn(move || {
                worker_loop(self);
            })
            .unwrap_or_else(|_| panic!("thread creation for worker: {} failed", name));
    }

    fn is_stopping(&self) -> bool {
        self.is_stopping.load(atomic::Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use scaling_adapter::{IntervalData, IntervalDerivedData, ScalingParameters};

    use super::*;

    pub fn written_bytes_per_ms(interval_data: &IntervalData) -> IntervalDerivedData {
        let duration_ms = interval_data.end_millis() - interval_data.start_millis();
        // conversion to f64 precise for durations under 1000 years for sure
        // conversion to f64 precise for under a petabyte of written bytes
        let write_bytes_per_ms = (interval_data.write_bytes as f64) / (duration_ms as f64);
        IntervalDerivedData {
            scale_metric: write_bytes_per_ms,
            reset_metric: write_bytes_per_ms,
        }
    }

    #[test]
    fn create() {
        let adapter_params = ScalingParameters::new(vec![1, 2], Box::new(written_bytes_per_ms));
        let adapter = ScalingAdapter::new(adapter_params).expect("adapter creation failed");
        let pool = AdaptiveThreadpool::new(adapter);
        pool.wait_completion();
        pool.destroy();
    }
}
