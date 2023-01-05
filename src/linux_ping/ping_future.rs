use std::os::fd::{AsRawFd};
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use std::{io, mem, thread};
use std::borrow::{Borrow, BorrowMut};
use std::future::Future;
use std::ops::Deref;
use std::os::raw::c_int;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use mio::{Events, Interest, Token};
use mio::unix::SourceFd;
use crate::linux_ping::{PingContext, WaitReplyType};
use crate::{PingApiOutput, PingError, Result};

pub(crate) struct PollerContext {
    context: PingContext,
    result: RwLock<Option<PingApiOutput>>,
    waker: RwLock<Option<Waker>>,
    started: AtomicBool,
}

impl PollerContext {
    pub(crate) fn new(context: PingContext) -> Self {
        Self {
            context,
            result: RwLock::new(None),
            waker: RwLock::new(None),
            started: AtomicBool::new(false),
        }
    }

    fn poll(&self) -> Result<()> {
        let fd = self.context.socket.as_raw_fd();
        println!("start polling {fd}");
        let mut poll = mio::Poll::new()?;
        let mut events = Events::with_capacity(8);
        poll.registry().register(&mut SourceFd(&fd), DUMMY_TOKEN, Interest::READABLE)?;

        poll.poll(&mut events, None)?;

        for event in &events {
            match event.token() {
                DUMMY_TOKEN => {
                    println!("awakened {fd}!");

                    let result = self.context.wait_reply.read().unwrap()(&self.context.socket, self.context.start_ts);
                    *self.result.write().unwrap() = Some(result);
                    self.waker.read().unwrap().clone().unwrap().wake();
                },
                _ => unimplemented!("impossible")
            }
        }
        println!("finish polling {fd}");
        Ok(())
    }
}

pub(crate) struct PingFuture(Arc<PollerContext>);

impl PingFuture {
    pub(crate) fn new(context: PingContext) -> Self {
        Self(Arc::new(PollerContext::new(context)))
    }
    fn start_poller(&self) {
        if let Ok(_) = self.0.started.compare_exchange(false, true, Ordering::SeqCst, Ordering::Relaxed) {
            let ctx = self.0.clone();
            thread::spawn(move || {
                let fd = ctx.context.socket.as_raw_fd();
                println!("start thread for {fd}");
                if let Some(e) = ctx.poll().err() {
                    *ctx.result.write().unwrap() = Some(Err(e));
                    ctx.waker.read().unwrap().clone().unwrap().wake();
                }
                ctx.started.store(false, Ordering::SeqCst);
            });
        }
    }
}

impl Future for PingFuture {
    type Output = PingApiOutput;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let fd = self.0.context.socket.as_raw_fd();
        print!("Get Reply for {fd} = ");
        let reply = self.0.result.read().unwrap().clone();
        println!("{fd} {reply:?}");
        match reply {
            Some(v) => Poll::Ready(v),
            None => {
                println!("waiting.. {fd}");
                *self.0.waker.write().unwrap() = Some(cx.waker().clone());
                self.start_poller();
                Poll::Pending
            },
        }
    }
}

// INTERNAL
const DUMMY_TOKEN: Token = Token(123);