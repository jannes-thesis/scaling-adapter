use std::{path::PathBuf, sync::Arc, time::Instant};

use clap::ArgMatches;
use scaling_adapter::{ScalingAdapter, ScalingParameters};
use threadpool::{
    adaptive::AdaptiveThreadpool, fixed::FixedThreadpool, watermark::WatermarkThreadpool,
    Threadpool,
};

use crate::{
    jobs::read_write_100kb_sync, jobs::read_write_buf_sync_1mb, jobs::read_write_buf_sync_2mb,
    loads::every100ms, loads::every100us, loads::every1ms,
};

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
        _ => panic!("invalid workload"),
    }
    let runtime = Instant::now().duration_since(start);
    println!(
        "all jobs completed, runtime {} seconds",
        runtime.as_secs_f32()
    );
}
