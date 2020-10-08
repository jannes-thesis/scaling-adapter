use chrono::{DateTime, Local};
use log::debug;
use scaling_adapter::{IntervalData, IntervalDerivedData, ScalingAdapter};
use std::{collections::VecDeque, error::Error, fs, path::Path, path::PathBuf, sync::Arc, sync::RwLock, thread::{self, JoinHandle}};

pub enum WorkItem {
    Write(usize),
    Clone,
    Terminate,
}

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
        idle_metric: write_bytes_per_ms,
    }
}

pub fn get_pid() -> i32 {
    unsafe { libc::syscall(libc::SYS_gettid) as i32 }
}

pub fn worker_function(
    queue: Arc<WorkQueue>,
    adapter: Arc<RwLock<ScalingAdapter>>,
    input_path: PathBuf,
    output_dir: PathBuf,
) {
    // get new jobs as long as workqueue is not empty
    while let Some(work_item) = queue.pop() {
        match work_item {
            WorkItem::Write(i) => {
                let _ = write_garbage(input_path.as_path(), output_dir.as_path(), i);
            }
            WorkItem::Clone => {
                spawn_worker(
                    queue.clone(),
                    adapter.clone(),
                    input_path.clone(),
                    output_dir.clone(),
                );
            }
            WorkItem::Terminate => {
                break;
            }
        }
    }
}

pub fn spawn_worker(
    queue: Arc<WorkQueue>,
    adapter: Arc<RwLock<ScalingAdapter>>,
    input_path: PathBuf,
    output_dir: PathBuf,
) {
    let adapter_clone = adapter.clone();
    let _handle: JoinHandle<_> = thread::spawn(move || {
        // register before starting worker loop
        let worker_pid = get_pid();
        debug!("worker startup, pid: {}", worker_pid);
        adapter.write().unwrap().add_tracee(worker_pid);
        // worker loop
        worker_function(queue, adapter, input_path, output_dir);
        // deregister before termination
        let worker_pid = get_pid();
        adapter_clone.write().unwrap().remove_tracee(worker_pid);
        debug!("worker terminating, pid: {}", worker_pid);
    });
}

pub fn write_garbage(
    input_path: &Path,
    output_dir: &Path,
    out_index: usize,
) -> Result<(), Box<dyn Error>> {
    let garbage = fs::read_to_string(input_path).expect("could not read file to string");
    let output_path = output_dir.join(format!("out{}.txt", out_index));
    fs::write(&output_path, garbage)?;
    fs::remove_file(&output_path)?;
    Ok(())
}
