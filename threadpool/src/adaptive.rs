#![allow(clippy::clippy::mutex_atomic)]
#![allow(dead_code)]
use std::{
    collections::HashSet,
    sync::{atomic, atomic::AtomicUsize, Arc, Condvar, Mutex, RwLock},
    thread,
    time::Duration,
};

use crossbeam::queue::SegQueue;
use log::debug;
use scaling_adapter::ScalingAdapter;

use crate::{get_pid, Job, Threadpool};

#[derive(Eq, PartialEq)]
enum State {
    Stopping,
    Active,
}

enum WorkItem {
    Execute(Job),
    Terminate,
    Clone,
}

pub struct AdaptiveThreadpool {
    work_queue: SegQueue<WorkItem>,
    workers: Mutex<HashSet<i32>>,
    busy_workers_count: Mutex<usize>,
    all_idle_cond: Condvar,
    all_exit_cond: Condvar,
    state: RwLock<State>,
    scaling_adapter: Mutex<ScalingAdapter>,
    next_worker_id: AtomicUsize,
}

impl Threadpool for AdaptiveThreadpool {
    fn submit_job(&self, job: Job) {
        self.work_queue.push(WorkItem::Execute(job));
    }

    fn wait_completion(&self) {
        debug!("wait for completion");
        let mut busy_count = self.busy_workers_count.lock().unwrap();
        while *busy_count > 0 {
            busy_count = self.all_idle_cond.wait(busy_count).unwrap();
        }
    }

    fn destroy(&self) {
        debug!("destroy");
        // self.wait_completion();
        {
            let mut state = self.state.write().unwrap();
            *state = State::Stopping;
        }
        let mut workers = self.workers.lock().unwrap();
        while workers.len() > 0 {
            debug!("some workers still active, wait on exit condition variable");
            workers = self.all_exit_cond.wait(workers).unwrap();
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
        let work_item = threadpool.work_queue.pop();
        threadpool.adapt_size();
        if work_item.is_none() && !threadpool.is_stopping() {
            thread::sleep(Duration::from_millis(1000));
            threadpool.adapt_size();
            continue;
        }
        if threadpool.is_stopping() {
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
                if !threadpool.is_stopping() && *busy_count == 0 && threadpool.work_queue.is_empty()
                {
                    threadpool.all_idle_cond.notify_one();
                }
            }
            WorkItem::Clone => {
                debug!("clone command: spawning new worker");
                threadpool.clone().spawn_worker();
            }
            WorkItem::Terminate => {
                let amount_workers = threadpool.workers.lock().unwrap().len();
                // only terminate self if not the last worker
                if amount_workers > 1 {
                    debug!("terminate command: worker {}", worker_pid);
                    break;
                }
            }
        }
    }
    let mut workers = threadpool.workers.lock().unwrap();
    debug!("worker terminating, pid: {}", worker_pid);
    workers.remove(&worker_pid);
    if workers.len() == 0 {
        threadpool.all_exit_cond.notify_all();
    }
}

impl AdaptiveThreadpool {
    pub fn new(scaling_adapter: ScalingAdapter) -> Arc<Self> {
        Arc::new(AdaptiveThreadpool {
            work_queue: SegQueue::new(),
            workers: Mutex::new(HashSet::new()),
            busy_workers_count: Mutex::new(0),
            all_idle_cond: Condvar::new(),
            all_exit_cond: Condvar::new(),
            state: RwLock::new(State::Active),
            scaling_adapter: Mutex::new(scaling_adapter),
            next_worker_id: AtomicUsize::new(0),
        })
    }

    fn adapt_size(&self) {
        let mut to_scale = self.scaling_adapter.lock().unwrap().get_scaling_advice();
        debug!("got scaling advice: {}", to_scale);
        let current_size = self.workers.lock().unwrap().len() as i32;
        //
        if current_size + to_scale < 1 {
            to_scale = -(current_size - 1);
        }
        match to_scale.cmp(&0) {
            std::cmp::Ordering::Greater => {
                for _ in 0..to_scale {
                    self.work_queue.push(WorkItem::Clone);
                }
            }
            std::cmp::Ordering::Less => {
                for _ in to_scale..0 {
                    self.work_queue.push(WorkItem::Terminate);
                }
            }
            std::cmp::Ordering::Equal => {}
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
        *self.state.read().unwrap() == State::Stopping
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
