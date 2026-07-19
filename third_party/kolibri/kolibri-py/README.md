# kolibri (Python)

Python bindings for [`kolibri-net`](../kolibri-net) — the Komet messaging protocol
in Rust. A synchronous `Session` that owns a tokio runtime internally;
MessagePack payloads are exposed as native Python dicts/lists, so you never touch
bytes.

## Build & install

```bash
python -m venv .venv && source .venv/bin/activate
pip install maturin
# Python ≥ 3.14 needs the abi3 forward-compat flag until pyo3 ships explicit support:
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop
```

`maturin develop` installs the module into the active venv. `maturin build
--release` produces a redistributable wheel.

## Usage

```python
import kolibri

s = kolibri.Session("api.oneme.ru", 443)      # + optional device/handshake kwargs
info = s.connect()                          # connect + sessionInit handshake
print(s.state())                            # "online"
print(info["calls_seed"])

resp = s.request(opcode, {"key": "value"})  # dict in → dict out (msgpack under the hood)
seq  = s.send(opcode, {"typing": True})     # fire-and-forget

push = s.next_push(timeout_secs=5)          # {"opcode": ..., "payload": ...} or None

s.disconnect()
```

### Phone auth

```bash
python examples/auth.py +7XXXXXXXXXX        # sends a real SMS, then verifies the code
python examples/handshake.py                # no SMS, just proves the stack
```

`kolibri.auth_mode(calls_seed, device_id)` computes the 96-byte anti-spoof `mode`
fingerprint for the authRequest payload.

## API

| Method | Description |
|--------|-------------|
| `Session(host, port=443, **device_kwargs)` | Create a session (device/handshake fields have defaults). |
| `.connect() -> dict` | Connect + handshake; returns `{calls_seed, device_name, payload}`. |
| `.request(opcode, payload) -> dict` | Send and await the response payload (raises on server error/timeout). |
| `.send(opcode, payload) -> int` | Fire-and-forget; returns the sequence number. |
| `.next_push(timeout_secs=None) -> dict \| None` | Wait for the next server push. |
| `.state() -> str` | `"disconnected"` / `"connecting"` / `"connected"` / `"online"`. |
| `.disconnect()` | Stop the session and disable auto-reconnect. |
| `kolibri.auth_mode(calls_seed, device_id) -> bytes` | Anti-spoof handshake fingerprint. |
