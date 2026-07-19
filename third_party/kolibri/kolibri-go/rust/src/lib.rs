//! C ABI over kolibri-net, for the Go binding (cgo). Blocking calls over an
//! owned tokio runtime, like the Python/Dart wrappers; bytes cross the boundary
//! as `(ptr, len)`. Fallible calls return an owned error string (NULL on
//! success) and write results into out-params.

use std::ffi::{c_char, c_void, CStr, CString};
use std::os::raw::c_int;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kolibri_net::auth::chat_cache_fingerprint;
use kolibri_net::calls::{
    parse_connection, parse_transmitted_data, ConnectionInfo, ConversationParams, TransmittedData,
    Ws2ClientInfo, Ws2Signaling,
};
use kolibri_net::protocol::value_to_json;
use kolibri_net::{
    ClientConfig, Direction, HandshakeConfig, Packet, ProxyConfig, Session, SessionConfig,
    SessionState, UserAgent, WireTap,
};
use serde_json::{json, Value as Json};
use tokio::runtime::Runtime;
use tokio::sync::broadcast;

/// A byte buffer owned by the library; free it with `kolibri_bytes_free`.
#[repr(C)]
pub struct KBytes {
    pub ptr: *mut u8,
    pub len: usize,
}

impl KBytes {
    fn from_vec(v: Vec<u8>) -> Self {
        let boxed = v.into_boxed_slice();
        let len = boxed.len();
        let ptr = Box::into_raw(boxed) as *mut u8;
        KBytes { ptr, len }
    }
    fn empty() -> Self {
        KBytes {
            ptr: ptr::null_mut(),
            len: 0,
        }
    }
}

/// device + connection options.
#[repr(C)]
pub struct KConfig {
    pub host: *const c_char,
    pub port: u16,
    pub device_id: *const c_char,
    pub instance_id: *const c_char,
    pub app_version: *const c_char,
    pub build_number: i64,
    pub device_type: *const c_char,
    pub os_version: *const c_char,
    pub timezone: *const c_char,
    pub screen: *const c_char,
    pub push_device_type: *const c_char,
    pub arch: *const c_char,
    pub locale: *const c_char,
    pub device_name: *const c_char,
    pub device_locale: *const c_char,
    pub client_session_id: i64,
    pub ping_interval_secs: u64,
    pub ping_interactive: bool,
    pub auto_reconnect: bool,
    pub insecure_tls: bool,
    /// proxy url, or NULL/empty for a direct connection
    pub proxy: *const c_char,
}

/// wire-tap callback: one call per packet in each direction.
pub type WireCb = extern "C" fn(
    user: *mut c_void,
    direction: *const c_char,
    cmd: *const c_char,
    opcode: u16,
    seq: u16,
    json: *const c_char,
);

struct WireCtx {
    cb: WireCb,
    user: *mut c_void,
}
// SAFETY: the Go side owns `user` and its callback is safe to call from any
// thread; Rust only passes the token straight back.
unsafe impl Send for WireCtx {}
unsafe impl Sync for WireCtx {}

pub struct KSession {
    rt: Arc<Runtime>,
    inner: Arc<Session>,
    push_rx: Mutex<broadcast::Receiver<Packet>>,
    proxy: Option<ProxyConfig>,
}

#[no_mangle]
pub extern "C" fn kolibri_bytes_free(b: KBytes) {
    if !b.ptr.is_null() {
        unsafe { drop(Box::from_raw(ptr::slice_from_raw_parts_mut(b.ptr, b.len))) };
    }
}

#[no_mangle]
pub extern "C" fn kolibri_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe { drop(CString::from_raw(s)) };
    }
}

#[no_mangle]
pub extern "C" fn kolibri_session_new(
    cfg: *const KConfig,
    wire_cb: Option<WireCb>,
    wire_user: *mut c_void,
    out: *mut *mut KSession,
) -> *mut c_char {
    if cfg.is_null() || out.is_null() {
        return err("null config or out pointer");
    }
    let cfg = unsafe { &*cfg };

    let proxy = match opt_str(cfg.proxy) {
        Some(url) => match ProxyConfig::parse(&url) {
            Ok(p) => Some(p),
            Err(e) => return err(e),
        },
        None => None,
    };

    let user_agent = UserAgent {
        device_type: cstr(cfg.device_type),
        app_version: cstr(cfg.app_version),
        os_version: cstr(cfg.os_version),
        timezone: cstr(cfg.timezone),
        screen: cstr(cfg.screen),
        push_device_type: cstr(cfg.push_device_type),
        arch: cstr(cfg.arch),
        locale: cstr(cfg.locale),
        build_number: cfg.build_number,
        device_name: cstr(cfg.device_name),
        device_locale: cstr(cfg.device_locale),
    };
    let handshake = HandshakeConfig {
        instance_id: cstr(cfg.instance_id),
        device_id: cstr(cfg.device_id),
        client_session_id: cfg.client_session_id,
        user_agent,
    };
    let mut client = ClientConfig::new(cstr(cfg.host), cfg.port);
    client.insecure_tls = cfg.insecure_tls;
    client.proxy = proxy.clone();
    let mut config = SessionConfig::new(client, handshake);
    config.ping_interval = Duration::from_secs(cfg.ping_interval_secs);
    config.ping_interactive = cfg.ping_interactive;
    config.auto_reconnect = cfg.auto_reconnect;

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => Arc::new(rt),
        Err(e) => return err(e.to_string()),
    };

    let tap: Option<WireTap> = wire_cb.map(|cb| {
        let ctx = WireCtx {
            cb,
            user: wire_user,
        };
        Arc::new(
            move |dir: Direction, cmd: u8, opcode: u16, seq: u16, payload: &[u8]| {
                let ctx = &ctx;
                let json = c_string(payload_to_json(payload));
                let c_dir = c_string(dir.as_str().to_string());
                let c_cmd = c_string(cmd_label(dir, cmd).to_string());
                (ctx.cb)(
                    ctx.user,
                    c_dir.as_ptr(),
                    c_cmd.as_ptr(),
                    opcode,
                    seq,
                    json.as_ptr(),
                );
            },
        ) as WireTap
    });

    let inner = Arc::new(Session::with_wire_tap(config, tap));
    let push_rx = Mutex::new(inner.subscribe());
    let session = Box::new(KSession {
        rt,
        inner,
        push_rx,
        proxy,
    });
    unsafe { *out = Box::into_raw(session) };
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn kolibri_session_connect(h: *mut KSession, out: *mut KBytes) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    match s.rt.block_on(s.inner.connect()) {
        Ok(info) => {
            let mut buf = Vec::new();
            if rmpv::encode::write_value(&mut buf, &info.payload).is_err() {
                return err("failed to encode handshake payload");
            }
            unsafe { *out = KBytes::from_vec(buf) };
            ptr::null_mut()
        }
        Err(e) => err(e.to_string()),
    }
}

/// like `kolibri_session_connect`, but the handshake payload as a JSON string.
#[no_mangle]
pub extern "C" fn kolibri_session_connect_json(
    h: *mut KSession,
    out: *mut *mut c_char,
) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    match s.rt.block_on(s.inner.connect()) {
        Ok(info) => {
            let json = kolibri_net::protocol::value_to_json_tagged(&info.payload).to_string();
            unsafe { *out = c_string(json).into_raw() };
            ptr::null_mut()
        }
        Err(e) => err(e.to_string()),
    }
}

#[no_mangle]
pub extern "C" fn kolibri_session_request(
    h: *mut KSession,
    opcode: u16,
    payload: *const u8,
    len: usize,
    out: *mut KBytes,
) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let bytes = slice(payload, len);
    match s.rt.block_on(s.inner.request(opcode, bytes)) {
        Ok(packet) => {
            unsafe { *out = KBytes::from_vec(packet.payload) };
            ptr::null_mut()
        }
        Err(e) => err(e.to_string()),
    }
}

/// JSON in, JSON out: `json_in` becomes msgpack (`{"$bin":"<b64>"}` -> binary),
/// and the response comes back as JSON.
#[no_mangle]
pub extern "C" fn kolibri_session_request_json(
    h: *mut KSession,
    opcode: u16,
    json_in: *const c_char,
    out: *mut *mut c_char,
) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let value: Json = match serde_json::from_str(&cstr(json_in)) {
        Ok(v) => v,
        Err(e) => return err(e.to_string()),
    };
    let mut payload = Vec::new();
    if rmpv::encode::write_value(&mut payload, &kolibri_net::protocol::json_to_value(&value))
        .is_err()
    {
        return err("failed to encode request payload");
    }
    match s.rt.block_on(s.inner.request(opcode, &payload)) {
        Ok(packet) => match packet.json_tagged() {
            Ok(json) => {
                unsafe { *out = c_string(json.to_string()).into_raw() };
                ptr::null_mut()
            }
            Err(e) => err(e.to_string()),
        },
        Err(e) => err(e.to_string()),
    }
}

#[no_mangle]
pub extern "C" fn kolibri_session_send(
    h: *mut KSession,
    opcode: u16,
    payload: *const u8,
    len: usize,
    out_seq: *mut u16,
) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    match s.inner.send(opcode, slice(payload, len)) {
        Ok(seq) => {
            if !out_seq.is_null() {
                unsafe { *out_seq = seq };
            }
            ptr::null_mut()
        }
        Err(e) => err(e.to_string()),
    }
}

#[no_mangle]
pub extern "C" fn kolibri_session_next_push(
    h: *mut KSession,
    timeout_ms: i64,
    out_opcode: *mut u16,
    out_payload: *mut KBytes,
    out_got: *mut bool,
) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let mut rx = s.push_rx.lock().unwrap();
    let packet = s.rt.block_on(async {
        if timeout_ms < 0 {
            rx.recv().await.ok()
        } else {
            tokio::time::timeout(Duration::from_millis(timeout_ms as u64), rx.recv())
                .await
                .ok()
                .and_then(|r| r.ok())
        }
    });
    match packet {
        Some(p) => {
            unsafe {
                *out_opcode = p.opcode;
                *out_payload = KBytes::from_vec(p.payload);
                *out_got = true;
            }
            ptr::null_mut()
        }
        None => {
            unsafe {
                *out_payload = KBytes::empty();
                *out_got = false;
            }
            ptr::null_mut()
        }
    }
}

/// like `kolibri_session_next_push`, but the push payload as a JSON string.
#[no_mangle]
pub extern "C" fn kolibri_session_next_push_json(
    h: *mut KSession,
    timeout_ms: i64,
    out_opcode: *mut u16,
    out_json: *mut *mut c_char,
    out_got: *mut bool,
) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let mut rx = s.push_rx.lock().unwrap();
    let packet = s.rt.block_on(async {
        if timeout_ms < 0 {
            rx.recv().await.ok()
        } else {
            tokio::time::timeout(Duration::from_millis(timeout_ms as u64), rx.recv())
                .await
                .ok()
                .and_then(|r| r.ok())
        }
    });
    match packet {
        Some(p) => unsafe {
            *out_opcode = p.opcode;
            *out_json = c_string(payload_to_json_tagged(&p.payload)).into_raw();
            *out_got = true;
        },
        None => unsafe {
            *out_json = ptr::null_mut();
            *out_got = false;
        },
    }
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn kolibri_session_state(h: *mut KSession) -> c_int {
    match handle(h) {
        Ok(s) => match s.inner.state() {
            SessionState::Disconnected => 0,
            SessionState::Connecting => 1,
            SessionState::Connected => 2,
            SessionState::Online => 3,
        },
        Err(e) => {
            kolibri_string_free(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn kolibri_session_ping_interactive(h: *mut KSession) -> bool {
    match handle(h) {
        Ok(s) => s.inner.ping_interactive(),
        Err(e) => {
            kolibri_string_free(e);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn kolibri_session_set_ping_interactive(h: *mut KSession, interactive: bool) {
    if let Ok(s) = handle(h) {
        s.inner.set_ping_interactive(interactive);
    }
}

#[no_mangle]
pub extern "C" fn kolibri_session_user_agent(h: *mut KSession) -> *mut c_char {
    match handle(h) {
        Ok(s) => c_string(s.inner.http_user_agent()).into_raw(),
        Err(e) => {
            kolibri_string_free(e);
            ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn kolibri_session_disconnect(h: *mut KSession) {
    if let Ok(s) = handle(h) {
        s.inner.disconnect();
    }
}

#[no_mangle]
pub extern "C" fn kolibri_session_free(h: *mut KSession) {
    if !h.is_null() {
        unsafe { drop(Box::from_raw(h)) };
    }
}

#[no_mangle]
pub extern "C" fn kolibri_upload_file(
    h: *mut KSession,
    url: *const c_char,
    data: *const u8,
    len: usize,
    filename: *const c_char,
    out_status: *mut u16,
    out_body: *mut KBytes,
) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let ua = s.inner.http_user_agent();
    let proxy = s.proxy.clone();
    let result = s.rt.block_on(kolibri_net::media::upload_file(
        &cstr(url),
        slice(data, len),
        &cstr(filename),
        false,
        proxy.as_ref(),
        None,
        &ua,
    ));
    finish_media(result, out_status, out_body)
}

#[no_mangle]
pub extern "C" fn kolibri_upload_photo(
    h: *mut KSession,
    url: *const c_char,
    data: *const u8,
    len: usize,
    filename: *const c_char,
    out_status: *mut u16,
    out_body: *mut KBytes,
) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let ua = s.inner.http_user_agent();
    let proxy = s.proxy.clone();
    let result = s.rt.block_on(kolibri_net::media::upload_photo(
        &cstr(url),
        slice(data, len),
        &cstr(filename),
        false,
        proxy.as_ref(),
        None,
        &ua,
    ));
    finish_media(result, out_status, out_body)
}

#[no_mangle]
pub extern "C" fn kolibri_upload_video(
    h: *mut KSession,
    url: *const c_char,
    data: *const u8,
    len: usize,
    chunk_size: usize,
    concurrency: usize,
    out_ok: *mut bool,
) -> *mut c_char {
    let s = match handle(h) {
        Ok(s) => s,
        Err(e) => return e,
    };
    let proxy = s.proxy.clone();
    let result = s.rt.block_on(kolibri_net::media::upload_video(
        &cstr(url),
        slice(data, len).to_vec(),
        chunk_size,
        concurrency,
        false,
        proxy,
        None,
    ));
    match result {
        Ok(ok) => {
            unsafe { *out_ok = ok };
            ptr::null_mut()
        }
        Err(e) => err(e.to_string()),
    }
}

/// 96-byte anti-spoof fingerprint (authRequest `mode` / login `chatCacheFingerprint`).
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn kolibri_auth_mode(
    signature: *const u8,
    signature_len: usize,
    dex: *const u8,
    dex_len: usize,
    so: *const u8,
    so_len: usize,
    calls_seed: i64,
    device_id: *const c_char,
    out: *mut KBytes,
) -> *mut c_char {
    let fp = chat_cache_fingerprint(
        slice(signature, signature_len),
        slice(dex, dex_len),
        slice(so, so_len),
        calls_seed,
        &cstr(device_id),
    );
    unsafe { *out = KBytes::from_vec(fp) };
    ptr::null_mut()
}

// ---- calls (ws2 signaling) ----

pub struct KCall {
    rt: Arc<Runtime>,
    inner: Arc<Ws2Signaling>,
    notif_rx: Mutex<broadcast::Receiver<Json>>,
}

/// vcp string -> JSON (token/endpoints/ice_servers/user_id, + ws2_url when
/// `conversation_id` is given). `out_got` is false if the vcp can't be decoded.
#[no_mangle]
pub extern "C" fn kolibri_decode_vcp(
    vcp: *const c_char,
    conversation_id: *const c_char,
    out_got: *mut bool,
    out_json: *mut *mut c_char,
) -> *mut c_char {
    match ConversationParams::decode(&cstr(vcp)) {
        Some(p) => {
            let json = vcp_json(&p, opt_str(conversation_id).as_deref());
            unsafe {
                *out_got = true;
                *out_json = c_string(json.to_string()).into_raw();
            }
        }
        None => unsafe {
            *out_got = false;
            *out_json = ptr::null_mut();
        },
    }
    ptr::null_mut()
}

/// `connection` notification JSON -> `{topology,is_sfu,participants,ice_servers[,peer]}`.
#[no_mangle]
pub extern "C" fn kolibri_parse_connection(
    notification: *const c_char,
    my_user_id: i64,
    has_user_id: bool,
    out_json: *mut *mut c_char,
) -> *mut c_char {
    let value: Json = match serde_json::from_str(&cstr(notification)) {
        Ok(v) => v,
        Err(e) => return err(e.to_string()),
    };
    let info = parse_connection(&value);
    let uid = if has_user_id { Some(my_user_id) } else { None };
    unsafe { *out_json = c_string(connection_json(&info, uid).to_string()).into_raw() };
    ptr::null_mut()
}

/// `transmitted-data` notification JSON -> `{kind:"sdp",...}` or
/// `{kind:"candidate",...}`. `out_got` is false when it's neither.
#[no_mangle]
pub extern "C" fn kolibri_parse_transmitted_data(
    notification: *const c_char,
    out_got: *mut bool,
    out_json: *mut *mut c_char,
) -> *mut c_char {
    let value: Json = match serde_json::from_str(&cstr(notification)) {
        Ok(v) => v,
        Err(e) => return err(e.to_string()),
    };
    match parse_transmitted_data(&value) {
        Some(data) => unsafe {
            *out_got = true;
            *out_json = c_string(transmitted_json(&data).to_string()).into_raw();
        },
        None => unsafe {
            *out_got = false;
            *out_json = ptr::null_mut();
        },
    }
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn kolibri_call_connect(
    url: *const c_char,
    user_agent: *const c_char,
    proxy: *const c_char,
    out: *mut *mut KCall,
) -> *mut c_char {
    let proxy = match opt_str(proxy) {
        Some(p) => match ProxyConfig::parse(&p) {
            Ok(p) => Some(p),
            Err(e) => return err(e),
        },
        None => None,
    };
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => Arc::new(rt),
        Err(e) => return err(e.to_string()),
    };
    let ua = opt_str(user_agent);
    let result = rt.block_on(Ws2Signaling::connect_via(
        &cstr(url),
        ua.as_deref(),
        proxy.as_ref(),
    ));
    match result {
        Ok(sig) => {
            let inner = Arc::new(sig);
            let notif_rx = Mutex::new(inner.notifications());
            let call = Box::new(KCall {
                rt,
                inner,
                notif_rx,
            });
            unsafe { *out = Box::into_raw(call) };
            ptr::null_mut()
        }
        Err(e) => err(e.to_string()),
    }
}

#[no_mangle]
pub extern "C" fn kolibri_call_accept(h: *mut KCall, out_json: *mut *mut c_char) -> *mut c_char {
    call_result(h, out_json, |c| c.rt.block_on(c.inner.accept_call()))
}

#[no_mangle]
pub extern "C" fn kolibri_call_hangup(
    h: *mut KCall,
    reason: *const c_char,
    out_json: *mut *mut c_char,
) -> *mut c_char {
    let reason = cstr(reason);
    call_result(h, out_json, |c| c.rt.block_on(c.inner.hangup(&reason)))
}

#[no_mangle]
pub extern "C" fn kolibri_call_transmit_sdp(
    h: *mut KCall,
    participant_id: i64,
    sdp_type: *const c_char,
    sdp: *const c_char,
    out_json: *mut *mut c_char,
) -> *mut c_char {
    let (t, d) = (cstr(sdp_type), cstr(sdp));
    call_result(h, out_json, |c| {
        c.rt.block_on(c.inner.transmit_sdp(participant_id, &t, &d))
    })
}

#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub extern "C" fn kolibri_call_transmit_candidate(
    h: *mut KCall,
    participant_id: i64,
    candidate: *const c_char,
    sdp_mid: *const c_char,
    sdp_mline_index: i64,
    out_json: *mut *mut c_char,
) -> *mut c_char {
    let (cand, mid) = (cstr(candidate), cstr(sdp_mid));
    call_result(h, out_json, |c| {
        c.rt.block_on(
            c.inner
                .transmit_candidate(participant_id, &cand, &mid, sdp_mline_index),
        )
    })
}

#[no_mangle]
pub extern "C" fn kolibri_call_change_media(
    h: *mut KCall,
    audio: bool,
    video: bool,
    screen: bool,
    out_json: *mut *mut c_char,
) -> *mut c_char {
    call_result(h, out_json, |c| {
        c.rt.block_on(c.inner.change_media_settings(audio, video, screen))
    })
}

#[no_mangle]
pub extern "C" fn kolibri_call_send_command(
    h: *mut KCall,
    command: *const c_char,
    extra_json: *const c_char,
    out_json: *mut *mut c_char,
) -> *mut c_char {
    let extra = match opt_str(extra_json) {
        Some(s) => match serde_json::from_str::<Json>(&s) {
            Ok(v) => v,
            Err(e) => return err(e.to_string()),
        },
        None => Json::Object(Default::default()),
    };
    let command = cstr(command);
    call_result(h, out_json, |c| {
        c.rt.block_on(c.inner.send_command(&command, extra))
    })
}

#[no_mangle]
pub extern "C" fn kolibri_call_next_notification(
    h: *mut KCall,
    timeout_ms: i64,
    out_json: *mut *mut c_char,
    out_got: *mut bool,
) -> *mut c_char {
    let c = match call_handle(h) {
        Ok(c) => c,
        Err(e) => return e,
    };
    let mut rx = c.notif_rx.lock().unwrap();
    let value = c.rt.block_on(async {
        if timeout_ms < 0 {
            rx.recv().await.ok()
        } else {
            tokio::time::timeout(Duration::from_millis(timeout_ms as u64), rx.recv())
                .await
                .ok()
                .and_then(|r| r.ok())
        }
    });
    match value {
        Some(v) => unsafe {
            *out_got = true;
            *out_json = c_string(v.to_string()).into_raw();
        },
        None => unsafe {
            *out_got = false;
            *out_json = ptr::null_mut();
        },
    }
    ptr::null_mut()
}

#[no_mangle]
pub extern "C" fn kolibri_call_is_connected(h: *mut KCall) -> bool {
    match call_handle(h) {
        Ok(c) => c.inner.is_connected(),
        Err(e) => {
            kolibri_string_free(e);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn kolibri_call_close(h: *mut KCall) {
    if !h.is_null() {
        let call = unsafe { Box::from_raw(h) };
        call.inner.close();
    }
}

fn call_result(
    h: *mut KCall,
    out_json: *mut *mut c_char,
    f: impl FnOnce(&KCall) -> Result<Json, kolibri_net::calls::CallError>,
) -> *mut c_char {
    let c = match call_handle(h) {
        Ok(c) => c,
        Err(e) => return e,
    };
    match f(c) {
        Ok(v) => {
            unsafe { *out_json = c_string(v.to_string()).into_raw() };
            ptr::null_mut()
        }
        Err(e) => err(e.to_string()),
    }
}

fn call_handle<'a>(h: *mut KCall) -> Result<&'a KCall, *mut c_char> {
    if h.is_null() {
        Err(err("null call handle"))
    } else {
        Ok(unsafe { &*h })
    }
}

fn ice_json(urls: &[String], username: &Option<String>, credential: &Option<String>) -> Json {
    json!({ "urls": urls, "username": username, "credential": credential })
}

fn vcp_json(p: &ConversationParams, conversation_id: Option<&str>) -> Json {
    let ice: Vec<Json> = p
        .ice_servers()
        .iter()
        .map(|s| ice_json(&s.urls, &s.username, &s.credential))
        .collect();
    let mut obj = json!({
        "token": p.token,
        "ws_endpoint": p.ws_endpoint,
        "stun": p.stun,
        "turn": p.turn,
        "turn_user": p.turn_user,
        "turn_password": p.turn_password,
        "is_video": p.is_video,
        "expires_at": p.expires_at,
        "user_id": p.user_id(),
        "ice_servers": ice,
    });
    if let Some(cid) = conversation_id {
        obj["ws2_url"] = json!(p.ws2_url(cid, &Ws2ClientInfo::default()));
    }
    obj
}

fn connection_json(info: &ConnectionInfo, my_user_id: Option<i64>) -> Json {
    let ice: Vec<Json> = info
        .ice_servers
        .iter()
        .map(|s| ice_json(&s.urls, &s.username, &s.credential))
        .collect();
    let mut obj = json!({
        "topology": info.topology,
        "is_sfu": info.is_sfu(),
        "participants": info.participants,
        "ice_servers": ice,
    });
    if let Some(uid) = my_user_id {
        obj["peer"] = json!(info.peer_of(uid));
    }
    obj
}

fn transmitted_json(data: &TransmittedData) -> Json {
    match data {
        TransmittedData::Sdp { sdp_type, sdp } => {
            json!({ "kind": "sdp", "type": sdp_type, "sdp": sdp })
        }
        TransmittedData::Candidate {
            candidate,
            sdp_mid,
            sdp_mline_index,
        } => json!({
            "kind": "candidate",
            "candidate": candidate,
            "sdp_mid": sdp_mid,
            "sdp_mline_index": sdp_mline_index,
        }),
    }
}

fn finish_media(
    result: Result<kolibri_net::media::HttpResponse, kolibri_net::media::MediaError>,
    out_status: *mut u16,
    out_body: *mut KBytes,
) -> *mut c_char {
    match result {
        Ok(resp) => {
            unsafe {
                *out_status = resp.status;
                *out_body = KBytes::from_vec(resp.body);
            }
            ptr::null_mut()
        }
        Err(e) => err(e.to_string()),
    }
}

fn handle<'a>(h: *mut KSession) -> Result<&'a KSession, *mut c_char> {
    if h.is_null() {
        Err(err("null session handle"))
    } else {
        Ok(unsafe { &*h })
    }
}

fn err(msg: impl Into<String>) -> *mut c_char {
    c_string(msg.into()).into_raw()
}

fn c_string(s: String) -> CString {
    CString::new(s).unwrap_or_else(|_| CString::new("<nul in string>").unwrap())
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}

fn opt_str(p: *const c_char) -> Option<String> {
    let s = cstr(p);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn slice<'a>(p: *const u8, len: usize) -> &'a [u8] {
    if p.is_null() || len == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(p, len) }
    }
}

fn payload_to_json(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "null".to_string();
    }
    match rmpv::decode::read_value(&mut &payload[..]) {
        Ok(v) => value_to_json(&v).to_string(),
        Err(_) => "null".to_string(),
    }
}

fn payload_to_json_tagged(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "null".to_string();
    }
    match rmpv::decode::read_value(&mut &payload[..]) {
        Ok(v) => kolibri_net::protocol::value_to_json_tagged(&v).to_string(),
        Err(_) => "null".to_string(),
    }
}

fn cmd_label(dir: Direction, cmd: u8) -> &'static str {
    match cmd {
        kolibri_net::cmd::OK => "ok",
        kolibri_net::cmd::NOT_FOUND => "not_found",
        kolibri_net::cmd::ERROR => "error",
        _ => match dir {
            Direction::Out => "request",
            Direction::In => "push",
        },
    }
}
