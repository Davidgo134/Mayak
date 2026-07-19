# kolibri-go

Go binding for [kolibri-net](../kolibri-net), over cgo. A `Session` owns a tokio
runtime in the Rust core; every call blocks until it completes. msgpack payloads
cross the boundary as bytes — decode them with a Go msgpack library.

## Build

The Go package links a Rust static library, so build that first:

```bash
cargo build --release --manifest-path rust/Cargo.toml   # -> rust/target/release/libkolibri_go.a
go build ./...
go run ./example/handshake
```

The cgo directives in `kolibri.go` point at `rust/target/release`; on macOS they
also link `CoreFoundation` and `Security`.

## Use

```go
cfg := kolibri.DefaultConfig("api.oneme.ru")
cfg.Proxy = "socks5://user:pass@127.0.0.1:1080" // optional
cfg.OnWire = func(e kolibri.WireEvent) {         // optional traffic log
    fmt.Printf("%s %s op=%d %s\n", e.Direction, e.Cmd, e.Opcode, e.JSON)
}

s, err := kolibri.Open(cfg)
if err != nil { log.Fatal(err) }
defer s.Close()

info, err := s.Connect()                // handshake -> decoded map
s.SetPingInteractive(false)             // foreground/background hint, live

// Build requests without a msgpack library — the core does the encoding:
resp, err := s.RequestMap(opcode, map[string]any{"field": "value"}) // map in/out
js, err := s.RequestJSON(opcode, `{"field":"value"}`)               // JSON in/out
raw, err := s.Request(opcode, msgpackBytes)                         // or raw bytes

op, push, ok, err := s.NextPush(5 * time.Second)                    // decoded push map
```

No msgpack dependency: `Connect`, `RequestMap`/`RequestJSON`, and `NextPush` all
speak Go `map`/JSON, and the core does the encoding. A `[]byte` in a request map
is sent as a binary field. Numbers in decoded maps are `json.Number` (so large
`int64` like `callsSeed` keep full precision) — read them with
`m["callsSeed"].(json.Number).Int64()`. `ConnectRaw` / `NextPushRaw` /
`Request` give raw msgpack bytes if you'd rather decode yourself.

## Surface

- Session: `Open`, `Connect` (map), `RequestMap` / `RequestJSON`, `NextPush`
  (map), `Send`, `State`, `PingInteractive` / `SetPingInteractive`, `UserAgent`,
  `Disconnect`, `Close`; `Request` / `ConnectRaw` / `NextPushRaw` for raw msgpack.
- Media: `UploadFile`, `UploadPhoto`, `UploadVideo` (through the session's proxy).
- Proxy: `Config.Proxy` (HTTP CONNECT or SOCKS5, with auth).
- Logging: `Config.OnWire` (both directions), `RequestJSON`.
- Auth: `AuthMode` (96-byte anti-spoof fingerprint; digests default to the
  reference client's, override per build).
- Calls (ws2 signaling): `DecodeVCP`, `ParseConnection`, `ParseTransmittedData`,
  and a `Call` client (`ConnectCall`, `Accept`, `Hangup`, `TransmitSDP`,
  `TransmitCandidate`, `ChangeMedia`, `SendCommand`, `NextNotification`,
  `Close`). Signaling only — the WebRTC media stack stays in your app.

## License

Dual MIT / Apache-2.0, matching the workspace.
