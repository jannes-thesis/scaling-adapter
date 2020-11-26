use std::{sync::Arc, path::PathBuf, thread, time::Duration};

use threadpool::{Job, Threadpool};

use crate::jobs::JobFunction;

pub fn every10ms(
    threadpool: Arc<dyn Threadpool>,
    job_function: Arc<JobFunction>,
    out_dir: Arc<PathBuf>,
    num_items: usize,
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
        thread::sleep(Duration::from_millis(10));
    }
}
