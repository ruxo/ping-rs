use ping_rs::{PingOptions, send_ping};

const PING_OPTS: PingOptions = PingOptions { ttl: 64, dont_fragment: true };

fn main() {
    let addr = "8.8.8.8".parse().unwrap();
    let buffer = [8; 32];

    let result = send_ping(addr, 1000, &buffer, Some(&PING_OPTS));

    println!("Result = {:?}", result);
}