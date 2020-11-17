use std::{ffi::CStr, os::raw::c_char, ptr, sync::RwLock};

use lazy_static::lazy_static;
use log::debug;
use scaling_adapter::tracesets::SyscallData;
use scaling_adapter::{IntervalData, IntervalDerivedData, ScalingAdapter, ScalingParameters};

type CalcMetricsFunFFI = unsafe extern "C" fn(&IntervalDataFFI) -> IntervalDerivedData;

// save adapter and the external C function for metrics calculation as globals
// these will be set when creating a scaling adapter -> maximum one adapter can be created
lazy_static! {
    static ref ADAPTER: RwLock<Option<ScalingAdapter>> = RwLock::new(None);
    // seems needed, wasn't able to constrain the passed C function to a static lifetime
    static ref CALC_METRICS_FFI: RwLock<Option<CalcMetricsFunFFI>> = RwLock::new(None);
}

#[repr(C)]
pub struct IntervalDataFFI {
    pub start_ms: u64,
    pub end_ms: u64,
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub syscalls_data: *const SyscallData,
    pub amount_targets: usize,
}

impl IntervalDataFFI {
    // create FFI version of IntervalData
    // the pointer to the SyscallData is used in the calculation function
    // when the original IntervalData reference is still valid
    pub fn new(data: &IntervalData) -> Self {
        IntervalDataFFI {
            start_ms: data.start_millis(),
            end_ms: data.start_millis(),
            read_bytes: data.read_bytes,
            write_bytes: data.write_bytes,
            syscalls_data: data.syscalls_data.as_ptr(),
            amount_targets: data.amount_targets,
        }
    }
}

#[repr(C)]
pub struct AdapterParameters {
    pub syscall_nrs: *const i32,
    pub amount_syscalls: usize,
    pub calc_interval_metrics: CalcMetricsFunFFI,
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
#[no_mangle]
/// create new adapter
/// adapter_params: tracked syscalls and metrics calculation function
/// algo_params: comma separated string of all algorithm parameters values (constants that tweak algo)
/// passing by string lets benchmarks use same code for all adapter versions
///
/// will panic for invalid algo parameter string, or invalid syscall number array
pub extern "C" fn new_adapter(
    parameters: &AdapterParameters,
    algo_params_str: *const c_char,
) -> bool {
    let mut adapter_global = ADAPTER.write().unwrap();
    let (syscalls_vec, calc_f) = convert_params(
        parameters.syscall_nrs,
        parameters.amount_syscalls,
        parameters.calc_interval_metrics,
    );

    let algo_parameters_str =
        unsafe { CStr::from_ptr(algo_params_str).to_str() }.expect("invalid parameters string");
    let algo_paramters = algo_parameters_str.split(',').collect::<Vec<&str>>();
    let check_interval_ms = algo_paramters
        .get(0)
        .expect("empty parameters string")
        .parse::<u64>()
        .expect("could not parse check interval ms");

    let params =
        ScalingParameters::new(syscalls_vec, calc_f).with_check_interval_ms(check_interval_ms);
    *adapter_global = ScalingAdapter::new(params).ok();
    (*adapter_global).is_some()
}

fn convert_params(
    syscall_nrs: *const i32,
    amount_syscalls: usize,
    calc_metrics_func: CalcMetricsFunFFI,
) -> (
    Vec<i32>,
    Box<dyn Fn(&IntervalData) -> IntervalDerivedData + Send + Sync>,
) {
    let mut calc_metrics_ffi_global = CALC_METRICS_FFI.write().unwrap();
    *calc_metrics_ffi_global = Some(calc_metrics_func);

    // create new vector from the passed C array of syscall numbers
    let syscalls_vec: Vec<i32> = unsafe {
        let mut syscalls_vec = Vec::with_capacity(amount_syscalls);
        syscalls_vec.set_len(amount_syscalls);
        ptr::copy(syscall_nrs, syscalls_vec.as_mut_ptr(), amount_syscalls);
        syscalls_vec
    };
    // convert C function pointer to correct Rust closure
    let calc_f = Box::new(|interval_data: &IntervalData| -> IntervalDerivedData {
        debug!("original interval data:{:?},", interval_data);
        let converted = IntervalDataFFI::new(interval_data);
        let derived_data = unsafe { CALC_METRICS_FFI.read().unwrap().unwrap()(&converted) };
        derived_data
    });

    (syscalls_vec, calc_f)
}

#[no_mangle]
pub extern "C" fn add_tracee(tracee_pid: i32) -> bool {
    let mut adapter_global = ADAPTER.write().unwrap();
    assert!((*adapter_global).is_some());
    let adapter = adapter_global.as_mut().unwrap();
    adapter.add_tracee(tracee_pid)
}

#[no_mangle]
pub extern "C" fn remove_tracee(tracee_pid: i32) -> bool {
    let mut adapter_global = ADAPTER.write().unwrap();
    assert!((*adapter_global).is_some());
    let adapter = adapter_global.as_mut().unwrap();
    adapter.remove_tracee(tracee_pid)
}

#[no_mangle]
pub extern "C" fn get_scaling_advice() -> i32 {
    let mut adapter_global = match ADAPTER.try_write() {
        Ok(option) => option,
        Err(_) => return 0,
    };
    assert!((*adapter_global).is_some());
    let adapter = adapter_global.as_mut().unwrap();
    adapter.get_scaling_advice()
}

#[no_mangle]
pub extern "C" fn close_adapter() {
    let mut adapter_global = ADAPTER.write().unwrap();
    assert!((*adapter_global).is_some());
    // this should automatically drop the adapter and thereby free the kernel space resources
    *adapter_global = None;
}

#[cfg(test)]
mod tests {
    use super::*;
    use env_logger::Env;
    use serial_test::serial;
    use std::{thread, time};
    use test_utils::{has_tracesets, spawn_sleeper};

    unsafe extern "C" fn dummy_calc_fn(_data: &IntervalDataFFI) -> IntervalDerivedData {
        IntervalDerivedData {
            scale_metric: 0.0,
            reset_metric: 0.0,
        }
    }

    unsafe extern "C" fn constant_calc_fn(data: &IntervalDataFFI) -> IntervalDerivedData {
        let mut syscalls_data_vec = Vec::with_capacity(1);
        syscalls_data_vec.set_len(1);
        ptr::copy(data.syscalls_data, syscalls_data_vec.as_mut_ptr(), 1);
        let nanosleep_call_count = syscalls_data_vec.get_unchecked(0).count;
        IntervalDerivedData {
            scale_metric: nanosleep_call_count as f64,
            reset_metric: 0.0,
        }
    }

    fn calc_new_interval_metrics() -> IntervalDerivedData {
        let mut adapter_global = ADAPTER.write().unwrap();
        assert!((*adapter_global).is_some());
        let adapter = adapter_global.as_mut().unwrap();
        // 1. update amount of targets, sleep long enough for one call to nanosleep, 2. update to get valid interval data
        adapter.update();
        thread::sleep(time::Duration::from_millis(2500));
        adapter.update();
        adapter.get_latest_metrics()
            .expect("latest metric to exist because between last two updates the amount of targets did not change")
            .derived_data
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[serial]
    fn create_close() {
        assert!(has_tracesets());
        let syscalls = vec![0, 1, 2];
        let parameters = AdapterParameters {
            check_interval_ms: 1000,
            syscall_nrs: syscalls.as_ptr(),
            amount_syscalls: syscalls.len(),
            calc_interval_metrics: dummy_calc_fn,
        };
        let is_created = new_adapter(&parameters);
        assert!(is_created);
        close_adapter();
    }

    #[cfg(target_os = "linux")]
    #[test]
    #[serial]
    fn with_target() {
        assert!(has_tracesets());
        let env = Env::default().filter_or("MY_LOG_LEVEL", "info");
        env_logger::init_from_env(env);
        // create child process that just sleeps in a loop
        let sleeper_process = spawn_sleeper();
        let sleeper_pid = sleeper_process.process.id();
        debug!("sleeper pid: {}", sleeper_pid);
        let wait_syscall_nr = 61;
        let syscalls = vec![wait_syscall_nr];
        // trace the nanosleep system call and set the scale_metric to the nanosleep call count
        let parameters = AdapterParameters {
            check_interval_ms: 1000,
            syscall_nrs: syscalls.as_ptr(),
            amount_syscalls: syscalls.len(),
            calc_interval_metrics: constant_calc_fn,
        };
        let is_created = new_adapter(&parameters);
        assert!(is_created);
        // add sleeper process to be traced
        let is_added = add_tracee(sleeper_pid as i32);
        assert!(is_added);
        // update adapter and get latest metric, verify scale_metric equals wait syscall count (should be more than 1)
        let lastest_metric = calc_new_interval_metrics();
        println!("latest metric: {:?}", &lastest_metric);
        assert!(lastest_metric.scale_metric > 0.9);
        // remove traceee
        let is_removed = remove_tracee(sleeper_pid as i32);
        assert!(is_removed);
        close_adapter();
    }
}
