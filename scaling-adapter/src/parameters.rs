use crate::intervals::{IntervalData, IntervalDerivedData};

pub struct ScalingParameters {
    pub syscall_nrs: Vec<i32>,
    // calc_interval_metrics: fn(&IntervalData) -> IntervalMetrics,
    // allow closures, but restrict to thread-safe (implement Send, Sync)
    pub calc_metrics: Box<dyn Fn(&IntervalData) -> IntervalDerivedData + Send + Sync>,
    /// minimum amount of time to pass before new interval starts
    pub check_interval_ms: u64,
    /// 0 < x < 1, margin of error when comparing scale metrics
    pub stability_factor: f64,
}

impl Default for ScalingParameters {
    fn default() -> Self {
        // read, write, fsync, openat, unlink (just use these for now)
        // remember: can trace max 8 syscalls
        let syscall_nrs = [0, 1, 74, 257, 87].to_vec();
        let calc_metrics = Box::new(|data: &IntervalData| {
            let rw_bytes = data.write_bytes + data.read_bytes;
            let interval_ms = data.end_millis() - data.start_millis();
            let throughput = rw_bytes as f64 / interval_ms as f64;
            let agg_syscall_time: u64 = data.syscalls_data.iter().map(|sd| sd.total_time).sum();
            let secondary_metric = (agg_syscall_time * data.blkio_delay) as f64
                / (interval_ms * data.amount_targets as u64) as f64;
            IntervalDerivedData {
                scale_metric: throughput,
                reset_metric: secondary_metric,
            }
        });
        ScalingParameters {
            syscall_nrs,
            calc_metrics,
            check_interval_ms: 1000,
            stability_factor: 0.9,
        }
    }
}

impl ScalingParameters {
    pub fn new(
        syscall_nrs: Vec<i32>,
        calc_metrics: Box<dyn Fn(&IntervalData) -> IntervalDerivedData + Send + Sync>,
    ) -> Self {
        let default_check_interval_ms = 1000;
        let default_stability_factor = 0.9;
        ScalingParameters {
            syscall_nrs,
            calc_metrics,
            check_interval_ms: default_check_interval_ms,
            stability_factor: default_stability_factor,
        }
    }

    /// take params separated as string "<param1>,<param2>"
    /// same order as in struct
    pub fn with_algo_params(mut self, params_untyped: &str) -> Self {
        let param_strs = params_untyped.split(',').collect::<Vec<&str>>();
        let check_interval_ms: u64 = param_strs
            .get(0)
            .expect("malformatted params string")
            .parse()
            .expect("invalid check interval ms parameter");
        let stability_factor: f64 = param_strs
            .get(1)
            .expect("malformatted params string")
            .parse()
            .expect("invalid stability factor parameter");
        self.check_interval_ms = check_interval_ms;
        self.stability_factor = stability_factor;
        self
    }

    pub fn with_check_interval_ms(mut self, check_interval_ms: u64) -> Self {
        self.check_interval_ms = check_interval_ms;
        self
    }

    /// stability factor should be > 0 and < 1
    pub fn with_stability_factor(mut self, stability_factor: f64) -> Self {
        self.stability_factor = stability_factor;
        self
    }
}
