use libc::*;

extern {
    pub fn select(nfds: c_int,
                  readfds: *mut fd_set,
                  writefds: *mut fd_set,
                  errorfds: *mut fd_set,
                  timeout: *mut timeval) -> c_int;
}

cfg_if! {
    if #[cfg(any(target_os = "macos", target_os = "ios"))] {
        pub const FD_SETSIZE: usize = 1024;

        #[repr(C)]
        pub struct fd_set {
            fds_bits: [i32; FD_SETSIZE / 32]
        }

        pub fn fd_set(set: &mut fd_set, fd: i32) {
            let fd = fd as usize;
            set.fds_bits[fd / 32] |= 1 << (fd % 32);
        }
    } else {
        pub const FD_SETSIZE: usize = 1024;

        #[repr(C)]
        pub struct fd_set {
            fds_bits: [u64; FD_SETSIZE / 64]
        }

        pub fn fd_set(set: &mut fd_set, fd: i32) {
            let fd = fd as usize;
            set.fds_bits[fd / 64] |= 1 << (fd % 64);
        }
    }
}
