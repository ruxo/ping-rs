use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use windows::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpHandle};
use crate::{MAX_UDP_PACKET, ping_future, PingApiOutput, PingError, PingHandle, PingOptions, PingReply};
use crate::ping_future::{FutureEchoReply, FutureEchoReplyAsyncState};

pub(crate) fn echo(handle: PingHandle, buffer: &[u8], timeout: Duration, options: Option<&PingOptions>) -> PingApiOutput {
    let mut reply_buffer: Vec<u8> = Vec::with_capacity(MAX_UDP_PACKET);
    todo!()
}

pub(crate) fn echo_async<'a>(handle: PingHandle, data: Arc<&'a [u8]>, timeout: Duration, options: Option<PingOptions>) -> impl Future<Output=PingApiOutput> + 'a {
    FutureEchoReply::pending(FutureEchoReplyAsyncState::new(handle, data, timeout, options))
}
