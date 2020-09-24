use std::collections::HashMap;

use tracesets::{SyscallData, Traceset, TracesetSnapshot};

struct IntervalData {
    read_bytes: u64,
    write_bytes: u64,
    syscalls_data: HashMap<i32, SyscallData>,
}

impl IntervalData {
    pub fn new(
        snapshot_ealier: &TracesetSnapshot,
        snapshot_later: &TracesetSnapshot,
    ) -> Option<IntervalData> {
        let mut targets_match = snapshot_ealier.targets.len() == snapshot_later.targets.len();
        if targets_match {
            targets_match = snapshot_ealier
                .targets
                .iter()
                .zip(snapshot_later.targets.iter())
                .filter(|&(a, b)| a == b)
                .count()
                == snapshot_ealier.targets.len();
        }
        if targets_match {
            let read_bytes = snapshot_later.read_bytes - snapshot_later.read_bytes;
            let write_bytes = snapshot_later.write_bytes - snapshot_later.write_bytes;
            let mut syscalls_data = HashMap::new();
            for syscall in snapshot_ealier.syscalls_data.keys() {
                let earlier_data = snapshot_ealier.syscalls_data.get(syscall).unwrap();
                let later_data = snapshot_later.syscalls_data.get(syscall).unwrap();
                let count_diff = later_data.count - earlier_data.count;
                let time_diff = later_data.total_time - earlier_data.total_time;
                let syscall_data_diff = SyscallData {
                    count: count_diff,
                    total_time: time_diff,
                };
                syscalls_data.insert(*syscall, syscall_data_diff);
            }
            Some(IntervalData {
                read_bytes,
                write_bytes,
                syscalls_data,
            })
        } else {
            None
        }
    }
}

struct IntervalMetrics {
    scale_metric: f64,
    idle_metric: f64,
    current_nr_targets: u32,
    current_time_ms: u64,
}

struct MetricsHistory {}

impl MetricsHistory {
    pub fn new() -> MetricsHistory {
        MetricsHistory {}
    }
}

struct ScalingParameters {
    check_interval_ms: u64,
    syscall_nrs: Vec<i32>,
    calc_interval_metrics: fn(&IntervalData) -> IntervalMetrics,
}

struct ScalingAdapter {
    parameters: ScalingParameters,
    traceset: Traceset,
    metrics_history: MetricsHistory,
    latest_snapshot: TracesetSnapshot,
}

impl ScalingAdapter {
    pub fn new(params: ScalingParameters) -> ScalingAdapter {
        let traceset = Traceset::new(&Vec::new(), &params.syscall_nrs).unwrap();
        let initial_snapshot = traceset.get_snapshot();
        ScalingAdapter {
            parameters: params,
            traceset,
            metrics_history: MetricsHistory::new(),
            latest_snapshot: initial_snapshot,
        }
    }

    pub fn get_scaling_advice(&self) -> i32 {
        0
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
