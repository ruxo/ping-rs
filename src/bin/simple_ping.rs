use std::net::IpAddr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use futures::executor::block_on;
use futures::future::join_all;
use futures::{FutureExt};
use ping_rs::*;

const PING_OPTS: PingOptions = PingOptions { ttl: 128, dont_fragment: true };

fn main() {
    let addrs = ["172.67.172.103", "8.8.8.8", "209.17.116.106", "209.17.116.160", "::1"]
        .map(|s| s.parse().unwrap());
    let data = [8; 8];

    sync_ping(&addrs, &data);
    async_ping(&addrs, Arc::new(&data));

    println!("Done.");
}

const TIMEOUT: Duration = Duration::from_secs(5);
fn sync_ping(addrs: &[IpAddr], data: &[u8]) {
    println!("Sync ping 5 times");
    for i in 0..addrs.len() {
        let result = send_ping(&addrs[i], TIMEOUT, &data, Some(&PING_OPTS));

        println!("{} > Result = {:?}", i+1, result);
    }
}

fn async_ping(addrs: &[IpAddr], data: Arc<&[u8]>) {
    println!("Async ping 5 times");

    let tasks = (0..addrs.len()).map(|i| {
        let d = data.clone();
        thread::sleep(Duration::from_millis(30));
        async move {
            (i, send_ping_async(&addrs[i], TIMEOUT, d, Some(&PING_OPTS)).await)
        }
    });
    let x = join_all(tasks);
    block_on(x.then(|result| async move {
        for i in result {
            println!("{} > Result = {:?}", i.0, i.1);
        }
    }));
}