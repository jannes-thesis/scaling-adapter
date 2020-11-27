#![allow(clippy::clippy::mutex_atomic)]
#![allow(dead_code)]
use std::{
    collections::{HashSet, VecDeque},
    sync::{
        atomic::AtomicUsize,
        atomic::{self, AtomicBool},
        Arc, Condvar, Mutex,
    },
    thread,
    time::Duration,
};

use log::debug;

use crate::{get_pid, Job, Threadpool};

enum WorkItem {
    Execute(Job),
    Clone,
}

pub struct WatermarkThreadpool {
    job_queue: Mutex<VecDeque<WorkItem>>,
    workers: Mutex<HashSet<i32>>,
    busy_workers_count: Mutex<usize>,
    // used to signal blocked wait handler
    all_workers_idle: Condvar,
    // used to signal blocked destroy handler
    all_workers_exited: Condvar,
    // used to signal blocked (on empty job_queue) workers
    queue_non_empty: Condvar,
    is_stopping: AtomicBool,
    next_worker_id: AtomicUsize,
    idle_threshold: Duration,
    min_size: usize,
    max_size: usize,
    // used to more cheaply track current size
    current_size: AtomicUsize,
}

impl Threadpool for WatermarkThreadpool {
    fn submit_job(&self, job: Job) {
        let mut job_queue = self.job_queue.lock().unwrap();
        let queue_size = job_queue.len();
        job_queue.push_back(WorkItem::Execute(job));
        // in case queue was empty all workers may be blocked, wake up one
        if queue_size == 0 {
            self.queue_non_empty.notify_one();
        } else if queue_size > 5 {
            let previous_size = self.current_size.fetch_add(1, atomic::Ordering::Relaxed);
            if previous_size >= self.max_size {
                self.current_size.fetch_sub(1, atomic::Ordering::Relaxed);
            }
            // if not at max size yet, let one worker clone
            else {
                job_queue.push_back(WorkItem::Clone);
            }
        }
    }

    fn wait_completion(&self) {
        debug!("wait for completion");
        let mut busy_count = self.busy_workers_count.lock().unwrap();
        // wait until no workers are active anymore and queue is empty
        // HOLDING 2 LOCKS HERE 1. busy count 2. job_queue
        // safe as long as it is the only place where 2 locks are grabbed simultaneously
        while *busy_count > 0 || !self.job_queue.lock().unwrap().is_empty() {
            busy_count = self.all_workers_idle.wait(busy_count).unwrap();
        }
    }

    fn destroy(&self) {
        debug!("destroy");
        self.is_stopping.store(true, atomic::Ordering::Relaxed);
        // workers are either a. completing last job b. blocked on empty queue
        // wake up one blocked worker, so it can exit and notify others
        self.queue_non_empty.notify_one();
        let mut workers = self.workers.lock().unwrap();
        while workers.len() > 0 {
            debug!("some workers still active, wait on exit condition variable");
            workers = self.all_workers_exited.wait(workers).unwrap();
        }
    }
}

fn worker_loop(threadpool: Arc<WatermarkThreadpool>) {
    let worker_pid = get_pid();
    debug!("worker startup, pid: {}", worker_pid);
    {
        let mut workers = threadpool.workers.lock().unwrap();
        workers.insert(worker_pid);
    }
    'outer: loop {
        // lock queue, get item, unlock queue
        let mut work_item = threadpool.job_queue.lock().unwrap().pop_front();
        while work_item.is_none() && !threadpool.is_stopping() {
            // relock queue
            let work_queue_guard = threadpool.job_queue.lock().unwrap();
            // release queue and block on signal, reaquire on return
            let (mut work_queue_reaquired, wait_result) = threadpool
                .queue_non_empty
                .wait_timeout(work_queue_guard, threadpool.idle_threshold)
                .unwrap();
            // if wait timed out, idle threshold is passed, so terminate worker
            if wait_result.timed_out() {
                // decrement size
                let size_before_terminate = threadpool
                    .current_size
                    .fetch_sub(1, atomic::Ordering::Relaxed);
                // if new size is below min size, revert and don't terminate
                if size_before_terminate <= threadpool.min_size {
                    threadpool
                        .current_size
                        .fetch_add(1, atomic::Ordering::Relaxed);
                } else {
                    break 'outer;
                }
            }
            // get next work item and release queue
            work_item = work_queue_reaquired.pop_front();
        }
        if threadpool.is_stopping() {
            threadpool
                .current_size
                .fetch_sub(1, atomic::Ordering::Relaxed);
            // signal other potentially blocked workers, so they can exit
            threadpool.queue_non_empty.notify_one();
            break;
        }
        let mut busy_count = threadpool.busy_workers_count.lock().unwrap();
        *busy_count += 1;
        drop(busy_count);

        // if threadpool is not stopping, work item option can't be none
        match work_item.unwrap() {
            WorkItem::Execute(job) => job.execute(),
            WorkItem::Clone => threadpool.clone().spawn_worker(),
        }

        let mut busy_count = threadpool.busy_workers_count.lock().unwrap();
        *busy_count -= 1;
        if !threadpool.is_stopping()
            && *busy_count == 0
            && threadpool.job_queue.lock().unwrap().is_empty()
        {
            threadpool.all_workers_idle.notify_one();
        }
    }
    let mut workers = threadpool.workers.lock().unwrap();
    debug!("worker terminating, pid: {}", worker_pid);
    workers.remove(&worker_pid);
    debug!("remaining workers that are still active: {}", workers.len());
    if workers.len() == 0 {
        // signal caller that is blocked in destroy handler
        threadpool.all_workers_exited.notify_one();
    }
}

impl WatermarkThreadpool {
    pub fn new(min_size: usize, max_size: usize, idle_threshold: Duration) -> Arc<Self> {
        let threadpool = Arc::new(WatermarkThreadpool {
            job_queue: Mutex::new(VecDeque::with_capacity(5000)),
            workers: Mutex::new(HashSet::new()),
            busy_workers_count: Mutex::new(0),
            all_workers_idle: Condvar::new(),
            all_workers_exited: Condvar::new(),
            queue_non_empty: Condvar::new(),
            is_stopping: AtomicBool::new(false),
            next_worker_id: AtomicUsize::new(0),
            idle_threshold,
            min_size,
            max_size,
            current_size: AtomicUsize::new(min_size),
        });
        for _ in 0..min_size {
            threadpool.clone().spawn_worker();
        }
        threadpool
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
    use std::time::Duration;

    use super::*;

    #[test]
    fn create() {
        let pool = WatermarkThreadpool::new(5, 10, Duration::from_secs(10));
        thread::sleep(Duration::from_millis(500));
        assert!(pool.workers.lock().unwrap().len() == 5);
        assert!(*pool.busy_workers_count.lock().unwrap() == 0);
    }
}
