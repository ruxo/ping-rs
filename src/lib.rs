extern crate core;

mod ping_future;
mod ping_common;
mod ping_v4;
mod ping_v6;

use std::net::IpAddr;

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
#[allow(dead_code)]
pub struct PingReply { address: IpAddr, rtt: u32, }

#[derive(Debug, Clone)]
pub enum PingError {
    BadParameter(&'static str),
    OsError(u32, String),
    IpError(IpStatus::Type),
    IoPending
}

pub type PingApiOutput = Result<PingReply, PingError>;

pub use ping_common::*;