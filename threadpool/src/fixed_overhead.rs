#![allow(clippy::clippy::mutex_atomic)]
#![allow(dead_code)]
use std::{
    collections::{HashSet, VecDeque},
    sync::{
        atomic::{self, AtomicBool},
        Arc, Condvar, Mutex,
    },
    thread,
};

use log::debug;
use scaling_adapter::ScalingAdapter;

use crate::{get_pid, Job, Threadpool};

pub struct FixedOverheadThreadpool {
    job_queue: Mutex<VecDeque<Job>>,
    workers: Mutex<HashSet<i32>>,
    busy_workers_count: Mutex<usize>,
    // used to signal blocked wait handler
    all_workers_idle: Condvar,
    // used to signal blocked destroy handler
    all_workers_exited: Condvar,
    // used to signal blocked (on empty job_queue) workers
    queue_non_empty: Condvar,
    is_stopping: AtomicBool,
    scaling_adapter: Mutex<ScalingAdapter>,
}

impl Threadpool for FixedOverheadThreadpool {
    fn submit_job(&self, job: Job) {
        let mut job_queue = self.job_queue.lock().unwrap();
        let was_empty = job_queue.is_empty();
        job_queue.push_back(job);
        // in case queue was empty all workers may be blocked, wake up one
        if was_empty {
            self.queue_non_empty.notify_one();
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

fn worker_loop(threadpool: Arc<FixedOverheadThreadpool>) {
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
        let mut job_queue = threadpool.job_queue.lock().unwrap();
        let mut job = job_queue.pop_front();
        while job.is_none() && !threadpool.is_stopping() {
            job_queue = threadpool.queue_non_empty.wait(job_queue).unwrap();
            job = job_queue.pop_front();
        }
        drop(job_queue);
        // signal other potentially blocked workers to continue processing / exit
        threadpool.queue_non_empty.notify_one();
        if threadpool.is_stopping() {
            break;
        }
        // if threadpool is not stopping, job option can't be none
        let job = job.unwrap();

        let mut busy_count = threadpool.busy_workers_count.lock().unwrap();
        *busy_count += 1;
        drop(busy_count);
        job.execute();

        let mut busy_count = threadpool.busy_workers_count.lock().unwrap();
        *busy_count -= 1;
        let all_workers_idle = *busy_count == 0;
        drop(busy_count);
        // in case of no more work, all workers idling, and thread pool not being stopped
        // the potentially 'waiting for completion' callee is signaled
        // when grabbing lock for workqueue, no other lock is being held
        if !threadpool.is_stopping()
            && all_workers_idle
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

impl FixedOverheadThreadpool {
    pub fn new(size: usize, scaling_adapter: ScalingAdapter) -> Arc<Self> {
        let threadpool = Arc::new(FixedOverheadThreadpool {
            job_queue: Mutex::new(VecDeque::with_capacity(5000)),
            workers: Mutex::new(HashSet::new()),
            busy_workers_count: Mutex::new(0),
            all_workers_idle: Condvar::new(),
            all_workers_exited: Condvar::new(),
            queue_non_empty: Condvar::new(),
            is_stopping: AtomicBool::new(false),
            scaling_adapter: Mutex::new(scaling_adapter),
        });
        for i in 0..size {
            let name = format!("worker-{}", i);
            let builder = thread::Builder::new().name(name.to_owned());
            let threadpool_clone = threadpool.clone();
            let _handle: thread::JoinHandle<()> = builder
                .spawn(move || {
                    worker_loop(threadpool_clone);
                })
                .unwrap_or_else(|_| panic!("thread creation for worker: {} failed", name));
        }
        threadpool
    }

    fn adapt_size_dummy(&self) {
        let queue_size = self.job_queue.lock().unwrap().len() as i32;
        let mut to_scale = self
            .scaling_adapter
            .lock()
            .unwrap()
            .get_scaling_advice(queue_size);
        debug!("got scaling advice: {}", to_scale);
        let current_size = self.workers.lock().unwrap().len() as i32;
        if current_size + to_scale < 1 {
            to_scale = -(current_size - 1);
        }
        let n = match to_scale.cmp(&0) {
            std::cmp::Ordering::Greater => to_scale,
            std::cmp::Ordering::Less => -to_scale,
            std::cmp::Ordering::Equal => 0,
        };
        if n > 0 {
            let _job_queue = self.job_queue.lock().unwrap();
        }
    }

    fn is_stopping(&self) -> bool {
        self.is_stopping.load(atomic::Ordering::Relaxed)
    }
}
