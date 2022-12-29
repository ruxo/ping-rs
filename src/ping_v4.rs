use std::ffi::c_void;
use std::net::{IpAddr, Ipv4Addr};
use std::ptr::{null_mut, slice_from_raw_parts};
use std::sync::Arc;
use windows::Win32::Foundation::{ERROR_IO_PENDING, GetLastError, HANDLE};
use windows::Win32::NetworkManagement::IpHelper::{ICMP_ECHO_REPLY, IcmpCloseHandle, IcmpHandle, IcmpSendEcho2, IP_OPTION_INFORMATION};
use crate::{DONT_FRAGMENT_FLAG, IpStatus, MAX_UDP_PACKET, ping_reply_error, PingApiOutput, PingError, PingOps, PingOptions, PingReply};
use crate::ping_future::{FutureEchoReply, FutureEchoReplyAsyncState};

pub(crate) struct PingV4(Ipv4Addr, IcmpHandle);

impl PingV4 {
    pub fn new(ip: Ipv4Addr, handle: IcmpHandle) -> PingV4 {
        Self(ip, handle)
    }
}

impl Drop for PingV4 {
    fn drop(&mut self) {
        unsafe { IcmpCloseHandle(self.1); }
    }
}

fn echo_v4(ip: &Ipv4Addr, handle: IcmpHandle, event: Option<HANDLE>, buffer: &[u8], reply_buffer: &mut [u8], timeout: u32, options: Option<&PingOptions>) -> PingApiOutput {
    let request_data = buffer.as_ptr() as *const c_void;
    let ip_options = IP_OPTION_INFORMATION {
        Ttl: options.clone().map(|v| v.ttl).unwrap_or(128),
        Tos: 0,
        Flags: options.and_then(|v| if v.dont_fragment { Some(DONT_FRAGMENT_FLAG) } else { None } ).unwrap_or(0),
        OptionsSize: 0,
        OptionsData: null_mut()
    };
    let ip_options_ptr = &ip_options as *const IP_OPTION_INFORMATION;
    let reply_buffer_ptr = reply_buffer.as_mut_ptr() as *mut c_void;
    let error = unsafe {
        let destination_address = *((&ip.octets() as *const u8) as *const u32);
        IcmpSendEcho2(handle, event, None, None, destination_address, request_data, buffer.len() as u16,
                      Some(ip_options_ptr), reply_buffer_ptr, MAX_UDP_PACKET as u32, timeout)
    };
    if error == 0 {
        let win_err = unsafe { GetLastError() };
        if win_err == ERROR_IO_PENDING { Err(PingError::IoPending) } else { Err(ping_reply_error(win_err.0)) }
    }
    else {
        let reply = reply_buffer_ptr as *mut ICMP_ECHO_REPLY;
        unsafe { create_ping_reply_v4(&*reply) }
    }
}

impl PingOps for PingV4 {
    fn echo(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> PingApiOutput {
        let mut reply_buffer: Vec<u8> = Vec::with_capacity(MAX_UDP_PACKET);
        echo_v4(&self.0, self.1, None, buffer, &mut reply_buffer, timeout, options)
    }
    fn echo_async(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> FutureEchoReply {
        fn to_reply(reply_buffer: &[u8]) -> PingApiOutput {
            let reply = reply_buffer.as_ptr() as *const ICMP_ECHO_REPLY;
            unsafe { create_ping_reply_v4(&*reply) }
        }

        let mut state = FutureEchoReplyAsyncState::new(to_reply);

        let result = echo_v4(&self.0, self.1, Some(state.ping_event()), buffer, state.mut_reply_buffer(), timeout, options);
        if let Err(PingError::IoPending) = result {
            FutureEchoReply::pending(state)
        }
        else {
            panic!("Unexpected result from echo_v4: {result:?}");
        }
    }
}

fn create_ping_reply_v4(reply: &ICMP_ECHO_REPLY) -> Result<PingReply, PingError> {
    let ip_status = if reply.Status as IpStatus::Type == IpStatus::Success { IpStatus::Success }
    else {
        match ping_reply_error(reply.Status) {
            v @ PingError::OsError(_, _) => return Err(v),
            PingError::IpError(v) => v,
            PingError::BadParameter(_) | PingError::IoPending => panic!("Dev bug!")
        }
    };
    if ip_status == IpStatus::Success {
        let mut b = vec![0u8; reply.DataSize as usize];
        unsafe {
            let slice = slice_from_raw_parts::<u8>(reply.Data as *const u8, reply.DataSize as usize);
            b.copy_from_slice(&*slice);
        }
        let options = Some(PingOptions { ttl: reply.Options.Ttl, dont_fragment: (reply.Options.Flags & DONT_FRAGMENT_FLAG) > 0 });
        Ok(PingReply {
            address: IpAddr::V4(Ipv4Addr::from(reply.Address)),
            options,
            ip_status,
            rtt: reply.RoundTripTime as u64,
            buffer: Arc::new(b)
        })
    } else {
        Err(ping_reply_error(ip_status))
    }
}
