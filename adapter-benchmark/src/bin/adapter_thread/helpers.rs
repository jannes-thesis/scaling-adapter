use std::{path::PathBuf, sync::{Arc, RwLock}, thread};


use adapter_benchmark::{WorkItem,  WorkQueue, get_pid, write_garbage};
use log::{debug};
use scaling_adapter::{ScalingAdapter};


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
    let _handle: thread::JoinHandle<()> = thread::spawn(move || {
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