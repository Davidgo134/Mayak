//! Persistent TLS connection with seq-multiplexed request/response and a
//! broadcast stream of server pushes. tokio + rustls over the protocol codec.

mod client;
mod dispatcher;
mod error;
pub(crate) mod proxy;
pub(crate) mod tls;
mod wiretap;

pub use client::{Client, ClientConfig};
pub use error::TransportError;
pub use proxy::{ProxyConfig, ProxyKind};
pub use wiretap::{Direction, WireTap};
