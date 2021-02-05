#![allow(dead_code)]
use std::time::SystemTime;

use crate::intervals::{AveragedIntervalMetrics, IntervalMetrics};
use log::debug;

pub struct MetricsHistory {
    capacity: usize,
    buffer: Vec<IntervalMetrics>,
    // index of latest metricpoint
    next_index: usize,
}

impl MetricsHistory {
    pub fn new() -> Self {
        let capacity = 100;
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
        debug!(
            "adding interval metrics to history at buffer index {}",
            self.next_index
        );
        if self.next_index >= self.buffer.len() {
            self.buffer.push(datapoint);
        } else {
            std::mem::replace(&mut self.buffer[self.next_index], datapoint);
        }
        self.next_index = (self.next_index + 1) % self.capacity;
    }

    /// return the last interval metric datapoints, from newest to oldest
    /// if given timestamp, only return intervals that started afterwards
    pub fn last(&self, since: Option<SystemTime>) -> Vec<&IntervalMetrics> {
        debug!(
            "getting last interval metrics, buffer size: {}, current next_index: {}",
            self.buffer.len(),
            self.next_index
        );
        let buffer_size = self.buffer.len();
        let mut result = Vec::with_capacity(buffer_size);
        match since {
            Some(time) => {
                for i in 0..buffer_size {
                    // maximum index is buffer size - 1, safe to unrwap option
                    let im = self.get(i).unwrap();
                    if im.interval_start < time {
                        break;
                    }
                    result.push(self.get(i).unwrap());
                }
            }
            None => {
                for i in 0..buffer_size {
                    result.push(self.get(i).unwrap());
                }
            }
        }
        result
    }

    /// get interval metrics for specified interval
    /// where index = 0 specifies latest interval, index = 1 previous etc.
    pub fn get(&self, index: usize) -> Option<&IntervalMetrics> {
        if index >= self.buffer.len() {
            return None;
        }
        // specified index denotes how many intervals before (next_index - 1)
        let buffer_index_unconverted = (self.next_index as i32) - 1 - (index as i32);
        let buffer_index = if buffer_index_unconverted >= 0 {
            buffer_index_unconverted as usize
        } else {
            ((self.capacity as i32) + buffer_index_unconverted) as usize
        };
        self.buffer.get(buffer_index)
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.next_index = 0;
    }

    pub fn size(&self) -> usize {
        self.buffer.len()
    }
}

pub struct AveragedMetricsHistory {
    capacity: usize,
    buffer: Vec<AveragedIntervalMetrics>,
    // index of latest metricpoint
    next_index: usize,
}

impl AveragedMetricsHistory {
    pub fn new() -> Self {
        let capacity = 5;
        AveragedMetricsHistory {
            capacity,
            buffer: Vec::with_capacity(capacity),
            next_index: 0,
        }
    }

    #[allow(unused_must_use)]
    /// add a new interval metric to the history
    /// if buffer is full, the oldest entry is removed
    pub fn add(&mut self, datapoint: AveragedIntervalMetrics) {
        debug!(
            "adding interval metrics to history at buffer index {}",
            self.next_index
        );
        if self.next_index >= self.buffer.len() {
            self.buffer.push(datapoint);
        } else {
            std::mem::replace(&mut self.buffer[self.next_index], datapoint);
        }
        self.next_index = (self.next_index + 1) % self.capacity;
    }

    /// return the last interval metric datapoints, from newest to oldest
    pub fn last(&self) -> Vec<&AveragedIntervalMetrics> {
        debug!(
            "getting last interval metrics, buffer size: {}, current next_index: {}",
            self.buffer.len(),
            self.next_index
        );
        let buffer_size = self.buffer.len();
        let mut result = Vec::with_capacity(buffer_size);
        for i in 0..buffer_size {
            // maximum index is buffer size - 1, safe to unrwap option
            result.push(self.get(i).unwrap());
        }
        result
    }

    /// get interval metrics for specified interval
    /// where index = 0 specifies latest interval, index = 1 previous etc.
    pub fn get(&self, index: usize) -> Option<&AveragedIntervalMetrics> {
        if index >= self.buffer.len() {
            return None;
        }
        // specified index denotes how many intervals before (next_index - 1)
        let buffer_index_unconverted = (self.next_index as i32) - 1 - (index as i32);
        let buffer_index = if buffer_index_unconverted >= 0 {
            buffer_index_unconverted as usize
        } else {
            ((self.capacity as i32) + buffer_index_unconverted) as usize
        };
        self.buffer.get(buffer_index)
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.next_index = 0;
    }

    pub fn size(&self) -> usize {
        self.buffer.len()
    }
}
