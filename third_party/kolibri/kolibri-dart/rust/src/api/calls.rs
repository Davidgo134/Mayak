use std::sync::Arc;

use flutter_rust_bridge::frb;
use tokio::runtime::Runtime;

use crate::frb_generated::StreamSink;

pub struct IceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

/// decoded vcp call params, plus the ready ws2 url for the given conversation
pub struct CallParams {
    pub token: String,
    pub ws_endpoint: String,
    pub stun: Option<String>,
    pub turn: Vec<String>,
    pub turn_user: Option<String>,
    pub turn_password: Option<String>,
    pub is_video: bool,
    pub user_id: i64,
    pub ice_servers: Vec<IceServer>,
    pub ws2_url: String,
}

#[frb(sync)]
pub fn decode_vcp(vcp: String, conversation_id: String) -> Option<CallParams> {
    let p = kolibri_net::calls::ConversationParams::decode(&vcp)?;
    let ws2_url = p.ws2_url(
        &conversation_id,
        &kolibri_net::calls::Ws2ClientInfo::default(),
    );
    Some(CallParams {
        token: p.token.clone(),
        ws_endpoint: p.ws_endpoint.clone(),
        stun: p.stun.clone(),
        turn: p.turn.clone(),
        turn_user: p.turn_user.clone(),
        turn_password: p.turn_password.clone(),
        is_video: p.is_video,
        user_id: p.user_id(),
        ice_servers: p
            .ice_servers()
            .into_iter()
            .map(|s| IceServer {
                urls: s.urls,
                username: s.username,
                credential: s.credential,
            })
            .collect(),
        ws2_url,
    })
}

/// ws2 call-signaling client. command results and notifications cross as JSON
/// strings, decode with dart:convert.
pub struct CallSignaling {
    rt: Arc<Runtime>,
    inner: Arc<kolibri_net::calls::Ws2Signaling>,
}

pub fn connect_call_signaling(
    url: String,
    user_agent: Option<String>,
    proxy: Option<String>,
) -> Result<CallSignaling, String> {
    let rt = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(|e| e.to_string())?,
    );
    let proxy = match proxy {
        Some(url) => Some(kolibri_net::ProxyConfig::parse(&url)?),
        None => None,
    };
    let inner = rt
        .block_on(kolibri_net::calls::Ws2Signaling::connect_via(
            &url,
            user_agent.as_deref(),
            proxy.as_ref(),
        ))
        .map_err(|e| e.to_string())?;
    Ok(CallSignaling {
        rt,
        inner: Arc::new(inner),
    })
}

impl CallSignaling {
    pub fn accept_call(&self) -> Result<String, String> {
        self.block(self.inner.accept_call())
    }

    pub fn hangup(&self, reason: String) -> Result<String, String> {
        self.block(self.inner.hangup(&reason))
    }

    pub fn transmit_sdp(
        &self,
        participant_id: i64,
        sdp_type: String,
        sdp: String,
    ) -> Result<String, String> {
        self.block(self.inner.transmit_sdp(participant_id, &sdp_type, &sdp))
    }

    pub fn transmit_candidate(
        &self,
        participant_id: i64,
        candidate: String,
        sdp_mid: String,
        sdp_mline_index: i64,
    ) -> Result<String, String> {
        self.block(self.inner.transmit_candidate(
            participant_id,
            &candidate,
            &sdp_mid,
            sdp_mline_index,
        ))
    }

    pub fn change_media_settings(
        &self,
        audio: bool,
        video: bool,
        screen: bool,
    ) -> Result<String, String> {
        self.block(self.inner.change_media_settings(audio, video, screen))
    }

    /// raw command; extra_json is a JSON object string
    pub fn send_command(&self, command: String, extra_json: String) -> Result<String, String> {
        let extra: serde_json::Value =
            serde_json::from_str(&extra_json).map_err(|e| e.to_string())?;
        self.block(self.inner.send_command(&command, extra))
    }

    /// ws2 notifications as JSON strings
    pub fn notifications(&self, sink: StreamSink<String>) {
        let mut rx = self.inner.notifications();
        self.rt.spawn(async move {
            while let Ok(value) = rx.recv().await {
                if sink.add(value.to_string()).is_err() {
                    break;
                }
            }
        });
    }

    #[frb(sync)]
    pub fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    #[frb(sync)]
    pub fn close(&self) {
        self.inner.close();
    }

    fn block(
        &self,
        fut: impl std::future::Future<Output = Result<serde_json::Value, kolibri_net::calls::CallError>>,
    ) -> Result<String, String> {
        self.rt
            .block_on(fut)
            .map(|v| v.to_string())
            .map_err(|e| e.to_string())
    }
}

/// parsed ws2 `connection` notification
pub struct ConnectionInfo {
    pub topology: Option<String>,
    pub is_sfu: bool,
    pub participants: Vec<i64>,
    pub peer: Option<i64>,
    pub ice_servers: Vec<IceServer>,
}

/// connection notification (JSON string). peer is the participant that isn't my_user_id.
#[flutter_rust_bridge::frb(sync)]
pub fn parse_connection(notification_json: String, my_user_id: i64) -> ConnectionInfo {
    let value: serde_json::Value =
        serde_json::from_str(&notification_json).unwrap_or(serde_json::Value::Null);
    let info = kolibri_net::calls::parse_connection(&value);
    ConnectionInfo {
        topology: info.topology.clone(),
        is_sfu: info.is_sfu(),
        participants: info.participants.clone(),
        peer: info.peer_of(my_user_id),
        ice_servers: info
            .ice_servers
            .into_iter()
            .map(|s| IceServer {
                urls: s.urls,
                username: s.username,
                credential: s.credential,
            })
            .collect(),
    }
}

/// SDP or ICE candidate from a `transmitted-data` notification. kind is "sdp" or
/// "candidate"; only the matching fields are set.
pub struct TransmittedData {
    pub kind: String,
    pub sdp_type: Option<String>,
    pub sdp: Option<String>,
    pub candidate: Option<String>,
    pub sdp_mid: Option<String>,
    pub sdp_mline_index: Option<i64>,
}

#[flutter_rust_bridge::frb(sync)]
pub fn parse_transmitted_data(notification_json: String) -> Option<TransmittedData> {
    let value: serde_json::Value = serde_json::from_str(&notification_json).ok()?;
    match kolibri_net::calls::parse_transmitted_data(&value)? {
        kolibri_net::calls::TransmittedData::Sdp { sdp_type, sdp } => Some(TransmittedData {
            kind: "sdp".to_string(),
            sdp_type: Some(sdp_type),
            sdp: Some(sdp),
            candidate: None,
            sdp_mid: None,
            sdp_mline_index: None,
        }),
        kolibri_net::calls::TransmittedData::Candidate {
            candidate,
            sdp_mid,
            sdp_mline_index,
        } => Some(TransmittedData {
            kind: "candidate".to_string(),
            sdp_type: None,
            sdp: None,
            candidate: Some(candidate),
            sdp_mid,
            sdp_mline_index,
        }),
    }
}
