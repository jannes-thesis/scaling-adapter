use scaling_adapter::{IntervalDerivedData, ScalingAdapter, ScalingParameters};

mod utils;

#[test]
fn simple_test() {
    let params = ScalingParameters {
        check_interval_ms: 1000,
        syscall_nrs: vec![1, 2],
        calc_interval_metrics: Box::new(|data|  IntervalDerivedData {
            scale_metric: data.write_bytes as f64,
            idle_metric: data.write_bytes as f64,
        }),
    };
    let _adapter = ScalingAdapter::new(params).expect("adapter creation failed");
    for i in 1..10 {
        assert!(i < 10);
        utils::write_garbage(i);
    }
}
