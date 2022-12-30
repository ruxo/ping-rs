use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use futures::executor::block_on;
use futures::future::join_all;
use futures::{FutureExt, join};
use ping_rs::{PingOptions, send_ping, send_ping_async};

const PING_OPTS: PingOptions = PingOptions { ttl: 64, dont_fragment: true };

fn main() {
    let addr = "209.17.116.160".parse().unwrap();
    let data = [8; 32];

    sync_ping(&addr, &data);
    async_ping(&addr, Arc::new(&data));

    println!("Done.");
}

const TIMEOUT: Duration = Duration::from_secs(5);
fn sync_ping(addr: &IpAddr, buffer: &[u8]) {
    println!("Sync ping 5 times");
    for i in 1..=5 {
        let result = send_ping(addr.clone(), TIMEOUT, &buffer, Some(&PING_OPTS));

        println!("{i} > Result = {:?}", result);
    }
}

fn async_ping(addr: &IpAddr, data: Arc<&[u8]>) {
    println!("Async ping 5 times");

    let tasks = (1..=5).map(|i| {
        let d = data.clone();
        async move {
            (i, send_ping_async(addr.clone(), TIMEOUT, d, Some(PING_OPTS)).await)
        }
    });
    let x = join_all(tasks);
    block_on(x.then(|result| async move {
        for i in result {
            println!("{} > Result = {:?}", i.0, i.1);
        }
    }));
}