use std::{ptr, sync::RwLock};

use lazy_static::lazy_static;
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
            read_bytes: data.read_bytes,
            write_bytes: data.write_bytes,
            syscalls_data: data.syscalls_data.as_ptr(),
            amount_targets: data.amount_targets,
        }
    }
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
    let calc_f = Box::new(|interval_data: &IntervalData| -> IntervalDerivedData {
        let converted = IntervalDataFFI::new(interval_data);
        let derived_data = unsafe { CALC_METRICS_FFI.read().unwrap().unwrap()(&converted) };
        derived_data
    });

    let params = ScalingParameters {
        check_interval_ms,
        syscall_nrs: syscalls_vec,
        calc_interval_metrics: calc_f,
    };
    *adapter_global = ScalingAdapter::new(params).ok();
    (*adapter_global).is_some()
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
    let adapter_global = match ADAPTER.try_write() {
        Ok(option) => option,
        Err(_) => return 0,
    };
    assert!((*adapter_global).is_some());
    let adapter = adapter_global.as_ref().unwrap();
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
    use serial_test::serial;
    use std::{
        process::{Child, Command},
        thread, time,
    };

    unsafe extern "C" fn dummy_calc_fn(_data: &IntervalDataFFI) -> IntervalDerivedData {
        IntervalDerivedData {
            scale_metric: 0.0,
            idle_metric: 0.0,
        }
    }

    unsafe extern "C" fn constant_calc_fn(data: &IntervalDataFFI) -> IntervalDerivedData {
        let mut syscalls_data_vec = Vec::with_capacity(1);
        syscalls_data_vec.set_len(1);
        ptr::copy(data.syscalls_data, syscalls_data_vec.as_mut_ptr(), 1);
        let nanosleep_call_count = syscalls_data_vec.get_unchecked(0).count;
        IntervalDerivedData {
            scale_metric: nanosleep_call_count as f64,
            idle_metric: 0.0,
        }
    }

    fn spawn_sleeper() -> Child {
        Command::new("bash")
            .arg("-c")
            .arg("while true; do sleep 1; done")
            .spawn()
            .expect("bash command to exist")
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

    #[test]
    fn test_sleeper() {
        let mut sleeper = spawn_sleeper();
        thread::sleep(time::Duration::from_millis(50));
        sleeper.kill().expect("no process to kill");
    }

    #[test]
    #[serial]
    fn create_close() {
        let syscalls = vec![0, 1, 2];
        let is_created = new_adapter(1000, syscalls.as_ptr(), syscalls.len(), dummy_calc_fn);
        assert!(is_created);
        close_adapter();
    }

    #[test]
    #[serial]
    fn with_target() {
        // create child process that just sleeps in a loop
        let mut sleeper_process = spawn_sleeper();
        let sleeper_pid = sleeper_process.id();
        let nanosleep_syscall_nr = 35;
        let syscalls = vec![nanosleep_syscall_nr];
        // trace the nanosleep system call and set the scale_metric to the nanosleep call count
        let is_created = new_adapter(1000, syscalls.as_ptr(), syscalls.len(), constant_calc_fn);
        assert!(is_created);
        // add sleeper process to be traced
        let is_added = add_tracee(sleeper_pid as i32);
        assert!(is_added);
        // update adapter and get latest metric, verify scale_metric equals nanosleep syscall count (should be 1)
        let lastest_metric = calc_new_interval_metrics();
        println!("latest metric: {:?}", &lastest_metric);
        assert!(lastest_metric.scale_metric > 0.9);
        assert!(lastest_metric.scale_metric < 1.1);
        // remove traceee
        let is_removed = remove_tracee(sleeper_pid as i32);
        assert!(is_removed);
        close_adapter();
        sleeper_process
            .kill()
            .expect("sleeper process to be killed gracefully");
    }
}
