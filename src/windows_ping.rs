#![cfg(windows)]

use std::ffi::c_void;
use std::net::IpAddr;
use std::ptr::null_mut;
use std::sync::Arc;
use std::time::Duration;
use windows::core::PSTR;
use windows::Win32::Foundation::{ERROR_IO_PENDING, GetLastError, HANDLE};
use windows::Win32::NetworkManagement::IpHelper::{Icmp6CreateFile, IcmpCloseHandle, IcmpCreateFile, IcmpHandle, IP_OPTION_INFORMATION, IP_STATUS_BASE};
use windows::Win32::System::Diagnostics::Debug::*;
use crate::{IpStatus, PingApiOutput, PingError, PingOptions, PingReply};

pub(crate) const MAX_UDP_PACKET: usize = 0xFFFF + 256; // size of ICMP_ECHO_REPLY * 2 + ip header info

/// Send ICMP Echo package (ping) to the given address.
pub fn send_ping(addr: &IpAddr, timeout: Duration, data: &[u8], options: Option<&PingOptions>) -> PingApiOutput {
    let _ = validate_buffer(data)?;
    let handle = initialize_icmp_handle(addr)?;
    let mut reply_buffer: Vec<u8> = vec![0; MAX_UDP_PACKET];

    let reply = echo(handle.icmp(), handle.1, None, data, reply_buffer.as_mut_ptr(), timeout, options)?;
    handle.icmp().create_raw_reply(reply).into()
}

/// Asynchronously schedule ICMP Echo package (ping) to the given address. Note that some parameter signatures are different
/// from [`send_ping`] function, as the caller should manage those parameters' lifetime.
pub async fn send_ping_async(addr: &IpAddr, timeout: Duration, data: Arc<&[u8]>, options: Option<&PingOptions>) -> PingApiOutput {
    let validation = validate_buffer(data.as_ref());
    if validation.is_err() {
        return Err(validation.err().unwrap());
    }
    let handle = initialize_icmp_handle(addr).unwrap();
    crate::ping_future::FutureEchoReplyAsyncState::new(handle, data, timeout, options).await
}

pub(crate) type ReplyBuffer = [u8; MAX_UDP_PACKET];

pub(crate) struct PingRawReply {
    pub address: IpAddr,
    pub status: u32,
    pub rtt: u32
}

impl Into<PingApiOutput> for PingRawReply {
    fn into(self) -> PingApiOutput {
        parse_raw_reply_status(self.status).map(|_| PingReply { address: self.address, rtt: self.rtt })
    }
}

pub(crate) trait IcmpEcho {
    fn send(&self, handle: IcmpHandle, event: Option<HANDLE>, data: *const c_void, data_len: u16, options: *const IP_OPTION_INFORMATION,
            reply_buffer: *mut c_void, reply_buffer_len: u32, timeout: u32) -> u32;
    fn create_raw_reply(&self, reply: *mut u8) -> PingRawReply;
}

pub(crate) struct PingHandle<'a>(pub &'a IpAddr, IcmpHandle);

impl<'a> PingHandle<'a> {
    pub(crate) fn icmp(&self) -> &'a dyn IcmpEcho {
        match &self.0 {
            IpAddr::V4(ip) => ip,
            IpAddr::V6(ip) => ip,
        }
    }

    pub(crate) fn icmp_handle(&self) -> &IcmpHandle {
        &self.1
    }
}

impl<'a> Drop for PingHandle<'a> {
    fn drop(&mut self) {
        let result = unsafe { IcmpCloseHandle(self.1) };
        assert!(result.as_bool());
    }
}

/// Artificial constraint due to win32 api limitations.
const MAX_BUFFER_SIZE: usize = 65500;
fn validate_buffer(buffer: &[u8]) -> Result<&[u8], PingError> {
    if buffer.len() > MAX_BUFFER_SIZE { Err(PingError::BadParameter("buffer")) } else { Ok(buffer) }
}

fn initialize_icmp_handle(addr: &IpAddr) -> Result<PingHandle, PingError> {
    unsafe {
        let handle = match addr {
            IpAddr::V4(_) => IcmpCreateFile().map(|h| PingHandle(addr, h)),
            IpAddr::V6(_) => Icmp6CreateFile().map(|h| PingHandle(addr, h))
        };
        handle.map_err(|e| e.code().0 as u32).map_err(ping_reply_error)
    }
}

const DONT_FRAGMENT_FLAG: u8 = 2;
pub(crate) fn echo(destination: &dyn IcmpEcho, handle: IcmpHandle, event: Option<HANDLE>, buffer: &[u8], reply_buffer: *mut u8, timeout: Duration,
                      options: Option<&PingOptions>) -> Result<*mut u8, PingError> {
    let request_data = buffer.as_ptr() as *const c_void;
    let ip_options = IP_OPTION_INFORMATION {
        Ttl: options.clone().map(|v| v.ttl).unwrap_or(128),
        Tos: 0,
        Flags: options.and_then(|v| if v.dont_fragment { Some(DONT_FRAGMENT_FLAG) } else { None } ).unwrap_or(0),
        OptionsSize: 0,
        OptionsData: null_mut()
    };
    let ip_options_ptr = &ip_options as *const IP_OPTION_INFORMATION;

    let error = destination.send(handle, event, request_data, buffer.len() as u16, ip_options_ptr,
                reply_buffer as *mut c_void, MAX_UDP_PACKET as u32, timeout.as_millis() as u32);
    if error == 0 {
        let win_err = unsafe { GetLastError() };
        if win_err == ERROR_IO_PENDING { Err(PingError::IoPending) } else { Err(ping_reply_error(win_err.0)) }
    }
    else {
        Ok(reply_buffer)
    }
}

pub(crate) fn parse_raw_reply_status(status: u32) -> Result<(), PingError> {
    let ip_status = if status as IpStatus::Type == IpStatus::Success { IpStatus::Success }
    else {
        match ping_reply_error(status) {
            v @ PingError::OsError(_, _) => return Err(v),
            PingError::IpError(v) => v,
            PingError::BadParameter(_) | PingError::IoPending => panic!("Dev bug!")
        }
    };
    if ip_status == IpStatus::Success {
        Ok(())
    } else {
        Err(ping_reply_error(ip_status))
    }
}

fn ping_reply_error(status_code: u32) -> PingError {
    if status_code < IP_STATUS_BASE {
        let mut buffer = [0u8; 32];
        let s = PSTR::from_raw(buffer.as_mut_ptr());
        let r = unsafe { FormatMessageA(FORMAT_MESSAGE_FROM_SYSTEM, None, status_code, 0, s, buffer.len() as u32, None) };
        PingError::OsError(status_code, if r == 0 {
            format!("Ping failed ({status_code})")
        } else {
            unsafe { s.to_string().unwrap() }
        })
    } else {
        PingError::IpError(status_code)
    }
}

