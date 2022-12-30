use std::ffi::c_void;
use std::future::Future;
use std::net::IpAddr;
use std::pin::Pin;
use std::sync::{Arc};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use windows::Win32::Foundation::{BOOLEAN, CloseHandle, GetLastError, HANDLE, WAIT_TIMEOUT, WAIT_OBJECT_0, WAIT_FAILED};
use windows::Win32::System::Threading::{CreateEventA, RegisterWaitForSingleObject, UnregisterWait, WaitForSingleObject, WT_EXECUTEONLYONCE};
use windows::Win32::System::WindowsProgramming::INFINITE;
use crate::{MAX_UDP_PACKET, ping_v4, PingApiOutput, PingError, PingHandle, PingOptions};

type ReplyBuffer = [u8; MAX_UDP_PACKET];

pub struct FutureEchoReplyAsyncState<'a> {
    handle: PingHandle,
    data: Arc<&'a [u8]>,
    timeout: Duration,
    options: Option<PingOptions>,

    ping_event: HANDLE,
    event_registration: HANDLE,

    /// A fixed address for ICMP reply
    reply_buffer: Pin<Arc<ReplyBuffer>>,

    waker: Pin<Arc<Option<Waker>>>,
}

unsafe extern "system" fn reply_callback(data: *mut c_void, _is_timeout: BOOLEAN){
    let waker = &*(data as *const Option<Waker>);

    if waker.is_some() {
        waker.clone().unwrap().wake();
    }
}

fn register_event(waker_address: *const c_void) -> (HANDLE, HANDLE) {
    let ping_event = unsafe { CreateEventA(None, true, false, None).unwrap() };
    let mut event_registration = HANDLE::default();

    unsafe {
        let result = RegisterWaitForSingleObject(&mut event_registration, ping_event, Some(reply_callback), Some(waker_address), INFINITE, WT_EXECUTEONLYONCE);
        assert!(result.as_bool());
    }
    (ping_event, event_registration)
}

impl<'a> FutureEchoReplyAsyncState<'a> {
    pub fn new(handle: PingHandle, data: Arc<&'a [u8]>, timeout: Duration, options: Option<PingOptions>) -> Self {
        Self {
            handle,
            data,
            timeout,
            options,
            ping_event: Default::default(),
            event_registration: Default::default(),
            reply_buffer: Arc::pin([0; MAX_UDP_PACKET]),
            waker: Arc::pin(None)
        }
    }

    pub fn waker_address(&self) -> *const Option<Waker> {
        Arc::into_raw(Pin::into_inner(self.waker.clone()))
    }

    /// [`reply_buffer`] is a fixed address, so a mutable reference shouldn't be an issue.
    pub fn mut_reply_buffer(&self) -> &mut [u8; MAX_UDP_PACKET] {
        unsafe {
            let addr = Arc::into_raw(Pin::into_inner(self.reply_buffer.clone())) as *mut [u8; MAX_UDP_PACKET];
            &mut *addr
        }
    }
}

impl<'a> Future for FutureEchoReplyAsyncState<'a> {
    type Output = PingApiOutput;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let async_state = self.get_mut();
        if async_state.ping_event.is_invalid() {
            (async_state.ping_event, async_state.event_registration) = register_event(async_state.waker_address() as *const c_void);

            let ip = async_state.handle.ip().clone();
            let result = match ip {
                IpAddr::V4(ip) => ping_v4::echo_v4(&ip, async_state.handle.1, Some(async_state.ping_event), async_state.data.as_ref(), async_state.mut_reply_buffer(),
                                                   async_state.timeout, async_state.options.as_ref()),
                _ => todo!()
            };
            match result {
                Err(PingError::IoPending) => (),
                _ => return Poll::Ready(result)
            }
        }

        let state = unsafe { WaitForSingleObject(async_state.ping_event, 0) };

        match state {
            WAIT_TIMEOUT => unsafe {
                let addr = async_state.waker_address() as *mut Option<Waker>;
                *addr = Some(cx.waker().clone());
                Poll::Pending
            },
            WAIT_OBJECT_0 => Poll::Ready(match async_state.handle.ip() {
                IpAddr::V4(_) => ping_v4::to_reply(async_state.reply_buffer.as_slice()),
                IpAddr::V6(_) => todo!(),
            }),
            WAIT_FAILED => Poll::Ready(Err(PingError::OsError(unsafe { GetLastError().0 }, "Wait event failed".to_string()))),
            _ => Poll::Ready(Err(PingError::OsError(state.0, "Unexpected return code!".to_string())))
        }
    }
}

impl<'a> Drop for FutureEchoReplyAsyncState<'a> {
    fn drop(&mut self) {
        if !self.ping_event.is_invalid() {
            unsafe { CloseHandle(self.ping_event); }
            self.ping_event = HANDLE::default();
        }
        if !self.event_registration.is_invalid() {
            unsafe { UnregisterWait(self.event_registration);
            }
            self.event_registration = HANDLE::default();
        }
    }
}
