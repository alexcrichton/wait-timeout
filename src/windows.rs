use std::io;
use std::os::windows::prelude::*;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

type DWORD = u32;
type HANDLE = *mut u8;

const WAIT_OBJECT_0: DWORD = 0x00000000;
const WAIT_TIMEOUT: DWORD = 258;

extern "system" {
    fn WaitForSingleObject(hHandle: HANDLE, dwMilliseconds: DWORD) -> DWORD;
}

pub fn wait_timeout(child: &mut Child, mut dur: Duration) -> io::Result<Option<ExitStatus>> {
    let start = Instant::now();
    loop {
        let elapsed = start.elapsed();
        if elapsed >= dur {
            break;
        }
        let timeout = dur - elapsed;
        let ms = timeout.as_millis();
        let ms = DWORD::try_from(ms).unwrap_or(DWORD::MAX);
        unsafe {
            match WaitForSingleObject(child.as_raw_handle().cast(), ms) {
                WAIT_OBJECT_0 => {}
                WAIT_TIMEOUT => return Ok(None),
                _ => return Err(io::Error::last_os_error()),
            }
        }
        return child.try_wait();
    }
}
