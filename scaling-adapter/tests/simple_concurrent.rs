use std::{sync::Arc, sync::RwLock, thread, time::Duration};

use env_logger::Env;
use log::debug;
use scaling_adapter::{IntervalDerivedData, ScalingAdapter, ScalingParameters};
use utils::{get_pid, spawn_worker, WorkItem, WorkQueue};

mod utils;

#[test]
fn simple_test() {
    let env = Env::default().filter_or("MY_LOG_LEVEL", "debug");
    env_logger::init_from_env(env);
    let pid = get_pid();
    debug!("main startup, pid: {}", pid);

    let params = ScalingParameters {
        check_interval_ms: 1000,
        syscall_nrs: vec![1, 2],
        calc_interval_metrics: Box::new(|data| IntervalDerivedData {
            scale_metric: data.write_bytes as f64,
            idle_metric: data.write_bytes as f64,
        }),
    };
    let adapter = Arc::new(RwLock::new(
        ScalingAdapter::new(params).expect("adapter creation failed"),
    ));
    let workqueue = Arc::new(WorkQueue::new());
    let workers = Arc::new(RwLock::new(Vec::new()));

    // fill up workqueue first before starting workers
    for i in 1..10000 {
        workqueue.push(WorkItem::Write(i as usize));
    }

    // first worker
    spawn_worker(workers.clone(), workqueue.clone(), adapter.clone());

    // adaptive scaling
    #[allow(clippy::comparison_chain)]
    while workqueue.size() > 0 {
        let scaling_advice = adapter.clone().write().unwrap().get_scaling_advice();
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
