extern crate time;
extern crate wait_timeout;

use std::env;
use std::process::{Command, Child};

use wait_timeout::ChildExt;

macro_rules! t {
    ($e:expr) => (match $e {
        Ok(e) => e,
        Err(e) => panic!("{} failed with {}", stringify!($e), e),
    })
}

fn sleeper(ms: u32) -> Child {
    let mut me = env::current_exe().unwrap();
    me.pop();
    me.push("sleep");
    t!(Command::new(me).arg(ms.to_string()).spawn())
}

#[test]
fn smoke_insta_timeout() {
    let mut child = sleeper(1_000);
    assert_eq!(t!(child.wait_timeout_ms(0)), None);

    t!(child.kill());
    let status = t!(child.wait());
    assert!(!status.success());
}

#[test]
fn smoke_success() {
    let start = time::precise_time_s();
    let mut child = sleeper(0);
    let status = t!(child.wait_timeout_ms(1_000)).expect("should have succeeded");
    let end = time::precise_time_s();
    assert!(status.success());

    assert!(end - start < 0.100);
}

#[test]
fn smoke_timeout() {
    let mut child = sleeper(1_000_000);
    let start = time::precise_time_ns();
    assert_eq!(t!(child.wait_timeout_ms(100)), None);
    let end = time::precise_time_ns();
    assert!(end - start > 80 * 1_000_000);

    t!(child.kill());
    let status = t!(child.wait());
    assert!(!status.success());
}
