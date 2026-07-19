//! End-to-end transport tests against a real self-signed TLS server, exercising
//! the TLS handshake, stream framing, seq-matched request/response, server
//! pushes, and error mapping.

use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use kolibri_net::protocol::{codec, framing::PacketReceiver, opcodes, packet::cmd};
use kolibri_net::{Client, ClientConfig, Direction, TransportError, WireTap};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;

/// Custom opcode the mock server answers with an error packet.
const OP_MAKE_ERROR: u16 = 999;

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

fn msgpack_map(pairs: &[(&str, &str)]) -> Vec<u8> {
    let value = rmpv::Value::Map(
        pairs
            .iter()
            .map(|(k, v)| (rmpv::Value::from(*k), rmpv::Value::from(*v)))
            .collect(),
    );
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, &value).unwrap();
    out
}

/// Handle exactly one client connection: echo each request as an OK response;
/// answer OP_MAKE_ERROR with an error packet; after a PING, also emit a push.
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

            if req.opcode == OP_MAKE_ERROR {
                let payload = msgpack_map(&[("message", "BOOM"), ("error", "E_BOOM")]);
                let resp = codec::encode_with_cmd(cmd::ERROR, req.opcode, &payload, req.seq);
                tls.write_all(&resp).await.unwrap();
            } else {
                let resp = codec::encode_with_cmd(cmd::OK, req.opcode, &req.payload, req.seq);
                tls.write_all(&resp).await.unwrap();
                if req.opcode == opcodes::PING {
                    let push_payload = msgpack_map(&[("event", "hello")]);
                    let push =
                        codec::encode_with_cmd(cmd::PUSH, opcodes::NOTIF_MESSAGE, &push_payload, 0);
                    tls.write_all(&push).await.unwrap();
                }
            }
            tls.flush().await.unwrap();
        }
    }
}

async fn start() -> Client {
    let acceptor = TlsAcceptor::from(server_config());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run_server(listener, acceptor));

    Client::connect(ClientConfig::new("127.0.0.1", addr.port()).insecure(true))
        .await
        .unwrap()
}

#[tokio::test]
async fn request_response_roundtrip() {
    let client = start().await;
    let payload = msgpack_map(&[("a", "b")]);
    let resp = client.request(opcodes::MSG_SEND, &payload).await.unwrap();

    assert!(resp.is_ok());
    assert_eq!(resp.opcode, opcodes::MSG_SEND);
    assert_eq!(
        resp.seq, 1,
        "first request must use seq 1 like the Dart sender"
    );
    assert_eq!(resp.payload, payload);
}

#[tokio::test]
async fn large_payload_compresses_and_roundtrips() {
    let client = start().await;
    // A repetitive >32 byte value triggers LZ4-frame compression on both sides.
    let big: Vec<u8> = {
        let value = rmpv::Value::from("x".repeat(500));
        let mut out = Vec::new();
        rmpv::encode::write_value(&mut out, &value).unwrap();
        out
    };
    let resp = client.request(opcodes::CHAT_HISTORY, &big).await.unwrap();
    assert_eq!(resp.payload, big);
}

#[tokio::test]
async fn server_error_maps_to_err() {
    let client = start().await;
    let err = client
        .request(OP_MAKE_ERROR, &msgpack_map(&[("q", "w")]))
        .await
        .unwrap_err();

    match err {
        TransportError::Server { message, error_key } => {
            assert_eq!(message, "BOOM");
            assert_eq!(error_key.as_deref(), Some("E_BOOM"));
        }
        other => panic!("expected Server error, got {other:?}"),
    }
}

#[tokio::test]
async fn receives_server_push() {
    let client = start().await;
    let mut pushes = client.subscribe();

    client
        .request(opcodes::PING, &msgpack_map(&[("x", "y")]))
        .await
        .unwrap();

    let push = timeout(Duration::from_secs(2), pushes.recv())
        .await
        .expect("push did not arrive")
        .expect("push channel closed");
    assert!(push.is_push());
    assert_eq!(push.opcode, opcodes::NOTIF_MESSAGE);
}

#[tokio::test]
async fn wire_tap_sees_both_directions() {
    let acceptor = TlsAcceptor::from(server_config());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(run_server(listener, acceptor));

    let seen: Arc<Mutex<Vec<(Direction, u16)>>> = Arc::new(Mutex::new(Vec::new()));
    let recorder = seen.clone();
    let tap: WireTap = Arc::new(move |dir, _cmd, opcode, _seq, _payload| {
        recorder.lock().unwrap().push((dir, opcode));
    });

    let client = Client::connect_with_tap(
        ClientConfig::new("127.0.0.1", addr.port()).insecure(true),
        Some(tap),
    )
    .await
    .unwrap();

    client
        .request(opcodes::MSG_SEND, &msgpack_map(&[("a", "b")]))
        .await
        .unwrap();

    let events = seen.lock().unwrap().clone();
    assert!(
        events.contains(&(Direction::Out, opcodes::MSG_SEND)),
        "outgoing not tapped: {events:?}"
    );
    assert!(
        events.contains(&(Direction::In, opcodes::MSG_SEND)),
        "incoming not tapped: {events:?}"
    );
}

#[tokio::test]
async fn sequence_numbers_increment() {
    let client = start().await;
    let p = msgpack_map(&[("k", "v")]);
    let first = client.request(opcodes::MSG_SEND, &p).await.unwrap();
    let second = client.request(opcodes::MSG_SEND, &p).await.unwrap();
    assert_eq!(first.seq, 1);
    assert_eq!(second.seq, 2);
}
