pub mod adaptive;
pub mod fixed;
pub mod fixed_tracer;
pub mod watermark;

pub struct Job {
    pub function: Box<dyn Fn() + Send>,
}

impl Job {
    pub fn execute(&self) {
        (self.function)();
    }
}

/// never call wait and destroy at the same time
/// -> deadlock
pub trait Threadpool {
    /// submit a job (function without return value)
    /// immediately returns, job is executed at some point after submission
    fn submit_job(&self, job: Job);
    /// wait until all jobs are completed
    /// do not submit any more jobs while blocking on this call
    /// should only be called from one thread at the same time
    fn wait_completion(&self);
    /// signal all workers to stop
    /// workers will complete their current job, then terminate
    /// returns when all workers have terminated
    /// a destroyed pool can not be reinitiated
    /// should only be called from one thread at the same time
    fn destroy(&self);
}

pub fn get_pid() -> i32 {
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Once},
        thread,
        time::Duration,
    };

    use adaptive::AdaptiveThreadpool;
    use env_logger::Env;
    use log::debug;
    use scaling_adapter::{IntervalDerivedData, ScalingAdapter, ScalingParameters};

    use crate::{fixed::FixedThreadpool, watermark::WatermarkThreadpool};

    use super::*;

    static INIT: Once = Once::new();

    /// Setup function that is only run once, even if called multiple times.
    fn setup() {
        INIT.call_once(|| {
            let env = Env::default().filter_or("MY_LOG_LEVEL", "info");
            env_logger::init_from_env(env);
        });
    }

    #[test]
    fn fixed_create_wait_destroy() {
        setup();
        let fixed_pool = FixedThreadpool::new(5);
        wait_destroy(fixed_pool);
    }

    #[test]
    fn fixed_print_jobs() {
        setup();
        let fixed_pool = FixedThreadpool::new(5);
        print_jobs(fixed_pool);
    }

    #[test]
    fn adaptive_create_wait_destroy() {
        setup();
        let adaptive_pool = AdaptiveThreadpool::new(get_dummy_adapter());
        wait_destroy(adaptive_pool);
    }

    #[test]
    fn adaptive_print_jobs() {
        setup();
        let adaptive_pool = AdaptiveThreadpool::new(get_dummy_adapter());
        print_jobs(adaptive_pool);
    }

    #[test]
    fn watermark_create_wait_destroy() {
        setup();
        let watermark_pool = WatermarkThreadpool::new(1, 10, Duration::from_secs(10));
        wait_destroy(watermark_pool);
    }

    #[test]
    fn watermark_print_jobs() {
        setup();
        let watermark_pool = WatermarkThreadpool::new(1, 10, Duration::from_secs(10));
        print_jobs(watermark_pool);
    }

    fn get_dummy_adapter() -> ScalingAdapter {
        let adapter_params = ScalingParameters::new(
            vec![1, 2],
            Box::new(|_| IntervalDerivedData {
                scale_metric: 0.0,
                reset_metric: 0.0,
            }),
        );
        ScalingAdapter::new(adapter_params).expect("adapter creation failed")
    }

    fn print_jobs(threadpool: Arc<dyn Threadpool>) {
        for i in 0..10 {
            let job_function = move || print_ix10(i);
            let job = Job {
                function: Box::new(job_function),
            };
            threadpool.submit_job(job);
        }
        thread::sleep(Duration::from_millis(2000));
        threadpool.wait_completion();
        threadpool.destroy();
    }

    fn wait_destroy(threadpool: Arc<dyn Threadpool>) {
        threadpool.wait_completion();
        threadpool.destroy();
    }

    fn print_ix10(i: i32) {
        for j in 0..10 {
            debug!("iteration {} of index {}", j, i);
            thread::sleep(Duration::from_millis(100));
        }
    }
}
