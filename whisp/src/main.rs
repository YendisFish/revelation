use oracle::{errors::Unsafe, io::{self, Logger}};
use std::ffi::CString;

fn main() {
    let logger = Logger::new("whisp");
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

    loop {}
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
