use std::ffi::c_void;
use std::net::{IpAddr, Ipv6Addr, SocketAddrV6};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::Networking::WinSock::SOCKADDR_IN6;
use windows::Win32::NetworkManagement::IpHelper::{Icmp6SendEcho2, IcmpHandle, ICMPV6_ECHO_REPLY_LH, IP_OPTION_INFORMATION};
use crate::ping_common::{IcmpEcho, PingRawReply};

impl IcmpEcho for Ipv6Addr {
    fn send(&self, handle: IcmpHandle, event: Option<HANDLE>, data: *const c_void, data_len: u16, options: *const IP_OPTION_INFORMATION, reply_buffer: *mut c_void, reply_buffer_len: u32, timeout: u32) -> u32 {
        let source = SOCKADDR_IN6::default();
        let destination_address = SOCKADDR_IN6::from(SocketAddrV6::new(self.clone().to_owned(), 0, 0, 0));

        unsafe {
            Icmp6SendEcho2(handle, event, None, None, &source, &destination_address, data, data_len as u16, Some(options),
                           reply_buffer, reply_buffer_len, timeout)
        }
    }

    fn create_raw_reply(&self, reply: *mut u8) -> PingRawReply {
        let reply = unsafe { *(reply as *const ICMPV6_ECHO_REPLY_LH) };

        // correct byte order..
        let mut addr = [0; 8];
        for i in 0..=7 {
            addr[i] = reply.Address.sin6_addr[i].swap_bytes();
        }

        PingRawReply { address: IpAddr::V6(Ipv6Addr::from(addr)), status: reply.Status, rtt: reply.RoundTripTime, }
    }
}
