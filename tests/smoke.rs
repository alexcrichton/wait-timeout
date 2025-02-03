extern crate wait_timeout;

use std::env;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use wait_timeout::ChildExt;

fn sleeper(ms: u32) -> Child {
    let mut me = env::current_exe().unwrap();
    me.pop();
    if me.ends_with("deps") {
        me.pop();
    }
    me.push("sleep");
    Command::new(me).arg(ms.to_string()).spawn().unwrap()
}

fn exit(code: u32) -> Child {
    let mut me = env::current_exe().unwrap();
    me.pop();
    if me.ends_with("deps") {
        me.pop();
    }
    me.push("exit");
    Command::new(me).arg(code.to_string()).spawn().unwrap()
}

fn reader() -> Child {
    let mut me = env::current_exe().unwrap();
    me.pop();
    if me.ends_with("deps") {
        me.pop();
    }
    me.push("reader");
    Command::new(me).stdin(Stdio::piped()).spawn().unwrap()
}

#[test]
fn smoke_insta_timeout() {
    let mut child = sleeper(1_000);
    assert_eq!(child.wait_timeout_ms(0).unwrap(), None);

    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
}

#[test]
fn smoke_success() {
    let start = Instant::now();
    let mut child = sleeper(0);
    let status = child
        .wait_timeout_ms(1_000)
        .unwrap()
        .expect("should have succeeded");
    assert!(status.success());

    assert!(start.elapsed() < Duration::from_millis(500));
}

#[test]
fn smoke_timeout() {
    let mut child = sleeper(1_000_000);
    let start = Instant::now();
    assert_eq!(child.wait_timeout_ms(100).unwrap(), None);
    assert!(start.elapsed() > Duration::from_millis(80));

    child.kill().unwrap();
    let status = child.wait().unwrap();
    assert!(!status.success());
}

#[test]
fn smoke_reader() {
    let mut child = reader();
    let dur = Duration::from_millis(100);
    let status = child.wait_timeout(dur).unwrap().unwrap();
    assert!(status.success());
}

#[test]
fn exit_codes() {
    let mut child = exit(0);
    let status = child.wait_timeout_ms(1_000).unwrap().unwrap();
    assert_eq!(status.code(), Some(0));

    let mut child = exit(1);
    let status = child.wait_timeout_ms(1_000).unwrap().unwrap();
    assert_eq!(status.code(), Some(1));

    // check STILL_ACTIVE on windows, on unix this ends up just getting
    // truncated so don't bother with it.
    if cfg!(windows) {
        let mut child = exit(259);
        let status = child.wait_timeout_ms(1_000).unwrap().unwrap();
        assert_eq!(status.code(), Some(259));
    }
}
