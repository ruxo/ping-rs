use std::net::Ipv4Addr;
use socket2::{Domain, Protocol};
use crate::linux_ping::{Proto, SocketConfig, Result};
use crate::{IpStatus, PingError};
use crate::linux_ping::icmp_header::{ICMP_HEADER_SIZE, IcmpEchoHeader};

const ICMP_REPLY_HEADER_SIZE: usize = 20;

// See https://en.wikipedia.org/wiki/Internet_Protocol_version_4#Header
#[repr(C)]
struct IcmpV4ReplyHeader {
    version: u8,
    _reserved1: [u8; 8],
    protocol: u8,
    _reserved2: [u8; 10],
    reply: IcmpEchoHeader
}

impl IcmpV4ReplyHeader {
    fn version(&self) -> u8 { (self.version & 0xF0) >> 4 }
    fn header_size(&self) -> usize { (self.version & 0x0F) as usize }
}

const ICMP_PROTOCOL: u8 = 1;

impl Proto for Ipv4Addr {
    const ECHO_REQUEST_TYPE: u8 = 8;
    const ECHO_REQUEST_CODE: u8 = 0;
    const ECHO_REPLY_TYPE: u8 = 0;
    const ECHO_REPLY_CODE: u8 = 0;
    const SOCKET_CONFIG: SocketConfig = SocketConfig(Domain::IPV4, Protocol::ICMPV4);

    fn get_reply_header(reply: &[u8]) -> Result<&IcmpEchoHeader> {
        let reply_header = unsafe { &*(reply.as_ptr() as *const IcmpV4ReplyHeader) };

        println!("Reply len = {}", reply.len());
        println!("Value: {reply:?}");
        if reply.len() < ICMP_REPLY_HEADER_SIZE + ICMP_HEADER_SIZE
            || reply_header.version() != 4
            || reply.len() < reply_header.header_size()
            || reply_header.protocol != ICMP_PROTOCOL
        {
            return Err(PingError::IpError(IpStatus::BadHeader));
        }
        Ok(&reply_header.reply)
    }
}