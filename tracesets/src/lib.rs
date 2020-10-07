extern crate tracesets_sys;
use std::{collections::HashMap, collections::HashSet, os::raw::c_int, slice};
use std::{iter::FromIterator, time::SystemTime};

use tracesets_sys::{
    __traceset_data, __traceset_syscall_data, deregister_traceset, deregister_traceset_target,
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
    pub targets: HashSet<i32>,
    pub syscalls: Vec<i32>,
}

pub struct TracesetSnapshot {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub syscalls_data: HashMap<i32, SyscallData>,
    pub targets: HashSet<i32>,
    pub timestamp: SystemTime,
}

impl Drop for Traceset {
    fn drop(&mut self) {
        unsafe {
            deregister_traceset(self.id as c_int);
        }
    }
}

// not using #[cfg_attr(feature = "c_repr", repr(C))]
// because then cbindgen will generate empty type for SyscallData

#[cfg(feature = "c_repr")]
#[repr(C)]
#[derive(Debug)]
pub struct SyscallData {
    pub count: u32,
    pub total_time: u64,
}

#[cfg(not(feature = "c_repr"))]
#[derive(Debug)]
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
            // has to be mutable because the generated bindings need mut pointer
            let mut syscalls_vec = syscalls.to_vec();
            let syscalls_ptr = syscalls_vec.as_mut_ptr();
            let syscalls_amount = syscalls.len() as c_int;
            let traceset =
                register_traceset(targets_ptr, targets_amount, syscalls_ptr, syscalls_amount);
            if traceset.is_null() {
                None
            } else {
                Some(Traceset {
                    _traceset: traceset,
                    id: (*(*traceset).data).traceset_id,
                    targets: HashSet::from_iter(targets.iter().copied()),
                    syscalls: syscalls_vec,
                })
            }
        }
    }

    pub fn get_snapshot(&self) -> TracesetSnapshot {
        // the data will be slightly out of sync because we are reading non-atomically
        // without holding a lock on the shared memory
        let read_bytes = self.get_read_bytes();
        let write_bytes = self.get_write_bytes();
        let syscalls_data = self.get_all_syscall_data();
        let targets = self.targets.clone();
        let timestamp = SystemTime::now();
        TracesetSnapshot {
            read_bytes,
            write_bytes,
            syscalls_data,
            targets,
            timestamp,
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

    pub fn get_amount_targets(&self) -> usize {
        let traceset_data: &__traceset_data =
            unsafe { self._traceset.as_ref().unwrap().data.as_ref().unwrap() };
        traceset_data.amount_targets as usize
    }

    pub fn register_target(&mut self, target: i32) -> bool {
        // problematic, as adding target may fail in kernel (but does not return err)
        self.targets.insert(target);
        unsafe { register_traceset_target(self.id as c_int, target) }
    }

    /// register targets and return the amount that were successfully registered
    pub fn register_targets(&mut self, targets: &[i32]) -> i32 {
        // problematic, as adding target may fail in kernel (but does not return err)
        for target in targets {
            self.targets.insert(*target);
        }
        unsafe {
            register_traceset_targets(
                self.id as c_int,
                targets.to_vec().as_mut_ptr(),
                targets.len() as i32,
            )
        }
    }

    /// true: no kernel error occurred, target is guaranteed not to be traced
    ///       (but may have not been a target before)
    /// false: kernel error
    pub fn deregister_target(&mut self, target: i32) -> bool {
        let is_success = unsafe { deregister_traceset_target(self.id as c_int, target) };
        if is_success {
            self.targets.remove(&target);
        }
        is_success
    }

    /// deregister targets and return the amount that were successfully deregistered
    /// if return value is >= 0, all passed targets are guaranteed not to be traced
    pub fn deregister_targets(&mut self, targets: &[i32]) -> i32 {
        let amount_removed = unsafe {
            deregister_traceset_targets(
                self.id as c_int,
                targets.to_vec().as_mut_ptr(),
                targets.len() as i32,
            )
        };
        if amount_removed >= 0 {
            for target in targets {
                self.targets.remove(target);
            }
            amount_removed
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time::Duration};
    use test_utils::{has_tracesets, spawn_echoer};

    #[cfg(target_os = "linux")]
    #[test]
    fn single_target_added() {
        assert!(has_tracesets());
        // create child process that just echos "hi" in a loop
        let echoer = spawn_echoer();
        let echoer_pid = echoer.process.id();
        println!("pid of echoer: {}", echoer_pid);
        let write_syscall_nr = 1;
        let syscalls = vec![write_syscall_nr];
        let no_targets: Vec<i32> = vec![];
        // trace the write system call (should be called for every echo)
        let mut traceset = match Traceset::new(&no_targets, &syscalls) {
            Some(traceset) => traceset,
            None => panic!("traceset creation failed"),
        };
        // add echoer process to be traced
        let is_added = traceset.register_target(echoer_pid as i32);
        assert!(is_added);
        thread::sleep(Duration::from_millis(1100));
        println!("read bytes: {}", traceset.get_read_bytes());
        println!("write bytes: {}", traceset.get_write_bytes());
        println!("traceset id: {}", traceset.id);
        println!("traceset targets: {:?}", &traceset.targets);
        println!("amount targets: {:?}", traceset.get_amount_targets());
        assert!(traceset.targets.len() == 1);
        let write_syscall_data = traceset.get_syscall_data(write_syscall_nr).unwrap();
        print!("write syscall data: {:?}", write_syscall_data);
        assert!(write_syscall_data.count > 0);
        // remove echoer process to be traced
        let is_removed = traceset.deregister_target(echoer_pid as i32);
        assert!(is_removed);
    }
}
