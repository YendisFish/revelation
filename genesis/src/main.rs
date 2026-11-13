use oracle::{
    errors::Unsafe,
    io::{self, Logger},
};
use std::{ffi::CString, fmt::format, fs, sync::Arc, time::Duration};

use crate::{
    devices::wait_for_partuuid, engine::{EngineError, GenesisEngine, setup_lifecycle}, types::{SysConfError, parse_config, type_check_config},
};

mod engine;
mod types;
mod devices;

fn main() {
    /*
     * First part of the init system sets up all mounting and
     * configuration of virtual filesystems related to devices
     * and the system. This is pretty simple, we just use a libc
     * wrapper.
     */
    let logger = Arc::new(Logger::new("genesis"));
    logger.write("Init system starting");

    // mount necessary system dirs into ramfs
    if let Err(_) = mount("proc", "/proc", "proc", 0) {
        logger.write("Failed to load proc");
    }

    if let Err(_) = mount("sysfs", "/sys", "sysfs", 0) {
        logger.write("Failed to load sysfs");
    }

    if let Err(_) = mount("devtmpfs", "/dev", "devtmpfs", 0) {
        logger.write("Failed to load dev");
    }

    // wait for the primary drive listed in boot.toml, and find it
    let drive = match wait_for_partuuid(logger.clone()) {
        Some(d) => d,
        None => {
            logger.write("Failed to detect drive");
            return;
        }
    };

    // mount drive and forward virtual directories into it
    logger.write("Mounting filesystem");
    _ = fs::create_dir_all("/mnt");
    if let Err(e) = mount(
        &drive,
        "/mnt",
        "ext4",
        libc::MS_NOATIME | libc::MS_NODIRATIME,
    ) {
        logger.write(&format!(
            "Failed to mount filesystem /dev/sda, error {}",
            match e {
                Unsafe::LibcMountError(e) => e,
                _ => -2,
            }
        ));
    }

    _ = fs::create_dir_all("/mnt/sys");
    _ = fs::create_dir_all("/mnt/sys/fs/cgroup");
    _ = fs::create_dir_all("/mnt/proc");
    _ = fs::create_dir_all("/mnt/dev");

    if let Err(_) = mount("/proc", "/mnt/proc", "", libc::MS_BIND | libc::MS_REC) {
        logger.write("Failed to remount proc");
    }

    if let Err(_) = mount("/sys", "/mnt/sys", "", libc::MS_BIND | libc::MS_REC) {
        logger.write("Failed to remount sysfs");
    }

    if let Err(_) = mount("cgroup2", "/mnt/sys/fs/cgroup", "cgroup2", libc::MS_NOSUID | libc::MS_NODEV | libc::MS_NOEXEC) {
        logger.write("Failed to load cgroup2");
    }

    if let Err(_) = mount("/dev", "/mnt/dev", "", libc::MS_BIND | libc::MS_REC) {
        logger.write("Failed to remount dev");
    }

    // switch to main system root
    if let Err(_) = chroot("/mnt") {
        logger.error(&format!("Could not chroot into mount"));
    }

    if let Err(_) = chdir("/") {
        logger.error(&format!("Could not change directory to new root"));
    }

    loop {
        logger.write("Beginning init loop");
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
                    logger.error(format!(
                        "Binary {} not present on system, cannot proceed",
                        binary
                    ));
                    std::thread::sleep(Duration::from_secs(5));
                    continue;
                }
                Err(SysConfError::DuplicatePriority { num, first, second }) => {
                    logger.error(format!(
                        "Duplicate priority '{}' found in services {} and {}",
                        num, first, second
                    ));
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
            setup_lifecycle(conf, eng.clone());

            match eng.start_engine() {
                Err(EngineError::TokioFailure(e)) => {
                    logger.error(format!("Tokio failed with error {}", e));
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(EngineError::TokioCommandError(e)) => {
                    logger.error(format!(
                        "Failed to run process/service with tokio error {}",
                        e
                    ));
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(EngineError::TokioPidFailure(e)) => {
                    logger.error(format!("Failed to get pid for service/program {}", e));
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(EngineError::TokioJoin(e)) => {
                    logger.error(format!("Tokio failed to join async task with error {}", e));
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(EngineError::Io(e)) => {
                    logger.error(format!(
                        "Failed to hand stdout to application with error {}",
                        e
                    ));
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(EngineError::ContainerStartError) => {
                    logger.error("Failed to start container");
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(EngineError::ContainerGenerationError) => {
                    logger.error("Failed to build container");
                    std::thread::sleep(Duration::from_secs(5));
                }
                Err(EngineError::AppNotFound(e)) => {
                    logger.error(format!("Could not find app {}", e));
                    std::thread::sleep(Duration::from_secs(5));
                }
                _ => loop {},
            }
        } else {
            logger.write("Could not find config file, trying again in 5 seconds");
            std::thread::sleep(Duration::from_secs(5));
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
            std::ptr::null(),
        )
    };

    if res == 0 {
        Ok(())
    } else {
        return Err(Unsafe::LibcMountError(
            std::io::Error::last_os_error().raw_os_error().unwrap_or(-1),
        ));
    }
}

#[cfg(target_os = "linux")]
fn chroot(name: &str) -> Result<(), Unsafe> {
    let c_name = CString::new(name).map_err(|_| Unsafe::LibcStringError)?;
    let res = unsafe {
        libc::chroot(c_name.as_ptr())
    };

    if res == 0 {
        Ok(())
    } else {
        return Err(Unsafe::LibcChrootError(
            std::io::Error::last_os_error().raw_os_error().unwrap_or(-1),
        ));
    }
}

#[cfg(target_os = "linux")]
fn chdir(name: &str) -> Result<(), Unsafe> {
    let c_name = CString::new(name).map_err(|_| Unsafe::LibcStringError)?;
    let res = unsafe {
        libc::chdir(c_name.as_ptr())
    };

    if res == 0 {
        Ok(())
    } else {
        return Err(Unsafe::LibcChdirError(
            std::io::Error::last_os_error().raw_os_error().unwrap_or(-1),
        ));
    }
}

#[cfg(target_os = "linux")]
pub fn bind_mount(source: &str, target: &str, recursive: bool) -> Result<(), Unsafe> {
    let mut flags: libc::c_ulong = libc::MS_BIND;
    if recursive {
        flags |= libc::MS_REC;
    }
    // fstype is ignored on MS_BIND, but passing empty string satisfies CString
    mount(source, target, "", flags)
}

// pub fn load_kernel_modules() -> Result<(), Unsafe> {}
