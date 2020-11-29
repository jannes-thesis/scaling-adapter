#![allow(non_snake_case)]
use std::{sync::Arc, path::PathBuf, thread, time::Duration};

use threadpool::{Job, Threadpool};

use crate::jobs::JobFunction;

pub fn every1ms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    everyXms(threadpool, job_function, out_dir, num_items, 1)
}

pub fn every10ms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    everyXms(threadpool, job_function, out_dir, num_items, 10)
}

pub fn every100ms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    everyXms(threadpool, job_function, out_dir, num_items, 100)
}

pub fn every1s(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    everyXms(threadpool, job_function, out_dir, num_items, 1000)
}

pub fn everyXms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
    interval_ms: u64,
) {
    for i in 0..num_items {
        let path = out_dir.clone();
        let f = job_function.clone();
        let job = Job {
            function: Box::new(move || {
                let p = path.clone();
                f(p, i);
            }),
        };
        threadpool.submit_job(job);
        thread::sleep(Duration::from_millis(interval_ms));
    }
}
