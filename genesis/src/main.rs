use oracle::{errors::Unsafe, io::{self, Logger}};
use std::{ffi::CString, fmt::format, sync::Arc, time::Duration};

use crate::{engine::{EngineError, GenesisEngine}, types::{SysConfError, parse_config, type_check_config}};

mod types;
mod engine;

fn main() {
    /*
     * First part of the init system sets up all mounting and
     * configuration of virtual filesystems related to devices
     * and the system. This is pretty simple, we just use a libc
     * wrapper.
     */
    let logger = Arc::new(Logger::new("genesis"));
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
        /*
        * Here we parse our init config, and we also
        * have the init system check if it's actually
        * safe to run on the system. And of course any
        * errors get serialized and pushed to stdout.
        */
        if let Some(conf) = parse_config() {
            let conf = match type_check_config(conf) {
                Ok(conf) => conf,
                Err(SysConfError::BinaryAbsent { binary, .. }) => {
                    logger.error(format!("Binary {} not present on system, cannot proceed", binary));
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
                Err(SysConfError::DuplicatePriority { num, first, second }) => {
                    logger.error(format!("Duplicate priority '{}' found in services {} and {}", num, first, second));
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
                Err(SysConfError::IncompatibleFlags { service, flag }) => {
                    logger.error(format!("Service {} cannot use flag {}", service, flag));
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
            };

            /*
             * Here we start the actual engine which starts and
             * keeps track of processes. We also handle and output
             * any errors that bubble up out of it.
             */
            let eng = GenesisEngine::new(logger.clone());
            match eng.start_engine() {
                Err(EngineError::TokioFailure(e)) => {
                    logger.error(format!("Tokio failed with error {}", e));
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(EngineError::TokioCommandError(e)) => {
                    logger.error(format!("Failed to run process/service with tokio error {}", e));
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(EngineError::TokioPidFailure(e)) => {
                    logger.error(format!("Failed to get pid for service/program {}", e));
                    std::thread::sleep(Duration::from_secs(5));
                },
                Err(EngineError::TokioJoin(e)) => {
                    logger.error(format!("Tokio failed to join async task with error {}", e));
                    std::thread::sleep(Duration::from_secs(5));
                }
                _ => {},
            }
        }
    }
}

#[cfg(target_os = "linux")]
// mount virtual directories for managing devices and processes
// we probably should move this function either into oracle or a dedicated file at least
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
