use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc, oneshot, watch};
use tokio::task::JoinHandle;
use tokio_tungstenite::client_async_tls_with_config;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;

use super::CallError;
use crate::transport::proxy::{connect_tcp, ProxyConfig};

const NOTIF_CAPACITY: usize = 256;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(15);

/// host + port out of a `ws://`/`wss://` url, for the proxy connect.
fn ws_host_port(url: &str) -> Result<(String, u16), CallError> {
    let (scheme, rest) = url
        .split_once("://")
        .ok_or_else(|| CallError::Ws(format!("bad ws url: {url}")))?;
    let default_port = if scheme == "wss" { 443 } else { 80 };
    let authority = rest.split(['/', '?']).next().unwrap_or(rest);
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    match authority.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => Ok((h.to_string(), p.parse().unwrap())),
        _ => Ok((authority.to_string(), default_port)),
    }
}

/// default ws2 User-Agent (the app's WebSocket lib). override via
/// [`Ws2Signaling::connect`].
pub const DEFAULT_USER_AGENT: &str = "okhttp/4.12.0";

/// Call signaling over the ws2 WebSocket: SDP offer/answer, ICE candidates,
/// accept/hangup, SFU negotiation, all as JSON.
///
/// envelope:
/// - request:      `{"command": ..., ..., "sequence": N}`
/// - response:     `{"sequence": N, "response": "<command>", "type": "response"}`
/// - notification: `{..., "notification": "<name>", "type": "notification"}`
/// - keepalive:    text frame `ping`, answered with `pong`
pub struct Ws2Signaling {
    seq: AtomicI64,
    write_tx: mpsc::UnboundedSender<String>,
    pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>,
    notif_tx: broadcast::Sender<Value>,
    connected_tx: watch::Sender<bool>,
    tasks: Vec<JoinHandle<()>>,
}

impl Ws2Signaling {
    pub async fn connect(url: &str, user_agent: Option<&str>) -> Result<Self, CallError> {
        Self::connect_via(url, user_agent, None).await
    }

    /// like [`Ws2Signaling::connect`], but through `proxy` (HTTP CONNECT or
    /// SOCKS5).
    pub async fn connect_via(
        url: &str,
        user_agent: Option<&str>,
        proxy: Option<&ProxyConfig>,
    ) -> Result<Self, CallError> {
        let mut request = url
            .into_client_request()
            .map_err(|e| CallError::Ws(e.to_string()))?;
        let ua = user_agent.unwrap_or(DEFAULT_USER_AGENT);
        request.headers_mut().insert(
            "User-Agent",
            ua.parse()
                .map_err(|_| CallError::Ws("invalid user agent".into()))?,
        );

        let ws = match proxy {
            None => {
                tokio_tungstenite::connect_async(request)
                    .await
                    .map_err(|e| CallError::Ws(e.to_string()))?
                    .0
            }
            Some(p) => {
                let (host, port) = ws_host_port(url)?;
                let tcp = connect_tcp(&host, port, DEFAULT_TIMEOUT, Some(p))
                    .await
                    .map_err(|e| CallError::Ws(e.to_string()))?;
                client_async_tls_with_config(request, tcp, None, None)
                    .await
                    .map_err(|e| CallError::Ws(e.to_string()))?
                    .0
            }
        };
        let (mut write, mut read) = ws.split();

        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<String>();
        let (notif_tx, _) = broadcast::channel(NOTIF_CAPACITY);
        let (connected_tx, _) = watch::channel(true);
        let pending: Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let writer = tokio::spawn(async move {
            while let Some(text) = write_rx.recv().await {
                if write.send(Message::text(text)).await.is_err() {
                    break;
                }
            }
        });

        let reader_pending = pending.clone();
        let reader_notif = notif_tx.clone();
        let reader_write = write_tx.clone();
        let reader_connected = connected_tx.clone();
        let reader = tokio::spawn(async move {
            while let Some(msg) = read.next().await {
                let text = match msg {
                    Ok(Message::Text(t)) => t.as_str().to_string(),
                    Ok(Message::Binary(b)) => String::from_utf8_lossy(&b).into_owned(),
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => continue,
                };
                route(&text, &reader_pending, &reader_notif, &reader_write);
            }
            reader_connected.send_replace(false);
            for (_, tx) in reader_pending.lock().unwrap().drain() {
                drop(tx);
            }
        });

        Ok(Self {
            seq: AtomicI64::new(0),
            write_tx,
            pending,
            notif_tx,
            connected_tx,
            tasks: vec![writer, reader],
        })
    }

    /// server notifications (`type == "notification"`); filter on `notification`.
    pub fn notifications(&self) -> broadcast::Receiver<Value> {
        self.notif_tx.subscribe()
    }

    pub fn is_connected(&self) -> bool {
        *self.connected_tx.borrow()
    }

    /// send a command, await the response; errors if the response carries one.
    pub async fn send_command(&self, command: &str, extra: Value) -> Result<Value, CallError> {
        if !self.is_connected() {
            return Err(CallError::Closed);
        }
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;

        let mut obj = match extra {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        obj.insert("command".into(), json!(command));
        obj.insert("sequence".into(), json!(seq));
        let text = Value::Object(obj).to_string();

        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap().insert(seq, tx);
        self.write_tx.send(text).map_err(|_| CallError::Closed)?;

        let resp = match tokio::time::timeout(DEFAULT_TIMEOUT, rx).await {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => return Err(CallError::Closed),
            Err(_) => return Err(CallError::Timeout),
        };

        let is_error = resp.get("type").and_then(|t| t.as_str()) == Some("error")
            || resp.get("error").is_some();
        if is_error {
            let error = resp
                .get("error")
                .map(|e| e.to_string())
                .unwrap_or_else(|| "error".to_string());
            return Err(CallError::Command {
                command: command.to_string(),
                error,
            });
        }
        Ok(resp)
    }

    /// SDP offer/answer to another participant.
    pub async fn transmit_sdp(
        &self,
        participant_id: i64,
        sdp_type: &str,
        sdp: &str,
    ) -> Result<Value, CallError> {
        self.send_command(
            "transmit-data",
            json!({
                "participantId": participant_id,
                "participantType": "USER",
                "deviceIdx": 0,
                "data": { "sdp": { "type": sdp_type, "sdp": sdp } },
                "capabilities": "1",
            }),
        )
        .await
    }

    /// trickle ICE candidate to another participant.
    pub async fn transmit_candidate(
        &self,
        participant_id: i64,
        candidate: &str,
        sdp_mid: &str,
        sdp_mline_index: i64,
    ) -> Result<Value, CallError> {
        self.send_command(
            "transmit-data",
            json!({
                "participantId": participant_id,
                "participantType": "USER",
                "deviceIdx": 0,
                "data": { "candidate": {
                    "candidate": candidate,
                    "sdpMid": sdp_mid,
                    "sdpMLineIndex": sdp_mline_index,
                }},
            }),
        )
        .await
    }

    pub async fn accept_call(&self) -> Result<Value, CallError> {
        self.send_command("accept-call", json!({})).await
    }

    pub async fn hangup(&self, reason: &str) -> Result<Value, CallError> {
        self.send_command("hangup", json!({ "reason": reason }))
            .await
    }

    pub async fn change_media_settings(
        &self,
        audio: bool,
        video: bool,
        screen: bool,
    ) -> Result<Value, CallError> {
        self.send_command(
            "change-media-settings",
            json!({ "mediaSettings": {
                "isAudioEnabled": audio,
                "isVideoEnabled": video,
                "isScreenSharingEnabled": screen,
                "isAnimojiEnabled": false,
            }}),
        )
        .await
    }

    pub fn close(&self) {
        self.connected_tx.send_replace(false);
        for (_, tx) in self.pending.lock().unwrap().drain() {
            drop(tx);
        }
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl Drop for Ws2Signaling {
    fn drop(&mut self) {
        self.close();
    }
}

fn route(
    text: &str,
    pending: &Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>,
    notif_tx: &broadcast::Sender<Value>,
    write_tx: &mpsc::UnboundedSender<String>,
) {
    if text == "ping" {
        let _ = write_tx.send("pong".to_string());
        return;
    }
    let value: Value = match serde_json::from_str(text) {
        Ok(v) => v,
        Err(_) => return,
    };
    let ty = value.get("type").and_then(|t| t.as_str());
    if ty == Some("response") || ty == Some("error") {
        if let Some(seq) = value.get("sequence").and_then(|s| s.as_i64()) {
            if let Some(tx) = pending.lock().unwrap().remove(&seq) {
                let _ = tx.send(value.clone());
            }
        }
        if ty == Some("error") {
            let _ = notif_tx.send(value);
        }
        return;
    }
    if ty == Some("notification") || value.get("notification").is_some() {
        let _ = notif_tx.send(value);
    }
}
