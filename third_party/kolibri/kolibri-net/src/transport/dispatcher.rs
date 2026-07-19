use std::collections::HashMap;
use std::sync::Mutex;

use tokio::sync::{broadcast, oneshot};

use super::error::TransportError;
use crate::protocol::packet::{cmd, Packet};

type PendingResult = Result<Packet, TransportError>;

/// Routes incoming packets: responses match a waiting request by `seq`, pushes
/// (cmd == 0) fan out to all subscribers.
pub struct Dispatcher {
    pending: Mutex<HashMap<u16, oneshot::Sender<PendingResult>>>,
    push_tx: broadcast::Sender<Packet>,
}

impl Dispatcher {
    pub fn new(push_capacity: usize) -> Self {
        let (push_tx, _) = broadcast::channel(push_capacity);
        Self {
            pending: Mutex::new(HashMap::new()),
            push_tx,
        }
    }

    /// If `seq` was reused before its response arrived, the previous waiter fails.
    pub fn register(&self, seq: u16) -> oneshot::Receiver<PendingResult> {
        let (tx, rx) = oneshot::channel();
        let mut pending = self.pending.lock().unwrap();
        if let Some(old) = pending.insert(seq, tx) {
            let _ = old.send(Err(TransportError::ConnectionClosed));
        }
        rx
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Packet> {
        self.push_tx.subscribe()
    }

    pub fn dispatch(&self, packet: Packet) {
        let is_response = matches!(packet.cmd, cmd::OK | cmd::ERROR | cmd::NOT_FOUND);
        if is_response {
            let waiter = self.pending.lock().unwrap().remove(&packet.seq);
            let Some(tx) = waiter else {
                return;
            };
            // deliver the raw packet (error packets included); the caller decides
            // whether to map an error packet to `Err` or read it raw.
            let _ = tx.send(Ok(packet));
        } else {
            // send errors only when there are no subscribers, ignore that
            let _ = self.push_tx.send(packet);
        }
    }

    /// Called on disconnect so awaiting callers get `ConnectionClosed` instead
    /// of hanging.
    pub fn fail_all(&self) {
        let mut pending = self.pending.lock().unwrap();
        for (_, tx) in pending.drain() {
            let _ = tx.send(Err(TransportError::ConnectionClosed));
        }
    }
}

pub(crate) fn error_from_payload(packet: &Packet) -> TransportError {
    let value = match packet.value() {
        Ok(v) => v,
        Err(_) => {
            return TransportError::Server {
                message: "unknown error".into(),
                error_key: None,
            }
        }
    };

    let message = extract_message(&value);
    if map_str(&value, "message").as_deref() == Some("FAIL_LOGIN_TOKEN") {
        return TransportError::SessionExpired(message);
    }
    TransportError::Server {
        message,
        error_key: map_str(&value, "error"),
    }
}

fn extract_message(value: &rmpv::Value) -> String {
    for key in ["localizedMessage", "message", "title"] {
        if let Some(s) = map_str(value, key) {
            let trimmed = s.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    "unknown error".to_string()
}

fn map_str(value: &rmpv::Value, key: &str) -> Option<String> {
    let map = value.as_map()?;
    map.iter()
        .find(|(k, _)| k.as_str() == Some(key))
        .and_then(|(_, v)| v.as_str().map(|s| s.to_string()))
}
