//! Provide ICMP Echo (ping) functionality for both Windows and Linux. This library does not need root/admin privilege for pinging.
//! It provides sync and async ping functions: [`send_ping`] and [`send_ping_async`].
//!
//! Linux version still does not support "Do not Fragment" flag yet.
//!
//! # Usage Example
//!
//! An example is also provided in `/bin/sample_ping.rs`
//!
//! ## Synchronous ping
//!
//! ```rust,no_run
//! use std::time::Duration;
//!
//! fn main(){
//!     let addr = "8.8.8.8".parse().unwrap();
//!     let data = [1,2,3,4];  // ping data
//!     let timeout = Duration::from_secs(1);
//!     let options = ping_rs::PingOptions { ttl: 128, dont_fragment: true };
//!     let result = ping_rs::send_ping(&addr, timeout, &data, Some(&options));
//!     match result {
//!         Ok(reply) => println!("Reply from {}: bytes={} time={}ms TTL={}", reply.address, data.len(), reply.rtt, options.ttl),
//!         Err(e) => println!("{:?}", e)
//!     }
//! }
//! ```
//!
//! ## Asynchronous ping
//!
//! Note that `futures` crate is used in this example. Also, data passed in the function has to be wrapped with `Arc` because in Windows' implementation
//! the address of this data will be passed to Win32 API.
//!
//! ```rust,no_run
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! fn main(){
//!     let addr = "8.8.8.8".parse().unwrap();
//!     let data = [1,2,3,4];  // ping data
//!     let data_arc = Arc::new(&data[..]);
//!     let timeout = Duration::from_secs(1);
//!     let options = ping_rs::PingOptions { ttl: 128, dont_fragment: true };
//!     let future = ping_rs::send_ping_async(&addr, timeout, data_arc, Some(&options));
//!     let result = futures::executor::block_on(future);
//!     match result {
//!         Ok(reply) => println!("Reply from {}: bytes={} time={}ms TTL={}", reply.address, data.len(), reply.rtt, options.ttl),
//!         Err(e) => println!("{:?}", e)
//!     }
//! }
//! ```

mod windows_ping;
mod linux_ping;

use std::io;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

/// Contains constant values represent general errors.
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
    pub const BadRoute: Type = 11000 + 12;

    pub const TtlExpired: Type = 11000 + 13;
    pub const TtlReassemblyTimeExceeded: Type = 11000 + 14;

    pub const ParameterProblem: Type = 11000 + 15;
    pub const SourceQuench: Type = 11000 + 16;
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
    /// Package TTL
    pub ttl: u8,

    /// Socket's Dont Fragment
    pub dont_fragment: bool
}

/// Ping reply contains the destination address (from ICMP reply) and Round-Trip Time
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PingReply {
    /// Destination address from ICMP reply
    pub address: IpAddr,
    /// Round-Trip Time in milliseconds
    pub rtt: u32,
}

/// Ping errors
#[derive(Debug, Clone)]
pub enum PingError {
    /// Bad request parameters
    BadParameter(&'static str),

    /// Unspecific OS errors
    OsError(u32, String),

    /// General Ping errors
    IpError(IpStatus::Type),

    /// Ping timed out
    TimedOut,

    /// I/O async pending
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
