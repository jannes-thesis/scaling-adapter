use std::{fs, path::PathBuf};

pub fn get_test_data_dir() -> PathBuf {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("tests/data");
    d
}

pub fn write_garbage(out_index: usize) {
    let garbage = get_garbage_input();
    println!("hi {} {}", out_index, garbage);
}

pub fn get_garbage_input() -> String {
    let mut path = get_test_data_dir();
    path.push("input.txt");
    fs::read_to_string(path).expect("could not read file to string")
}