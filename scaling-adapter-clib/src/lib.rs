use std::{ptr, sync::RwLock};

use lazy_static::lazy_static;
use scaling_adapter::{IntervalData, IntervalMetrics, ScalingAdapter, ScalingParameters};
use tracesets::SyscallData;

lazy_static! {
    static ref ADAPTER: RwLock<Option<ScalingAdapter>> = RwLock::new(None);
    static ref CALC_F: RwLock<Option<CalcMetricsFunFFI>> = RwLock::new(None);
}

#[repr(C)]
pub struct IntervalDataFFI {
    read_bytes: u64,
    write_bytes: u64,
    syscalls_data: *const SyscallData,
}

impl IntervalDataFFI {
    pub fn new(data: &IntervalData) -> Self {
        IntervalDataFFI {
            read_bytes: data.read_bytes,
            write_bytes: data.write_bytes,
            syscalls_data: data.syscalls_data.as_ptr(),
        }
    }
}

#[repr(C)]
pub struct IntervalMetricsFFI {
    scale_metric: f64,
    idle_metric: f64,
    current_nr_targets: u32,
}

#[repr(C)]
pub struct ScalingParametersFFI {
    check_interval_ms: u64,
    syscall_nrs: Vec<i32>,
    calc_interval_metrics: fn(&IntervalDataFFI) -> IntervalMetricsFFI,
}

type CalcMetricsFunFFI = unsafe extern "C" fn(&IntervalDataFFI) -> IntervalMetricsFFI;

#[no_mangle]
pub extern "C" fn new_adapter(
    check_interval_ms: u64,
    syscall_nrs: *const i32,
    amount_syscalls: usize,
    calc_interval_metrics: CalcMetricsFunFFI,
) -> bool {
    let mut adapter_global = ADAPTER.write().unwrap();
    let mut calc_f_global = CALC_F.write().unwrap();
    *calc_f_global = Some(calc_interval_metrics);

    // create new vector from the passed C array of syscall numbers
    let syscalls_vec: Vec<i32> = unsafe {
        let mut syscalls_vec = Vec::with_capacity(amount_syscalls);
        syscalls_vec.set_len(amount_syscalls);
        ptr::copy(syscall_nrs, syscalls_vec.as_mut_ptr(), amount_syscalls);
        syscalls_vec
    };

    // convert C function pointer to correct Rust closure
    let calc_f = Box::new(|interval_data: &IntervalData| -> IntervalMetrics {
        let converted = IntervalDataFFI::new(interval_data);
        let metrics_ffi = unsafe { CALC_F.read().unwrap().unwrap()(&converted) };
        IntervalMetrics {
            scale_metric: metrics_ffi.scale_metric,
            idle_metric: metrics_ffi.idle_metric,
            current_nr_targets: metrics_ffi.current_nr_targets,
        }
    });

    let params = ScalingParameters {
        check_interval_ms,
        syscall_nrs: syscalls_vec,
        calc_interval_metrics: calc_f,
    };
    *adapter_global = ScalingAdapter::new(params).ok();
    (*adapter_global).is_some()
}
