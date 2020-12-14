use std::{sync::Arc, sync::RwLock, thread, time::Duration};

use env_logger::Env;
use log::debug;
use scaling_adapter::{ScalingAdapter, ScalingParameters};
use utils::{
    get_pid, setup_garbage_input, spawn_worker, written_bytes_per_ms, WorkItem, WorkQueue,
};

mod utils;

#[test]
fn simple_test() {
    assert!(setup_garbage_input());
    // let env = Env::default().filter_or("MY_LOG_LEVEL", "simple_concurrent=debug");
    let env = Env::default().filter_or("MY_LOG_LEVEL", "debug");
    env_logger::init_from_env(env);
    let pid = get_pid();
    debug!("main startup, pid: {}", pid);

    let params = ScalingParameters::new(vec![1, 2], Box::new(written_bytes_per_ms));
    let adapter = Arc::new(RwLock::new(
        ScalingAdapter::new(params).expect("adapter creation failed"),
    ));
    let workqueue = Arc::new(WorkQueue::new());
    let workers = Arc::new(RwLock::new(Vec::new()));

    // fill up workqueue first before starting workers
    for i in 1..20000 {
        workqueue.push(WorkItem::Write(i as usize));
    }

    // first worker
    spawn_worker(workers.clone(), workqueue.clone(), adapter.clone());

    // adaptive scaling
    #[allow(clippy::comparison_chain)]
    while workqueue.size() > 0 {
        let scaling_advice = adapter.clone().write().unwrap().get_scaling_advice(-1);
        debug!("got scaling advice: scale by {}", scaling_advice);
        if scaling_advice > 0 {
            for _i in 0..scaling_advice {
                workqueue.push(WorkItem::Clone);
            }
        } else if scaling_advice < 0 {
            for _i in scaling_advice..0 {
                workqueue.push(WorkItem::Terminate);
            }
        }
        thread::sleep(Duration::from_millis(1500));
    }
}
