use std::future::Future;
use std::os::fd::{AsRawFd};
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use std::{io, mem, thread};
use std::os::raw::c_int;
use mio::{Events, Interest, Token};
use mio::unix::SourceFd;
use socket2::Socket;
use crate::linux_ping::{get_timestamp, PingContext};
use crate::{PingApiOutput, PingError};

pub(crate) struct PingFuture {
    context: PingContext,
    error: Arc<RwLock<Option<io::Error>>>
}

impl PingFuture {
    pub(crate) fn new(context: PingContext) -> Self {
        Self { context, error: Arc::new(RwLock::new(None)) }
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
                start_poller(&self.context.socket, cx.waker().clone(), self.error.clone());
                Poll::Pending
            },
            Ok(_) => {
                let mut error_wrapper = self.error.write().unwrap();
                let error = mem::replace(&mut *error_wrapper, None);
                Poll::Ready(match error {
                    None => reply,
                    Some(e) => Err(e.into())
                })
            },
            _ => Poll::Ready(reply)
        }
    }
}

// INTERNAL
const DUMMY_TOKEN: Token = Token(123);

fn start_poller(socket: &Socket, waker: Waker, error: Arc<RwLock<Option<io::Error>>>) {
    let fd = socket.as_raw_fd();

    fn poll(fd: c_int, waker: Waker) -> io::Result<()> {
        println!("start polling");
        let mut poll = mio::Poll::new()?;
        let mut events = Events::with_capacity(64);

        poll.registry().register(&mut SourceFd(&fd), DUMMY_TOKEN, Interest::READABLE)?;
        poll.poll(&mut events, None)?;

        for event in &events {
            match event.token() {
                DUMMY_TOKEN => {
                    println!("awakened!");
                    waker.clone().wake();
                },
                _ => unimplemented!("impossible")
            }
        }
        println!("end thread");
        Ok(())
    }

    thread::spawn(move || {
        let result = poll(fd, waker).err();
        match error.write() {
            Ok(mut e) => *e = result,
            Err(_) => panic!("impossible")
        }
    });
    // sender
}