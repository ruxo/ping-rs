use std::net::Ipv6Addr;
use socket2::{Domain, Protocol};
use crate::linux_ping::{Proto, SocketConfig};
use crate::linux_ping::icmp_header::{ICMP_HEADER_SIZE, IcmpEchoHeader};
use crate::{IpStatus, PingError};

impl Proto for Ipv6Addr {
    const ECHO_REQUEST_TYPE: u8 = 128;
    const ECHO_REQUEST_CODE: u8 = 0;
    const ECHO_REPLY_TYPE: u8 = 129;
    const ECHO_REPLY_CODE: u8 = 0;
    const SOCKET_CONFIG: SocketConfig = SocketConfig(Domain::IPV6, Protocol::ICMPV6);

    fn get_reply_header(reply: &[u8]) -> crate::Result<&IcmpEchoHeader> {
        if reply.len() < ICMP_HEADER_SIZE { return Err(PingError::IpError(IpStatus::BadHeader)); }
        Ok(IcmpEchoHeader::get_ref(reply))
    }
}
