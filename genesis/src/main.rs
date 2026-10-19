use oracle::{errors::Unsafe, io::{self, Logger}};
use std::{ffi::CString, fmt::format};

use crate::types::{SysConfError, parse_config, type_check_config};

mod types;
mod engine;

fn main() {
    let logger = Logger::new("genesis");
    logger.write("Init system starting");

    logger.write("Mounting system");

    if let Err(_) = mount("proc", "/proc", "proc", 0) {
        logger.write("Failed to load proc");
    }

    if let Err(_) = mount("sysfs", "/sys", "sysfs", 0) {
        logger.write("Failed to load sysfs");
    }

    if let Err(_) = mount("devtmpfs", "/dev", "devtmpfs", 0) {
        logger.write("Failed to load dev");
    }

    loop {
        if let Some(conf) = parse_config() {
            let conf = match type_check_config(conf) {
                Ok(conf) => conf,
                Err(SysConfError::BinaryAbsent { binary, .. }) => {
                    logger.error(format!("Binary {} not present on system, cannot proceed", binary));
                    loop { }
                }
                Err(SysConfError::DuplicatePriority { num, first, second }) => {
                    logger.error(format!("Duplicate priority '{}' found in services {} and {}", num, first, second));
                    loop { }
                }
                Err(SysConfError::IncompatibleFlags { service, flag }) => {
                    logger.error(format!("Service {} cannot use flag {}", service, flag));
                    loop { }
                }
            };
        }
    }
}

#[cfg(target_os = "linux")]
// mount virtual directories for managing devices and processes
fn mount(source: &str, target: &str, fstype: &str, flags: libc::c_ulong) -> Result<(), Unsafe> {
    let c_source = CString::new(source).map_err(|_| Unsafe::LibcStringError)?;
    let c_target = CString::new(target).map_err(|_| Unsafe::LibcStringError)?;
    let c_fstype = CString::new(fstype).map_err(|_| Unsafe::LibcStringError)?;

    let res = unsafe {
        libc::mount(
            c_source.as_ptr(),
            c_target.as_ptr(),
            c_fstype.as_ptr(),
            flags,
            std::ptr::null()
        )
    };

    if res == 0 {
        Ok(())
    } else {
        return Err(Unsafe::LibcMountError);
    }
}
