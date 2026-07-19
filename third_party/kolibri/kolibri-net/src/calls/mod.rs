//! Call setup and signaling. The main protocol socket only bootstraps a call
//! (opcode 78/166 hand back a `vcp`/endpoint, push 137 announces an incoming
//! one); everything live runs on a separate ws2 WebSocket. WebRTC media stays in
//! the host.

mod events;
mod signaling;
mod vcp;

use thiserror::Error;

pub use events::{
    parse_connection, parse_transmitted_data, CallEvent, ConnectionInfo, TransmittedData,
};
pub use signaling::{Ws2Signaling, DEFAULT_USER_AGENT as DEFAULT_WS2_USER_AGENT};
pub use vcp::{ws2_url_from_endpoint, ConversationParams, IceServer, Ws2ClientInfo};

#[derive(Debug, Error)]
pub enum CallError {
    #[error("websocket error: {0}")]
    Ws(String),
    #[error("command '{command}' failed: {error}")]
    Command { command: String, error: String },
    #[error("request timed out")]
    Timeout,
    #[error("connection closed")]
    Closed,
}
