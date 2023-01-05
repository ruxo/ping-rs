//! Provide ICMP Echo (ping) functionality.

mod windows_ping;
mod linux_ping;

use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

#[allow(non_snake_case)]
pub mod IpStatus {
    #![allow(non_upper_case_globals)]

    pub type Type = u32;
    pub const Success: Type = 0;
    //BufferTooSmall = 11000 + 1;

    pub const DestinationNetworkUnreachable: Type = 11000 + 2;
    pub const DestinationHostUnreachable: Type = 11000 + 3;
    pub const DestinationProtocolUnreachable: Type = 11000 + 4;
    pub const DestinationPortUnreachable: Type = 11000 + 5;
    pub const DestinationProhibited: Type = 11000 + 19;

    pub const NoResources: Type = 11000 + 6;
    pub const BadOption: Type = 11000 + 7;
    pub const HardwareError: Type = 11000 + 8;
    pub const PacketTooBig: Type = 11000 + 9;
    pub const TimedOut: Type = 11000 + 10;
    //  BadRequest: Type = 11000 + 11;
    pub const BadRoute: Type = 11000 + 12;

    pub const TtlExpired: Type = 11000 + 13;
    pub const TtlReassemblyTimeExceeded: Type = 11000 + 14;

    pub const ParameterProblem: Type = 11000 + 15;
    pub const SourceQuench: Type = 11000 + 16;
    //OptionTooBig: Type = 11000 + 17;
    pub const BadDestination: Type = 11000 + 18;

    pub const DestinationUnreachable: Type = 11000 + 40;
    pub const TimeExceeded: Type = 11000 + 41;
    pub const BadHeader: Type = 11000 + 42;
    pub const UnrecognizedNextHeader: Type = 11000 + 43;
    pub const IcmpError: Type = 11000 + 44;
    pub const DestinationScopeMismatch: Type = 11000 + 45;

    // for example, no network interfaces are suitable to route the ping package.
    pub const GeneralFailure: Type = 11000 + 50;
}

#[derive(Debug, Clone)]
pub struct PingOptions {
    pub ttl: u8,
    pub dont_fragment: bool
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PingReply { address: IpAddr, rtt: u32, }

#[derive(Debug, Clone)]
pub enum PingError {
    BadParameter(&'static str),
    OsError(u32, String),
    IpError(IpStatus::Type),
    IoPending,

    /// size of data buffer for ping is too big. The first parameter is the maximum allowed size.
    DataSizeTooBig(usize),
}

impl From<io::Error> for PingError {
    fn from(value: io::Error) -> Self {
        if value.kind() == io::ErrorKind::WouldBlock { PingError::IoPending }
        else { PingError::OsError(value.raw_os_error().unwrap_or(-1) as u32, value.to_string()) }
    }
}

pub type Result<T> = std::result::Result<T, PingError>;
pub type PingApiOutput = Result<PingReply>;

#[cfg(windows)]
use windows_ping as ping_mod;

#[cfg(unix)]
use linux_ping as ping_mod;

/// Send ICMP Echo package (ping) to the given address.
#[inline(always)]
pub fn send_ping(addr: &IpAddr, timeout: Duration, data: &[u8], options: Option<&PingOptions>) -> PingApiOutput {
    ping_mod::send_ping(addr, timeout, data, options)
}

/// Asynchronously schedule ICMP Echo package (ping) to the given address. Note that some parameter signatures are different
/// from [`send_ping`] function, as the caller should manage those parameters' lifetime.
#[inline(always)]
pub async fn send_ping_async(addr: &IpAddr, timeout: Duration, data: Arc<&[u8]>, options: Option<&PingOptions>) -> PingApiOutput {
    ping_mod::send_ping_async(addr, timeout, data, options).await
}
