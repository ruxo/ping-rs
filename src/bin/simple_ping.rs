use std::future::Future;
use std::net::IpAddr;
use futures::executor::block_on;
use ping_rs::{FutureEchoReply, PingError, PingOptions, PingReply, send_ping, send_ping_async};

const PING_OPTS: PingOptions = PingOptions { ttl: 64, dont_fragment: true };

fn main() {
    let addr = "8.8.8.8".parse().unwrap();
    let buffer = [8; 32];

    sync_ping(&addr, &buffer);
    async_ping(&addr, &buffer);
}

fn sync_ping(addr: &IpAddr, buffer: &[u8]) {
    let result = send_ping(&addr, 1000, &buffer, Some(&PING_OPTS));

    println!("Result = {:?}", result);
}

fn async_ping(addr: &IpAddr, buffer: &[u8]) {
    let future = send_ping_async(&addr, 1000, &buffer, Some(&PING_OPTS));

    let result = block_on(future);

    println!("Result = {:?}", result);
}