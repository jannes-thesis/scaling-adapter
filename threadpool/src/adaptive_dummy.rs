#![allow(clippy::clippy::mutex_atomic)]
#![allow(dead_code)]
use std::{
    collections::{HashSet, VecDeque},
    sync::{atomic, atomic::AtomicBool, atomic::AtomicUsize, Arc, Condvar, Mutex, RwLock},
    thread,
    time::{Duration, Instant},
};

use log::debug;
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

pub struct AdaptiveDummyThreadpool {
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
    inc_interval_ms: u64,
    next_inc_time: RwLock<Instant>,
    stop_size: usize,
}

impl Threadpool for AdaptiveDummyThreadpool {
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
        // wait until no workers are active anymore and queue is empty
        // HOLDING 2 LOCKS HERE 1. busy count 2. work_queue
        // safe as long as it is the only place where 2 locks are grabbed simultaneously
        while *busy_count > 0 || !self.work_queue.lock().unwrap().is_empty() {
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

fn worker_loop(threadpool: Arc<AdaptiveDummyThreadpool>) {
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
        let mut work_item = threadpool.work_queue.lock().unwrap().pop_front();
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
        // signal other potentially blocked workers to continue processing / exit
        threadpool.work_queue_non_empty.notify_one();
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
                let all_workers_idle = *busy_count == 0;
                drop(busy_count);
                // in case of no more work, all workers idling, and thread pool not being stopped
                // the potentially 'waiting for completion' callee is signaled
                // when grabbing lock for workqueue, no other lock is being held
                if !threadpool.is_stopping()
                    && all_workers_idle
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
    threadpool
        .scaling_adapter
        .lock()
        .unwrap()
        .remove_tracee(worker_pid);
    if threadpool.workers.lock().unwrap().len() == 0 {
        threadpool.all_workers_exited.notify_all();
    }
}

impl AdaptiveDummyThreadpool {
    pub fn new(scaling_adapter: ScalingAdapter, interval_ms: u64, stop_size: usize) -> Arc<Self> {
        let thread_pool = Arc::new(AdaptiveDummyThreadpool {
            work_queue: Mutex::new(VecDeque::new()),
            workers: Mutex::new(HashSet::new()),
            busy_workers_count: Mutex::new(0),
            all_workers_idle: Condvar::new(),
            all_workers_exited: Condvar::new(),
            work_queue_non_empty: Condvar::new(),
            is_stopping: AtomicBool::new(false),
            scaling_adapter: Mutex::new(scaling_adapter),
            next_worker_id: AtomicUsize::new(0),
            inc_interval_ms: interval_ms,
            next_inc_time: RwLock::new(
                Instant::now()
                    .checked_add(Duration::from_millis(interval_ms))
                    .unwrap(),
            ),
            stop_size,
        });
        thread_pool.clone().spawn_worker();
        thread_pool
    }

    fn adapt_size(&self) {
        let queue_size = self.work_queue.lock().unwrap().len() as i32;
        let to_scale = self
            .scaling_adapter
            .lock()
            .unwrap()
            .get_scaling_advice(queue_size);
        debug!("got scaling advice: {}", to_scale);
        // ignore scale advice and check if time for extra worker has come
        let psize = self.workers.lock().unwrap().len();
        if psize >= self.stop_size {
            return;
        }
        let now = Instant::now();
        // do first check as reader only as it is cheaper
        if *self.next_inc_time.read().unwrap() > now {
            return;
        }
        // only if first check passed, attempt to grab write lock
        let mut nit_guard = match self.next_inc_time.try_write() {
            Ok(g) => g,
            // someone else is already doing the logging for this interval
            Err(_) => {
                return;
            }
        };
        // if someone sneaked in between and already spawned new worker
        if *nit_guard > now {
            return;
        }
        let next_inc_time = now
            .checked_add(Duration::from_millis(self.inc_interval_ms))
            .unwrap();
        *nit_guard = next_inc_time;
        drop(nit_guard);
        // push the scale command
        let mut work_queue = self.work_queue.lock().unwrap();
        work_queue.push_front(WorkItem::ScaleCommand(ScaleCommand::Clone));
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
