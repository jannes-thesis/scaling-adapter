use std::io::Write;
use std::{panic, process};

use clap::{App, Arg};

use multi_phase::do_multi_phase_run;
use single_phase::do_single_phase_run;

mod jobs;
mod loads;
mod multi_phase;
mod single_phase;

fn main() {
    let matches = App::new("threadpool_bench")
        .subcommand(
            App::new("single")
                .about("run single-phase workload")
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
                ),
        )
        .subcommand(
            App::new("multi")
                .about("run multi-phase workload")
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
                    Arg::new("workload_name")
                        .about("name of the multi-phase workload")
                        .required(true)
                        .index(3),
                )
                .arg(
                    Arg::new("num_jobs")
                        .about("amount of jobs to be submitted to pool")
                        .required(true)
                        .index(4),
                )
                .arg(
                    Arg::new("output_dir")
                        .about("which input/output directory to use")
                        .required(true)
                        .index(5),
                ),
        )
        .get_matches();

    // pass log level via RUST_LOG environment variable
    env_logger::builder()
        .format(|buf, record| {
            let ts = buf.timestamp_millis();
            let module_path = record.module_path().unwrap_or("");
            writeln!(
                buf,
                "[{} {} {}]: {}",
                ts,
                record.level(),
                module_path,
                record.args()
            )
        })
        .init();

    // make sure if a worker thread panics that the whole benchmark fails
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        println!("some thread paniced, exit whole process");
        process::exit(1);
    }));

    match matches.subcommand_name() {
        Some("single") => do_single_phase_run(matches),
        Some("multi") => do_multi_phase_run(matches),
        _ => panic!("invalid subcommand!"),
    }
}
