#![allow(clippy::clippy::mutex_atomic)]
#![allow(dead_code)]
use std::{
    collections::{HashSet, VecDeque},
    fs,
    sync::{
        atomic::{self, AtomicBool},
        Arc, Condvar, Mutex, RwLock,
    },
    thread,
    time::{Duration, Instant},
};

use log::{debug, info};
use scaling_adapter::tracesets::Traceset;

use crate::{get_pid, Job, Threadpool};

pub struct FixedTracerThreadpool {
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
    traceset: Mutex<Traceset>,
    next_log_time: RwLock<Instant>,
}

impl Threadpool for FixedTracerThreadpool {
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

fn worker_loop(threadpool: Arc<FixedTracerThreadpool>) {
    let worker_pid = get_pid();
    threadpool
        .traceset
        .lock()
        .unwrap()
        .register_target(worker_pid);
    debug!("worker startup, pid: {}", worker_pid);
    {
        let mut workers = threadpool.workers.lock().unwrap();
        workers.insert(worker_pid);
    }
    loop {
        threadpool.log_metrics();
        let mut job_queue = threadpool.job_queue.lock().unwrap();
        let mut job = job_queue.pop_front();
        while job.is_none() && !threadpool.is_stopping() {
            job_queue = threadpool.queue_non_empty.wait(job_queue).unwrap();
            job = job_queue.pop_front();
        }
        drop(job_queue);
        if threadpool.is_stopping() {
            // signal other potentially blocked workers, so they can exit
            threadpool.queue_non_empty.notify_one();
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

impl FixedTracerThreadpool {
    pub fn new(size: usize) -> Arc<Self> {
        let targets = [];
        let syscalls = [0, 1, 74, 257, 871];
        let traceset = Traceset::new(&targets, &syscalls).expect("traceset creation fail");
        let threadpool = Arc::new(FixedTracerThreadpool {
            job_queue: Mutex::new(VecDeque::with_capacity(5000)),
            workers: Mutex::new(HashSet::new()),
            busy_workers_count: Mutex::new(0),
            all_workers_idle: Condvar::new(),
            all_workers_exited: Condvar::new(),
            queue_non_empty: Condvar::new(),
            is_stopping: AtomicBool::new(false),
            traceset: Mutex::new(traceset),
            next_log_time: RwLock::new(Instant::now().checked_add(Duration::from_secs(1)).unwrap()),
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
        info!("_METRICS_init");
        threadpool
    }

    pub fn log_metrics(&self) {
        let now = Instant::now();
        // do first check as reader only as it is cheaper
        if *self.next_log_time.read().unwrap() > now {
            return;
        }
        // only if first check passed, attempt to grab write lock
        let mut nlt_guard = match self.next_log_time.try_write() {
            Ok(g) => g,
            // someone else is already doing the logging for this interval
            Err(_) => {
                return;
            }
        };
        // if someone sneaked in between and already logged interval
        if *nlt_guard > now {
            return;
        }
        let next_log_time = now.checked_add(Duration::from_secs(1)).unwrap();
        *nlt_guard = next_log_time;
        drop(nlt_guard);

        let qsize = self.job_queue.lock().unwrap().len();
        info!("_METRICS_qsize: {}", qsize);
        let tids = self.workers.lock().unwrap().clone();
        let (r_bytes, w_bytes) = get_rw_bytes(tids);
        info!("_METRICS_read_bytes: {}", r_bytes);
        info!("_METRICS_write_bytes: {}", w_bytes);
        let snapshot = self.traceset.lock().unwrap().get_snapshot();
        info!("_METRICS_rchar: {}", snapshot.read_bytes);
        info!("_METRICS_wchar: {}", snapshot.write_bytes);
        info!("_METRICS_blkio: {}", snapshot.blkio_delay);
        let total_syscall_time: u64 = snapshot
            .syscalls_data
            .values()
            .map(|sd| sd.total_time)
            .sum();
        let total_syscall_calls: u32 = snapshot.syscalls_data.values().map(|sd| sd.count).sum();
        info!("_METRICS_sysc-time: {}", total_syscall_time);
        info!("_METRICS_sysc-count: {}", total_syscall_calls);
    }

    fn is_stopping(&self) -> bool {
        self.is_stopping.load(atomic::Ordering::Relaxed)
    }
}

fn get_rw_bytes(tids: HashSet<i32>) -> (u64, u64) {
    let mut read_bytes = 0;
    let mut write_bytes = 0;
    for tid in tids {
        let path = format!("/proc/{}/io", tid);
        let text = fs::read_to_string(path).expect("failed to read from proc");
        let w_bytes: u64 = text
            .lines()
            .find(|line| line.starts_with("write_bytes"))
            .expect("did not find write_bytes line")
            .split(' ')
            .last()
            .expect("did not find write_bytes in split")
            .parse()
            .expect("parse to u64 failed");
        let r_bytes: u64 = text
            .lines()
            .find(|line| line.starts_with("read_bytes"))
            .expect("did not find read_bytes line")
            .split(' ')
            .last()
            .expect("did not find read_bytes in split")
            .parse()
            .expect("parse to u64 failed");
        read_bytes += r_bytes;
        write_bytes += w_bytes;
    }
    (read_bytes, write_bytes)
}
