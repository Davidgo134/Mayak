use crate::protocol::codec::CodecError;
use crate::protocol::framing::OverflowError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("server error: {message}")]
    Server {
        message: String,
        error_key: Option<String>,
    },

    /// `FAIL_LOGIN_TOKEN`; session must be re-established
    #[error("session expired: {0}")]
    SessionExpired(String),

    #[error("request timed out")]
    Timeout,

    #[error("connection closed")]
    ConnectionClosed,

    #[error("connect timed out")]
    ConnectTimeout,

    #[error("TLS error: {0}")]
    Tls(String),

    #[error("invalid configuration: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Codec(#[from] CodecError),

    #[error(transparent)]
    Overflow(#[from] OverflowError),
}
