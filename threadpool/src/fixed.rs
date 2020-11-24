use std::{
    collections::HashSet,
    sync::{Arc, Condvar, Mutex, RwLock},
    thread::{self},
    time::Duration,
};

use crossbeam::queue::SegQueue;
use log::debug;

use crate::{get_pid, Job, Threadpool};

#[derive(Eq, PartialEq)]
enum State {
    Stopping,
    Active,
}

pub struct FixedThreadpool {
    job_queue: SegQueue<Job>,
    workers: Mutex<HashSet<i32>>,
    busy_workers_count: Mutex<usize>,
    all_idle_cond: Condvar,
    all_exit_cond: Condvar,
    state: RwLock<State>,
}

impl Threadpool for FixedThreadpool {
    fn submit_job(&self, job: Job) {
        self.job_queue.push(job);
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

fn worker_loop(threadpool: Arc<FixedThreadpool>) {
    let worker_pid = get_pid();
    debug!("worker startup, pid: {}", worker_pid);
    {
        let mut workers = threadpool.workers.lock().unwrap();
        workers.insert(worker_pid);
    }
    loop {
        let job = threadpool.job_queue.pop();
        if job.is_none() && !threadpool.is_stopping() {
            thread::sleep(Duration::from_millis(1000));
            continue;
        }
        if threadpool.is_stopping() {
            break;
        }
        let job = job.unwrap();

        let mut busy_count = threadpool.busy_workers_count.lock().unwrap();
        *busy_count += 1;
        drop(busy_count);
        job.execute();

        let mut busy_count = threadpool.busy_workers_count.lock().unwrap();
        *busy_count -= 1;
        if !threadpool.is_stopping() && *busy_count == 0 && threadpool.job_queue.is_empty() {
            threadpool.all_idle_cond.notify_one();
        }
    }
    let mut workers = threadpool.workers.lock().unwrap();
    debug!("worker terminating, pid: {}", worker_pid);
    workers.remove(&worker_pid);
    if workers.len() == 0 {
        threadpool.all_exit_cond.notify_all();
    }
}

impl FixedThreadpool {
    pub fn new(size: usize) -> Arc<Self> {
        let threadpool = Arc::new(FixedThreadpool {
            job_queue: SegQueue::new(),
            workers: Mutex::new(HashSet::new()),
            busy_workers_count: Mutex::new(0),
            all_idle_cond: Condvar::new(),
            all_exit_cond: Condvar::new(),
            state: RwLock::new(State::Active),
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

    fn is_stopping(&self) -> bool {
        *self.state.read().unwrap() == State::Stopping
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create() {
        let pool = FixedThreadpool::new(5);
        thread::sleep(Duration::from_millis(500));
        assert!(pool.workers.lock().unwrap().len() == 5);
        assert!(*pool.busy_workers_count.lock().unwrap() == 0);
    }
}
