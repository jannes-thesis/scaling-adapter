extern crate tracesets_sys;
use std::{collections::HashMap, os::raw::c_int, slice};

use tracesets_sys::{
    __traceset_syscall_data, deregister_traceset, deregister_traceset_target,
    deregister_traceset_targets, register_traceset, register_traceset_target,
    register_traceset_targets, traceset,
};

// as Traceset is read-only this should be safe
unsafe impl std::marker::Send for Traceset {}
unsafe impl std::marker::Sync for Traceset {}

pub struct Traceset {
    // _traceset: *mut traceset,
    _traceset: *const traceset,
    pub id: i32,
    pub targets: Vec<i32>,
    pub syscalls: Vec<i32>,
}

pub struct TracesetSnapshot {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub syscalls_data: HashMap<i32, SyscallData>,
    pub targets: Vec<i32>,
}

impl Drop for Traceset {
    fn drop(&mut self) {
        unsafe {
            deregister_traceset(self.id as c_int);
        }
    }
}

#[cfg(feature="c_repr")]
#[repr(C)]
pub struct SyscallData {
    pub count: u32,
    pub total_time: u64,
}

#[cfg(not(feature="c_repr"))]
pub struct SyscallData {
    pub count: u32,
    pub total_time: u64,
}

impl Traceset {
    // create a new traceset, returns None on failure
    pub fn new(targets: &[i32], syscalls: &[i32]) -> Option<Traceset> {
        unsafe {
            let targets_ptr = targets.to_vec().as_mut_ptr();
            let targets_amount = targets.len() as c_int;
            let syscalls_ptr = syscalls.to_vec().as_mut_ptr();
            let syscalls_amount = syscalls.len() as c_int;
            let traceset =
                register_traceset(targets_ptr, targets_amount, syscalls_ptr, syscalls_amount);
            if traceset.is_null() {
                None
            } else {
                Some(Traceset {
                    _traceset: traceset,
                    id: (*(*traceset).data).traceset_id,
                    targets: targets.to_vec(),
                    syscalls: syscalls.to_vec(),
                })
            }
        }
    }

    pub fn get_snapshot(&self) -> TracesetSnapshot {
        let read_bytes = self.get_read_bytes();
        let write_bytes = self.get_write_bytes();
        let syscalls_data = self.get_all_syscall_data();
        let targets = self.targets.clone();
        TracesetSnapshot {
            read_bytes,
            write_bytes,
            syscalls_data,
            targets,
        }
    }

    pub fn get_read_bytes(&self) -> u64 {
        unsafe { (*(*self._traceset).data).read_bytes as u64 }
    }

    pub fn get_write_bytes(&self) -> u64 {
        unsafe { (*(*self._traceset).data).write_bytes as u64 }
    }

    pub fn get_syscall_data(&self, syscall: i32) -> Option<SyscallData> {
        let index = self.syscalls.iter().position(|&s| s == syscall);
        match index {
            Some(index) => {
                let syscall_data: &[__traceset_syscall_data] = unsafe {
                    slice::from_raw_parts((*self._traceset).sdata_arr, self.syscalls.len())
                };
                let count = syscall_data[index].count as u32;
                let total_time = syscall_data[index].total_time as u64;
                Some(SyscallData { count, total_time })
            }
            None => None,
        }
    }

    pub fn get_all_syscall_data(&self) -> HashMap<i32, SyscallData> {
        let mut result = HashMap::new();
        for (index, &syscall) in self.syscalls.iter().enumerate() {
            let syscall_data: &[__traceset_syscall_data] =
                unsafe { slice::from_raw_parts((*self._traceset).sdata_arr, self.syscalls.len()) };
            let count = syscall_data[index].count as u32;
            let total_time = syscall_data[index].total_time as u64;
            result.insert(syscall, SyscallData { count, total_time });
        }
        result
    }

    pub fn register_target(&self, target: i32) -> bool {
        unsafe { register_traceset_target(self.id as c_int, target) }
    }

    /// register targets and return the amount that were successfully registered
    pub fn register_targets(&self, targets: &[i32]) -> i32 {
        unsafe {
            deregister_traceset_targets(
                self.id as c_int,
                targets.to_vec().as_mut_ptr(),
                targets.len() as i32,
            )
        }
    }

    pub fn deregister_target(&self, target: i32) -> bool {
        unsafe { deregister_traceset_target(self.id as c_int, target) }
    }

    /// deregister targets and return the amount that were successfully deregistered
    pub fn deregister_targets(&self, targets: &[i32]) -> i32 {
        unsafe {
            register_traceset_targets(
                self.id as c_int,
                targets.to_vec().as_mut_ptr(),
                targets.len() as i32,
            )
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
