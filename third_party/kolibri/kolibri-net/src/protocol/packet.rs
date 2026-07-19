/// Wire header, 10 bytes big-endian:
///
/// ```text
/// [0]      ver       protocol version (default 10)
/// [1]      cmd       command type
/// [2..4]   seq       sequence number
/// [4..6]   opcode    operation code
/// [6..10]  packedLen high byte = compression flag, low 24 bits = payload length
/// [10..]   payload   MessagePack, optionally compressed
/// ```
pub const HEADER_SIZE: usize = 10;

pub const PROTOCOL_VERSION: u8 = 10;

/// Request and push both carry cmd == 0; direction disambiguates.
pub mod cmd {
    pub const REQUEST: u8 = 0;
    pub const PUSH: u8 = 0;
    pub const OK: u8 = 1;
    pub const NOT_FOUND: u8 = 2;
    pub const ERROR: u8 = 3;
}

/// Decoded packet. `payload` is decompressed MessagePack bytes (empty for no
/// body); the caller turns it into a concrete value via [`Packet::value`], so
/// the core stays representation-agnostic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    pub ver: u8,
    pub cmd: u8,
    pub seq: u16,
    pub opcode: u16,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn is_ok(&self) -> bool {
        self.cmd == cmd::OK
    }

    pub fn is_error(&self) -> bool {
        self.cmd == cmd::ERROR
    }

    pub fn is_not_found(&self) -> bool {
        self.cmd == cmd::NOT_FOUND
    }

    /// Push has cmd == 0 like an outgoing request; only meaningful on inbound.
    pub fn is_push(&self) -> bool {
        self.cmd == cmd::PUSH
    }

    /// Empty payload decodes to `Value::Nil`.
    pub fn value(&self) -> Result<rmpv::Value, rmpv::decode::Error> {
        if self.payload.is_empty() {
            return Ok(rmpv::Value::Nil);
        }
        rmpv::decode::read_value(&mut &self.payload[..])
    }

    /// payload as JSON, for logs (lossy — see [`crate::protocol::json`]).
    #[cfg(feature = "json")]
    pub fn json(&self) -> Result<serde_json::Value, rmpv::decode::Error> {
        self.value().map(|v| super::json::value_to_json(&v))
    }

    /// payload as JSON with binary tagged `{"$bin":...}` — round-trips, unlike
    /// [`Packet::json`] (see [`crate::protocol::json`]).
    #[cfg(feature = "json")]
    pub fn json_tagged(&self) -> Result<serde_json::Value, rmpv::decode::Error> {
        self.value().map(|v| super::json::value_to_json_tagged(&v))
    }
}
