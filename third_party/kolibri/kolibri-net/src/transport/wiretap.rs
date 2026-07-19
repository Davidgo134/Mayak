//! Optional tap on the traffic: fires once per packet each way, with the
//! uncompressed msgpack, before dispatch. Keep the callback cheap — it runs on
//! the I/O tasks.

use std::sync::Arc;

/// packet direction as the tap saw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// client -> server (request, send, ping)
    Out,
    /// server -> client (response or push)
    In,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Direction::Out => "out",
            Direction::In => "in",
        }
    }
}

/// `(direction, cmd, opcode, seq, msgpack)`. cmd is the command byte
/// ([`crate::protocol::cmd`]): out is always REQUEST; in is OK/NOT_FOUND/ERROR,
/// or PUSH (== REQUEST == 0) for a push. payload is uncompressed msgpack — run it
/// through [`crate::protocol::value_to_json`] for a log line.
pub type WireTap = Arc<dyn Fn(Direction, u8, u16, u16, &[u8]) + Send + Sync>;
