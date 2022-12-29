mod ping_future;
mod ping_v4;
mod ping_v6;

use std::net::{IpAddr};
use std::sync::{Arc};
use windows::core::PSTR;
use windows::Win32::NetworkManagement::IpHelper::{Icmp6CreateFile, IcmpCreateFile};
use windows::Win32::System::Diagnostics::Debug::*;

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
}

#[derive(Debug, Clone)]
pub struct PingOptions {
    pub ttl: u8,
    pub dont_fragment: bool
}

#[derive(Debug, Clone)]
pub struct PingReply {
    address: IpAddr,
    options: Option<PingOptions>,
    ip_status: IpStatus::Type,
    rtt: u64,
    buffer: Arc<Vec<u8>>
}

#[derive(Debug, Clone)]
pub enum PingError {
    BadParameter(&'static str),
    OsError(u32, String),
    IpError(IpStatus::Type),
    IoPending
}

pub type PingApiOutput = Result<PingReply, PingError>;

pub trait PingOps {
    fn echo(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> PingApiOutput;
    fn echo_async(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> ping_future::FutureEchoReply;
}

pub fn send_ping_async(addr: &IpAddr, timeout: u32, buffer: &[u8], options: Option<&PingOptions>) -> ping_future::FutureEchoReply {
    let validation = validate_buffer(buffer);
    if validation.is_err() {
        return ping_future::FutureEchoReply::immediate(Err(validation.err().unwrap()));
    }
    let handle_ping = initialize_icmp_handle(addr).unwrap();
    let ops = handle_ping.ops();
    ops.echo_async(buffer, timeout, options)
}

pub fn send_ping(addr: &IpAddr, timeout: u32, buffer: &[u8], options: Option<&PingOptions>) -> PingApiOutput {
    let buffer = validate_buffer(buffer)?;
    let handle_ping = initialize_icmp_handle(addr)?;
    let ops = handle_ping.ops();
    ops.echo(buffer, timeout, options)
}

/// Artificial constraint due to win32 api limitations.
const MAX_BUFFER_SIZE: usize = 65500;
const MAX_UDP_PACKET: usize = 0xFFFF + 256; // size of ICMP_ECHO_REPLY * 2 + ip header info

enum PingHandle {
    V4(ping_v4::PingV4), V6(ping_v6::PingV6)
}

impl<'a> PingHandle {
    fn ops(&self) -> &dyn PingOps {
        match self {
            PingHandle::V4(v) => v,
            PingHandle::V6(v) => v
        }
    }
}

const IP_STATUS_BASE: u32 = 11_000;
const DONT_FRAGMENT_FLAG: u8 = 2;

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

fn validate_buffer(buffer: &[u8]) -> Result<&[u8], PingError> {
    if buffer.len() > MAX_BUFFER_SIZE { Err(PingError::BadParameter("buffer")) } else { Ok(buffer) }
}

fn to_ping_error(win_err: windows::core::Error) -> PingError {
    PingError::OsError(win_err.code().0 as u32, win_err.message().to_string())
}

fn initialize_icmp_handle(addr: &IpAddr) -> Result<PingHandle, PingError> {
    unsafe {
        let handle = match addr {
            IpAddr::V4(ip) => IcmpCreateFile().map(|h| PingHandle::V4(ping_v4::PingV4::new(ip.clone(), h))),
            IpAddr::V6(ip) => Icmp6CreateFile().map(|h| PingHandle::V6(ping_v6::PingV6::new(ip.clone(), h)))
        };
        handle.map_err(to_ping_error)
    }
}