#![allow(dead_code)]
use std::time::{Duration, SystemTime};

use errors::AdapterError;
use intervals::IntervalMetrics;
use log::debug;
use tracesets::{Traceset, TracesetSnapshot};
use AdapterState::Settled;

// need to make import public for it to be visible in dependant library/exe
// https://stackoverflow.com/questions/62933825/why-we-need-to-specify-all-dependenciesincluding-transitives-in-rust
pub use intervals::{IntervalData, IntervalDerivedData};
pub use parameters::ScalingParameters;
pub use tracesets;

mod errors;
mod intervals;
mod parameters;

pub struct ScalingAdapter {
    parameters: ScalingParameters,
    traceset: Traceset,
    state: AdapterState,
    metrics_history: MetricsHistory,
    latest_snapshot: TracesetSnapshot,
    latest_snapshot_time: SystemTime,
    recent_invalid_intervals: usize,
    // maximum of idle metric in current phase
    max_reset_metric: f64,
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
            state: AdapterState::Startup,
            metrics_history: MetricsHistory::new(),
            latest_snapshot: initial_snapshot,
            latest_snapshot_time: SystemTime::now(),
            recent_invalid_intervals: 0,
            max_reset_metric: 0.0,
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
                let metrics = (self.parameters.calc_metrics)(&data);
                let history_point = IntervalMetrics {
                    derived_data: metrics,
                    amount_targets: self.latest_snapshot.targets.len(),
                    interval_start: self.latest_snapshot_time,
                    interval_end: snapshot_time,
                };
                self.max_reset_metric =
                    if self.max_reset_metric > history_point.derived_data.reset_metric {
                        self.max_reset_metric
                    } else {
                        history_point.derived_data.reset_metric
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

    fn scaling_advice_startup(&mut self) -> i32 {
        self.state = AdapterState::Scaling(1);
        1
    }

    fn scaling_advice_settled(&mut self, last_direction: Direction) -> i32 {
        match last_direction {
            Direction::Up => {
                debug!("{}", "Exploring DOWN");
                self.state = AdapterState::Exploring(Direction::Down);
                -1
            }
            Direction::Down => {
                debug!("{}", "Exploring UP");
                self.state = AdapterState::Exploring(Direction::Up);
                1
            }
        }
    }

    fn scaling_advice_exploring(&mut self, direction: Direction) -> i32 {
        // compare latest interval with previous
        // metrics_history must already contain 2 entries
        let latest = self.metrics_history.get(0).unwrap();
        let previous = self.metrics_history.get(1).unwrap();
        let step_size = match direction {
            Direction::Up => 1,
            Direction::Down => -1,
        };
        // enter scaling state
        if latest.derived_data.scale_metric * self.parameters.stability_factor
            > previous.derived_data.scale_metric
        {
            self.state = AdapterState::Scaling(step_size);
            step_size
        // scale back to previous & enter settled state
        // set timeout for next explore move
        } else {
            self.state = Settled(
                SystemTime::now()
                    .checked_add(Duration::from_millis(2000))
                    .unwrap(),
                direction,
            );
            step_size
        }
    }

    fn scaling_advice_scaling(&mut self, step_size: i32) -> i32 {
        // compare latest interval with previous
        // metrics_history must already contain 2 entries
        let latest = self.metrics_history.get(0).unwrap();
        let previous = self.metrics_history.get(1).unwrap();
        let direction = Direction::from_step_size(step_size);
        // step sizes will always grow 1 -> 2 -> 4
        let new_step_size = if step_size.abs() < 4 {
            step_size + 1
        } else {
            step_size
        };
        // scale further
        if latest.derived_data.scale_metric * self.parameters.stability_factor
            > previous.derived_data.scale_metric
        {
            self.state = AdapterState::Scaling(new_step_size);
            new_step_size
        // scale back to previous & enter settled state
        // set no timeout, so next action will be exploring step
        } else if previous.derived_data.scale_metric * self.parameters.stability_factor
            > latest.derived_data.scale_metric
            && self.traceset.get_amount_targets() > 1
        {
            self.state = Settled(SystemTime::now(), direction);
            -step_size
        // enter settled state
        } else {
            self.state = Settled(SystemTime::now(), direction);
            0
        }
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
                return 0;
            }
            match self.state {
                AdapterState::Startup => self.scaling_advice_startup(),
                AdapterState::Settled(timeout, direction) => {
                    if SystemTime::now() > timeout {
                        self.scaling_advice_settled(direction)
                    } else {
                        0
                    }
                }
                AdapterState::Scaling(i) => self.scaling_advice_scaling(i),
                AdapterState::Exploring(direction) => self.scaling_advice_exploring(direction),
            }
        } else {
            0
        }
    }
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
    pub fn last(&self) -> Vec<&IntervalMetrics> {
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

#[derive(Clone, Copy)]
enum Direction {
    Up,
    Down,
}

impl Direction {
    pub fn get_opposite(&self) -> Direction {
        match self {
            Direction::Up => Direction::Down,
            Direction::Down => Direction::Up,
        }
    }

    pub fn from_step_size(step_size: i32) -> Direction {
        if step_size >= 0 {
            Direction::Up
        } else {
            Direction::Down
        }
    }
}

enum AdapterState {
    Startup,
    Scaling(i32),
    Exploring(Direction),
    Settled(SystemTime, Direction),
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger::Env;
    use std::{sync::Once, time::Duration};
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
                    reset_metric: i as f64,
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
        let latest_metrics = history.get(0);
        let previous_metrics = history.get(0);
        assert!(latest_metrics.is_some());
        assert!(previous_metrics.is_some());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn create_empty_adapter() {
        assert!(has_tracesets());
        let params = ScalingParameters::new(
            vec![1, 2],
            Box::new(|_data| IntervalDerivedData {
                scale_metric: 0.0,
                reset_metric: 0.0,
            }),
        );
        let adapter = ScalingAdapter::new(params);
        assert!(adapter.is_ok())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn empty_adapter_scaling_advice() {
        assert!(has_tracesets());
        let params = ScalingParameters::new(
            vec![1, 2],
            Box::new(|_data| IntervalDerivedData {
                scale_metric: 0.0,
                reset_metric: 0.0,
            }),
        )
        .with_check_interval_ms(1);
        let mut adapter = ScalingAdapter::new(params).unwrap();
        for i in 0..45 {
            println!("index: {}", i);
            let _advice = adapter.get_scaling_advice();
            thread::sleep(Duration::from_millis(10));
        }
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
        let write_syscall_nr = 1;
        let syscalls = vec![write_syscall_nr];
        // trace the write system call (should be called for every echo)
        // and set the scale_metric to the close call count
        let params = ScalingParameters::new(
            syscalls,
            Box::new(|data| IntervalDerivedData {
                scale_metric: data.syscalls_data.get(0).unwrap().count as f64,
                reset_metric: 0.0,
            }),
        );
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
