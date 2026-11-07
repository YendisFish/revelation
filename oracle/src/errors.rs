pub enum Unsafe {
    LibcMountError(i32),
    LibcChrootError(i32),
    LibcChdirError(i32),
    LibcStringError,
}
