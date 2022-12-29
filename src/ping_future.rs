use std::ffi::c_void;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use windows::Win32::Foundation::{BOOLEAN, CloseHandle, GetLastError, HANDLE};
use windows::Win32::System::Threading::{CreateEventA, RegisterWaitForSingleObject, UnregisterWait, WaitForSingleObject, WT_EXECUTEONLYONCE};
use windows::Win32::System::WindowsProgramming::INFINITE;
use crate::{MAX_UDP_PACKET, PingApiOutput, PingError};

type AsyncToReply = fn(&[u8]) -> PingApiOutput;

pub struct FutureEchoReplyAsyncState {
    ping_event: HANDLE,
    event_registration: HANDLE,

    reply_buffer: Pin<Arc<[u8; MAX_UDP_PACKET]>>,
    to_reply: AsyncToReply,

    waker: Pin<Arc<Option<Waker>>>
}

unsafe extern "system" fn reply_callback(data: *mut c_void, _is_timeout: BOOLEAN){
    let waker = &*(data as *const Option<Waker>);

    if waker.is_some() {
        waker.clone().unwrap().wake();
    }
}

impl FutureEchoReplyAsyncState {
    pub fn new(to_reply: AsyncToReply) -> FutureEchoReplyAsyncState {
        let ping_event = unsafe { CreateEventA(None, true, false, None).unwrap() };
        let mut event_registration = HANDLE::default();
        let state = FutureEchoReplyAsyncState {
            ping_event,
            event_registration,
            reply_buffer: Arc::pin([0; MAX_UDP_PACKET]),
            to_reply,
            waker: Arc::pin(None)
        };

        unsafe {
            let result = RegisterWaitForSingleObject(&mut event_registration, ping_event,
                                                     Some(reply_callback),                               // callback function for Windows OS
                                                     Some(state.waker_address() as *const c_void),   // associated state to the callback function
                                                     INFINITE, WT_EXECUTEONLYONCE);
            assert!(result.as_bool());
        }
        state
    }

    pub fn waker_address(&self) -> *const Option<Waker> {
        Arc::into_raw(Pin::into_inner(self.waker.clone()))
    }
    pub fn mut_reply_buffer(&mut self) -> &mut [u8; MAX_UDP_PACKET] {
        unsafe {
            let addr = Arc::into_raw(Pin::into_inner(self.reply_buffer.clone())) as *mut [u8; MAX_UDP_PACKET];
            &mut *addr
        }
    }

    pub fn ping_event(&self) -> HANDLE {
        self.ping_event
    }

    fn poll(&mut self, cx: &Context) -> Poll<PingApiOutput> {
        assert!(!self.ping_event.is_invalid());
        unsafe {
            let state = WaitForSingleObject(self.ping_event, 0);

            match state {
                WAIT_TIMEOUT => {
                    let addr = self.waker_address() as *mut Option<Waker>;
                    *addr = Some(cx.waker().clone());
                    Poll::Pending
                },
                WAIT_OBJECT_0 => Poll::Ready((self.to_reply)(self.reply_buffer.as_slice())),
                WAIT_FAILED => Poll::Ready(Err(PingError::OsError(GetLastError().0, "Wait event failed".to_string()))),
                _ => Poll::Ready(Err(PingError::OsError(state.0, "Unexpected return code!".to_string())))
            }
        }
    }
}

impl Drop for FutureEchoReplyAsyncState {
    fn drop(&mut self) {
        if !self.ping_event.is_invalid() {
            unsafe {
                CloseHandle(self.ping_event);
                UnregisterWait(self.event_registration);
            }
        }
        self.ping_event = HANDLE::default();
    }
}

enum FutureEchoReplyState {
    Sync(PingApiOutput),
    Async(RwLock<FutureEchoReplyAsyncState>)
}

pub struct FutureEchoReply {
    state: FutureEchoReplyState
}

impl FutureEchoReply {
    pub fn immediate(reply: PingApiOutput) -> FutureEchoReply {
        FutureEchoReply { state: FutureEchoReplyState::Sync(reply) }
    }
    pub fn pending(state: FutureEchoReplyAsyncState) -> FutureEchoReply {
        FutureEchoReply { state: FutureEchoReplyState::Async(RwLock::new(state)) }
    }
}

impl Future for FutureEchoReply {
    type Output = PingApiOutput;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &self.state {
            FutureEchoReplyState::Sync(reply) => Poll::Ready(reply.to_owned().clone()),
            FutureEchoReplyState::Async(locker) => locker.write().unwrap().poll(cx)
        }
    }
}

