use base64::Engine;

use crate::protocol::compress::decompress_lz4_block;

/// ICE server in the shape a WebRTC stack expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

/// Call connection params (`vcp`), sent in the incoming-call push (opcode 137)
/// and the outgoing-call response.
///
/// wire format: `<rawLen>:<base64(LZ4-block(JSON))>`, JSON keys are short.
#[derive(Debug, Clone)]
pub struct ConversationParams {
    pub token: String,
    pub ws_endpoint: String,
    pub ws_ips: Vec<String>,
    pub wt_endpoint: Option<String>,
    pub calls_api_endpoint: Option<String>,
    pub client_type: Option<String>,
    pub expires_at: Option<i64>,
    pub stun: Option<String>,
    pub turn: Vec<String>,
    pub turn_user: Option<String>,
    pub turn_password: Option<String>,
    pub is_video: bool,
}

impl ConversationParams {
    pub fn decode(vcp: &str) -> Option<Self> {
        let sep = vcp.find(':')?;
        if sep == 0 {
            return None;
        }
        let raw_len: usize = vcp[..sep].parse().ok()?;
        if raw_len == 0 {
            return None;
        }

        let compressed = base64::engine::general_purpose::STANDARD
            .decode(&vcp[sep + 1..])
            .ok()?;
        let decompressed = decompress_lz4_block(&compressed, raw_len).ok()?;
        let bytes = if decompressed.len() > raw_len {
            &decompressed[..raw_len]
        } else {
            &decompressed[..]
        };

        let json: serde_json::Value = serde_json::from_slice(bytes).ok()?;
        let obj = json.as_object()?;

        Some(ConversationParams {
            token: obj.get("tkn")?.as_str()?.to_string(),
            ws_endpoint: obj.get("wse")?.as_str()?.to_string(),
            ws_ips: string_list(obj.get("wsip")),
            wt_endpoint: str_field(obj.get("wte")),
            calls_api_endpoint: str_field(obj.get("vcae")),
            client_type: str_field(obj.get("srcp")),
            expires_at: obj.get("et").and_then(|v| v.as_i64()),
            stun: str_field(obj.get("stne")),
            turn: split_csv(obj.get("trne")),
            turn_user: str_field(obj.get("trnu")),
            turn_password: str_field(obj.get("trnp")),
            is_video: obj.get("iv").and_then(|v| v.as_bool()).unwrap_or(false),
        })
    }

    pub fn ice_servers(&self) -> Vec<IceServer> {
        let mut servers = Vec::new();
        if let Some(stun) = self.stun.as_ref().filter(|s| !s.is_empty()) {
            servers.push(IceServer {
                urls: vec![stun.clone()],
                username: None,
                credential: None,
            });
        }
        if !self.turn.is_empty() {
            servers.push(IceServer {
                urls: self.turn.clone(),
                username: self.turn_user.clone(),
                credential: self.turn_password.clone(),
            });
        }
        servers
    }

    /// treats "expires within 5s" as expired. `now_secs` is unix time.
    pub fn is_expired(&self, now_secs: i64) -> bool {
        match self.expires_at {
            Some(exp) => now_secs >= exp - 5,
            None => false,
        }
    }

    /// calls user id, the part after the last `:` in the TURN username.
    pub fn user_id(&self) -> i64 {
        self.turn_user
            .as_deref()
            .and_then(|u| u.rsplit(':').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    }

    /// ws2 connect URL for an incoming call.
    pub fn ws2_url(&self, conversation_id: &str, client: &Ws2ClientInfo) -> String {
        let params = [
            ("userId", self.user_id().to_string()),
            ("entityType", "USER".to_string()),
            ("conversationId", conversation_id.to_string()),
            ("token", self.token.clone()),
            ("version", "5".to_string()),
            ("capabilities", client.capabilities.clone()),
            ("device", client.device.clone()),
            ("platform", client.platform.clone()),
            ("clientType", client.client_type.clone()),
            ("appVersion", client.app_version.clone()),
            ("osVersion", client.os_version.clone()),
        ];
        set_query(&self.ws_endpoint, &params)
    }
}

/// ws2 params that don't come from the server.
#[derive(Debug, Clone)]
pub struct Ws2ClientInfo {
    pub capabilities: String,
    pub device: String,
    pub platform: String,
    pub client_type: String,
    pub app_version: String,
    pub os_version: String,
}

impl Default for Ws2ClientInfo {
    fn default() -> Self {
        Self {
            capabilities: "3c03f".to_string(),
            device: "Kolibri".to_string(),
            platform: "ANDROID".to_string(),
            client_type: "ONE_ME".to_string(),
            app_version: "sdk-0.1.16.4".to_string(),
            os_version: "36".to_string(),
        }
    }
}

/// append client params to an outgoing-call `endpoint` (already carries token
/// and conversation/user ids in its query), overriding on key clash.
pub fn ws2_url_from_endpoint(endpoint: &str, client: &Ws2ClientInfo) -> String {
    let extra = [
        ("platform", client.platform.clone()),
        ("version", "5".to_string()),
        ("capabilities", client.capabilities.clone()),
        ("clientType", client.client_type.clone()),
        ("appVersion", client.app_version.clone()),
        ("device", client.device.clone()),
        ("tgt", "start".to_string()),
    ];
    merge_query(endpoint, &extra)
}

fn str_field(v: Option<&serde_json::Value>) -> Option<String> {
    v.and_then(|v| v.as_str()).map(|s| s.to_string())
}

fn string_list(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| e.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

fn split_csv(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|v| v.as_str())
        .map(|s| {
            s.split(',')
                .map(|e| e.trim())
                .filter(|e| !e.is_empty())
                .map(|e| e.to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// replace the whole query of `base` with `params`.
fn set_query(base: &str, params: &[(&str, String)]) -> String {
    let path = base.split('?').next().unwrap_or(base);
    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", k, encode_query(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{path}?{query}")
}

/// merge `extra` into the existing query of `base`, extra wins on clash.
fn merge_query(base: &str, extra: &[(&str, String)]) -> String {
    let (path, existing) = base.split_once('?').unwrap_or((base, ""));
    let mut pairs: Vec<(String, String)> = Vec::new();
    for pair in existing.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        pairs.push((k.to_string(), v.to_string()));
    }
    for (k, v) in extra {
        let encoded = encode_query(v);
        if let Some(slot) = pairs.iter_mut().find(|(ek, _)| ek == k) {
            slot.1 = encoded;
        } else {
            pairs.push((k.to_string(), encoded));
        }
    }
    let query = pairs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&");
    format!("{path}?{query}")
}

fn encode_query(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
