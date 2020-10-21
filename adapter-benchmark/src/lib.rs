use chrono::{DateTime, Local};
use log::debug;
use scaling_adapter::{IntervalData, IntervalDerivedData};
use std::{
    collections::VecDeque,
    error::Error,
    fs::{self, File},
    io::Write,
    path::Path,
    sync::RwLock,
};

pub enum WorkItem {
    Write(usize),
    Clone,
    Terminate,
}

#[derive(Default)]
pub struct WorkQueue {
    buffer: RwLock<VecDeque<WorkItem>>,
}

impl WorkQueue {
    pub fn new() -> Self {
        WorkQueue {
            buffer: RwLock::new(VecDeque::new()),
        }
    }

    pub fn pop(&self) -> Option<WorkItem> {
        self.buffer.write().unwrap().pop_front()
    }

    pub fn push(&self, work_item: WorkItem) {
        self.buffer.write().unwrap().push_front(work_item);
    }

    pub fn size(&self) -> usize {
        self.buffer.read().unwrap().len()
    }
}

pub fn written_bytes_per_ms(interval_data: &IntervalData) -> IntervalDerivedData {
    let duration_ms = interval_data.end_millis() - interval_data.start_millis();
    // conversion to f64 precise for durations under 1000 years for sure
    // conversion to f64 precise for under a petabyte of written bytes
    let write_bytes_per_ms = (interval_data.write_bytes as f64) / (duration_ms as f64);
    let interval_start: DateTime<Local> = interval_data.start.into();
    let interval_end: DateTime<Local> = interval_data.end.into();
    debug!(
        "{} MB/sec written in interval from {} to {}",
        write_bytes_per_ms / 1000.0,
        interval_start.format("%H:%M:%S::%3f"),
        interval_end.format("%H:%M:%S::%3f")
    );
    IntervalDerivedData {
        scale_metric: write_bytes_per_ms,
        reset_metric: write_bytes_per_ms,
    }
}

pub fn get_pid() -> i32 {
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

pub fn write_remove_garbage(
    garbage_path: &Path,
    output_dir: &Path,
    out_index: usize,
) -> Result<(), Box<dyn Error>> {
    let garbage = fs::read_to_string(garbage_path).expect("could not read file to string");
    let output_path = output_dir.join(format!("out{}.txt", out_index));
    write_remove(&garbage, &output_path)
}

pub fn write_remove(content: &str, output_path: &Path) -> Result<(), Box<dyn Error>> {
    let mut file = File::create(output_path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    drop(file);
    fs::remove_file(&output_path)?;
    Ok(())
}
