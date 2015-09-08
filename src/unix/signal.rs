#![allow(dead_code)]

use libc::*;

extern {
    pub fn sigaction(signum: c_int,
                     act: *const sigaction,
                     oldact: *mut sigaction) -> c_int;
}

cfg_if! {
    if #[cfg(any(target_os = "linux", target_os = "android"))] {
        pub const SA_NOCLDSTOP: c_ulong = 0x00000001;
        pub const SA_NOCLDWAIT: c_ulong = 0x00000002;
        pub const SA_NODEFER: c_ulong = 0x40000000;
        pub const SA_ONSTACK: c_ulong = 0x08000000;
        pub const SA_RESETHAND: c_ulong = 0x80000000;
        pub const SA_RESTART: c_ulong = 0x10000000;
        pub const SA_SIGINFO: c_ulong = 0x00000004;
        pub const SIGCHLD: c_int = 17;

        // This definition is not as accurate as it could be, {pid, uid, status}
        // is actually a giant union. Currently we're only interested in these
        // fields, however.
        #[repr(C)]
        pub struct siginfo {
            si_signo: c_int,
            si_errno: c_int,
            si_code: c_int,
            pub pid: pid_t,
            pub uid: uid_t,
            pub status: c_int,
        }

        #[repr(C)]
        pub struct sigaction {
            pub sa_handler: extern fn(c_int),
            pub sa_mask: sigset_t,
            pub sa_flags: c_ulong,
            sa_restorer: *mut c_void,
        }

        #[repr(C)]
        #[cfg(target_pointer_width = "32")]
        pub struct sigset_t {
            __val: [c_ulong; 32],
        }

        #[repr(C)]
        #[cfg(target_pointer_width = "64")]
        pub struct sigset_t {
            __val: [c_ulong; 16],
        }
    } else {
        pub const SA_ONSTACK: c_int = 0x0001;
        pub const SA_RESTART: c_int = 0x0002;
        pub const SA_RESETHAND: c_int = 0x0004;
        pub const SA_NOCLDSTOP: c_int = 0x0008;
        pub const SA_NODEFER: c_int = 0x0010;
        pub const SA_NOCLDWAIT: c_int = 0x0020;
        pub const SA_SIGINFO: c_int = 0x0040;
        pub const SIGCHLD: c_int = 20;

        pub type sigset_t = u32;

        // This structure has more fields, but we're not all that interested in
        // them.
        #[repr(C)]
        pub struct siginfo {
            pub si_signo: c_int,
            pub si_errno: c_int,
            pub si_code: c_int,
            pub pid: pid_t,
            pub uid: uid_t,
            pub status: c_int,
        }

        #[repr(C)]
        pub struct sigaction {
            pub sa_handler: extern fn(c_int),
            pub sa_flags: c_int,
            pub sa_mask: sigset_t,
        }
    }
}
