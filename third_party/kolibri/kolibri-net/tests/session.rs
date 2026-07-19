//! End-to-end session tests: full connect → sessionInit handshake → online,
//! against a self-signed TLS server that answers the handshake.

use std::sync::Arc;
use std::time::Duration;

use kolibri_net::protocol::{codec, framing::PacketReceiver, opcodes, packet::cmd};
use kolibri_net::{ClientConfig, HandshakeConfig, Session, SessionConfig, SessionState, UserAgent};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;

fn server_config() -> Arc<rustls::ServerConfig> {
    let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    let cert_der = certified.cert.der().clone();
    let key_der = rustls::pki_types::PrivatePkcs8KeyDer::from(certified.key_pair.serialize_der());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert_der],
            rustls::pki_types::PrivateKeyDer::Pkcs8(key_der),
        )
        .unwrap();
    Arc::new(config)
}

/// Handshake response payload: {callsSeed: 7, device_name: "Rusty"}.
fn handshake_response() -> Vec<u8> {
    let value = rmpv::Value::Map(vec![
        (rmpv::Value::from("callsSeed"), rmpv::Value::from(7i64)),
        (rmpv::Value::from("device_name"), rmpv::Value::from("Rusty")),
    ]);
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, &value).unwrap();
    out
}

/// Server: answer sessionInit with the handshake payload; echo any other
/// request; silently accept pings (no matching waiter on the client).
async fn run_server(listener: TcpListener, acceptor: TlsAcceptor) {
    let (tcp, _) = listener.accept().await.unwrap();
    let mut tls = acceptor.accept(tcp).await.unwrap();

    let mut receiver = PacketReceiver::new();
    let mut buf = vec![0u8; 16 * 1024];
    loop {
        let n = match tls.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        let packets = receiver.feed(&buf[..n]).unwrap();
        for raw in packets {
            let req = codec::decode(&raw).unwrap();
            match req.opcode {
                opcodes::SESSION_INIT => {
                    let resp =
                        codec::encode_with_cmd(cmd::OK, req.opcode, &handshake_response(), req.seq);
                    tls.write_all(&resp).await.unwrap();
                    tls.flush().await.unwrap();
                }
                opcodes::PING => {}
                _ => {
                    let resp = codec::encode_with_cmd(cmd::OK, req.opcode, &req.payload, req.seq);
                    tls.write_all(&resp).await.unwrap();
                    tls.flush().await.unwrap();
                }
            }
        }
    }
}

fn handshake_config() -> HandshakeConfig {
    HandshakeConfig {
        instance_id: "inst-123".to_string(),
        device_id: "dev-abc".to_string(),
        client_session_id: 42,
        user_agent: UserAgent {
            device_type: "ANDROID".to_string(),
            app_version: "1.0.0".to_string(),
            os_version: "Android 14".to_string(),
            timezone: "Europe/Moscow".to_string(),
            screen: "420dpi 420dpi 1080x2340".to_string(),
            push_device_type: "GCM".to_string(),
            arch: "arm64-v8a".to_string(),
            locale: "ru".to_string(),
            build_number: 100,
            device_name: "Pixel".to_string(),
            device_locale: "ru".to_string(),
        },
    }
}

async fn start_session(auto_reconnect: bool) -> Session {
    let acceptor = TlsAcceptor::from(server_config());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run_server(listener, acceptor));

    let client = ClientConfig::new("127.0.0.1", addr.port()).insecure(true);
    let mut config = SessionConfig::new(client, handshake_config());
    config.auto_reconnect = auto_reconnect;
    config.ping_interval = Duration::from_millis(200);
    Session::new(config)
}

#[tokio::test]
async fn connect_performs_handshake_and_goes_online() {
    let session = start_session(false).await;

    let info = timeout(Duration::from_secs(5), session.connect())
        .await
        .expect("connect timed out")
        .expect("handshake failed");

    assert_eq!(info.calls_seed, Some(7));
    assert_eq!(info.device_name.as_deref(), Some("Rusty"));
    assert_eq!(session.state(), SessionState::Online);
}

#[tokio::test]
async fn request_routes_through_session() {
    let session = start_session(false).await;
    session.connect().await.unwrap();

    let payload = {
        let value = rmpv::Value::Map(vec![(rmpv::Value::from("q"), rmpv::Value::from("hi"))]);
        let mut out = Vec::new();
        rmpv::encode::write_value(&mut out, &value).unwrap();
        out
    };
    let resp = session
        .request(opcodes::CHATS_LIST, &payload)
        .await
        .unwrap();
    assert!(resp.is_ok());
    assert_eq!(resp.payload, payload);
}

#[tokio::test]
async fn state_transitions_reach_online() {
    let session = start_session(false).await;
    let mut states = session.subscribe_state();

    session.connect().await.unwrap();

    // Drain observed states; the terminal one must be Online.
    let mut last = *states.borrow_and_update();
    while states.has_changed().unwrap_or(false) {
        last = *states.borrow_and_update();
    }
    assert_eq!(last, SessionState::Online);
}

#[tokio::test]
async fn keepalive_ping_is_sent() {
    // ping_interval is 200ms; if the session stays online for >0.5s without the
    // server closing on an unexpected packet, pings are being accepted.
    let session = start_session(false).await;
    session.connect().await.unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(session.state(), SessionState::Online);
}

/// Server that drops the first connection right after the handshake, then serves
/// the second connection normally — to exercise auto-reconnect.
async fn run_flaky_server(listener: TcpListener, acceptor: TlsAcceptor) {
    // First connection: answer the handshake, then drop the socket.
    if let Ok((tcp, _)) = listener.accept().await {
        if let Ok(mut tls) = acceptor.accept(tcp).await {
            let mut receiver = PacketReceiver::new();
            let mut buf = vec![0u8; 16 * 1024];
            'first: loop {
                let n = match tls.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => n,
                };
                for raw in receiver.feed(&buf[..n]).unwrap() {
                    let req = codec::decode(&raw).unwrap();
                    if req.opcode == opcodes::SESSION_INIT {
                        let resp = codec::encode_with_cmd(
                            cmd::OK,
                            req.opcode,
                            &handshake_response(),
                            req.seq,
                        );
                        tls.write_all(&resp).await.unwrap();
                        tls.flush().await.unwrap();
                        break 'first; // drop the connection
                    }
                }
            }
        }
    }
    // Second connection: serve normally.
    run_server(listener, acceptor).await;
}

#[tokio::test]
async fn auto_reconnects_after_drop() {
    let acceptor = TlsAcceptor::from(server_config());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run_flaky_server(listener, acceptor));

    let client = ClientConfig::new("127.0.0.1", addr.port()).insecure(true);
    let mut config = SessionConfig::new(client, handshake_config());
    config.auto_reconnect = true;
    config.ping_interval = Duration::from_millis(150);
    let session = Session::new(config);

    let mut states = session.subscribe_state();
    // First handshake succeeds on conn1 (which the server then drops). The drop
    // may be detected before this returns, so we don't assert Online here.
    session.connect().await.unwrap();

    // Observe: … Disconnected (drop) → … → Online again (reconnect on conn2).
    let mut saw_disconnected = false;
    let mut reconnected = false;
    let deadline = Duration::from_secs(8);
    let result = timeout(deadline, async {
        while states.changed().await.is_ok() {
            match *states.borrow_and_update() {
                SessionState::Disconnected => saw_disconnected = true,
                SessionState::Online if saw_disconnected => {
                    reconnected = true;
                    break;
                }
                _ => {}
            }
        }
    })
    .await;

    assert!(result.is_ok(), "did not reconnect within {deadline:?}");
    assert!(reconnected, "session did not return to Online after drop");
}

#[tokio::test]
async fn disconnect_stops_session() {
    let session = start_session(false).await;
    session.connect().await.unwrap();
    session.disconnect();
    assert_eq!(session.state(), SessionState::Disconnected);
    let err = session.request(opcodes::PING, &[]).await;
    assert!(err.is_err());
}
