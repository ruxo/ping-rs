use std::{
    thread,
    task::{Context, Poll, Waker},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering}
    },
    os::fd::AsRawFd,
    future::Future,
    pin::Pin,
};
use mio::{
    Events, Interest, Token,
    unix::SourceFd,
};
use crate::linux_ping::{PingContext};
use crate::{IpStatus, PingApiOutput, PingError, Result};

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
        let mut poll = mio::Poll::new()?;
        let mut events = Events::with_capacity(8);
        poll.registry().register(&mut SourceFd(&fd), DUMMY_TOKEN, Interest::READABLE)?;

        poll.poll(&mut events, Some(self.context.timeout))?;

        let mut responded = 0;
        for event in &events {
            match event.token() {
                DUMMY_TOKEN => {
                    responded += 1;

                    let result = self.context.wait_reply.read().unwrap()(&self.context.socket, self.context.start_ts);
                    *self.result.write().unwrap() = Some(result);
                    self.waker.read().unwrap().clone().unwrap().wake();
                },
                _ => unimplemented!("impossible")
            }
        }
        if responded == 1 { Ok(()) }
        else { Err(PingError::IpError(IpStatus::TimedOut)) }
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
        let reply = self.0.result.read().unwrap().clone();
        match reply {
            Some(v) => Poll::Ready(v),
            None => {
                *self.0.waker.write().unwrap() = Some(cx.waker().clone());
                self.start_poller();
                Poll::Pending
            },
        }
    }
}

// INTERNAL
const DUMMY_TOKEN: Token = Token(123);