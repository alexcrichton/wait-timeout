fn main() {
    let amt = std::env::args().nth(1).unwrap().parse().unwrap();
    std::thread::sleep_ms(amt);
}
