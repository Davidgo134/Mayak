use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rmpv::Value;
use tokio::sync::{broadcast, oneshot, watch};
use tokio::task::JoinHandle;
use tokio::time::sleep;

use super::config::{HandshakeConfig, SessionConfig};
use crate::protocol::opcodes;
use crate::protocol::packet::Packet;
use crate::transport::{Client, TransportError, WireTap};

const PUSH_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Disconnected,
    Connecting,
    Connected,
    Online,
}

/// fields pulled from the sessionInit response; `payload` keeps the raw map so
/// the host can grab anything else (`reg-country-code`, `location`, ...).
#[derive(Debug, Clone)]
pub struct HandshakeInfo {
    pub calls_seed: Option<i64>,
    pub device_name: Option<String>,
    pub payload: Value,
}

impl HandshakeInfo {
    fn from_packet(packet: &Packet) -> Self {
        let payload = packet.value().unwrap_or(Value::Nil);
        Self {
            calls_seed: map_i64(&payload, "callsSeed"),
            device_name: map_string(&payload, "device_name"),
            payload,
        }
    }
}

struct Shared {
    config: SessionConfig,
    wire_tap: Option<WireTap>,
    ping_interactive: AtomicBool,
    client: Mutex<Option<Arc<Client>>>,
    state_tx: watch::Sender<SessionState>,
    push_tx: broadcast::Sender<Packet>,
    stop: AtomicBool,
}

impl Shared {
    fn set_state(&self, state: SessionState) {
        self.state_tx.send_if_modified(|current| {
            if *current == state {
                false
            } else {
                *current = state;
                true
            }
        });
    }
}

/// Managed session: connects, handshakes, pings to stay alive, and optionally
/// reconnects with backoff. Requests and pushes route through whichever
/// underlying [`Client`] is currently connected.
pub struct Session {
    shared: Arc<Shared>,
    supervisor: Mutex<Option<JoinHandle<()>>>,
}

impl Session {
    pub fn new(config: SessionConfig) -> Self {
        Self::with_wire_tap(config, None)
    }

    /// like [`Session::new`], but `wire_tap` sees every packet both ways, across
    /// reconnects.
    pub fn with_wire_tap(config: SessionConfig, wire_tap: Option<WireTap>) -> Self {
        let (state_tx, _) = watch::channel(SessionState::Disconnected);
        let (push_tx, _) = broadcast::channel(PUSH_CHANNEL_CAPACITY);
        let ping_interactive = AtomicBool::new(config.ping_interactive);
        Self {
            shared: Arc::new(Shared {
                config,
                wire_tap,
                ping_interactive,
                client: Mutex::new(None),
                state_tx,
                push_tx,
                stop: AtomicBool::new(false),
            }),
            supervisor: Mutex::new(None),
        }
    }

    /// resolves once online. if the first attempt fails with `auto_reconnect`
    /// set, the supervisor keeps retrying in the background but this call still
    /// returns that first error.
    pub async fn connect(&self) -> Result<HandshakeInfo, TransportError> {
        self.shared.stop.store(false, Ordering::SeqCst);
        let (first_tx, first_rx) = oneshot::channel();
        let shared = self.shared.clone();
        let handle = tokio::spawn(supervise(shared, first_tx));
        *self.supervisor.lock().unwrap() = Some(handle);

        first_rx
            .await
            .map_err(|_| TransportError::ConnectionClosed)?
    }

    pub async fn request(&self, opcode: u16, payload: &[u8]) -> Result<Packet, TransportError> {
        let client = self.shared.client.lock().unwrap().clone();
        match client {
            Some(c) => c.request(opcode, payload).await,
            None => Err(TransportError::ConnectionClosed),
        }
    }

    /// like [`Session::request`], but returns the raw response packet — an error
    /// packet comes back as `Ok` with its payload, not mapped to `Err`.
    pub async fn request_raw(&self, opcode: u16, payload: &[u8]) -> Result<Packet, TransportError> {
        let client = self.shared.client.lock().unwrap().clone();
        match client {
            Some(c) => c.request_raw(opcode, payload).await,
            None => Err(TransportError::ConnectionClosed),
        }
    }

    pub fn send(&self, opcode: u16, payload: &[u8]) -> Result<u16, TransportError> {
        let client = self.shared.client.lock().unwrap().clone();
        match client {
            Some(c) => c.send(opcode, payload),
            None => Err(TransportError::ConnectionClosed),
        }
    }

    /// keepalive `interactive` flag (foreground/background hint).
    pub fn ping_interactive(&self) -> bool {
        self.shared.ping_interactive.load(Ordering::Relaxed)
    }

    /// flip the keepalive `interactive` flag on a live session. later pings pick
    /// it up; one goes out now (best-effort) so the server hears it right away.
    pub fn set_ping_interactive(&self, interactive: bool) {
        self.shared
            .ping_interactive
            .store(interactive, Ordering::Relaxed);
        let _ = self.send(opcodes::PING, &build_ping_payload(interactive));
    }

    /// stream survives reconnects; pushes from every underlying connection land here.
    pub fn subscribe(&self) -> broadcast::Receiver<Packet> {
        self.shared.push_tx.subscribe()
    }

    pub fn state(&self) -> SessionState {
        *self.shared.state_tx.borrow()
    }

    /// HTTP User-Agent for media uploads, from this session's handshake device
    /// (opcode 6) so uploads look like the same device.
    pub fn http_user_agent(&self) -> String {
        self.shared.config.handshake.user_agent.http_user_agent()
    }

    pub fn subscribe_state(&self) -> watch::Receiver<SessionState> {
        self.shared.state_tx.subscribe()
    }

    /// stop and disable auto-reconnect.
    pub fn disconnect(&self) {
        self.shared.stop.store(true, Ordering::SeqCst);
        if let Some(client) = self.shared.client.lock().unwrap().take() {
            client.close();
        }
        if let Some(handle) = self.supervisor.lock().unwrap().take() {
            handle.abort();
        }
        self.shared.set_state(SessionState::Disconnected);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.disconnect();
    }
}

/// supervisor loop: connect, handshake, maintain, backoff, reconnect.
async fn supervise(
    shared: Arc<Shared>,
    first_tx: oneshot::Sender<Result<HandshakeInfo, TransportError>>,
) {
    let mut first_tx = Some(first_tx);
    let mut attempt: u32 = 0;

    loop {
        if shared.stop.load(Ordering::SeqCst) {
            break;
        }

        shared.set_state(SessionState::Connecting);
        match connect_and_handshake(&shared).await {
            Ok((client, info)) => {
                attempt = 0;
                *shared.client.lock().unwrap() = Some(client.clone());
                shared.set_state(SessionState::Online);
                if let Some(tx) = first_tx.take() {
                    let _ = tx.send(Ok(info));
                }

                maintain(&shared, client).await;

                *shared.client.lock().unwrap() = None;
                shared.set_state(SessionState::Disconnected);
            }
            Err(e) => {
                shared.set_state(SessionState::Disconnected);
                if let Some(tx) = first_tx.take() {
                    let _ = tx.send(Err(e));
                }
            }
        }

        if shared.stop.load(Ordering::SeqCst) || !shared.config.auto_reconnect {
            break;
        }

        let delay = reconnect_delay(attempt);
        attempt = attempt.saturating_add(1);
        sleep(delay).await;
    }
}

async fn connect_and_handshake(
    shared: &Shared,
) -> Result<(Arc<Client>, HandshakeInfo), TransportError> {
    let client = Arc::new(
        Client::connect_with_tap(shared.config.client.clone(), shared.wire_tap.clone()).await?,
    );
    let payload = build_handshake_payload(&shared.config.handshake);
    let response = client.request(opcodes::SESSION_INIT, &payload).await?;
    if !response.is_ok() {
        client.close();
        return Err(TransportError::Server {
            message: "handshake rejected by server".to_string(),
            error_key: None,
        });
    }
    let info = HandshakeInfo::from_packet(&response);
    Ok((client, info))
}

/// pings on the interval, forwards pushes into the session-wide channel,
/// returns once the connection drops.
async fn maintain(shared: &Arc<Shared>, client: Arc<Client>) {
    let ping_client = client.clone();
    let interval = shared.config.ping_interval;
    let ping_shared = shared.clone();
    let ping_task = tokio::spawn(async move {
        // first keepalive fires one interval after connect, not immediately
        let mut tick = tokio::time::interval_at(tokio::time::Instant::now() + interval, interval);
        loop {
            tick.tick().await;
            let interactive = ping_shared.ping_interactive.load(Ordering::Relaxed);
            if ping_client
                .send(opcodes::PING, &build_ping_payload(interactive))
                .is_err()
            {
                break;
            }
        }
    });

    let mut client_pushes = client.subscribe();
    let push_tx = shared.push_tx.clone();
    let forward_task = tokio::spawn(async move {
        while let Ok(packet) = client_pushes.recv().await {
            let _ = push_tx.send(packet);
        }
    });

    let mut connected = client.subscribe_connected();
    loop {
        let is_connected = *connected.borrow_and_update();
        if !is_connected {
            break;
        }
        if connected.changed().await.is_err() {
            break;
        }
    }

    ping_task.abort();
    forward_task.abort();
    client.close();
}

/// `(2 * 2^min(attempt,3)).clamp(2, 15)` => 2, 4, 8, 15, 15, ...
fn reconnect_delay(attempt: u32) -> Duration {
    let shift = attempt.min(3);
    let secs = (2u64 * (1u64 << shift)).clamp(2, 15);
    Duration::from_secs(secs)
}

fn build_handshake_payload(cfg: &HandshakeConfig) -> Vec<u8> {
    let ua = &cfg.user_agent;
    let user_agent = Value::Map(vec![
        (
            Value::from("deviceType"),
            Value::from(ua.device_type.clone()),
        ),
        (
            Value::from("appVersion"),
            Value::from(ua.app_version.clone()),
        ),
        (Value::from("osVersion"), Value::from(ua.os_version.clone())),
        (Value::from("timezone"), Value::from(ua.timezone.clone())),
        (Value::from("screen"), Value::from(ua.screen.clone())),
        (
            Value::from("pushDeviceType"),
            Value::from(ua.push_device_type.clone()),
        ),
        (Value::from("arch"), Value::from(ua.arch.clone())),
        (Value::from("locale"), Value::from(ua.locale.clone())),
        (Value::from("buildNumber"), Value::from(ua.build_number)),
        (
            Value::from("deviceName"),
            Value::from(ua.device_name.clone()),
        ),
        (
            Value::from("deviceLocale"),
            Value::from(ua.device_locale.clone()),
        ),
    ]);
    let payload = Value::Map(vec![
        (
            Value::from("mt_instanceid"),
            Value::from(cfg.instance_id.clone()),
        ),
        (Value::from("userAgent"), user_agent),
        (
            Value::from("clientSessionId"),
            Value::from(cfg.client_session_id),
        ),
        (Value::from("deviceId"), Value::from(cfg.device_id.clone())),
    ]);
    encode_value(&payload)
}

fn build_ping_payload(interactive: bool) -> Vec<u8> {
    let payload = Value::Map(vec![(Value::from("interactive"), Value::from(interactive))]);
    encode_value(&payload)
}

fn encode_value(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, value).expect("in-memory msgpack write cannot fail");
    out
}

fn map_i64(value: &Value, key: &str) -> Option<i64> {
    value
        .as_map()?
        .iter()
        .find(|(k, _)| k.as_str() == Some(key))
        .and_then(|(_, v)| v.as_i64())
}

fn map_string(value: &Value, key: &str) -> Option<String> {
    value
        .as_map()?
        .iter()
        .find(|(k, _)| k.as_str() == Some(key))
        .and_then(|(_, v)| v.as_str().map(|s| s.to_string()))
}

#[cfg(test)]
mod tests {
    use super::reconnect_delay;
    use std::time::Duration;

    #[test]
    fn backoff_matches_dart_schedule() {
        assert_eq!(reconnect_delay(0), Duration::from_secs(2));
        assert_eq!(reconnect_delay(1), Duration::from_secs(4));
        assert_eq!(reconnect_delay(2), Duration::from_secs(8));
        assert_eq!(reconnect_delay(3), Duration::from_secs(15));
        assert_eq!(reconnect_delay(4), Duration::from_secs(15));
        assert_eq!(reconnect_delay(99), Duration::from_secs(15));
    }
}
