use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine;
use futures_util::{SinkExt, StreamExt};
use kolibri_net::calls::{ConversationParams, Ws2ClientInfo, Ws2Signaling};
use serde_json::json;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

fn make_vcp() -> String {
    let payload = json!({
        "tkn": "TOK123",
        "wse": "wss://vid.example/ws2",
        "stne": "stun:s.example:3478",
        "trne": "turn:t1.example:3478, turn:t2.example:3478",
        "trnu": "user:42",
        "trnp": "secret",
        "iv": true,
        "et": 9999999999i64,
    });
    let bytes = serde_json::to_vec(&payload).unwrap();
    let raw_len = bytes.len();
    let compressed = lz4_flex::block::compress(&bytes);
    let b64 = base64::engine::general_purpose::STANDARD.encode(&compressed);
    format!("{raw_len}:{b64}")
}

#[test]
fn vcp_decodes_all_fields() {
    let params = ConversationParams::decode(&make_vcp()).expect("decode");
    assert_eq!(params.token, "TOK123");
    assert_eq!(params.ws_endpoint, "wss://vid.example/ws2");
    assert_eq!(params.stun.as_deref(), Some("stun:s.example:3478"));
    assert_eq!(params.turn.len(), 2);
    assert_eq!(params.turn[1], "turn:t2.example:3478");
    assert_eq!(params.turn_user.as_deref(), Some("user:42"));
    assert_eq!(params.turn_password.as_deref(), Some("secret"));
    assert!(params.is_video);
    assert_eq!(params.user_id(), 42);

    let ice = params.ice_servers();
    assert_eq!(ice.len(), 2);
    assert_eq!(ice[1].username.as_deref(), Some("user:42"));
}

#[test]
fn vcp_builds_ws2_url() {
    let params = ConversationParams::decode(&make_vcp()).unwrap();
    let url = params.ws2_url("conv-1", &Ws2ClientInfo::default());
    assert!(url.starts_with("wss://vid.example/ws2?"));
    assert!(url.contains("userId=42"));
    assert!(url.contains("conversationId=conv-1"));
    assert!(url.contains("token=TOK123"));
    assert!(url.contains("clientType=ONE_ME"));
}

#[test]
fn vcp_rejects_garbage() {
    assert!(ConversationParams::decode("not-a-vcp").is_none());
    assert!(ConversationParams::decode(":abc").is_none());
    assert!(ConversationParams::decode("0:abc").is_none());
}

/// Mock ws2 server: sends an app-level `ping` on connect, echoes commands as
/// responses, and pushes a `connection` notification after `accept-call`.
async fn mock_ws2(listener: TcpListener, got_pong: Arc<AtomicBool>) {
    let (stream, _) = listener.accept().await.unwrap();
    let ws = accept_async(stream).await.unwrap();
    let (mut write, mut read) = ws.split();

    write.send(Message::text("ping")).await.unwrap();

    while let Some(msg) = read.next().await {
        let Ok(Message::Text(t)) = msg else { break };
        let text = t.as_str();
        if text == "pong" {
            got_pong.store(true, Ordering::SeqCst);
            continue;
        }
        let v: serde_json::Value = serde_json::from_str(text).unwrap();
        let seq = v["sequence"].as_i64().unwrap();
        let command = v["command"].as_str().unwrap().to_string();
        let resp = json!({"sequence": seq, "response": command, "type": "response"});
        write.send(Message::text(resp.to_string())).await.unwrap();

        if command == "accept-call" {
            let notif =
                json!({"type": "notification", "notification": "connection", "topology": "P2P"});
            write.send(Message::text(notif.to_string())).await.unwrap();
        }
    }
}

#[tokio::test]
async fn ws2_command_response_notification_and_ping() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let got_pong = Arc::new(AtomicBool::new(false));
    tokio::spawn(mock_ws2(listener, got_pong.clone()));

    let url = format!("ws://127.0.0.1:{}/ws2", addr.port());
    let sig = Ws2Signaling::connect(&url, None).await.unwrap();

    let mut notifs = sig.notifications();

    let resp = sig.accept_call().await.unwrap();
    assert_eq!(resp["response"], "accept-call");
    assert_eq!(resp["sequence"], 1);

    let notif = timeout(Duration::from_secs(2), notifs.recv())
        .await
        .expect("notification timed out")
        .expect("notif channel closed");
    assert_eq!(notif["notification"], "connection");
    assert_eq!(notif["topology"], "P2P");

    // the client should have answered the server's app-level ping
    for _ in 0..20 {
        if got_pong.load(Ordering::SeqCst) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(got_pong.load(Ordering::SeqCst), "client did not reply pong");
}

#[tokio::test]
async fn ws2_transmit_sdp_shape() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    // capture the command the server receives
    let captured: Arc<tokio::sync::Mutex<Option<serde_json::Value>>> =
        Arc::new(tokio::sync::Mutex::new(None));
    let captured2 = captured.clone();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let ws = accept_async(stream).await.unwrap();
        let (mut write, mut read) = ws.split();
        while let Some(msg) = read.next().await {
            let Ok(Message::Text(t)) = msg else { break };
            let v: serde_json::Value = serde_json::from_str(t.as_str()).unwrap();
            *captured2.lock().await = Some(v.clone());
            let resp =
                json!({"sequence": v["sequence"], "response": "transmit-data", "type": "response"});
            write.send(Message::text(resp.to_string())).await.unwrap();
        }
    });

    let url = format!("ws://127.0.0.1:{}/ws2", addr.port());
    let sig = Ws2Signaling::connect(&url, None).await.unwrap();
    sig.transmit_sdp(42, "offer", "v=0...").await.unwrap();

    let cmd = captured.lock().await.clone().expect("no command captured");
    assert_eq!(cmd["command"], "transmit-data");
    assert_eq!(cmd["participantId"], 42);
    assert_eq!(cmd["data"]["sdp"]["type"], "offer");
    assert_eq!(cmd["data"]["sdp"]["sdp"], "v=0...");
}
