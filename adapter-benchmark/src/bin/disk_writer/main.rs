use std::{
    fs,
    path::Path,
    sync::mpsc::{self, TryRecvError},
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
        .get_matches();

    let output_dir = matches.value_of("output_dir").unwrap();
    let input_path = matches.value_of("input_file").unwrap();
    let sleep_ms: u64 = matches
        .value_of("sleep_ms")
        .unwrap()
        .parse()
        .expect("invalid sleep interval");

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

    let garbage = fs::read_to_string(input_path).expect("could not read file to string");

    while let Err(TryRecvError::Empty) = receiver.try_recv() {
        let output_path = output_dir.join("tmp.txt");
        write_remove(&garbage, &output_path).expect("error writing/removing garbage file");
        thread::sleep(Duration::from_millis(sleep_ms));
    }
}
