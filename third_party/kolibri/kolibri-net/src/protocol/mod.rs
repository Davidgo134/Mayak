//! Komet binary protocol: packet layout, wire codec, stream framing,
//! compression, opcodes. Transport-agnostic, no I/O; just bytes <-> [`Packet`].

pub mod codec;
pub mod compress;
pub mod framing;
#[cfg(feature = "json")]
pub mod json;
pub mod opcodes;
pub mod packet;

pub use codec::{
    decode, encode, encode_with_cmd, packet_total_len, CodecError, COMPRESSION_THRESHOLD,
};
pub use framing::{OverflowError, PacketReceiver};
#[cfg(feature = "json")]
pub use json::{json_to_value, value_to_json, value_to_json_tagged};
pub use packet::{cmd, Packet, HEADER_SIZE, PROTOCOL_VERSION};
