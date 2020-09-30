use std::{ptr, sync::RwLock};

use lazy_static::lazy_static;
use scaling_adapter::{IntervalData, IntervalMetrics, ScalingAdapter, ScalingParameters};
use scaling_adapter::tracesets::SyscallData;

type CalcMetricsFunFFI = unsafe extern "C" fn(&IntervalDataFFI) -> IntervalMetricsFFI;

// save adapter and the external C function for metrics calculation as globals
// these will be set when creating a scaling adapter -> maximum one adapter can be created
lazy_static! {
    static ref ADAPTER: RwLock<Option<ScalingAdapter>> = RwLock::new(None);
    // seems needed, wasn't able to constrain the passed C function to a static lifetime
    static ref CALC_METRICS_FFI: RwLock<Option<CalcMetricsFunFFI>> = RwLock::new(None);
}

#[repr(C)]
pub struct IntervalDataFFI {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub syscalls_data: *const SyscallData,
}

impl IntervalDataFFI {
    // create FFI version of IntervalData
    // the pointer to the SyscallData is used in the calculation function
    // when the original IntervalData reference is still valid
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

#[no_mangle]
pub extern "C" fn new_adapter(
    check_interval_ms: u64,
    syscall_nrs: *const i32,
    amount_syscalls: usize,
    calc_interval_metrics: CalcMetricsFunFFI,
) -> bool {
    let mut adapter_global = ADAPTER.write().unwrap();
    let mut calc_metrics_ffi_global = CALC_METRICS_FFI.write().unwrap();
    *calc_metrics_ffi_global = Some(calc_interval_metrics);

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
        let metrics_ffi = unsafe { CALC_METRICS_FFI.read().unwrap().unwrap()(&converted) };
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
