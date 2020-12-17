use std::{path::PathBuf, sync::Arc, time::Instant};

use clap::ArgMatches;
use scaling_adapter::{ScalingAdapter, ScalingParameters};
use threadpool::{
    adaptive::AdaptiveThreadpool, fixed::FixedThreadpool, watermark::WatermarkThreadpool,
    Threadpool,
};

use crate::{jobs::{JobFunction, read_write_100kb_sync, read_write_1mb_sync, read_write_2mb_nosync, read_write_2mb_sync, read_write_4kb_sync, read_write_4mb_sync, read_write_buf_sync_1mb, read_write_buf_sync_2mb}, loads::{every100ms, every100us, every10ms, every1ms, every1s, every200ms, every50ms}};

pub fn do_single_phase_run(matches: ArgMatches) {
    let matches = matches.subcommand_matches("single").unwrap();
    let pool_type = matches.value_of("pool_type").unwrap();
    let pool_params = matches.value_of("pool_params").unwrap();
    let load_type = matches.value_of("load_type").unwrap();
    let worker_function = matches.value_of("worker_function").unwrap();
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
    let worker_function: Arc<JobFunction> = match worker_function {
        "read_write_4kb_sync" => Arc::new(read_write_4kb_sync),
        "read_write_100kb_sync" => Arc::new(read_write_100kb_sync),
        "read_write_1mb_sync" => Arc::new(read_write_1mb_sync),
        "read_write_2mb_sync" => Arc::new(read_write_2mb_sync),
        "read_write_2mb_nosync" => Arc::new(read_write_2mb_nosync),
        "read_write_4mb_sync" => Arc::new(read_write_4mb_sync),
        "read_write_buf_sync_1mb" => Arc::new(read_write_buf_sync_1mb),
        "read_write_buf_sync_2mb" => Arc::new(read_write_buf_sync_2mb),
        _ => {
            panic!("invalid worker function argument");
        }
    };
    let load_function = match load_type {
        "every100us" => every100us,
        "every1ms" => every1ms,
        "every10ms" => every10ms,
        "every50ms" => every50ms,
        "every100ms" => every100ms,
        "every200ms" => every200ms,
        "every1s" => every1s,
        _ => {
            panic!("invalid load function parameter");
        }
    };
    println!("starting benchmark");
    load_function(
        thread_pool.clone(),
        worker_function,
        Arc::new(output_dir),
        num_jobs,
    );
    let start = Instant::now();
    println!("submitted all jobs, waiting for completion");
    thread_pool.wait_completion();
    let wait_time = Instant::now().duration_since(start);
    println!(
        "all jobs completed, waited for {} seconds, destroying pool",
        wait_time.as_secs_f32()
    );
    thread_pool.destroy()
}
