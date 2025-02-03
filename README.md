# wait-timeout

[![Build Status](https://github.com/alexcrichton/wait-timeout/actions/workflows/main.yml/badge.svg?branch=master)](https://github.com/alexcrichton/wait-timeout/actions/workflows/main.yml)

[Documentation](https://docs.rs/wait-timeout)

Rust crate for waiting on a `Child` process with a timeout specified.

```sh
$ cargo add wait-timeout
```

Example:

```rust
use std::io;
use std::process::Command;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

fn main() -> io::Result<()> {
    let mut child = Command::new("sleep").arg("100").spawn()?;

    let start = Instant::now();
    assert!(child.wait_timeout(Duration::from_millis(100))?.is_none());
    assert!(start.elapsed() > Duration::from_millis(100));

    child.kill()?;

    let start = Instant::now();
    assert!(child.wait_timeout(Duration::from_millis(100))?.is_some());
    assert!(start.elapsed() < Duration::from_millis(100));

    Ok(())
}
```
