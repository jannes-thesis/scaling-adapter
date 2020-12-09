use std::{
    path::Path,
    sync::{Arc, RwLock},
    thread,
    time::Duration,
};

use adapter_benchmark::{get_pid, written_bytes_per_ms, WorkItem, WorkQueue};
use clap::{App, Arg};
use env_logger::Env;
use helpers::spawn_worker;
use log::{debug, info};
use scaling_adapter::{ScalingAdapter, ScalingParameters};

mod helpers;

fn run(input_path: &Path, output_dir: &Path, amount_items: usize, static_size: Option<usize>) {
    let env = Env::default().filter_or("LOG_LEVEL", "info");
    env_logger::init_from_env(env);
    let pid = get_pid();
    debug!("main startup, pid: {}", pid);

    let params = ScalingParameters::new(vec![1, 2], Box::new(written_bytes_per_ms));
    let adapter = Arc::new(RwLock::new(
        ScalingAdapter::new(params).expect("adapter creation failed"),
    ));
    let workqueue = Arc::new(WorkQueue::new());

    // fill up workqueue first before starting workers
    for i in 1..amount_items {
        workqueue.push(WorkItem::Write(i as usize));
    }

    // first worker
    spawn_worker(
        workqueue.clone(),
        adapter.clone(),
        input_path.to_path_buf(),
        output_dir.to_path_buf(),
    );
    let mut pool_size = 1;

    match static_size {
        None => {
            // adaptive scaling
            #[allow(clippy::comparison_chain)]
            while workqueue.size() > 0 {
                let scaling_advice = adapter.clone().write().unwrap().get_scaling_advice(-1);
                debug!("got scaling advice: scale by {}", scaling_advice);
                if scaling_advice != 0 {
                    pool_size += scaling_advice;
                    info!("scale by {}, new pool size: {}", scaling_advice, pool_size);
                }
                if scaling_advice > 0 {
                    for _i in 0..scaling_advice {
                        workqueue.push(WorkItem::Clone);
                    }
                } else if scaling_advice < 0 {
                    for _i in scaling_advice..0 {
                        workqueue.push(WorkItem::Terminate);
                    }
                }
                thread::sleep(Duration::from_millis(1500));
            }
        }
        Some(size) => {
            // no scaling, just perform same amount of work in main thread
            // spin up extra workers
            for _i in 0..size - 1 {
                spawn_worker(
                    workqueue.clone(),
                    adapter.clone(),
                    input_path.to_path_buf(),
                    output_dir.to_path_buf(),
                );
                pool_size += 1;
            }
            while workqueue.size() > 0 {
                let scaling_advice = adapter.clone().write().unwrap().get_scaling_advice(-1);
                debug!("got scaling advice: scale by {}", scaling_advice);
                if scaling_advice != 0 {
                    pool_size += scaling_advice;
                    info!(
                        "would scale by {}, new pool size: {}",
                        scaling_advice, pool_size
                    );
                }
                thread::sleep(Duration::from_millis(1500));
            }
        }
    }
}

fn main() {
    let matches = App::new("adapter benchmark")
        .version("1.0")
        .arg(
            Arg::new("input_path")
                .required(true)
                .value_name("INPUT_FILE")
                .about("path to file containing random text"),
        )
        .arg(
            Arg::new("output_dir")
                .required(true)
                .value_name("OUTPUT_DIR")
                .about("path to directory where temp files are created/deleted"),
        )
        .arg(
            Arg::new("amount_items")
                .required(true)
                .value_name("AMOUNT_ITEMS")
                .about("amount of read-write-delete operations the workers should perform"),
        )
        .arg(
            Arg::new("static")
                .long("static")
                .takes_value(true)
                .about("force static worker pool and set size"),
        )
        .get_matches();

    let input_path = matches.value_of("input_path").unwrap();
    let output_dir = matches.value_of("output_dir").unwrap();
    let amount_items = matches
        .value_of_t::<usize>("amount_items")
        .expect("passed amount of items must be non-negative integer");
    let static_size = matches.value_of("static");

    let input_path = Path::new(input_path);
    let output_dir = Path::new(output_dir);
    assert!(
        input_path.is_file(),
        "given input path does not point to an existing file"
    );
    assert!(
        output_dir.is_dir(),
        "given output path does not point to an existing directory"
    );

    match static_size {
        Some(size_str) => {
            let size = size_str
                .parse::<usize>()
                .expect("pool size must be positive");
            assert!(size > 0, "pool size must be positive");
            run(input_path, output_dir, amount_items, Some(size))
        }
        None => run(input_path, output_dir, amount_items, None),
    }
}
