use std::{
    fs,
    path::Path,
    sync::{
        mpsc::{self, TryRecvError},
        Arc,
    },
    thread,
    time::Duration,
};

use adapter_benchmark::write_remove;
use clap::{App, Arg};

fn main() {
    let (sender, receiver) = mpsc::channel();

    ctrlc::set_handler(move || {
        sender.send("shutdown").unwrap();
        println!("\nshutdown");
    })
    .expect("could not set ctrlc handler");

    let matches = App::new("disk writer")
        .version("1.0")
        .arg(
            Arg::new("input_file")
                .required(true)
                .value_name("FILE_PATH")
                .about("path to file with garbage input"),
        )
        .arg(
            Arg::new("output_dir")
                .required(true)
                .value_name("OUTPUT_DIR")
                .about("path to directory where temp files are created/deleted"),
        )
        .arg(
            Arg::new("sleep_ms")
                .required(true)
                .value_name("SLEEP_INTERVAL_MS")
                .about("how many ms to sleep between reps"),
        )
        .arg(
            Arg::new("nr_threads")
                .required(true)
                .value_name("AMOUNT_THREADS")
                .about("how many threads to use"),
        )
        .get_matches();

    let output_dir = matches.value_of("output_dir").unwrap();
    let input_path = matches.value_of("input_file").unwrap();
    let sleep_ms: u64 = matches
        .value_of("sleep_ms")
        .unwrap()
        .parse()
        .expect("invalid sleep interval");
    let nr_threads: u64 = matches
        .value_of("nr_threads")
        .unwrap()
        .parse()
        .expect("invalid nr threads");
    if nr_threads < 1 {
        panic!("thread count below 1");
    }

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

    let garbage = Arc::new(fs::read_to_string(input_path).expect("could not read file to string"));

    let mut worker_senders: Vec<mpsc::Sender<&str>> = Vec::new();
    let mut worker_handles = Vec::new();
    for i in 0..nr_threads {
        let (sender, receiver) = mpsc::channel();
        worker_senders.push(sender);
        let output_filename = format!("tmp-{}.txt", i);
        let output_path = output_dir.join(&output_filename);
        let garbage = garbage.clone();
        let handle = thread::spawn(move || {
            while let Err(TryRecvError::Empty) = receiver.try_recv() {
                write_remove(&garbage, &output_path).expect("error writing/removing garbage file");
                thread::sleep(Duration::from_millis(sleep_ms));
            }
        });
        worker_handles.push(handle);
    }
    let _exit_msg = receiver.recv().unwrap();
    for worker_sender in worker_senders {
        worker_sender.send("shutdown").unwrap();
    }
    for worker_handle in worker_handles {
        let _res = worker_handle.join();
    }
}
