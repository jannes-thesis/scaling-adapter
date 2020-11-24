mod fixed;
mod watermark;
mod adaptive;

pub struct Job {
    function: Box<dyn Fn() + Sync + Send>
}

impl Job {
    pub fn execute(&self) {
        (self.function)();
    }
}

pub trait Threadpool {
    /// submit a job (function without return value)
    /// immediately returns, job is executed at some point after submission
    fn submit_job(&self, job: Job);
    /// wait until all jobs are completed
    /// do not submit any more jobs while blocking on this call
    fn wait_completion(&self);
    /// signal all workers to stop 
    /// workers will complete their current job, then terminate
    /// returns when all workers have terminated
    /// a destroyed pool can not be reinitiated
    fn destroy(&self);
}

pub fn get_pid() -> i32 {
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

#[cfg(test)]
mod tests {
    use std::{sync::{Arc, Once}, thread, time::Duration};

    use env_logger::Env;

    use crate::fixed::FixedThreadpool;

    use super::*;

    static INIT: Once = Once::new();

    /// Setup function that is only run once, even if called multiple times.
    fn setup() {
        INIT.call_once(|| {
            let env = Env::default().filter_or("MY_LOG_LEVEL", "debug");
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
            println!("iteration {} of index {}", j, i);
            thread::sleep(Duration::from_millis(100));
        }
    }

}
