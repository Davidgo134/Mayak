//! Reusable core of the Komet protocol: binary framing over a persistent TLS
//! socket, MessagePack payloads, LZ4/Zstd compression.
//!
//! This module set is the pure, I/O-free protocol layer (encode/decode, stream
//! de-framer, compression). Transport, session, and FFI bindings build on it.

pub mod auth;
pub mod protocol;

#[cfg(feature = "transport")]
pub mod transport;

#[cfg(feature = "transport")]
pub mod session;

#[cfg(feature = "transport")]
pub mod media;

#[cfg(feature = "calls")]
pub mod calls;

pub use protocol::{
    cmd, decode, encode, opcodes, Packet, PacketReceiver, HEADER_SIZE, PROTOCOL_VERSION,
};

#[cfg(feature = "transport")]
pub use transport::{
    Client, ClientConfig, Direction, ProxyConfig, ProxyKind, TransportError, WireTap,
};

#[cfg(feature = "transport")]
pub use session::{
    HandshakeConfig, HandshakeInfo, Session, SessionConfig, SessionState, UserAgent,
};
