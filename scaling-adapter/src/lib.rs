#![allow(dead_code)]
use std::time::SystemTime;

use errors::AdapterError;
// need to make import public for it to be visible in dependant library/exe
// https://stackoverflow.com/questions/62933825/why-we-need-to-specify-all-dependenciesincluding-transitives-in-rust
pub use tracesets;
use tracesets::{SyscallData, Traceset, TracesetSnapshot};

mod errors;

pub struct IntervalData {
    pub read_bytes: u64,
    pub write_bytes: u64,
    // same order as syscall_nr vec passed in ScalingParameters
    pub syscalls_data: Vec<SyscallData>,
}

// as IntervalData is read-only this should be safe
unsafe impl std::marker::Send for IntervalData {}
unsafe impl std::marker::Sync for IntervalData {}

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
            let mut syscalls_data = Vec::new();
            for syscall in snapshot_earlier.syscalls_data.keys() {
                let earlier_data = snapshot_earlier.syscalls_data.get(syscall).unwrap();
                let later_data = snapshot_later.syscalls_data.get(syscall).unwrap();
                let count_diff = later_data.count - earlier_data.count;
                let time_diff = later_data.total_time - earlier_data.total_time;
                let syscall_data_diff = SyscallData {
                    count: count_diff,
                    total_time: time_diff,
                };
                syscalls_data.push(syscall_data_diff);
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

#[derive(Clone, Copy, Debug)]
pub struct IntervalMetrics {
    pub scale_metric: f64,
    pub idle_metric: f64,
    pub current_nr_targets: u32,
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

pub struct ScalingParameters {
    pub check_interval_ms: u64,
    pub syscall_nrs: Vec<i32>,
    // calc_interval_metrics: fn(&IntervalData) -> IntervalMetrics,
    // allow closures, but restrict to thread-safe (implement Send, Sync)
    pub calc_interval_metrics: Box<dyn Fn(&IntervalData) -> IntervalMetrics + Send + Sync>,
}

pub struct ScalingAdapter {
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

    /// take new snapshot and take difference with previous snapshot
    /// if interval is valid (amount of targets matches)
    ///      update history and return true
    /// else
    ///      return false
    pub fn update(&mut self) -> bool {
        let snapshot = self.traceset.get_snapshot();
        let snapshot_time = SystemTime::now();
        let interval_data = IntervalData::new(&self.latest_snapshot, &snapshot);
        match interval_data {
            Some(data) => {
                self.latest_snapshot = snapshot;
                self.latest_snapshot_time = snapshot_time;
                let metrics = (self.parameters.calc_interval_metrics)(&data);
                self.metrics_history.add(metrics);
                true
            }
            None => false,
        }
    }

    pub fn get_latest_metrics(&self) -> Option<&IntervalMetrics> {
        self.metrics_history.last().get(0).copied()
    }

    pub fn get_scaling_advice(&self) -> i32 {
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(self.latest_snapshot_time)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        if elapsed >= self.parameters.check_interval_ms as u128 {
            unimplemented!();
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        process::{Command, Stdio},
    };

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

    fn has_tracesets() -> bool {
        let mut script_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        script_path.push("../kernel_has_tracesets.sh");
        let process = match Command::new(script_path).stdout(Stdio::piped()).spawn() {
            Ok(process) => process,
            Err(err) => panic!("could not run kernel patch detection script: {}", err),
        };
        let output = match process.wait_with_output() {
            Ok(output) => output,
            Err(why) => panic!("couldn't read script stdout: {}", why),
        };
        let output = String::from_utf8(output.stdout).expect("valid utf8");
        output.starts_with("yes")
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

    #[cfg(target_os = "linux")]
    #[test]
    fn create_empty_adapter() {
        assert!(has_tracesets());
        let params = ScalingParameters {
            check_interval_ms: 1000,
            syscall_nrs: vec![1, 2],
            calc_interval_metrics: Box::new(|_data| IntervalMetrics {
                scale_metric: 0.0,
                idle_metric: 0.0,
                current_nr_targets: 0,
            }),
        };
        let adapter = ScalingAdapter::new(params);
        assert!(adapter.is_ok())
    }
}
