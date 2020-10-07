#![allow(dead_code)]
use std::time::SystemTime;

use errors::AdapterError;
// need to make import public for it to be visible in dependant library/exe
// https://stackoverflow.com/questions/62933825/why-we-need-to-specify-all-dependenciesincluding-transitives-in-rust
use log::debug;
pub use tracesets;
use tracesets::{SyscallData, Traceset, TracesetSnapshot};

mod errors;

/// describes one interval during execution
/// all data is referring to the timeframe of interval
pub struct IntervalData {
    pub start: SystemTime,
    pub end: SystemTime,
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
        debug!(
            "create interval data, earlier snapshot targets: {:?}, new snapshot targets: {:?}",
            snapshot_earlier.targets, snapshot_later.targets
        );
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
            let start = snapshot_earlier.timestamp;
            let end = snapshot_later.timestamp;
            Some(IntervalData {
                start,
                end,
                read_bytes,
                write_bytes,
                syscalls_data,
                amount_targets,
            })
        } else {
            None
        }
    }

    // can safely use as_millis as u64 (only overflow at unix epoch + half billion years)
    pub fn start_millis(&self) -> u64 {
        self.start
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("interval start before unix epoch")
            .as_millis() as u64
    }

    // can safely use as_millis as u64 (only overflow at unix epoch + half billion years)
    pub fn end_millis(&self) -> u64 {
        self.end
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("interval start before unix epoch")
            .as_millis() as u64
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
        debug!(
            "getting last interval metrics, buffer size: {}, current next_index: {}",
            counter, self.next_index
        );
        // if next_index is 0, counter will be 0 -> index's garbage value does not matter
        // TODO: fix bug, where index goes below 0
        let mut result = Vec::with_capacity(counter);
        let mut index = self.next_index;
        while counter > 0 {
            index = if index == 0 {
                self.capacity - 1
            } else {
                index - 1
            };
            result.push(self.buffer.get(index).unwrap());
            counter -= 1;
        }
        result
    }

    /// get interval metrics for specified interval
    /// where index = 0 specifies latest interval, index = 1 previous etc.
    pub fn get(&self, index: usize) -> Option<&IntervalMetrics> {
        if index >= self.buffer.len() {
            return None;
        }
        let buffer_index_latest = (self.next_index as i32) - 1;
        let buffer_index =
            ((buffer_index_latest - (index as i32)) % (self.capacity as i32)) as usize;
        self.buffer.get(buffer_index)
    }

    pub fn size(&self) -> usize {
        self.buffer.len()
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
    recent_invalid_intervals: usize,
    // 0 < x < 1, margin of error when comparing scale metrics
    stability_factor: f64,
}

// synchronize access by wrapping with Arc<Mutex<_>>
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
            recent_invalid_intervals: 0,
            stability_factor: 0.9,
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
        let is_success = match interval_data {
            Some(data) => {
                let metrics = (self.parameters.calc_interval_metrics)(&data);
                let history_point = IntervalMetrics {
                    derived_data: metrics,
                    amount_targets: self.latest_snapshot.targets.len(),
                    interval_start: self.latest_snapshot_time,
                    interval_end: snapshot_time,
                };
                self.metrics_history.add(history_point);
                self.recent_invalid_intervals = 0;
                true
            }
            None => {
                self.recent_invalid_intervals += 1;
                false
            }
        };
        self.latest_snapshot = snapshot;
        self.latest_snapshot_time = snapshot_time;
        is_success
    }

    pub fn get_latest_metrics(&self) -> Option<&IntervalMetrics> {
        self.metrics_history.last().get(0).copied()
    }

    pub fn get_scaling_advice(&mut self) -> i32 {
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(self.latest_snapshot_time)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        if elapsed >= self.parameters.check_interval_ms as u128 {
            self.update();
            // if latest interval not valid (amount targets changed)
            if self.recent_invalid_intervals > 0 {
                0
            }
            // after first valid interval always try scaling up
            else if self.metrics_history.size() == 1 {
                1
            }
            // otherwise compare latest interval with previous
            // metrics_history must already contain 2 entries
            else {
                // TODO: fix bug that causes metrics_history.get(0) to return None here
                let latest = self.metrics_history.get(0).unwrap();
                let previous = self.metrics_history.get(1).unwrap();
                if latest.derived_data.scale_metric * self.stability_factor
                    > previous.derived_data.scale_metric
                {
                    1
                } else if previous.derived_data.scale_metric * self.stability_factor
                    > latest.derived_data.scale_metric && self.traceset.get_amount_targets() > 1
                {
                    -1
                } else {
                    0
                }
            }
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger::Env;
    use std::sync::Once;
    use std::{thread, time};
    use test_utils::{has_tracesets, spawn_echoer};

    static INIT: Once = Once::new();

    /// Setup function that is only run once, even if called multiple times.
    fn setup() {
        INIT.call_once(|| {
            let env = Env::default().filter_or("MY_LOG_LEVEL", "debug");
            env_logger::init_from_env(env);
        });
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
        setup();
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
        thread::sleep(time::Duration::from_millis(1000));
        // update adapter and get latest metric, verify scale_metric is > 0
        let interval_valid = adapter.update();
        // frst interval should not be valid, amount of targets changed
        assert!(!interval_valid);
        thread::sleep(time::Duration::from_millis(2000));
        let interval_valid = adapter.update();
        // second interval should be valid, amount of targets did not change
        assert!(interval_valid);
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
