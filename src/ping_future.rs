use std::ffi::c_void;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc};
use std::task::{Context, Poll, Waker};
use std::time::Duration;
use windows::Win32::Foundation::{BOOLEAN, CloseHandle, GetLastError, HANDLE, WAIT_TIMEOUT, WAIT_OBJECT_0, WAIT_FAILED};
use windows::Win32::System::Threading::{CreateEventA, RegisterWaitForSingleObject, UnregisterWait, WaitForSingleObject, WT_EXECUTEONLYONCE};
use windows::Win32::System::WindowsProgramming::INFINITE;
use crate::{ping_common, PingApiOutput, PingError, PingOptions};
use crate::ping_common::{MAX_UDP_PACKET, PingHandle};

pub struct FutureEchoReplyAsyncState<'a> {
    handle: PingHandle<'a>,
    data: Arc<&'a [u8]>,
    timeout: Duration,
    options: Option<&'a PingOptions>,

    ping_event: HANDLE,
    event_registration: HANDLE,

    /// A fixed address for ICMP reply
    reply_buffer: Pin<Arc<ping_common::ReplyBuffer>>,

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
    pub(crate) fn new(handle: PingHandle<'a>, data: Arc<&'a [u8]>, timeout: Duration, options: Option<&'a PingOptions>) -> Self {
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

    fn waker_address(&self) -> *mut Option<Waker> {
        Arc::into_raw(Pin::into_inner(self.waker.clone())) as *mut Option<Waker>
    }

    /// [`reply_buffer`] is a fixed address, so a mutable reference shouldn't be an issue.
    fn mut_reply_buffer(&self) -> *mut u8 {
        Arc::into_raw(Pin::into_inner(self.reply_buffer.clone())) as *mut u8
    }

    fn start(&mut self) -> Option<Poll<PingApiOutput>> {
        (self.ping_event, self.event_registration) = register_event(self.waker_address() as *const c_void);

        let raw_reply = ping_common::echo(self.handle.icmp(), *self.handle.icmp_handle(), Some(self.ping_event), self.data.as_ref(),
                                          self.mut_reply_buffer(), self.timeout, self.options)
            .map(|reply| self.handle.icmp().create_raw_reply(reply));
        match raw_reply {
            Err(PingError::IoPending) => None,
            result => Some(Poll::Ready(result.and_then(|x| x.into())))
        }
    }
}

impl<'a> Future for FutureEchoReplyAsyncState<'a> {
    type Output = PingApiOutput;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let async_state = self.get_mut();

        // TODO not thread-safe.. but do we need it?
        if async_state.ping_event.is_invalid() {
            if let Some(result) = async_state.start() { return result; }
        }

        let ping_state = unsafe { WaitForSingleObject(async_state.ping_event, 0) };

        match ping_state {
            WAIT_TIMEOUT => unsafe {
                let addr = async_state.waker_address();
                *addr = Some(cx.waker().clone());
                Poll::Pending
            },
            WAIT_OBJECT_0 => Poll::Ready(async_state.handle.icmp().create_raw_reply(async_state.mut_reply_buffer()).into()),
            WAIT_FAILED => Poll::Ready(Err(PingError::OsError(unsafe { GetLastError().0 }, "Wait event failed".to_string()))),
            _ => Poll::Ready(Err(PingError::OsError(ping_state.0, "Unexpected return code!".to_string())))
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
