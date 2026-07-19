use serde_json::Value;

use super::vcp::IceServer;

/// ws2 `connection` notification: topology, participants, and ICE servers
/// (from `conversationParams`, authoritative over the vcp).
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub topology: Option<String>,
    pub participants: Vec<i64>,
    pub ice_servers: Vec<IceServer>,
}

impl ConnectionInfo {
    pub fn is_sfu(&self) -> bool {
        self.topology.as_deref() == Some("SERVER")
    }

    /// the participant that isn't us, the peer to answer.
    pub fn peer_of(&self, my_user_id: i64) -> Option<i64> {
        self.participants
            .iter()
            .copied()
            .find(|&id| id != my_user_id)
    }
}

/// payload of a `transmitted-data` notification: SDP offer/answer or ICE candidate.
#[derive(Debug, Clone)]
pub enum TransmittedData {
    Sdp {
        sdp_type: String,
        sdp: String,
    },
    Candidate {
        candidate: String,
        sdp_mid: Option<String>,
        sdp_mline_index: Option<i64>,
    },
}

/// categorised ws2 notification.
#[derive(Debug, Clone)]
pub enum CallEvent {
    Connection(ConnectionInfo),
    Transmitted(TransmittedData),
    Hungup,
    Closed,
    TopologyChanged(Option<String>),
    Error(String),
    Other(String),
}

impl CallEvent {
    pub fn parse(value: &Value) -> CallEvent {
        if value.get("type").and_then(|t| t.as_str()) == Some("error") {
            let msg = value
                .get("error")
                .map(|e| e.to_string())
                .unwrap_or_default();
            return CallEvent::Error(msg);
        }
        let name = value
            .get("notification")
            .and_then(|n| n.as_str())
            .unwrap_or("");
        match name {
            "connection" => CallEvent::Connection(parse_connection(value)),
            "transmitted-data" => match parse_transmitted_data(value) {
                Some(td) => CallEvent::Transmitted(td),
                None => CallEvent::Other(name.to_string()),
            },
            "hungup" => CallEvent::Hungup,
            "closed-conversation" => CallEvent::Closed,
            "topology-changed" => CallEvent::TopologyChanged(topology_of(value)),
            other => CallEvent::Other(other.to_string()),
        }
    }
}

pub fn parse_connection(value: &Value) -> ConnectionInfo {
    let conversation = value.get("conversation");
    ConnectionInfo {
        topology: conversation
            .and_then(|c| c.get("topology"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string()),
        participants: conversation
            .and_then(|c| c.get("participants"))
            .and_then(|p| p.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|p| p.get("id").and_then(|i| i.as_i64()))
                    .collect()
            })
            .unwrap_or_default(),
        ice_servers: ice_from_conversation_params(value.get("conversationParams")),
    }
}

/// `transmitted-data` notification, either an SDP or a candidate.
pub fn parse_transmitted_data(value: &Value) -> Option<TransmittedData> {
    let data = value.get("data")?;
    if let Some(sdp) = data.get("sdp") {
        return Some(TransmittedData::Sdp {
            sdp_type: sdp.get("type")?.as_str()?.to_string(),
            sdp: sdp.get("sdp")?.as_str()?.to_string(),
        });
    }
    if let Some(c) = data.get("candidate") {
        return Some(TransmittedData::Candidate {
            candidate: c.get("candidate")?.as_str()?.to_string(),
            sdp_mid: c
                .get("sdpMid")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            sdp_mline_index: c.get("sdpMLineIndex").and_then(|v| v.as_i64()),
        });
    }
    None
}

fn topology_of(value: &Value) -> Option<String> {
    value
        .get("conversation")
        .and_then(|c| c.get("topology"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
}

fn ice_from_conversation_params(cp: Option<&Value>) -> Vec<IceServer> {
    let mut servers = Vec::new();
    let Some(cp) = cp else {
        return servers;
    };
    for kind in ["stun", "turn"] {
        let Some(entry) = cp.get(kind).and_then(|v| v.as_object()) else {
            continue;
        };
        let urls: Vec<String> = match entry.get("urls") {
            Some(Value::Array(a)) => a
                .iter()
                .filter_map(|u| u.as_str().map(String::from))
                .collect(),
            Some(Value::String(u)) => vec![u.clone()],
            _ => continue,
        };
        if urls.is_empty() {
            continue;
        }
        servers.push(IceServer {
            urls,
            username: entry
                .get("username")
                .and_then(|v| v.as_str())
                .map(String::from),
            credential: entry
                .get("credential")
                .and_then(|v| v.as_str())
                .map(String::from),
        });
    }
    servers
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_connection() {
        let n = json!({
            "type": "notification",
            "notification": "connection",
            "conversation": {
                "topology": "DIRECT",
                "participants": [{"id": 42}, {"id": 99}],
            },
            "conversationParams": {
                "stun": {"urls": ["stun:s:3478"]},
                "turn": {"urls": ["turn:t:3478"], "username": "u", "credential": "p"},
            },
        });
        let info = parse_connection(&n);
        assert_eq!(info.topology.as_deref(), Some("DIRECT"));
        assert_eq!(info.participants, vec![42, 99]);
        assert!(!info.is_sfu());
        assert_eq!(info.peer_of(42), Some(99));
        assert_eq!(info.ice_servers.len(), 2);
        assert_eq!(info.ice_servers[1].username.as_deref(), Some("u"));
    }

    #[test]
    fn parses_transmitted_sdp_and_candidate() {
        let offer = json!({"notification": "transmitted-data",
            "data": {"sdp": {"type": "offer", "sdp": "v=0..."}}});
        match parse_transmitted_data(&offer) {
            Some(TransmittedData::Sdp { sdp_type, sdp }) => {
                assert_eq!(sdp_type, "offer");
                assert_eq!(sdp, "v=0...");
            }
            other => panic!("expected sdp, got {other:?}"),
        }

        let cand = json!({"notification": "transmitted-data",
            "data": {"candidate": {"candidate": "candidate:1 ...", "sdpMid": "0", "sdpMLineIndex": 0}}});
        match parse_transmitted_data(&cand) {
            Some(TransmittedData::Candidate {
                candidate,
                sdp_mid,
                sdp_mline_index,
            }) => {
                assert!(candidate.starts_with("candidate:"));
                assert_eq!(sdp_mid.as_deref(), Some("0"));
                assert_eq!(sdp_mline_index, Some(0));
            }
            other => panic!("expected candidate, got {other:?}"),
        }
    }

    #[test]
    fn categorises_events() {
        assert!(matches!(
            CallEvent::parse(&json!({"notification": "hungup"})),
            CallEvent::Hungup
        ));
        assert!(matches!(
            CallEvent::parse(&json!({"type": "error", "error": "boom"})),
            CallEvent::Error(_)
        ));
    }
}
