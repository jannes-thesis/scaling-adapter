#![allow(dead_code)]
use std::{collections::HashMap, time::SystemTime};

use errors::AdapterError;
use tracesets::{SyscallData, Traceset, TracesetSnapshot};

mod errors;

struct IntervalData {
    read_bytes: u64,
    write_bytes: u64,
    syscalls_data: HashMap<i32, SyscallData>,
}

impl IntervalData {
    pub fn new(
        snapshot_earlier: &TracesetSnapshot,
        snapshot_later: &TracesetSnapshot,
    ) -> Option<IntervalData> {
        let targets_match =
            IntervalData::targets_equal(&snapshot_earlier.targets, &snapshot_earlier.targets);
        if targets_match {
            let read_bytes = snapshot_later.read_bytes - snapshot_earlier.read_bytes;
            let write_bytes = snapshot_later.write_bytes - snapshot_earlier.write_bytes;
            let mut syscalls_data = HashMap::new();
            for syscall in snapshot_earlier.syscalls_data.keys() {
                let earlier_data = snapshot_earlier.syscalls_data.get(syscall).unwrap();
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

    fn targets_equal(targets1: &[i32], targets2: &[i32]) -> bool {
        if targets1.len() != targets2.len() {
            false
        } else {
            targets1
                .iter()
                .zip(targets2.iter())
                .filter(|&(a, b)| a == b)
                .count()
                == targets1.len()
        }
    }
}

struct IntervalMetrics {
    scale_metric: f64,
    idle_metric: f64,
    current_nr_targets: u32,
}

struct MetricsHistory {
    capacity: usize,
    buffer: Vec<IntervalMetrics>,
    // index of latest metricpoint
    next_index: usize,
}

impl MetricsHistory {
    pub fn new() -> Self {
        let capacity = 20;
        MetricsHistory {
            capacity,
            buffer: Vec::with_capacity(capacity),
            next_index: 0,
        }
    }

    #[allow(unused_must_use)]
    /// add a new interval metric to the history
    /// if buffer is full, the oldest entry is removed
    pub fn add(&mut self, datapoint: IntervalMetrics) {
        if self.next_index >= self.buffer.len() {
            self.buffer.push(datapoint);
        } else {
            std::mem::replace(&mut self.buffer[self.next_index], datapoint);
        }
        self.next_index = (self.next_index + 1) % self.capacity;
    }

    /// return the last interval metric datapoints, from newest to oldest
    pub fn last(&self) -> Vec<&IntervalMetrics> {
        let mut counter = self.buffer.len();
        // if next_index is 0, counter will be 0 -> index's garbage value does not matter
        let mut index = self.next_index - 1;
        let mut result = Vec::with_capacity(counter);
        while counter > 0 {
            println!("{}", index);
            result.push(self.buffer.get(index).unwrap());
            counter -= 1;
            index = if index == 0 {
                self.capacity - 1
            } else {
                index - 1
            };
        }
        result
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
    latest_snapshot_time: SystemTime,
}

impl ScalingAdapter {
    pub fn new(params: ScalingParameters) -> Result<ScalingAdapter, AdapterError> {
        let traceset = Traceset::new(&Vec::new(), &params.syscall_nrs)
            .ok_or(AdapterError::TracesetInitFailure)?;
        let initial_snapshot = traceset.get_snapshot();
        Ok(ScalingAdapter {
            parameters: params,
            traceset,
            metrics_history: MetricsHistory::new(),
            latest_snapshot: initial_snapshot,
            latest_snapshot_time: SystemTime::now(),
        })
    }

    pub fn add_tracee(&self, tracee_pid: i32) -> bool {
        self.traceset.register_target(tracee_pid)
    }

    pub fn remove_tracee(&self, tracee_pid: i32) -> bool {
        self.traceset.deregister_target(tracee_pid)
    }

    pub fn get_scaling_advice(&self) -> i32 {
        0
    }
}

#[cfg(test)]
mod tests {
    use crate::{IntervalMetrics, MetricsHistory};

    fn construct_dummy_history_big() -> MetricsHistory {
        let mut result = MetricsHistory::new();
        for i in 1..25 {
            let dummy = IntervalMetrics {
                scale_metric: i as f64,
                idle_metric: i as f64,
                current_nr_targets: i,
            };
            result.add(dummy);
        }
        result
    }

    #[test]
    fn metrics_history() {
        let history = construct_dummy_history_big();
        let mut latest = 24;
        for metrics in history.last() {
            assert_eq!(metrics.current_nr_targets, latest);
            latest -= 1;
        }
    }
}
