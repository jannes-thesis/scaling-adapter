#![allow(dead_code)]
use std::time::SystemTime;

use log::{debug, info};
use tracesets::{SyscallData, TracesetSnapshot};

use crate::statistics::{mean, std_deviation};

/// describes one interval during execution
/// all data is referring to the timeframe of interval
#[derive(Debug)]
pub struct IntervalData {
    pub start: SystemTime,
    pub end: SystemTime,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub blkio_delay: u64,
    // same order as syscall_nr vec passed in ScalingParameters
    pub syscalls_data: Vec<SyscallData>,
    pub amount_targets: usize,
}

// as IntervalData is read-only this should be safe
unsafe impl std::marker::Send for IntervalData {}
unsafe impl std::marker::Sync for IntervalData {}

fn subtract_or_zero(a: u64, b: u64, context: &str) -> u64 {
    match a.checked_sub(b) {
        Some(result) => result,
        None => {
            info!("u64 subtract overflow, context: {}", context);
            0
        }
    }
}

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
            let read_bytes =
                subtract_or_zero(snapshot_later.read_bytes, snapshot_earlier.read_bytes, "rb");
            let write_bytes = subtract_or_zero(
                snapshot_later.write_bytes,
                snapshot_earlier.write_bytes,
                "wb",
            );
            let blkio_delay = subtract_or_zero(
                snapshot_later.blkio_delay,
                snapshot_earlier.blkio_delay,
                "blkio",
            );
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
                blkio_delay,
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
    pub reset_metric: f64,
}

#[cfg(not(feature = "c_repr"))]
#[derive(Clone, Copy, Debug)]
pub struct IntervalDerivedData {
    pub scale_metric: f64,
    pub reset_metric: f64,
}

fn system_time_to_millis(st: &SystemTime) -> u64 {
    st.duration_since(SystemTime::UNIX_EPOCH)
        .expect("interval start before unix epoch")
        .as_millis() as u64
}

pub struct IntervalMetrics {
    pub derived_data: IntervalDerivedData,
    pub amount_targets: usize,
    pub interval_start: SystemTime,
    pub interval_end: SystemTime,
}

impl IntervalMetrics {
    pub fn start_millis(&self) -> u64 {
        system_time_to_millis(&self.interval_start)
    }

    pub fn end_millis(&self) -> u64 {
        system_time_to_millis(&self.interval_end)
    }

    pub fn duration_millis(&self) -> u64 {
        self.end_millis() - self.start_millis()
    }
}

// the averages and stddevs of target metrics over several intervals
#[derive(Debug)]
pub struct AveragedIntervalMetrics {
    pub derived_data_avg: IntervalDerivedData,
    pub derived_data_stddev: IntervalDerivedData,
    pub interval_start: SystemTime,
    pub interval_end: SystemTime,
    pub amount_intervals: usize,
}

impl AveragedIntervalMetrics {
    // first metric should be newest, last oldest
    pub fn compute(interval_metrics: Vec<&IntervalMetrics>) -> Self {
        if interval_metrics.is_empty() {
            panic!("expected at least one interval metrics");
        }
        let start = interval_metrics.last().unwrap().interval_start;
        let end = interval_metrics.first().unwrap().interval_end;
        let duration_millis = system_time_to_millis(&end) - system_time_to_millis(&start);
        let mut datapoints_each_milli = Vec::with_capacity(duration_millis as usize);
        for m in &interval_metrics {
            for _x in 0..m.duration_millis() {
                datapoints_each_milli.push(m.derived_data);
            }
        }
        let avg_m1 = mean(
            &datapoints_each_milli
                .iter()
                .map(|data| data.scale_metric)
                .collect::<Vec<f64>>(),
        );
        let avg_m2 = mean(
            &datapoints_each_milli
                .iter()
                .map(|data| data.reset_metric)
                .collect::<Vec<f64>>(),
        );
        let stddev_m1 = std_deviation(
            &datapoints_each_milli
                .iter()
                .map(|data| data.scale_metric)
                .collect::<Vec<f64>>(),
        );
        let stddev_m2 = std_deviation(
            &datapoints_each_milli
                .iter()
                .map(|data| data.reset_metric)
                .collect::<Vec<f64>>(),
        );
        let derived_data_avg = IntervalDerivedData {
            scale_metric: avg_m1,
            reset_metric: avg_m2,
        };
        let derived_data_stddev = IntervalDerivedData {
            scale_metric: stddev_m1,
            reset_metric: stddev_m2,
        };
        Self {
            derived_data_avg,
            derived_data_stddev,
            interval_start: start,
            interval_end: end,
            amount_intervals: interval_metrics.len(),
        }
    }

    pub fn start_millis(&self) -> u64 {
        system_time_to_millis(&self.interval_start)
    }

    pub fn end_millis(&self) -> u64 {
        system_time_to_millis(&self.interval_end)
    }

    pub fn duration_millis(&self) -> u64 {
        self.end_millis() - self.start_millis()
    }
}
