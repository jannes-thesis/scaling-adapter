use std::{path::PathBuf, sync::Arc, thread};

use adapter_benchmark::{get_pid, write_remove_garbage, WorkItem, WorkQueue};
use log::debug;

pub fn worker_function(queue: Arc<WorkQueue>, input_path: PathBuf, output_dir: PathBuf) {
    // get new jobs as long as workqueue is not empty
    while let Some(work_item) = queue.pop() {
        match work_item {
            WorkItem::Write(i) => {
                let _ = write_remove_garbage(input_path.as_path(), output_dir.as_path(), i);
            }
            WorkItem::Clone => {
                spawn_worker(queue.clone(), input_path.clone(), output_dir.clone());
            }
            WorkItem::Terminate => {
                break;
            }
        }
    }
}

pub fn spawn_worker(queue: Arc<WorkQueue>, input_path: PathBuf, output_dir: PathBuf) {
    let _handle: thread::JoinHandle<()> = thread::spawn(move || {
        let worker_pid = get_pid();
        debug!("worker startup, pid: {}", worker_pid);
        // worker loop
        worker_function(queue, input_path, output_dir);
        let worker_pid = get_pid();
        debug!("worker terminating, pid: {}", worker_pid);
    });
}
