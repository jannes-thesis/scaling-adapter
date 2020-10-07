use log::debug;
use scaling_adapter::ScalingAdapter;
use std::{
    collections::VecDeque,
    error::Error,
    fs,
    path::PathBuf,
    sync::Arc,
    sync::RwLock,
    thread::{self, JoinHandle},
};

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

pub fn get_pid() -> i32 {
    unsafe { libc::syscall(libc::SYS_gettid) as i32}
}

pub fn worker_function(
    workers: Arc<RwLock<Vec<JoinHandle<()>>>>,
    queue: Arc<WorkQueue>,
    adapter: Arc<RwLock<ScalingAdapter>>,
) {
    // get new jobs as long as workqueue is not empty
    while let Some(work_item) = queue.pop() {
        match work_item {
            WorkItem::Write(i) => {
                let _ = write_garbage(i);
            }
            WorkItem::Clone => {
                spawn_worker(workers.clone(), queue.clone(), adapter.clone());
            }
            WorkItem::Terminate => {
                break;
            }
        }
    }
}

pub fn spawn_worker(
    workers: Arc<RwLock<Vec<JoinHandle<()>>>>,
    queue: Arc<WorkQueue>,
    adapter: Arc<RwLock<ScalingAdapter>>,
) {
    let workers_clone = workers.clone();
    let adapter_clone = adapter.clone();
    let handle = thread::spawn(move || {
        // register before starting worker loop
        let worker_pid = get_pid();
        debug!("worker startup, pid: {}", worker_pid);
        adapter.write().unwrap().add_tracee(worker_pid);
        // worker loop
        worker_function(workers_clone, queue, adapter);
        // deregister before termination
        let worker_pid = get_pid();
        adapter_clone.write().unwrap().remove_tracee(worker_pid); 
        debug!("worker terminating, pid: {}", worker_pid);
    });
    workers.write().unwrap().push(handle);
}

pub fn get_test_data_dir() -> PathBuf {
    let mut d = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    d.push("tests/data");
    d
}

pub fn write_garbage(out_index: usize) -> Result<(), Box<dyn Error>> {
    let garbage = get_garbage_input();
    let mut output_path = get_test_data_dir();
    output_path.push(format!("out{}.txt", out_index));
    fs::write(&output_path, garbage)?;
    fs::remove_file(&output_path)?;
    Ok(())
}

pub fn get_garbage_input() -> String {
    let mut path = get_test_data_dir();
    path.push("input.txt");
    fs::read_to_string(path).expect("could not read file to string")
}
