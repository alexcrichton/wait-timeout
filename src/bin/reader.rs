use std::io::{stdin, Read};

fn main() {
    let mut buffer: [u8; 32] = Default::default();
    println!("about to block");
    let _ = stdin().read(&mut buffer);
}
