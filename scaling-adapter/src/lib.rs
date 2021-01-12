#![allow(dead_code)]
use std::time::{Duration, SystemTime};

use errors::AdapterError;
use history::{AveragedMetricsHistory, MetricsHistory};
use intervals::{AveragedIntervalMetrics, IntervalMetrics};
use log::{debug, info};
use tracesets::{Traceset, TracesetSnapshot};
use AdapterState::Settled;

// need to make import public for it to be visible in dependant library/exe
// https://stackoverflow.com/questions/62933825/why-we-need-to-specify-all-dependenciesincluding-transitives-in-rust
pub use intervals::{IntervalData, IntervalDerivedData};
pub use parameters::ScalingParameters;
pub use tracesets;

mod errors;
mod history;
mod intervals;
mod parameters;
mod statistics;

// duration of interval that snapshots are taken at
const INTERVAL_MS: u64 = 200;

// metric intervals are diffs of snapshot
// a metric interval is only valid if over the whole interval the amount of targets is unchanged
// averaged intervals are averages of metric intervals over param supplied amount of time
pub struct ScalingAdapter {
    parameters: ScalingParameters,
    traceset: Traceset,
    state: AdapterState,
    metrics_history: MetricsHistory,
    metrics_history_averaged: AveragedMetricsHistory,
    latest_snapshot: TracesetSnapshot,
    latest_snapshot_time: SystemTime,
    latest_avg_interval_end: SystemTime,
    recent_invalid_avg_intervals: usize,
}

// synchronize access by wrapping with Arc<Mutex<_>>
impl ScalingAdapter {
    pub fn new(params: ScalingParameters) -> Result<ScalingAdapter, AdapterError> {
        let traceset = Traceset::new(&Vec::new(), &params.syscall_nrs)
            .ok_or(AdapterError::TracesetInitFailure)?;
        let initial_snapshot = traceset.get_snapshot();
        info!("_I_AdapterInit");
        Ok(ScalingAdapter {
            parameters: params,
            traceset,
            state: AdapterState::Startup,
            metrics_history: MetricsHistory::new(),
            metrics_history_averaged: AveragedMetricsHistory::new(),
            latest_snapshot: initial_snapshot,
            latest_snapshot_time: SystemTime::now(),
            latest_avg_interval_end: SystemTime::now(),
            recent_invalid_avg_intervals: 0,
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
    pub fn update_history(&mut self) -> bool {
        let snapshot = self.traceset.get_snapshot();
        let snapshot_time = SystemTime::now();
        let interval_data = IntervalData::new(&self.latest_snapshot, &snapshot);
        let is_success = match interval_data {
            Some(data) => {
                debug!("UPDATE: {:?}", data);
                let metrics = (self.parameters.calc_metrics)(&data);
                let history_point = IntervalMetrics {
                    derived_data: metrics,
                    amount_targets: self.latest_snapshot.targets.len(),
                    interval_start: self.latest_snapshot_time,
                    interval_end: snapshot_time,
                };
                self.metrics_history.add(history_point);
                true
            }
            None => false,
        };
        self.latest_snapshot = snapshot;
        self.latest_snapshot_time = snapshot_time;
        is_success
    }

    pub fn get_latest_metrics(&self) -> Option<&IntervalMetrics> {
        self.metrics_history.last(None).get(0).copied()
    }

    fn update_avg_history(&mut self) -> bool {
        let new_intervals = self
            .metrics_history
            .last(Some(self.latest_avg_interval_end));
        if new_intervals.is_empty() {
            self.recent_invalid_avg_intervals += 1;
            return false;
        }
        self.latest_avg_interval_end = new_intervals.first().unwrap().interval_end;
        let avgd_interval = AveragedIntervalMetrics::compute(new_intervals);
        self.metrics_history_averaged.add(avgd_interval);
        self.recent_invalid_avg_intervals = 0;
        true
    }

    fn scaling_advice_startup(&mut self) -> i32 {
        self.state = AdapterState::Scaling(2);
        1
    }

    fn scaling_advice_settled(&mut self, last_direction: Direction) -> i32 {
        let latest = self.metrics_history_averaged.get(0).unwrap();
        let previous = self.metrics_history_averaged.get(1).unwrap();
        let direction = if latest.derived_data_avg.scale_metric * self.parameters.stability_factor
            > previous.derived_data_avg.scale_metric
        {
            // if factored throughput higher in new interval
            Direction::Up
        } else if previous.derived_data_avg.scale_metric * self.parameters.stability_factor
            > latest.derived_data_avg.scale_metric
            || latest.derived_data_stddev.scale_metric * 0.8
                > previous.derived_data_stddev.scale_metric
        {
            // if factored throughput lower in new interval
            // or factored stddev higher in new interval
            Direction::Down
        } else {
            // if throughput within stability bounds and factored stddev has not increased
            // do explore in opposite than last direction
            last_direction.get_opposite()
        };
        match direction {
            Direction::Down => {
                debug!("{}", "Exploring DOWN");
                self.state = AdapterState::Exploring(Direction::Down);
                -1
            }
            Direction::Up => {
                debug!("{}", "Exploring UP");
                self.state = AdapterState::Exploring(Direction::Up);
                1
            }
        }
    }

    fn scaling_advice_exploring(&mut self, direction: Direction) -> i32 {
        // compare latest interval with previous
        // metrics_history must already contain 2 entries
        let latest = self.metrics_history_averaged.get(0).unwrap();
        let previous = self.metrics_history_averaged.get(1).unwrap();
        let step_size = match direction {
            Direction::Up => 1,
            Direction::Down => -1,
        };
        // if higher factored perf: enter scaling state
        if latest.derived_data_avg.scale_metric * self.parameters.stability_factor
            > previous.derived_data_avg.scale_metric
        {
            self.state = AdapterState::Scaling(step_size);
            step_size
        // if (lower perf and exploring down) or exploring up:
        // scale back to previous & enter settled state
        // set timeout for next explore move
        } else if (previous.derived_data_avg.scale_metric > latest.derived_data_avg.scale_metric
            && direction == Direction::Down)
            || direction == Direction::Up
        {
            self.state = Settled(
                SystemTime::now()
                    .checked_add(Duration::from_millis(2000))
                    .unwrap(),
                direction,
            );
            -step_size
        }
        // if same or higher perf while exploring down:
        // keep exploring downwards with step size one
        else {
            step_size
        }
    }

    fn scaling_advice_scaling(&mut self, step_size: i32) -> i32 {
        // compare latest interval with previous
        // metrics_history must already contain 2 entries
        let latest = self.metrics_history_averaged.get(0).unwrap();
        let previous = self.metrics_history_averaged.get(1).unwrap();
        let direction = Direction::from_step_size(step_size);
        // step sizes will always grow 1 -> 2 -> 3 -> 4
        let new_step_size = if step_size.abs() < 4 {
            step_size + 1
        } else {
            step_size
        };
        // if higher factored perf, scale further
        if latest.derived_data_avg.scale_metric * self.parameters.stability_factor
            > previous.derived_data_avg.scale_metric
        {
            self.state = AdapterState::Scaling(new_step_size);
            new_step_size
        // enter settled state
        // set no timeout, so next action will be exploring step
        } else {
            self.state = Settled(SystemTime::now(), direction);
            0
        }
    }

    pub fn get_scaling_advice(&mut self, queue_size: i32) -> i32 {
        let now = SystemTime::now();
        let elapsed_since_snapshot = now
            .duration_since(self.latest_snapshot_time)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        // record new interval in metrics if interval ms passed
        if elapsed_since_snapshot >= INTERVAL_MS as u128 {
            self.update_history();
        }
        else {
            return 0;
        }
        let elapsed_since_latest_avg_interval_end = now
            .duration_since(self.latest_avg_interval_end)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        // if check advice period passed, compute advice
        if elapsed_since_latest_avg_interval_end >= self.parameters.check_interval_ms as u128 {
            self.update_avg_history();
            info!("ADVICE: new advice, enough time elapsed");
            // if latest avg interval not valid
            if self.recent_invalid_avg_intervals > 0 {
                info!("ADVICE: invalid averaged interval, advice 0");
                return 0;
            }
            info!("_I_QSIZE: {}", queue_size);
            info!("ADVICE: current state: {:?}", self.state);
            let advice = match self.state {
                AdapterState::Startup => return self.scaling_advice_startup(),
                AdapterState::Settled(timeout, direction) => {
                    if SystemTime::now() > timeout {
                        self.scaling_advice_settled(direction)
                    } else {
                        0
                    }
                }
                AdapterState::Scaling(i) => self.scaling_advice_scaling(i),
                AdapterState::Exploring(direction) => self.scaling_advice_exploring(direction),
            };
            info!("ADVICE: new state: {:?}", self.state);
            // at least one recent interval must exists, as we are not in startup state anymore
            let latest_avg_interval = self.metrics_history_averaged.get(0).unwrap();
            let interval_duration = latest_avg_interval.duration_millis();
            let m1 = latest_avg_interval.derived_data_avg.scale_metric;
            let m2 = latest_avg_interval.derived_data_avg.reset_metric;
            let m1_stddev = latest_avg_interval.derived_data_stddev.scale_metric;
            let m2_stddev = latest_avg_interval.derived_data_stddev.reset_metric;
            let amount_targets = self.traceset.get_amount_targets();
            info!("ADVICE: last interval ms: {}", interval_duration);
            info!("_I_PSIZE: {}", amount_targets);
            info!("_I_M1_VAL: {}", m1);
            info!("_I_M2_VAL: {}", m2);
            info!("_I_M1_STDDEV: {}", m1_stddev);
            info!("_I_M2_STDDEV: {}", m2_stddev);
            info!("ADVICE: {}", advice);
            advice
        } else {
            0
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
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

#[derive(Debug)]
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
            let _advice = adapter.get_scaling_advice(0);
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
        let interval_valid = adapter.update_history();
        // frst interval should not be valid, amount of targets changed
        assert!(!interval_valid);
        thread::sleep(time::Duration::from_millis(2000));
        let interval_valid = adapter.update_history();
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
