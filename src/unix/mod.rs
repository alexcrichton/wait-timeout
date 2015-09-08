//! Unix implementation of waiting for children with timeouts
//!
//! On unix, wait() and its friends have no timeout parameters, so there is
//! no way to time out a thread in wait(). From some googling and some
//! thinking, it appears that there are a few ways to handle timeouts in
//! wait(), but the only real reasonable one for a multi-threaded program is
//! to listen for SIGCHLD.
//!
//! With this in mind, the waiting mechanism with a timeout barely uses
//! waitpid() at all. There are a few times that waitpid() is invoked with
//! WNOHANG, but otherwise all the necessary blocking is done by waiting for
//! a SIGCHLD to arrive (and that blocking has a timeout). Note, however,
//! that waitpid() is still used to actually reap the child.
//!
//! Signal handling is super tricky in general, and this is no exception. Due
//! to the async nature of SIGCHLD, we use the self-pipe trick to transmit
//! data out of the signal handler to the rest of the application.
//!
//! TODO: more dox about impl

#![allow(bad_style)]

use std::cmp;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write, Read};
use std::mem;
use std::os::unix::prelude::*;
use std::process::Child;
use std::sync::{Once, ONCE_INIT, Mutex};

use libc::{self, c_int, c_ulong, timeval, suseconds_t, time_t};
use libc::funcs::bsd44::ioctl;
use time;

mod signal;
use self::signal::*;
mod select;
use self::select::*;

const WNOHANG: c_int = 1;

cfg_if! {
    if #[cfg(target_os = "macos")] {
        const FIONBIO: c_ulong = 0x8004667e;
    } else if #[cfg(target_os = "linux")] {
        const FIONBIO: c_ulong = 0x5421;
    } else {
        // unknown ...
    }
}

static INIT: Once = ONCE_INIT;
static mut STATE: *mut State = 0 as *mut _;

struct State {
    prev: sigaction,
    write: File,
    read: File,
    map: Mutex<StateMap>,
}

type StateMap = HashMap<c_int, (File, Option<ExitStatus>)>;

#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub struct ExitStatus(c_int);

pub fn wait_timeout_ms(child: &mut Child, ms: u32)
                       -> io::Result<Option<ExitStatus>> {
    INIT.call_once(State::init);
    unsafe {
        (*STATE).wait_timeout_ms(child, ms)
    }
}

impl State {
    fn init() {
        // Ensure we're Send/Sync to safely throw into a static
        fn assert<T: Send + Sync>() {}
        assert::<State>();

        unsafe {
            // Create our "self pipe" and then set both ends to nonblocking
            // mode.
            let (read, write) = pipe().unwrap();

            let mut state = Box::new(State {
                prev: mem::zeroed(),
                write: write,
                read: read,
                map: Mutex::new(HashMap::new()),
            });

            // Register our sigchld handler
            let mut new: sigaction = mem::zeroed();
            new.sa_handler = sigchld_handler;
            new.sa_flags = SA_NOCLDSTOP | SA_RESTART;
            assert_eq!(sigaction(SIGCHLD, &new, &mut state.prev), 0);

            STATE = mem::transmute(state);
        }
    }

    fn wait_timeout_ms(&self, child: &mut Child, ms: u32)
                       -> io::Result<Option<ExitStatus>> {
        // First up, prep our notification pipe which will tell us when our
        // child has been reaped (other threads may signal this pipe).
        let (read, write) = try!(pipe());
        let id = child.id() as c_int;

        // Next, take a lock on the map of children currently waiting. Right
        // after this, **before** we add ourselves to the map, we check to see
        // if our child has actually already exited via a `try_wait`. If the
        // child has exited then we return immediately as we'll never otherwise
        // receive a SIGCHLD notification.
        //
        // If the wait reports the child is still running, however, we add
        // ourselves to the map and then block in `select` waiting for something
        // to happen.
        let mut map = self.map.lock().unwrap();
        if let Some(status) = try!(try_wait(id)) {
            return Ok(Some(status))
        }
        assert!(map.insert(id, (write, None)).is_none());
        drop(map);


        // Alright, we're guaranteed that we'll eventually get a SIGCHLD due
        // to our `try_wait` failing, and we're also guaranteed that we'll
        // get notified about this because we're in the map. Next up wait
        // for an event.
        //
        // Note that this happens in a loop for two reasons; we could
        // receive EINTR or we could pick up a SIGCHLD for other threads but not
        // actually be ready oureslves.
        let end_time = time::precise_time_ns() + (ms as u64) * 1_000_000;
        loop {
            let cur_time = time::precise_time_ns();
            if cur_time > end_time {
                break
            }
            let timeout = end_time - cur_time;
            let mut timeout = timeval {
                tv_sec: (timeout / 1_000_000_000) as time_t,
                tv_usec: ((timeout % 1_000_000_000) / 1000) as suseconds_t,
            };
            let r = unsafe {
                let mut set: fd_set = mem::zeroed();
                fd_set(&mut set, self.read.as_raw_fd());
                fd_set(&mut set, read.as_raw_fd());
                let max = cmp::max(self.read.as_raw_fd(), read.as_raw_fd()) + 1;
                select(max, &mut set, 0 as *mut _, 0 as *mut _, &mut timeout)
            };
            let timeout = match r {
                0 => true,
                1 | 2 => false,
                n => {
                    let err = io::Error::last_os_error();
                    if err.kind() == io::ErrorKind::Interrupted {
                        continue
                    } else {
                        panic!("error in select = {}: {}", n, err)
                    }
                }
            };

            // Now that something has happened, we need to process what actually
            // happened. There's are three reasons we could have woken up:
            //
            // 1. The file descriptor in our SIGCHLD handler was written to.
            //    This means that a SIGCHLD was received and we need to poll the
            //    entire list of waiting processes to figure out which ones
            //    actually exited.
            // 2. Our file descriptor was written to. This means that another
            //    thread reaped our child and listed the exit status in the
            //    local map.
            // 3. We timed out. This means we need to remove ourselves from the
            //    map and simply carry on.
            //
            // In the case that a SIGCHLD signal was received, we do that
            // processing and keep going. If our fd was written to or a timeout
            // was received then we break out of the loop and return from this
            // call.
            let mut map = self.map.lock().unwrap();
            if drain(&self.read) {
                self.process_sigchlds(&mut map);
            }

            if drain(&read) || timeout {
                break
            }
        }

        let mut map = self.map.lock().unwrap();
        let (_write, ret) = map.remove(&id).unwrap();
        Ok(ret)
    }

    fn process_sigchlds(&self, map: &mut StateMap) {
        for (&k, &mut (ref write, ref mut status)) in map {
            // Already reaped, nothing to do here
            if status.is_some() {
                continue
            }

            *status = try_wait(k).unwrap();
            if status.is_some() {
                notify(write);
            }
        }
    }
}

fn pipe() -> io::Result<(File, File)> {
    // TODO: CLOEXEC
    unsafe {
        let mut pipes = [0; 2];
        if libc::pipe(pipes.as_mut_ptr()) != 0 {
            return Err(io::Error::last_os_error())
        }
        let set = 1 as c_int;
        assert_eq!(ioctl(pipes[0], FIONBIO, &set), 0);
        assert_eq!(ioctl(pipes[1], FIONBIO, &set), 0);
        Ok((File::from_raw_fd(pipes[0]), File::from_raw_fd(pipes[1])))
    }
}

fn try_wait(id: c_int) -> io::Result<Option<ExitStatus>> {
    let mut status = 0;
    match unsafe { libc::waitpid(id, &mut status, WNOHANG) } {
        0 => Ok(None),
        n if n < 0 => return Err(io::Error::last_os_error()),
        n => {
            assert_eq!(n, id);
            Ok(Some(ExitStatus(status)))
        }
    }
}

fn drain(mut file: &File) -> bool {
    let mut ret = false;
    let mut buf = [0u8; 16];
    loop {
        match file.read(&mut buf) {
            Ok(0) => return true, // EOF == something happened
            Ok(..) => ret = true, // data read, but keep draining
            Err(e) => {
                if e.kind() == io::ErrorKind::WouldBlock {
                    return ret
                } else {
                    panic!("bad read: {}", e)
                }
            }
        }
    }
}

fn notify(mut file: &File) {
    match file.write(&[1]) {
        Ok(..) => {}
        Err(e) => {
            if e.kind() != io::ErrorKind::WouldBlock {
                panic!("bad error on write fd: {}", e)
            }
        }
    }
}

// Signal handler for SIGCHLD signals, must be async-signal-safe!
//
// This function will write to the writing half of the "self pipe" to wake
// up the helper thread if it's waiting. Note that this write must be
// nonblocking because if it blocks and the reader is the thread we
// interrupted, then we'll deadlock.
//
// When writing, if the write returns EWOULDBLOCK then we choose to ignore
// it. At that point we're guaranteed that there's something in the pipe
// which will wake up the other end at some point, so we just allow this
// signal to be coalesced with the pending signals on the pipe.
extern fn sigchld_handler(_signum: c_int) {
    let state = unsafe { &*STATE };
    notify(&state.write);
}

cfg_if! {
    if #[cfg(any(target_os = "linux", target_os = "android"))] {
        fn WIFEXITED(status: i32) -> bool { (status & 0xff) == 0 }
        fn WEXITSTATUS(status: i32) -> i32 { (status >> 8) & 0xff }
        fn WTERMSIG(status: i32) -> i32 { status & 0x7f }
    } else {
        fn WIFEXITED(status: i32) -> bool { (status & 0x7f) == 0 }
        fn WEXITSTATUS(status: i32) -> i32 { status >> 8 }
        fn WTERMSIG(status: i32) -> i32 { status & 0o177 }
    }
}

impl ExitStatus {
    pub fn success(&self) -> bool {
        self.code() == Some(0)
    }

    pub fn code(&self) -> Option<i32> {
        if WIFEXITED(self.0) {
            Some(WEXITSTATUS(self.0))
        } else {
            None
        }
    }

    pub fn unix_signal(&self) -> Option<i32> {
        if !WIFEXITED(self.0) {
            Some(WTERMSIG(self.0))
        } else {
            None
        }
    }
}
