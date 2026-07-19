//! Session state machine over the transport client: connect, sessionInit
//! handshake, online, with keepalive pings and exponential-backoff reconnect.
//! Host supplies device values; the wire shape and connect/ping/reconnect
//! sequence live here.

mod config;
mod manager;

pub use config::{HandshakeConfig, SessionConfig, UserAgent};
pub use manager::{HandshakeInfo, Session, SessionState};
