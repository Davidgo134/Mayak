use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{broadcast, mpsc, watch};
use tokio::task::JoinHandle;
use tokio::time::timeout;

use super::dispatcher::Dispatcher;
use super::error::TransportError;
use super::proxy::{connect_tcp, ProxyConfig};
use super::tls::build_connector;
use super::wiretap::{Direction, WireTap};
use crate::protocol::codec;
use crate::protocol::framing::PacketReceiver;
use crate::protocol::packet::{cmd, Packet};

const READ_CHUNK: usize = 64 * 1024;
const PUSH_CHANNEL_CAPACITY: usize = 256;

/// `host` doubles as the TLS server name (SNI).
#[derive(Debug, Clone)]
pub struct ClientConfig {
    pub host: String,
    pub port: u16,
    /// accept any TLS cert, debug only
    pub insecure_tls: bool,
    pub connect_timeout: Duration,
    pub request_timeout: Duration,
    /// route the connection through a proxy (HTTP CONNECT or SOCKS5)
    pub proxy: Option<ProxyConfig>,
}

impl ClientConfig {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
            insecure_tls: false,
            connect_timeout: Duration::from_secs(15),
            request_timeout: Duration::from_secs(30),
            proxy: None,
        }
    }

    pub fn insecure(mut self, insecure: bool) -> Self {
        self.insecure_tls = insecure;
        self
    }

    pub fn proxy(mut self, proxy: Option<ProxyConfig>) -> Self {
        self.proxy = proxy;
        self
    }
}

/// Async client over the persistent TLS connection: [`Client::request`] awaits
/// the matching response, [`Client::subscribe`] observes server pushes.
pub struct Client {
    seq: AtomicU16,
    write_tx: mpsc::UnboundedSender<Vec<u8>>,
    dispatcher: Arc<Dispatcher>,
    request_timeout: Duration,
    connected_tx: watch::Sender<bool>,
    tap: Option<WireTap>,
    tasks: Vec<JoinHandle<()>>,
}

impl Client {
    pub async fn connect(config: ClientConfig) -> Result<Self, TransportError> {
        Self::connect_with_tap(config, None).await
    }

    /// like [`Client::connect`], but hands every packet, both ways, to `tap`.
    pub async fn connect_with_tap(
        config: ClientConfig,
        tap: Option<WireTap>,
    ) -> Result<Self, TransportError> {
        let connector = build_connector(config.insecure_tls)?;

        let tcp = connect_tcp(
            &config.host,
            config.port,
            config.connect_timeout,
            config.proxy.as_ref(),
        )
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::TimedOut => TransportError::ConnectTimeout,
            _ => TransportError::Io(e),
        })?;

        let server_name = rustls::pki_types::ServerName::try_from(config.host.clone())
            .map_err(|e| TransportError::Config(format!("invalid server name: {e}")))?;

        let tls = timeout(config.connect_timeout, connector.connect(server_name, tcp))
            .await
            .map_err(|_| TransportError::ConnectTimeout)?
            .map_err(|e| TransportError::Tls(e.to_string()))?;

        let (mut read_half, mut write_half) = tokio::io::split(tls);

        let dispatcher = Arc::new(Dispatcher::new(PUSH_CHANNEL_CAPACITY));
        let (connected_tx, _) = watch::channel(true);

        let (write_tx, mut write_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        let writer = tokio::spawn(async move {
            while let Some(bytes) = write_rx.recv().await {
                if write_half.write_all(&bytes).await.is_err() {
                    break;
                }
                if write_half.flush().await.is_err() {
                    break;
                }
            }
        });

        let reader_dispatcher = dispatcher.clone();
        let reader_connected = connected_tx.clone();
        let reader_tap = tap.clone();
        let reader = tokio::spawn(async move {
            let mut receiver = PacketReceiver::new();
            let mut buf = vec![0u8; READ_CHUNK];
            loop {
                match read_half.read(&mut buf).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let packets = match receiver.feed(&buf[..n]) {
                            Ok(p) => p,
                            Err(_) => break,
                        };
                        for raw in packets {
                            match codec::decode(&raw) {
                                Ok(packet) => {
                                    if let Some(t) = &reader_tap {
                                        t(
                                            Direction::In,
                                            packet.cmd,
                                            packet.opcode,
                                            packet.seq,
                                            &packet.payload,
                                        );
                                    }
                                    reader_dispatcher.dispatch(packet);
                                }
                                Err(_) => continue,
                            }
                        }
                    }
                }
            }
            reader_connected.send_replace(false);
            reader_dispatcher.fail_all();
        });

        Ok(Self {
            seq: AtomicU16::new(0),
            write_tx,
            dispatcher,
            request_timeout: config.request_timeout,
            connected_tx,
            tap,
            tasks: vec![writer, reader],
        })
    }

    /// pre-increment wrapping at 2^16, so the first request is seq 1
    fn next_seq(&self) -> u16 {
        self.seq.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }

    pub fn is_connected(&self) -> bool {
        *self.connected_tx.borrow()
    }

    /// flips to `false` when the connection drops; drives supervisor reconnect
    pub fn subscribe_connected(&self) -> watch::Receiver<bool> {
        self.connected_tx.subscribe()
    }

    /// `payload` is already-serialized msgpack. not-found comes back as `Ok`
    /// (see [`Packet::is_not_found`]); only an error packet is `Err`.
    pub async fn request(&self, opcode: u16, payload: &[u8]) -> Result<Packet, TransportError> {
        let packet = self.request_raw(opcode, payload).await?;
        if packet.is_error() {
            Err(super::dispatcher::error_from_payload(&packet))
        } else {
            Ok(packet)
        }
    }

    /// Like [`Client::request`], but returns the raw response packet for any
    /// command — an error packet comes back as `Ok` (with its payload) rather
    /// than mapped to `Err`. Only a lost connection or timeout is `Err`.
    pub async fn request_raw(&self, opcode: u16, payload: &[u8]) -> Result<Packet, TransportError> {
        if !self.is_connected() {
            return Err(TransportError::ConnectionClosed);
        }

        let seq = self.next_seq();
        if let Some(t) = &self.tap {
            t(Direction::Out, cmd::REQUEST, opcode, seq, payload);
        }
        let bytes = codec::encode(opcode, payload, seq);
        let rx = self.dispatcher.register(seq);

        self.write_tx
            .send(bytes)
            .map_err(|_| TransportError::ConnectionClosed)?;

        match timeout(self.request_timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(TransportError::ConnectionClosed),
            Err(_) => Err(TransportError::Timeout),
        }
    }

    /// Fire-and-forget, no response tracking (typing indicators, pings).
    /// Returns the assigned seq.
    pub fn send(&self, opcode: u16, payload: &[u8]) -> Result<u16, TransportError> {
        if !self.is_connected() {
            return Err(TransportError::ConnectionClosed);
        }
        let seq = self.next_seq();
        if let Some(t) = &self.tap {
            t(Direction::Out, cmd::REQUEST, opcode, seq, payload);
        }
        let bytes = codec::encode(opcode, payload, seq);
        self.write_tx
            .send(bytes)
            .map_err(|_| TransportError::ConnectionClosed)?;
        Ok(seq)
    }

    /// Each subscriber gets every push sent after it subscribes.
    pub fn subscribe(&self) -> broadcast::Receiver<Packet> {
        self.dispatcher.subscribe()
    }

    pub fn close(&self) {
        self.connected_tx.send_replace(false);
        self.dispatcher.fail_all();
        for task in &self.tasks {
            task.abort();
        }
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.close();
    }
}
