use std::os::windows::prelude::*;
use std::process::Child;
use std::io;

use kernel32::*;
use winapi::*;

const STILL_ACTIVE: DWORD = 259;

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct ExitStatus(DWORD);

pub fn wait_timeout_ms(child: &mut Child, ms: u32)
                       -> io::Result<Option<ExitStatus>> {
    if let Some(status) = try!(try_wait(child)) {
        return Ok(Some(status))
    }
    unsafe {
        match WaitForSingleObject(child.as_raw_handle(), ms) {
            WAIT_OBJECT_0 |
            WAIT_TIMEOUT => {}
            _ => return Err(io::Error::last_os_error()),
        }
    }
    try_wait(child)
}

fn try_wait(child: &mut Child) -> io::Result<Option<ExitStatus>> {
    unsafe {
        let mut status = 0;
        if GetExitCodeProcess(child.as_raw_handle(), &mut status) == FALSE {
            Err(io::Error::last_os_error())
        } else if status != STILL_ACTIVE {
            Ok(Some(ExitStatus(status)))
        } else {
            Ok(None)
        }
    }
}

impl ExitStatus {
    pub fn success(&self) -> bool { self.code() == Some(0) }
    pub fn code(&self) -> Option<i32> { Some(self.0 as i32) }
    pub fn unix_signal(&self) -> Option<i32> { None }
}

