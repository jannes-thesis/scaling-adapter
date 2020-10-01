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
    pub amount_targets: usize,
}

// as IntervalData is read-only this should be safe
unsafe impl std::marker::Send for IntervalData {}
unsafe impl std::marker::Sync for IntervalData {}

impl IntervalData {
    pub fn new(
        snapshot_earlier: &TracesetSnapshot,
        snapshot_later: &TracesetSnapshot,
    ) -> Option<IntervalData> {
        let targets_match = snapshot_earlier.targets.eq(&snapshot_later.targets);
        if targets_match {
            let read_bytes = snapshot_later.read_bytes - snapshot_earlier.read_bytes;
            let write_bytes = snapshot_later.write_bytes - snapshot_earlier.write_bytes;
            let amount_targets = snapshot_earlier.targets.len();
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
                amount_targets,
            })
        } else {
            None
        }
    }
}

#[cfg(feature = "c_repr")]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct IntervalDerivedData {
    pub scale_metric: f64,
    pub idle_metric: f64,
}

#[cfg(not(feature = "c_repr"))]
#[derive(Clone, Copy, Debug)]
pub struct IntervalDerivedData {
    pub scale_metric: f64,
    pub idle_metric: f64,
}

pub struct IntervalMetrics {
    pub derived_data: IntervalDerivedData,
    pub amount_targets: usize,
    pub interval_start: SystemTime,
    pub interval_end: SystemTime,
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
    pub calc_interval_metrics: Box<dyn Fn(&IntervalData) -> IntervalDerivedData + Send + Sync>,
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

    pub fn add_tracee(&mut self, tracee_pid: i32) -> bool {
        self.traceset.register_target(tracee_pid)
    }

    pub fn remove_tracee(&mut self, tracee_pid: i32) -> bool {
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
                let metrics = (self.parameters.calc_interval_metrics)(&data);
                let history_point = IntervalMetrics {
                    derived_data: metrics,
                    amount_targets: self.latest_snapshot.targets.len(),
                    interval_start: self.latest_snapshot_time,
                    interval_end: snapshot_time,
                };
                self.metrics_history.add(history_point);
                self.latest_snapshot = snapshot;
                self.latest_snapshot_time = snapshot_time;
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
        process::{Child, Command, Stdio},
        thread, time,
    };

    // need to wrap child process so we can auto cleanup when tests panic
    struct ProcessWrapper {
        pub process: Child,
    }

    impl Drop for ProcessWrapper {
        fn drop(&mut self) {
            let _ = self.process.kill();
        }
    }

    fn spawn_echoer() -> ProcessWrapper {
        ProcessWrapper {
            process: Command::new("bash")
            .arg("-c")
            .arg("while true; do echo hi; sleep 1; done")
            .spawn()
            .expect("bash command to exist"),
        }
    }

    fn construct_dummy_history_big() -> MetricsHistory {
        let mut result = MetricsHistory::new();
        for i in 1..25 {
            let dummy = IntervalMetrics {
                derived_data: IntervalDerivedData {
                    scale_metric: i as f64,
                    idle_metric: i as f64,
                },
                amount_targets: i,
                interval_start: SystemTime::now(),
                interval_end: SystemTime::now(),
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
            assert_eq!(metrics.amount_targets, latest);
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
            calc_interval_metrics: Box::new(|_data| IntervalDerivedData {
                scale_metric: 0.0,
                idle_metric: 0.0,
            }),
        };
        let adapter = ScalingAdapter::new(params);
        assert!(adapter.is_ok())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn adapter_with_target() {
        assert!(has_tracesets());
        // create child process that just echos "hi" in a loop
        let echoer = spawn_echoer();
        let echoer_pid = echoer.process.id();
        println!("pid of echoer: {}", echoer_pid);
        let write_syscall_nr = 61;
        let syscalls = vec![write_syscall_nr];
        // trace the write system call (should be called for every echo)
        // and set the scale_metric to the close call count
        let params = ScalingParameters {
            check_interval_ms: 1000,
            syscall_nrs: syscalls,
            calc_interval_metrics: Box::new(|data| IntervalDerivedData {
                scale_metric: data.syscalls_data.get(0).unwrap().count as f64,
                idle_metric: 0.0,
            }),
        };
        let mut adapter = match ScalingAdapter::new(params) {
            Ok(a) => a,
            _ => panic!("adapter creation failed"),
        };
        // add sleeper process to be traced
        let is_added = adapter.add_tracee(echoer_pid as i32);
        assert!(is_added);
        // update adapter and get latest metric, verify scale_metric is > 0
        adapter.update();
        thread::sleep(time::Duration::from_millis(20000));
        adapter.update();
        let latest_metrics = adapter
            .get_latest_metrics()
            .expect("adapter should have at least one datapoint in metrics history");
        println!("latest metric: {:?}", latest_metrics.derived_data);
        assert!(latest_metrics.derived_data.scale_metric > 0.9);
        // remove traceee
        let is_removed = adapter.remove_tracee(echoer_pid as i32);
        assert!(is_removed);
    }
}
