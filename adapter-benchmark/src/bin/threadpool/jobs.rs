use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
    sync::Arc,
};
use std::fs::File;

pub type JobFunction = dyn Fn(Arc<PathBuf>, usize) + Send + Sync;

pub fn read_write_4kb_sync(file_dir: Arc<PathBuf>, index: usize) {
    let input_filename = format!("4kb-{}.txt", index);
    let output_filename = format!("wout-{}.txt", index);
    let input = fs::read_to_string(file_dir.join(input_filename)).expect("error reading file to string");
    let mut output_file =
        File::create(file_dir.join(output_filename)).expect("unexpected file error");
    output_file
        .write_all(input.as_bytes())
        .expect("error writing input string to output");
    output_file.sync_all().expect("error fsyncing output file");
}

pub fn read_write_buf_10mb_sync(file_dir: Arc<PathBuf>, index: usize) {
    let input_filename = format!("10mb-{}.txt", index);
    let output_filename = format!("wout-{}.txt", index);
    let mut input_file = File::open(file_dir.join(input_filename)).expect("unexpected file error");
    let mut output_file =
        File::create(file_dir.join(output_filename)).expect("unexpected file error");
    let mut buffer = [0; 4096];
    loop {
        let n = match input_file.read(&mut buffer) {
            Ok(n) => n,
            Err(ref e) => {
                if e.kind() == io::ErrorKind::Interrupted {
                    continue;
                } else {
                    panic!("ecountered unexpected error reading from input file");
                }
            }
        };
        if n == 0 {
            break;
        }
        let read_slice = &buffer[0..n];
        output_file
            .write_all(read_slice)
            .expect("error writing to file");
        output_file.sync_all().expect("error fsyncing file");
    }
}
