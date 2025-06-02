//! Unix implementation of waiting for children with timeouts
//!
//! On unix, wait() and its friends have no timeout parameters, so there is
//! no way to time out a thread in wait(). From some googling and some
//! thinking, it appears that there are a few ways to handle timeouts in
//! wait(), but the only real reasonable one for a multi-threaded program is
//! to listen for SIGCHLD.

use std::io;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

pub fn wait_timeout(child: &mut Child, dur: Duration) -> io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + dur;
    let mut waiter = sigchld::Waiter::new()?;
    loop {
        // Poll the child before waiting, in case of missed signals.
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() > deadline {
            return Ok(None);
        }
        // Wait for SIGCHLD to arrive from *any* child exiting. We don't know whether it was this
        // child, so loop and check again.
        waiter.wait_deadline(deadline)?;
    }
}
