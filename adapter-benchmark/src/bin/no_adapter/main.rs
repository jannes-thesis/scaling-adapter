use std::{path::Path, sync::Arc, thread, time::Duration};

use adapter_benchmark::{WorkItem, WorkQueue, get_pid};
use clap::{App, Arg};
use env_logger::Env;
use helpers::spawn_worker;
use log::debug;

mod helpers;

fn run(input_path: &Path, output_dir: &Path, amount_items: usize, static_size: usize) {
    let env = Env::default().filter_or("LOG_LEVEL", "info");
    env_logger::init_from_env(env);
    let pid = get_pid();
    debug!("main startup, pid: {}", pid);

    let workqueue = Arc::new(WorkQueue::new());

    // fill up workqueue first before starting workers
    for i in 1..amount_items {
        workqueue.push(WorkItem::Write(i as usize));
    }

    // start workers
    for _i in 0..static_size {
        spawn_worker(
            workqueue.clone(),
            input_path.to_path_buf(),
            output_dir.to_path_buf(),
        );
    }
    while workqueue.size() > 0 {
        thread::sleep(Duration::from_millis(1500));
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
            Arg::new("size")
                .required(true)
                .value_name("POOL_SIZE")
                .about("size of worker pool"),
        )
        .get_matches();

    let input_path = matches.value_of("input_path").unwrap();
    let output_dir = matches.value_of("output_dir").unwrap();
    let amount_items = matches
        .value_of_t::<usize>("amount_items")
        .expect("passed amount of items must be non-negative integer");
    let static_size = matches.value_of("size").unwrap();
    let size = static_size
        .parse::<usize>()
        .expect("pool size must be positive");
    assert!(size > 0, "pool size must be positive");

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
    run(input_path, output_dir, amount_items, size);
}
