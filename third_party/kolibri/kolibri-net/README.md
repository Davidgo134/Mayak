# kolibri-net

Reusable Rust core of the Komet messaging protocol — a hand-rolled binary
framing over a persistent TLS TCP socket, with MessagePack payloads and LZ4/Zstd
compression. Extracted from the Flutter client (`lib/core/transport/` +
`lib/core/protocol/`) so the same protocol implementation can be shared across
Flutter (via `flutter_rust_bridge`), Python (via PyO3/maturin), and any other
host over a C ABI.

## Status

| Phase | Scope | State |
|-------|-------|-------|
| **1** | Protocol core: packet codec, framing, compression, opcodes | ✅ done |
| **2** | Async transport: tokio TCP + TLS (rustls), seq/opcode dispatcher | ✅ done |
| **3** | Session state machine: handshake, ping keepalive, reconnect+backoff | ✅ done |
| **4** | FFI: Python (`pyo3`/`maturin`) ✅ · Dart (`flutter_rust_bridge`) planned | 🚧 |
| 5 | Swap Dart transport behind a flag, live-compare, remove | planned |

Proxy: `ClientConfig::proxy` takes a `ProxyConfig` (HTTP CONNECT or SOCKS5, with
optional user/pass), applied to the main socket, media uploads, and ws2 calls.
Parse one from a url with `ProxyConfig::parse("http://user:pass@host:port")`
(`http` / `socks5` / `socks5h`).

Not yet ported: VPN-bypass (Android). Production TLS uses the bundled Mozilla
root store — swap for `rustls-platform-verifier` to match Dart's OS trust store.

## Wire format (10-byte big-endian header)

```text
[0]      ver       protocol version (u8, = 10)
[1]      cmd       0 request/push · 1 ok · 2 not_found · 3 error
[2..4]   seq       sequence number (u16 BE)
[4..6]   opcode    operation code (u16 BE)
[6..10]  packedLen high byte = compression flag, low 24 bits = payload length
[10..]   payload   MessagePack, LZ4-block / LZ4-frame / Zstd (sniffed by magic)
```

Payloads under 32 bytes are sent uncompressed. Outgoing compression is LZ4
frame; incoming is sniffed by magic number (Zstd `28 B5 2F FD`, LZ4 frame
`04 22 4D 18`, otherwise LZ4 block).

## Design

The core is I/O-free and representation-agnostic: `encode`/`decode` work on raw
MessagePack bytes so a Dart `Map`, a Python `dict`, and a Rust struct all come
from the same `Packet.payload`. `PacketReceiver` de-frames the raw TLS byte
stream into complete packets.

```rust
use kolibri_net::{encode, decode, protocol::opcodes};

let wire = encode(opcodes::MSG_SEND, &msgpack_bytes, seq);
let packet = decode(&wire)?;
let value = packet.value()?; // rmpv::Value
```

## Build & test

```bash
cargo test        # 14 protocol vectors + 5 transport + 6 session + 1 backoff
cargo clippy --all-targets
```

The `transport` feature (async client, on by default) can be disabled to build
the pure protocol codec with no tokio/rustls dependency:

```bash
cargo build --no-default-features
```

### Transport usage

```rust
use kolibri_net::{Client, ClientConfig, protocol::opcodes};

let client = Client::connect(ClientConfig::new("host.example", 443)).await?;
let mut pushes = client.subscribe();               // broadcast of server pushes
let resp = client.request(opcodes::CHATS_LIST, &msgpack_bytes).await?;
```

### Session usage (handshake + keepalive + reconnect)

```rust
use kolibri_net::{Session, SessionConfig, ClientConfig, HandshakeConfig, UserAgent};

let config = SessionConfig::new(
    ClientConfig::new("host.example", 443),
    HandshakeConfig { /* device values from the host */ },
);
let session = Session::new(config);
let info = session.connect().await?;               // connect + sessionInit → Online
let resp = session.request(opcodes::AUTH_REQUEST, &msgpack_bytes).await?;
```

The host (Flutter, Python, …) supplies device values for the handshake; the wire
shape and the connect → handshake → ping → reconnect sequence live in Rust.
