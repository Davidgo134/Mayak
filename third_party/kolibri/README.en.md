# kolibri

*[Русская версия](README.md)*

A portable, Rust-based client core for a binary messaging protocol: hand-rolled
framing over a persistent TLS socket, MessagePack payloads, LZ4/Zstd compression,
a full session state machine (handshake, keepalive, reconnect), CDN media
uploads, and call signaling (ws2). The protocol is written **once in Rust** and
thin wrappers expose it to Python, Dart, and any host via a C ABI.

The core is UI- and platform-agnostic: bytes in, bytes out.

## Layout

| Crate | What's inside |
|-------|---------------|
| [`kolibri-net`](kolibri-net) | Rust core: packet codec, framing, compression, async TLS transport, session state machine, anti-spoof fingerprint, media uploads, calls (vcp + ws2 signaling). |
| [`kolibri-py`](kolibri-py) | Python bindings (pyo3/maturin): a synchronous `Session`, calls, uploads, native dicts instead of bytes. |
| [`kolibri-dart`](kolibri-dart) | Dart/Flutter bindings (`flutter_rust_bridge`): async `Future`s + push/progress `Stream`s. |
| [`kolibri-go`](kolibri-go) | Go bindings (cgo over a C ABI): a blocking `Session`, wire-log via callback. |

The idea: the session machine (handshake, ping, reconnect) and protocol parsing
live in the core, so every binding gets them for free.

## Features

- **Transport** — persistent TLS socket (tokio + rustls), request/response
  multiplexing by seq, a broadcast stream of server pushes, auto-reconnect with
  exponential backoff.
- **Protocol** — 10-byte binary header, MessagePack, LZ4-block compression
  outbound with Zstd/LZ4-frame/LZ4-block sniffing inbound.
- **Session** — sessionInit handshake (opcode 6) from device fields, keepalive
  ping, `disconnected/connecting/connected/online` states.
- **Auth** — request code → verify → login; anti-spoof fingerprint (3×SHA-256)
  whose digests are supplied by the host (not baked in).
- **Device spoofing** — every handshake `userAgent` field is host-supplied;
  ready-made real-device presets. The media UA is derived from those same fields.
- **Media** — CDN upload: file (single POST), photo (multipart), video (parallel
  resumable chunks), with progress callbacks.
- **Calls** — `vcp` decode, ws2 signaling (SDP/ICE/accept/hangup), typed parsing
  of incoming notifications. The WebRTC media stack itself stays in the host.
- **Proxy** — HTTP CONNECT and SOCKS5 (with auth) for the main socket, media
  uploads, and ws2 calls.

## Quick start

**Rust:**
```rust
use kolibri_net::{Session, SessionConfig, ClientConfig, HandshakeConfig};

let session = Session::new(SessionConfig::new(
    ClientConfig::new("host.example", 443),
    handshake, // device fields
));
let info = session.connect().await?;                 // handshake
let resp = session.request(opcode, &msgpack).await?; // request/response
```

**Python:**
```python
import kolibri

s = kolibri.Session("host.example", 443)  # + optional device kwargs
info = s.connect()                        # handshake -> online
resp = s.request(opcode, {"key": "value"})  # dict in -> dict out
```

**Dart:**
```dart
import 'package:kolibri/kolibri.dart';

await initKolibri(libraryPath: '.../libkolibri_dart.dylib'); // not needed on Flutter
final s = openSession(host: 'host.example');
final info = await s.connect();
final resp = await s.request(opcode: 64, payload: msgpackBytes);
```

**Go:**
```go
s, _ := kolibri.Open(kolibri.DefaultConfig("host.example"))
defer s.Close()
info, _ := s.Connect()                      // handshake
resp, _ := s.Request(opcode, msgpackBytes)  // request/response
```

## Build & test

**Rust core:**
```bash
cargo test              # protocol + transport + session + media + calls
cargo clippy
cargo build --no-default-features   # pure codec, no tokio/rustls
```

**Python:**
```bash
cd kolibri-py
python -m venv ../.venv && source ../.venv/bin/activate && pip install maturin
maturin develop         # on a very new Python + abi3: PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
python examples/handshake.py
```

**Dart:**
```bash
cd kolibri-dart
dart pub get
flutter_rust_bridge_codegen generate
dart run build_runner build --delete-conflicting-outputs
cargo build --manifest-path rust/Cargo.toml
dart run example/handshake.dart
```

## Feature flags

- `transport` (default on) — async transport (tokio + rustls). Turn it off for
  the pure protocol codec with no networking.
- `calls` (default on) — calls (ws2 signaling, WebSocket). Disable it if you
  don't need calls and don't want WebSocket pulled into the binary.

## Proxy

The connection (main socket + media + ws2 calls) can go through a proxy — HTTP
CONNECT or SOCKS5, with auth. Set it with a url `scheme://[user:pass@]host:port`,
schemes `http` / `socks5` / `socks5h`.

```python
s = kolibri.Session("api.oneme.ru", proxy="http://user:pass@10.0.0.1:8080")
s = kolibri.Session("api.oneme.ru", proxy="socks5://127.0.0.1:1080")
```
```dart
final s = openSession(host: 'api.oneme.ru', proxy: 'socks5://user:pass@127.0.0.1:1080');
```
In Rust — `ClientConfig::new(host, port).proxy(Some(ProxyConfig::parse(url)?))`.

## Traffic logging

The wire is MessagePack, but there's a JSON view for logs (lossy: binary and ext
become base64, non-string map keys are stringified — read-only, don't round-trip).

- **Responses:** `Session.request_json(opcode, payload)` (Python/Dart) — same as
  `request` but returns a JSON string.
- **Everything, both directions** — a "wire tap": one callback per packet
  (requests, pushes, handshake, ping) that survives reconnects.

```python
def on_wire(direction, cmd, opcode, seq, js):   # direction "out"|"in", cmd "request"/"ok"/"error"/"push"/"not_found"
    print(f'{direction} cmd={cmd} op={opcode} seq={seq} {js}')

s = kolibri.Session("host.example", on_wire=on_wire)
s.connect()   # -> out request op=6 ... / <- in ok op=6 ...
```

In Dart, `openSessionWithWireLog(...)` returns `(session, Stream<WireLogEvent>)`:
```dart
final (session, wire) = openSessionWithWireLog(host: 'host.example');
wire.listen((e) => print('${e.direction} ${e.cmd} op=${e.opcode} ${e.json}'));
await session.connect();
```

In Rust it's `Session::with_wire_tap(config, tap)` with
`WireTap = Arc<dyn Fn(Direction, u8 cmd, u16 opcode, u16 seq, &[u8])>`.

## Example: answering music bot

`kolibri-py/examples/call_bot.py` logs in, waits for an incoming call,
auto-accepts, and plays an audio track into it (`.opus`/`.mp3`/`.wav`/`.webm`).
Signaling goes through kolibri, WebRTC media through
[aiortc](https://github.com/aiortc/aiortc).

```bash
cd kolibri-py
KOLIBRI_TRACK=track.opus KOLIBRI_PHONE=+7XXXXXXXXXX \
  ../.venv/bin/python examples/call_bot.py
```

> **Calls:** keep the VPN off, it breaks the UDP media path (ICE completes but
> DTLS won't establish through NAT/VPN). For a machine behind NAT a public IP is
> more reliable.

## Status

The protocol core, transport, session machine, media uploads, and calls are done
and verified live against a production server (full auth flow, login, upload, a
call with media). The Python and Dart bindings work and drive the same core.

## License

Dual-licensed, at your option:

- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT ([LICENSE-MIT](LICENSE-MIT))

Any contribution submitted to the project is dual-licensed as above, without any
additional terms.
