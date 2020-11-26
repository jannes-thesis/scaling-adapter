use std::{path::PathBuf, sync::Arc};

use clap::{App, Arg};

use jobs::{read_write_4kb_sync, JobFunction};
use loads::every10ms;
use scaling_adapter::{ScalingAdapter, ScalingParameters};
use threadpool::{adaptive::AdaptiveThreadpool, fixed::FixedThreadpool, Threadpool};

mod jobs;
mod loads;

fn main() {
    let matches = App::new("threadpool_bench")
        .arg(
            Arg::new("pool_type")
                .about("select one of <adaptive>, <fixed>, <watermark>")
                .required(true)
                .index(1),
        )
        .arg(
            Arg::new("pool_params")
                .about("fixed: size, adaptive: algo_params_str, watermark: lower-upper_str")
                .required(true)
                .index(2),
        )
        .arg(
            Arg::new("load_type")
                .about("which load function to select")
                .required(true)
                .index(3),
        )
        .arg(
            Arg::new("worker_function")
                .about("which work function is used for jobs")
                .required(true)
                .index(4),
        )
        .arg(
            Arg::new("num_jobs")
                .about("amount of jobs to be submitted to pool")
                .required(true)
                .index(5),
        )
        .arg(
            Arg::new("output_dir")
                .about("which input/output directory to use")
                .required(true)
                .index(6),
        )
        .get_matches();

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
        "watermark" => {
            unimplemented!();
        }
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
        _ => {
            panic!("invalid worker function argument");
        }
    };
    println!("starting benchmark");
    match load_type {
        "every10ms" => {
            every10ms(
                thread_pool,
                worker_function,
                Arc::new(output_dir),
                num_jobs,
            );
        }
        _ => {
            panic!("invalid load function parameter");
        }
    }
}
