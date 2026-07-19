use std::sync::Arc;
use std::time::Duration;

use crate::frb_generated::StreamSink;
use flutter_rust_bridge::frb;
use kolibri_net::{
    ClientConfig, Direction, HandshakeConfig, Packet, ProxyConfig, Session, SessionConfig,
    SessionState, UserAgent, WireTap,
};
use tokio::runtime::Runtime;

/// device + connection options. device fields feed the sessionInit handshake.
pub struct SessionOptions {
    pub host: String,
    pub port: u16,
    pub device_id: String,
    pub instance_id: String,
    pub app_version: String,
    pub build_number: i64,
    pub device_type: String,
    pub os_version: String,
    pub timezone: String,
    pub screen: String,
    pub push_device_type: String,
    pub arch: String,
    pub locale: String,
    pub device_name: String,
    pub device_locale: String,
    pub client_session_id: i64,
    pub ping_interval_secs: u64,
    pub ping_interactive: bool,
    pub auto_reconnect: bool,
    pub insecure_tls: bool,
    /// proxy url `scheme://[user:pass@]host:port` (http/socks5/socks5h), or none
    pub proxy: Option<String>,
}

/// sessionInit handshake result. `payload` is raw msgpack, `payload_json` the
/// same rendered as JSON (for decoding without a msgpack package).
pub struct HandshakeInfo {
    pub calls_seed: Option<i64>,
    pub device_name: Option<String>,
    pub payload: Vec<u8>,
    pub payload_json: String,
}

/// server push; `payload` is raw msgpack, `payload_json` the same as JSON.
pub struct PushEvent {
    pub opcode: u16,
    pub payload: Vec<u8>,
    pub payload_json: String,
}

/// full request result: `cmd` is the packet command (1=ok, 2=not_found,
/// 3=error), `payload_json` the tagged JSON, `error_*` a server error. A server
/// error is reported here, not thrown.
pub struct RequestOutcome {
    pub cmd: u8,
    pub opcode: u16,
    pub payload_json: String,
    pub error_message: Option<String>,
    pub error_key: Option<String>,
}

/// one tapped packet for logs. `direction` "out"/"in", `cmd`
/// "request"/"ok"/"not_found"/"error"/"push", `json` the payload (lossy: binary
/// -> base64).
pub struct WireLogEvent {
    pub direction: String,
    pub cmd: String,
    pub opcode: u16,
    pub seq: u16,
    pub json: String,
}

fn wire_json(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "null".to_string();
    }
    match rmpv::decode::read_value(&mut &payload[..]) {
        Ok(v) => kolibri_net::protocol::value_to_json(&v).to_string(),
        Err(_) => "null".to_string(),
    }
}

fn cmd_label(dir: Direction, cmd: u8) -> String {
    match cmd {
        kolibri_net::cmd::OK => "ok",
        kolibri_net::cmd::NOT_FOUND => "not_found",
        kolibri_net::cmd::ERROR => "error",
        _ => match dir {
            Direction::Out => "request",
            Direction::In => "push",
        },
    }
    .to_string()
}

/// (message, error_key) pulled from an error packet's payload, for convenience;
/// the host still gets the full body in `payload_json`.
fn error_fields(packet: &Packet) -> (Option<String>, Option<String>) {
    let Ok(value) = packet.value() else {
        return (None, None);
    };
    let message = ["localizedMessage", "message", "title"]
        .iter()
        .find_map(|k| map_str(&value, k));
    (message, map_str(&value, "error"))
}

fn map_str(value: &rmpv::Value, key: &str) -> Option<String> {
    value
        .as_map()?
        .iter()
        .find(|(k, _)| k.as_str() == Some(key))
        .and_then(|(_, v)| v.as_str().map(|s| s.to_string()))
}

/// progress updates, then a terminal Done (status + body) or Error
pub enum UploadEvent {
    Progress { sent: u64, total: u64 },
    Done { status: u16, body: Vec<u8> },
    Error { message: String },
}

/// run an upload, forwarding progress + terminal result to sink
async fn drive_upload<F, Fut>(sink: StreamSink<UploadEvent>, upload: F)
where
    F: FnOnce(kolibri_net::media::ProgressFn) -> Fut,
    Fut: std::future::Future<Output = Result<(u16, Vec<u8>), String>>,
{
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<UploadEvent>();
    let drain = tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if sink.add(event).is_err() {
                break;
            }
        }
    });

    let tx_progress = tx.clone();
    let progress: kolibri_net::media::ProgressFn = Arc::new(move |sent, total| {
        let _ = tx_progress.send(UploadEvent::Progress { sent, total });
    });

    let terminal = match upload(progress).await {
        Ok((status, body)) => UploadEvent::Done { status, body },
        Err(message) => UploadEvent::Error { message },
    };
    let _ = tx.send(terminal);
    drop(tx);
    let _ = drain.await;
}

/// frb runs the blocking methods on a worker thread, so dart sees Futures
pub struct KolibriSession {
    rt: Arc<Runtime>,
    inner: Arc<Session>,
    proxy: Option<ProxyConfig>,
}

impl KolibriSession {
    /// `wire_log`, if given, gets every packet both ways (requests, pushes,
    /// handshake, ping), across reconnects.
    #[frb(sync)]
    pub fn new(
        options: SessionOptions,
        wire_log: Option<StreamSink<WireLogEvent>>,
    ) -> Result<KolibriSession, String> {
        let user_agent = UserAgent {
            device_type: options.device_type,
            app_version: options.app_version,
            os_version: options.os_version,
            timezone: options.timezone,
            screen: options.screen,
            push_device_type: options.push_device_type,
            arch: options.arch,
            locale: options.locale,
            build_number: options.build_number,
            device_name: options.device_name,
            device_locale: options.device_locale,
        };
        let handshake = HandshakeConfig {
            instance_id: options.instance_id,
            device_id: options.device_id,
            client_session_id: options.client_session_id,
            user_agent,
        };
        let proxy = match options.proxy {
            Some(url) => Some(ProxyConfig::parse(&url)?),
            None => None,
        };
        let mut client = ClientConfig::new(options.host, options.port);
        client.insecure_tls = options.insecure_tls;
        client.proxy = proxy.clone();
        let mut config = SessionConfig::new(client, handshake);
        config.ping_interval = Duration::from_secs(options.ping_interval_secs);
        config.ping_interactive = options.ping_interactive;
        config.auto_reconnect = options.auto_reconnect;

        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?,
        );
        let tap: Option<WireTap> = wire_log.map(|sink| {
            let tap: WireTap = Arc::new(move |dir, cmd, opcode, seq, payload| {
                let _ = sink.add(WireLogEvent {
                    direction: dir.as_str().to_string(),
                    cmd: cmd_label(dir, cmd),
                    opcode,
                    seq,
                    json: wire_json(payload),
                });
            });
            tap
        });
        let inner = Arc::new(Session::with_wire_tap(config, tap));
        Ok(KolibriSession { rt, inner, proxy })
    }

    /// connect + sessionInit handshake
    pub fn connect(&self) -> Result<HandshakeInfo, String> {
        let info = self
            .rt
            .block_on(self.inner.connect())
            .map_err(|e| e.to_string())?;
        let mut payload = Vec::new();
        rmpv::encode::write_value(&mut payload, &info.payload).map_err(|e| e.to_string())?;
        let payload_json = kolibri_net::protocol::value_to_json_tagged(&info.payload).to_string();
        Ok(HandshakeInfo {
            calls_seed: info.calls_seed,
            device_name: info.device_name,
            payload,
            payload_json,
        })
    }

    /// awaits the response payload (raw msgpack); errors on server error or timeout
    pub fn request(&self, opcode: u16, payload: Vec<u8>) -> Result<Vec<u8>, String> {
        let packet = self
            .rt
            .block_on(self.inner.request(opcode, &payload))
            .map_err(|e| e.to_string())?;
        Ok(packet.payload)
    }

    /// JSON in, JSON out: `json_in` becomes msgpack (`{"$bin":"<b64>"}` ->
    /// binary), and the response comes back as JSON.
    pub fn request_json(&self, opcode: u16, json_in: String) -> Result<String, String> {
        let value: serde_json::Value = serde_json::from_str(&json_in).map_err(|e| e.to_string())?;
        let mut payload = Vec::new();
        rmpv::encode::write_value(&mut payload, &kolibri_net::protocol::json_to_value(&value))
            .map_err(|e| e.to_string())?;
        let packet = self
            .rt
            .block_on(self.inner.request(opcode, &payload))
            .map_err(|e| e.to_string())?;
        packet
            .json_tagged()
            .map(|j| j.to_string())
            .map_err(|e| e.to_string())
    }

    /// like `request_json`, but reports the packet command and, for an error
    /// packet, its full payload (as tagged JSON) plus extracted message/key —
    /// nothing is thrown, so the host can run its own rules over the error body
    /// (e.g. treat `FAIL_LOGIN_TOKEN`/`FAIL_WRONG_PASSWORD` as expired). Only a
    /// lost connection or timeout comes back as `Err`.
    pub fn request_full(&self, opcode: u16, json_in: String) -> Result<RequestOutcome, String> {
        let value: serde_json::Value = serde_json::from_str(&json_in).map_err(|e| e.to_string())?;
        let mut payload = Vec::new();
        rmpv::encode::write_value(&mut payload, &kolibri_net::protocol::json_to_value(&value))
            .map_err(|e| e.to_string())?;
        let packet = self
            .rt
            .block_on(self.inner.request_raw(opcode, &payload))
            .map_err(|e| e.to_string())?;
        let payload_json = packet
            .json_tagged()
            .map(|j| j.to_string())
            .unwrap_or_else(|_| "null".to_string());
        let (error_message, error_key) = if packet.is_error() {
            error_fields(&packet)
        } else {
            (None, None)
        };
        Ok(RequestOutcome {
            cmd: packet.cmd,
            opcode: packet.opcode,
            payload_json,
            error_message,
            error_key,
        })
    }

    /// fire-and-forget; returns the seq number
    #[frb(sync)]
    pub fn send(&self, opcode: u16, payload: Vec<u8>) -> Result<u32, String> {
        self.inner
            .send(opcode, &payload)
            .map(|seq| seq as u32)
            .map_err(|e| e.to_string())
    }

    /// keepalive `interactive` flag (foreground/background hint)
    #[frb(sync)]
    pub fn ping_interactive(&self) -> bool {
        self.inner.ping_interactive()
    }

    /// flip `interactive` on a live session; one ping goes out now so the server
    /// hears it right away
    #[frb(sync)]
    pub fn set_ping_interactive(&self, interactive: bool) {
        self.inner.set_ping_interactive(interactive);
    }

    /// generic file upload to a CDN url. streams progress, then Done/Error.
    /// user_agent defaults to the session's handshake device.
    pub fn upload_file(
        &self,
        url: String,
        data: Vec<u8>,
        filename: String,
        user_agent: Option<String>,
        sink: StreamSink<UploadEvent>,
    ) {
        let ua = user_agent.unwrap_or_else(|| self.inner.http_user_agent());
        let proxy = self.proxy.clone();
        self.rt
            .spawn(drive_upload(sink, move |progress| async move {
                kolibri_net::media::upload_file(
                    &url,
                    &data,
                    &filename,
                    false,
                    proxy.as_ref(),
                    Some(progress),
                    &ua,
                )
                .await
                .map(|r| (r.status, r.body))
                .map_err(|e| e.to_string())
            }));
    }

    /// photo upload, multipart/form-data. photoToken comes back in the Done body.
    pub fn upload_photo(
        &self,
        url: String,
        data: Vec<u8>,
        filename: String,
        user_agent: Option<String>,
        sink: StreamSink<UploadEvent>,
    ) {
        let ua = user_agent.unwrap_or_else(|| self.inner.http_user_agent());
        let proxy = self.proxy.clone();
        self.rt
            .spawn(drive_upload(sink, move |progress| async move {
                kolibri_net::media::upload_photo(
                    &url,
                    &data,
                    &filename,
                    false,
                    proxy.as_ref(),
                    Some(progress),
                    &ua,
                )
                .await
                .map(|r| (r.status, r.body))
                .map_err(|e| e.to_string())
            }));
    }

    /// video upload, parallel resumable chunks. Done{status:200} means success.
    pub fn upload_video(
        &self,
        url: String,
        data: Vec<u8>,
        chunk_size: u32,
        concurrency: u32,
        sink: StreamSink<UploadEvent>,
    ) {
        let proxy = self.proxy.clone();
        self.rt
            .spawn(drive_upload(sink, move |progress| async move {
                match kolibri_net::media::upload_video(
                    &url,
                    data,
                    chunk_size as usize,
                    concurrency as usize,
                    false,
                    proxy,
                    Some(progress),
                )
                .await
                {
                    Ok(true) => Ok((200, Vec::new())),
                    Ok(false) => Err("upload failed".to_string()),
                    Err(e) => Err(e.to_string()),
                }
            }));
    }

    /// like [`Self::upload_file`], but streams the body off disk from `path`
    /// (never loads the whole file into memory).
    #[allow(clippy::too_many_arguments)]
    pub fn upload_file_path(
        &self,
        url: String,
        path: String,
        filename: String,
        content_type: Option<String>,
        connection: Option<String>,
        user_agent: Option<String>,
        sink: StreamSink<UploadEvent>,
    ) {
        let ua = user_agent.unwrap_or_else(|| self.inner.http_user_agent());
        let proxy = self.proxy.clone();
        self.rt
            .spawn(drive_upload(sink, move |progress| async move {
                kolibri_net::media::upload_file_path(
                    &url,
                    &path,
                    &filename,
                    content_type.as_deref(),
                    connection.as_deref(),
                    false,
                    proxy.as_ref(),
                    Some(progress),
                    &ua,
                )
                .await
                .map(|r| (r.status, r.body))
                .map_err(|e| e.to_string())
            }));
    }

    /// like [`Self::upload_photo`], but streams the file part off disk from `path`.
    pub fn upload_photo_path(
        &self,
        url: String,
        path: String,
        filename: String,
        user_agent: Option<String>,
        sink: StreamSink<UploadEvent>,
    ) {
        let ua = user_agent.unwrap_or_else(|| self.inner.http_user_agent());
        let proxy = self.proxy.clone();
        self.rt
            .spawn(drive_upload(sink, move |progress| async move {
                kolibri_net::media::upload_photo_path(
                    &url,
                    &path,
                    &filename,
                    false,
                    proxy.as_ref(),
                    Some(progress),
                    &ua,
                )
                .await
                .map(|r| (r.status, r.body))
                .map_err(|e| e.to_string())
            }));
    }

    /// like [`Self::upload_video`], but reads each chunk off disk from `path` on
    /// demand (only one chunk per worker in memory).
    pub fn upload_video_path(
        &self,
        url: String,
        path: String,
        chunk_size: u32,
        concurrency: u32,
        sink: StreamSink<UploadEvent>,
    ) {
        let proxy = self.proxy.clone();
        self.rt
            .spawn(drive_upload(sink, move |progress| async move {
                match kolibri_net::media::upload_video_path(
                    &url,
                    &path,
                    chunk_size as usize,
                    concurrency as usize,
                    false,
                    proxy,
                    Some(progress),
                )
                .await
                {
                    Ok(true) => Ok((200, Vec::new())),
                    Ok(false) => Err("upload failed".to_string()),
                    Err(e) => Err(e.to_string()),
                }
            }));
    }

    /// server pushes; yields until the session is dropped
    pub fn pushes(&self, sink: StreamSink<PushEvent>) {
        let mut rx = self.inner.subscribe();
        self.rt.spawn(async move {
            while let Ok(packet) = rx.recv().await {
                let payload_json = packet
                    .json_tagged()
                    .map(|j| j.to_string())
                    .unwrap_or_else(|_| "null".to_string());
                let event = PushEvent {
                    opcode: packet.opcode,
                    payload: packet.payload,
                    payload_json,
                };
                if sink.add(event).is_err() {
                    break;
                }
            }
        });
    }

    /// HTTP User-Agent derived from the handshake device (opcode 6):
    /// `OKMessages/{appVersion} ({osVersion}; {deviceName}; {screen})`. Same
    /// string media uploads use; suitable for webviews that should look like
    /// the native app.
    #[frb(sync)]
    pub fn user_agent(&self) -> String {
        self.inner.http_user_agent()
    }

    #[frb(sync)]
    pub fn state(&self) -> String {
        match self.inner.state() {
            SessionState::Disconnected => "disconnected",
            SessionState::Connecting => "connecting",
            SessionState::Connected => "connected",
            SessionState::Online => "online",
        }
        .to_string()
    }

    #[frb(sync)]
    pub fn disconnect(&self) {
        self.inner.disconnect();
    }
}

/// 96-byte anti-spoof fingerprint (authRequest `mode` / login `chatCacheFingerprint`).
/// signature/dex/so are raw digest bytes, passed in so they can change per app version
#[frb(sync)]
pub fn auth_mode(
    signature: Vec<u8>,
    dex: Vec<u8>,
    so: Vec<u8>,
    calls_seed: i64,
    device_id: String,
) -> Vec<u8> {
    kolibri_net::auth::chat_cache_fingerprint(&signature, &dex, &so, calls_seed, &device_id)
}
