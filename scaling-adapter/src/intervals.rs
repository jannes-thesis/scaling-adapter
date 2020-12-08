#![allow(dead_code)]
use std::time::{SystemTime};

use log::debug;
use tracesets::{SyscallData, TracesetSnapshot};


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
            let blkio_delay = snapshot_later.blkio_delay - snapshot_earlier.blkio_delay;
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

pub struct IntervalMetrics {
    pub derived_data: IntervalDerivedData,
    pub amount_targets: usize,
    pub interval_start: SystemTime,
    pub interval_end: SystemTime,
}