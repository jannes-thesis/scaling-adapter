use std::{
    path::PathBuf,
    process::{Child, Command},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

use clap::ArgMatches;
use scaling_adapter::{ScalingAdapter, ScalingParameters};
use threadpool::{
    adaptive::AdaptiveThreadpool, fixed::FixedThreadpool, fixed_tracer::FixedTracerThreadpool,
    watermark::WatermarkThreadpool, Job, Threadpool,
};

use crate::{jobs::read_write_100kb_sync, jobs::read_write_buf_sync_1mb, jobs::{read_2mb, read_write_2mb_nosync, read_write_2mb_sync, read_write_buf_sync_2mb}, loads::every100ms, loads::every100us, loads::{every1ms, every30ms}};

enum BgProcess {
    NotYetStarted,
    Running(Child),
    Killed,
}

fn rw2mb_30ms_oneshot(threadpool: Arc<dyn Threadpool>, num_jobs: usize, output_dir: PathBuf) {
    let output_dir = Arc::new(output_dir);
    let rw_function = Arc::new(read_write_2mb_sync);
    every30ms(
        threadpool.clone(),
        rw_function.clone(),
        output_dir.clone(),
        num_jobs / 2 ,
    );
    threadpool.wait_completion();
    println!("phase 1 done");
    for i in num_jobs / 2 + 1..num_jobs {
        let path = output_dir.clone();
        let job = {
            let f = rw_function.clone();
            Job {
                function: Box::new(move || {
                    let p = path.clone();
                    f(p, i);
                }),
            }
        };
        threadpool.submit_job(job);
    }
    threadpool.wait_completion();
    println!("phase 2 done");
    threadpool.destroy();
}

fn rw_rwbuf_rw_2mb(threadpool: Arc<dyn Threadpool>, num_jobs: usize, output_dir: PathBuf) {
    let output_dir = Arc::new(output_dir);
    let rw_function = Arc::new(read_write_2mb_sync);
    let rwbuf_function = Arc::new(read_write_buf_sync_2mb);
    let num_jobs_f = num_jobs as f64;
    for i in 0..num_jobs {
        let path = output_dir.clone();
        let i_f = i as f64;
        let job = if i_f < num_jobs_f * 0.45 || i_f > num_jobs_f * 0.55 {
            let f = rw_function.clone();
            Job {
                function: Box::new(move || {
                    let p = path.clone();
                    f(p, i);
                }),
            }
        } else {
            let f = rwbuf_function.clone();
            Job {
                function: Box::new(move || {
                    let p = path.clone();
                    f(p, i);
                }),
            }
        };
        threadpool.submit_job(job);
    }
    threadpool.wait_completion();
    threadpool.destroy();
}

fn read_rwbuf_read_2mb(threadpool: Arc<dyn Threadpool>, num_jobs: usize, output_dir: PathBuf) {
    let output_dir = Arc::new(output_dir);
    let read_function = Arc::new(read_2mb);
    let rwsync_function = Arc::new(read_write_2mb_sync);
    let num_jobs_f = num_jobs as f64;
    for i in 0..num_jobs {
        let path = output_dir.clone();
        let i_f = i as f64;
        let job = if i_f < num_jobs_f * 0.33 || i_f > num_jobs_f * 0.66 {
            let f = read_function.clone();
            Job {
                function: Box::new(move || {
                    let p = path.clone();
                    f(p, i);
                }),
            }
        } else {
            let f = rwsync_function.clone();
            Job {
                function: Box::new(move || {
                    let p = path.clone();
                    f(p, i);
                }),
            }
        };
        threadpool.submit_job(job);
    }
    threadpool.wait_completion();
    threadpool.destroy();
}

fn sync_nosync_sync_2mb(threadpool: Arc<dyn Threadpool>, num_jobs: usize, output_dir: PathBuf) {
    let output_dir = Arc::new(output_dir);
    let sync_function = Arc::new(read_write_2mb_sync);
    let nosync_function = Arc::new(read_write_2mb_nosync);
    let num_jobs_f = num_jobs as f64;
    for i in 0..num_jobs {
        let path = output_dir.clone();
        let i_f = i as f64;
        let job = if i_f < num_jobs_f * 0.15 || i_f > num_jobs_f * 0.85 {
            let f = sync_function.clone();
            Job {
                function: Box::new(move || {
                    let p = path.clone();
                    f(p, i);
                }),
            }
        } else {
            let f = nosync_function.clone();
            Job {
                function: Box::new(move || {
                    let p = path.clone();
                    f(p, i);
                }),
            }
        };
        threadpool.submit_job(job);
    }
    threadpool.wait_completion();
    threadpool.destroy();
}

fn rw_buf_1mb_100ms_bgload(
    threadpool: Arc<dyn Threadpool>,
    num_jobs: usize,
    output_dir: PathBuf,
    bgload_start: Duration,
    bgload_end: Duration,
) {
    let job_function = Arc::new(read_write_buf_sync_1mb);
    let output_dir = Arc::new(output_dir);
    let start_time = Instant::now();
    let mut bg_process = BgProcess::NotYetStarted;
    for i in 0..num_jobs {
        let path = output_dir.clone();
        let f = job_function.clone();
        let job = Job {
            function: Box::new(move || {
                let p = path.clone();
                f(p, i);
            }),
        };
        threadpool.submit_job(job);
        thread::sleep(Duration::from_millis(100));
        match bg_process {
            BgProcess::NotYetStarted => {
                if Instant::now().duration_since(start_time) > bgload_start {
                    println!("spawning bg disk writer");
                    bg_process = BgProcess::Running(
                        Command::new(
                            "/home/jannes/MasterThesis/scaling-adapter/target/release/disk_writer",
                        )
                        .args(&[
                            "/ssd2/adapter-benchmark/files/1mb/1mb-1.txt",
                            "/ssd2/adapter-benchmark/files",
                            "1",
                            "1",
                        ])
                        .spawn()
                        .expect("failed to spawn background process"),
                    );
                }
            }
            BgProcess::Running(ref child) => {
                if Instant::now().duration_since(start_time) > bgload_end {
                    println!("killing bg disk writer");
                    unsafe {
                        libc::kill(child.id() as i32, libc::SIGINT);
                    }
                    bg_process = BgProcess::Killed;
                }
            }
            BgProcess::Killed => {}
        }
    }
    if let BgProcess::Running(ref child) = bg_process {
        thread::sleep(
            bgload_end
                .checked_sub(Instant::now().duration_since(start_time))
                .unwrap_or(Duration::from_millis(0)),
        );
        println!("killing bg disk writer");
        unsafe {
            libc::kill(child.id() as i32, libc::SIGINT);
        }
    }
    threadpool.wait_completion();
    threadpool.destroy();
}

fn lml_rw_100kb(threadpool: Arc<dyn Threadpool>, num_jobs: usize, output_dir: PathBuf) {
    let output_dir = Arc::new(output_dir);
    let num_jobs_per_phase = num_jobs / 3;
    let job_function = Arc::new(read_write_100kb_sync);

    every1ms(
        threadpool.clone(),
        job_function.clone(),
        output_dir.clone(),
        num_jobs_per_phase,
    );
    threadpool.wait_completion();
    println!("phase 1 done");
    every100us(
        threadpool.clone(),
        job_function.clone(),
        output_dir.clone(),
        num_jobs_per_phase,
    );
    threadpool.wait_completion();
    println!("phase 2 done");
    every1ms(
        threadpool.clone(),
        job_function,
        output_dir,
        num_jobs_per_phase,
    );
    threadpool.wait_completion();
    println!("phase 3 done");
    threadpool.destroy();
}

fn lml_rw_buf_100ms(threadpool: Arc<dyn Threadpool>, num_jobs: usize, output_dir: PathBuf) {
    let output_dir = Arc::new(output_dir);
    let num_jobs_per_phase = num_jobs / 3;
    let low_job_function = Arc::new(read_write_buf_sync_1mb);
    let maxed_job_function = Arc::new(read_write_buf_sync_2mb);

    every100ms(
        threadpool.clone(),
        low_job_function.clone(),
        output_dir.clone(),
        num_jobs_per_phase,
    );
    threadpool.wait_completion();
    println!("phase 1 done");
    every100ms(
        threadpool.clone(),
        maxed_job_function,
        output_dir.clone(),
        num_jobs_per_phase,
    );
    threadpool.wait_completion();
    println!("phase 2 done");
    every100ms(
        threadpool.clone(),
        low_job_function,
        output_dir,
        num_jobs_per_phase,
    );
    threadpool.wait_completion();
    println!("phase 3 done");
    threadpool.destroy();
}

pub fn do_multi_phase_run(matches: ArgMatches) {
    let matches = matches.subcommand_matches("multi").unwrap();
    let pool_type = matches.value_of("pool_type").unwrap();
    let pool_params = matches.value_of("pool_params").unwrap();
    let workload = matches.value_of("workload_name").unwrap();
    let num_jobs = matches.value_of("num_jobs").unwrap();
    let output_dir = matches.value_of("output_dir").unwrap();

    let thread_pool: Arc<dyn Threadpool> = match pool_type {
        "adaptive" => {
            let adapter_params = ScalingParameters::default();
            AdaptiveThreadpool::new(
                ScalingAdapter::new(adapter_params.with_algo_params(pool_params))
                    .expect("failed to construct adapter parameters"),
            )
        }
        "fixed" => {
            let pool_size: usize = pool_params.parse().expect("invalid pool size");
            FixedThreadpool::new(pool_size)
        }
        "fixed-tracer" => {
            let pool_size: usize = pool_params.parse().expect("invalid pool size");
            FixedTracerThreadpool::new(pool_size)
        }
        "watermark" => WatermarkThreadpool::new_untyped(pool_params),
        _ => {
            panic!("invalid pool_type parameter");
        }
    };
    let num_jobs: usize = num_jobs.parse().expect("invalid num_jobs parameters");
    let output_dir = PathBuf::from(output_dir);
    if !output_dir.is_dir() {
        panic!("given output_dir does not exist or is not a directory");
    }

    println!("starting benchmark");
    let start = Instant::now();
    match workload {
        "lml-rw_buf_100ms" => lml_rw_buf_100ms(thread_pool, num_jobs, output_dir),
        "lml-rw_100kb" => lml_rw_100kb(thread_pool, num_jobs, output_dir),
        "r_rw_r_2mb" => read_rwbuf_read_2mb(thread_pool, num_jobs, output_dir),
        "rw2mb_30ms_oneshot" => rw2mb_30ms_oneshot(thread_pool, num_jobs, output_dir),
        "rw_rwb_rw_2mb" => rw_rwbuf_rw_2mb(thread_pool, num_jobs, output_dir),
        "rw_buf_1mb_100ms_bgload_25-75" => rw_buf_1mb_100ms_bgload(
            thread_pool,
            num_jobs,
            output_dir,
            Duration::from_secs(25),
            Duration::from_secs(75),
        ),
        "sync_nosync_sync_2mb" => sync_nosync_sync_2mb(thread_pool, num_jobs, output_dir),
        _ => panic!("invalid workload"),
    }
    let runtime = Instant::now().duration_since(start);
    println!(
        "all jobs completed, runtime {} seconds",
        runtime.as_secs_f32()
    );
}
