#![allow(non_snake_case)]
use std::{sync::Arc, path::PathBuf, thread, time::Duration};

use threadpool::{Job, Threadpool};

use crate::jobs::JobFunction;

pub fn every100us(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    every_X_micros(threadpool, job_function, out_dir, num_items, 100)
}

pub fn every1ms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    every_X_millis(threadpool, job_function, out_dir, num_items, 1)
}

pub fn every10ms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    every_X_millis(threadpool, job_function, out_dir, num_items, 10)
}

pub fn every50ms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    every_X_millis(threadpool, job_function, out_dir, num_items, 50)
}

pub fn every100ms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    every_X_millis(threadpool, job_function, out_dir, num_items, 100)
}

pub fn every200ms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    every_X_millis(threadpool, job_function, out_dir, num_items, 200)
}

pub fn every1s(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
) {
    every_X_millis(threadpool, job_function, out_dir, num_items, 1000)
}

pub fn every_X_millis(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
    interval_millis: u64,
) {
    every_X_micros(threadpool, job_function, out_dir, num_items, interval_millis * 1000);
}

pub fn every_X_micros(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
    interval_micros: u64,
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
        thread::sleep(Duration::from_micros(interval_micros));
    }
}
