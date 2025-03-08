use std::os::raw::c_int;

unsafe extern "C" {
    fn __libc_current_sigrtmin() -> c_int;
    fn __libc_current_sigrtmax() -> c_int;
}

pub fn sig_rt_min() -> c_int {
    unsafe { __libc_current_sigrtmin() }
}

pub fn sig_rt_max() -> c_int {
    unsafe { __libc_current_sigrtmax() }
}

pub fn valid_rt_signum(x: c_int) -> bool {
    sig_rt_min() + x < sig_rt_max()
}
