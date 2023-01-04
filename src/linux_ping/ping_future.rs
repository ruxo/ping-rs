use std::future::Future;
use std::os::fd::{AsRawFd, RawFd};
use std::pin::Pin;
use std::sync::mpsc::{Receiver, sync_channel, SyncSender};
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;
use std::task::{Context, Poll, Waker};
use std::thread;
use mio::{Events, Interest, Token};
use mio::unix::SourceFd;
use socket2::Socket;
use crate::linux_ping::PingContext;
use crate::{PingApiOutput, PingError};

pub(crate) struct RawFdSocket(Socket);
pub(crate) struct PingFuture {
    context: PingContext,
}

impl PingFuture {
    pub(crate) fn new(context: PingContext) -> Self {
        Self { context }
    }
}

impl Future for PingFuture {
    type Output = PingApiOutput;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let reply = (self.context.wait_reply)(&mut self.context);
        println!("Reply = {reply:?}");
        match reply {
            Err(PingError::IoPending) => {
                println!("waiting..");
                start_poller(&self.context.socket, cx.waker().clone());
                Poll::Pending
            },
            _ => Poll::Ready(reply)
        }
    }
}

// INTERNAL
static WAKER_QUEUE: RwLock<Option<SyncSender<Waker>>> = RwLock::new(None);
static PING_POLLER: AtomicBool = AtomicBool::new(false);

fn start_poller(socket: &Socket, waker: Waker) {
    /*
    let (sender, receiver) = sync_channel(64);
    let mut queue = WAKER_QUEUE.write().unwrap();
    *queue = Some(sender.clone());

     */
    let fd = socket.as_raw_fd();
    thread::spawn(move || {
        // TODO handle error properly

        println!("start polling");
        let mut poll = mio::Poll::new().unwrap();
        let mut events = Events::with_capacity(64);

        poll.registry().register(&mut SourceFd(&fd), Token(123), Interest::READABLE).unwrap();
        poll.poll(&mut events, None).unwrap();

        for event in &events {
            match event.token() {
                Token(123) => {
                    println!("awakened!");
                    waker.clone().wake();
                },
                _ => unimplemented!("impossible")
            }
        }
        println!("end thread");
    });
    // sender
}