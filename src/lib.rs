use std::ffi::c_void;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::pin::Pin;
use std::ptr::{null_mut, slice_from_raw_parts};
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use windows::core::PCSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT, WAIT_FAILED};
use windows::Win32::NetworkManagement::IpHelper::{Icmp6CreateFile, ICMP_ECHO_REPLY, IcmpCloseHandle, IcmpCreateFile, IcmpHandle, IcmpSendEcho2, IP_OPTION_INFORMATION};
use windows::Win32::System::Threading::{CREATE_EVENT_MANUAL_RESET, CreateEventExA, WaitForSingleObject};

#[allow(non_snake_case)]
pub mod IpStatus {
    #![allow(non_upper_case_globals)]

    pub type Type = i32;
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
    pub const Unknown: Type = -1;
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
    OsError(u32, String),
    IpError(IpStatus::Type)
}

/// Artificial constraint due to win32 api limitations.
const MAX_BUFFER_SIZE: u16 = 65500;
const MAX_UDP_PACKET: usize = 0xFFFF + 256; // size of ICMP_ECHO_REPLY * 2 + ip header info

type PingApiOutput = Result<PingReply, PingError>;

struct FutureEchoReplyAsyncState {
    ping_event: HANDLE,
    reply_buffer: Vec<u8>,
    to_reply: fn(&Vec<u8>) -> PingApiOutput,

    waker: Option<Waker>
}

impl FutureEchoReplyAsyncState {
    fn poll(&mut self, cx: &Context) -> Poll<PingApiOutput> {
        unsafe {
            let state = WaitForSingleObject(self.ping_event, 0);

            match state {
                WAIT_TIMEOUT => {
                    self.waker = Some(cx.waker().clone());
                    Poll::Pending
                },
                WAIT_OBJECT_0 => Poll::Ready((self.to_reply)(&self.reply_buffer)),
                WAIT_FAILED => Poll::Ready(Err(PingError::OsError(GetLastError().0, "Wait event failed".to_string()))),
                _ => Poll::Ready(Err(PingError::OsError(state.0, "Unexpected return code!".to_string())))
            }
        }
    }
}

impl Drop for FutureEchoReplyAsyncState {
    fn drop(&mut self) {
        if !self.ping_event.is_invalid() {
            println!("close handle");
            unsafe { CloseHandle(self.ping_event); }
        }
        println!("Handle now is invalid? {}", self.ping_event.is_invalid())
    }
}

enum FutureEchoReplyState {
    Sync(PingApiOutput),
    Async(RwLock<FutureEchoReplyAsyncState>)
}

pub struct FutureEchoReply {
    state: FutureEchoReplyState
}

impl FutureEchoReply {
    fn immediate(reply: PingApiOutput) -> FutureEchoReply {
        FutureEchoReply { state: FutureEchoReplyState::Sync(reply) }
    }
    fn pending(state: FutureEchoReplyAsyncState) -> FutureEchoReply {
        FutureEchoReply { state: FutureEchoReplyState::Async(RwLock::new(state)) }
    }
}

impl Future for FutureEchoReply {
    type Output = PingApiOutput;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match &self.state {
            FutureEchoReplyState::Sync(reply) => Poll::Ready(reply.to_owned().clone()),
            FutureEchoReplyState::Async(locker) => locker.write().unwrap().poll(cx)
        }
    }
}

pub trait PingOps {
    fn echo(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> PingApiOutput;
    fn echo_async(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> FutureEchoReply;
}

enum PingHandle {
    V4(PingV4), V6(PingV6)
}

impl<'a> PingHandle {
    fn ops(&self) -> &dyn PingOps {
        match self {
            PingHandle::V4(v) => v,
            PingHandle::V6(v) => v
        }
    }
}

struct PingV4(Ipv4Addr, IcmpHandle);
struct PingV6(Ipv6Addr, IcmpHandle);

const IP_STATUS_BASE: u32 = 11_000;
const DONT_FRAGMENT_FLAG: u8 = 2;

impl PingOps for PingV4 {
    fn echo(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> PingApiOutput {
        let handle = self.1;
        let ip = self.0;
        let request_data = unsafe { buffer.as_ptr() as *const c_void };
        let ip_options = IP_OPTION_INFORMATION {
            Ttl: options.clone().map(|v| v.ttl).unwrap_or(128),
            Tos: 0,
            Flags: options.and_then(|v| if v.dont_fragment { Some(DONT_FRAGMENT_FLAG) } else { None } ).unwrap_or(0),
            OptionsSize: 0,
            OptionsData: null_mut()
        };
        let ip_options_ptr = &ip_options as *const IP_OPTION_INFORMATION;
        let mut reply_buffer: Vec<u8> = Vec::with_capacity(MAX_UDP_PACKET);
        let reply_buffer_ptr = reply_buffer.as_mut_ptr() as *mut c_void;
        unsafe {
            let destination_address = *((&ip.octets() as *const u8) as *const u32);
            let error = IcmpSendEcho2(handle, HANDLE::from(None), None, None, destination_address, request_data, buffer.len() as u16,
                                      Some(ip_options_ptr), reply_buffer_ptr, MAX_UDP_PACKET as u32, timeout);
            if error == 0 {
                Err(ping_reply_error(GetLastError().0))
            }
            else {
                let reply = reply_buffer_ptr as *mut ICMP_ECHO_REPLY;
                create_ping_reply_v4(&*reply)
            }
        }
    }
    fn echo_async(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> FutureEchoReply {
        fn to_reply(me: &FutureEchoReplyAsyncState) -> PingApiOutput {
            let reply = me.reply_buffer.as_ptr() as *const ICMP_ECHO_REPLY;
            unsafe { create_ping_reply_v4(&*reply) }
        }

        FutureEchoReply::immediate(Err(PingError::OsError(123, "dummy error".to_string())))
    }
}

impl PingOps for PingV6 {
    fn echo(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> Result<PingReply, PingError> {
        Err(PingError::OsError(123, "".to_string()))
    }
    fn echo_async<'a>(&self, buffer: &[u8], timeout: u32, options: Option<&PingOptions>) -> FutureEchoReply {
        FutureEchoReply::immediate(Err(PingError::OsError(123, "dummy error".to_string())))
    }
}

impl Drop for PingV4 {
    fn drop(&mut self) {
        unsafe { IcmpCloseHandle(self.1); }
    }
}
impl Drop for PingV6 {
    fn drop(&mut self) {
        unsafe { IcmpCloseHandle(self.1); }
    }
}

fn ping_reply_error(status_code: u32) -> PingError {
    if status_code < IP_STATUS_BASE { PingError::OsError(status_code, format!("Ping failed ({status_code})")) }
    else { PingError::IpError(status_code as i32) }
}

fn create_ping_reply_v4(reply: &ICMP_ECHO_REPLY) -> Result<PingReply, PingError> {
    let ip_status = if reply.Status as IpStatus::Type == IpStatus::Success { IpStatus::Success }
    else {
        match ping_reply_error(reply.Status) {
            v @ PingError::OsError(_, _) => return Err(v),
            PingError::IpError(v) => v
        }
    };
    let (rtt, options, buffer) = if ip_status == IpStatus::Success {
        let mut b = vec![0u8; reply.DataSize as usize];
        unsafe {
            let slice = slice_from_raw_parts::<u8>(reply.Data as *const u8, reply.DataSize as usize);
            b.copy_from_slice(&*slice);
        }
        (reply.RoundTripTime as u64,
         Some(PingOptions { ttl: reply.Options.Ttl, dont_fragment: (reply.Options.Flags & DONT_FRAGMENT_FLAG) > 0 }),
         b)
    } else {
        (0, None, Vec::new())
    };
    Ok(PingReply {
        address: IpAddr::V4(Ipv4Addr::from(reply.Address)),
        options,
        ip_status,
        rtt,
        buffer: Arc::new(buffer)
    })
}

pub fn send_ping_async(addr: &IpAddr, timeout: u32, buffer: &[u8], options: Option<&PingOptions>) -> FutureEchoReply {
    let handle_ping = initialize_icmp_handle(addr).unwrap();
    let ops = handle_ping.ops();
    ops.echo_async(buffer, timeout, options)
}

pub fn send_ping(addr: &IpAddr, timeout: u32, buffer: &[u8], options: Option<&PingOptions>) -> Result<PingReply, PingError> {
    let handle_ping = initialize_icmp_handle(addr)?;
    let ops = handle_ping.ops();
    ops.echo(buffer, timeout, options)
}

fn to_ping_error(win_err: windows::core::Error) -> PingError {
    PingError::OsError(win_err.code().0 as u32, win_err.message().to_string())
}

fn initialize_icmp_handle(addr: &IpAddr) -> Result<PingHandle, PingError> {
    unsafe {
        let handle = match addr {
            IpAddr::V4(ip) => IcmpCreateFile().map(|h| PingHandle::V4(PingV4(ip.clone(), h))),
            IpAddr::V6(ip) => Icmp6CreateFile().map(|h| PingHandle::V6(PingV6(ip.clone(), h)))
        };
        handle.map_err(to_ping_error)
    }
}

unsafe fn register_wait_handle() -> Result<HANDLE, PingError> {
    let handle = CreateEventExA(None, PCSTR::null(), CREATE_EVENT_MANUAL_RESET, 0);
    handle.map_err(to_ping_error)
}