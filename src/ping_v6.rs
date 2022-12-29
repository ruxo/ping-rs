use std::net::Ipv6Addr;
use windows::Win32::NetworkManagement::IpHelper::{IcmpCloseHandle, IcmpHandle};
use crate::{ping_future, PingApiOutput, PingError, PingOps, PingOptions, PingReply};

pub(crate) struct PingV6(Ipv6Addr, IcmpHandle);

impl PingV6 {
    pub fn new(ip: Ipv6Addr, handle: IcmpHandle) -> PingV6 {
        Self(ip, handle)
    }
}

impl PingOps for PingV6 {
    fn echo(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> PingApiOutput {
        Err(PingError::OsError(123, "".to_string()))
    }
    fn echo_async<'a>(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> ping_future::FutureEchoReply {
        ping_future::FutureEchoReply::immediate(Err(PingError::OsError(123, "dummy error".to_string())))
    }
}

impl Drop for PingV6 {
    fn drop(&mut self) {
        unsafe { IcmpCloseHandle(self.1); }
    }
}

