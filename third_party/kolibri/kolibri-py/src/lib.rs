//! python bindings for kolibri-net. blocking Session owns its own tokio runtime;
//! msgpack payloads convert to/from native python dicts/lists.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use kolibri_net::{
    ClientConfig, Direction, HandshakeConfig, Packet, ProxyConfig, Session as NetSession,
    SessionConfig, SessionState, TransportError, UserAgent, WireTap,
};
use pyo3::exceptions::{PyRuntimeError, PyTimeoutError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use rmpv::Value;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;

/// each method blocks the caller, driving the tokio runtime to completion
#[pyclass]
struct Session {
    rt: Arc<Runtime>,
    inner: Arc<NetSession>,
    push_rx: Mutex<broadcast::Receiver<Packet>>,
    proxy: Option<ProxyConfig>,
}

#[pymethods]
impl Session {
    #[new]
    #[pyo3(signature = (
        host,
        port = 443,
        device_id = "kolibri-rs-device",
        instance_id = "kolibri-rs-instance",
        app_version = "26.20.2",
        build_number = 6758,
        device_type = "ANDROID",
        os_version = "Android 14",
        timezone = "Europe/Moscow",
        screen = "420dpi 420dpi 1080x2340",
        push_device_type = "GCM",
        arch = "arm64-v8a",
        locale = "ru",
        device_name = "Rust",
        device_locale = "ru",
        client_session_id = 1_700_000_000i64,
        ping_interval_secs = 30u64,
        ping_interactive = true,
        auto_reconnect = true,
        insecure_tls = false,
        proxy = None,
        on_wire = None
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        host: &str,
        port: u16,
        device_id: &str,
        instance_id: &str,
        app_version: &str,
        build_number: i64,
        device_type: &str,
        os_version: &str,
        timezone: &str,
        screen: &str,
        push_device_type: &str,
        arch: &str,
        locale: &str,
        device_name: &str,
        device_locale: &str,
        client_session_id: i64,
        ping_interval_secs: u64,
        ping_interactive: bool,
        auto_reconnect: bool,
        insecure_tls: bool,
        proxy: Option<String>,
        on_wire: Option<PyObject>,
    ) -> PyResult<Self> {
        let user_agent = UserAgent {
            device_type: device_type.to_string(),
            app_version: app_version.to_string(),
            os_version: os_version.to_string(),
            timezone: timezone.to_string(),
            screen: screen.to_string(),
            push_device_type: push_device_type.to_string(),
            arch: arch.to_string(),
            locale: locale.to_string(),
            build_number,
            device_name: device_name.to_string(),
            device_locale: device_locale.to_string(),
        };
        let handshake = HandshakeConfig {
            instance_id: instance_id.to_string(),
            device_id: device_id.to_string(),
            client_session_id,
            user_agent,
        };
        let proxy = match proxy {
            Some(url) => Some(ProxyConfig::parse(&url).map_err(PyValueError::new_err)?),
            None => None,
        };
        let mut client = ClientConfig::new(host, port);
        client.insecure_tls = insecure_tls;
        client.proxy = proxy.clone();
        let mut config = SessionConfig::new(client, handshake);
        config.ping_interval = Duration::from_secs(ping_interval_secs);
        config.ping_interactive = ping_interactive;
        config.auto_reconnect = auto_reconnect;

        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
        );
        let tap = on_wire.map(wire_tap_from_callable);
        let inner = Arc::new(NetSession::with_wire_tap(config, tap));
        let push_rx = Mutex::new(inner.subscribe());

        Ok(Self {
            rt,
            inner,
            push_rx,
            proxy,
        })
    }

    /// connect + sessionInit handshake => {calls_seed, device_name, payload}
    fn connect(&self, py: Python<'_>) -> PyResult<PyObject> {
        let rt = self.rt.clone();
        let inner = self.inner.clone();
        let info = py
            .allow_threads(move || rt.block_on(inner.connect()))
            .map_err(to_pyerr)?;

        let dict = PyDict::new(py);
        dict.set_item("calls_seed", info.calls_seed)?;
        dict.set_item("device_name", info.device_name)?;
        dict.set_item("payload", value_to_py(py, &info.payload)?)?;
        Ok(dict.into_any().unbind())
    }

    /// decoded response; raises on server error packet or timeout
    fn request(
        &self,
        py: Python<'_>,
        opcode: u16,
        payload: &Bound<'_, PyAny>,
    ) -> PyResult<PyObject> {
        let bytes = encode_value(&py_to_value(payload)?);
        let rt = self.rt.clone();
        let inner = self.inner.clone();
        let packet = py
            .allow_threads(move || rt.block_on(inner.request(opcode, &bytes)))
            .map_err(to_pyerr)?;
        let value = packet
            .value()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(value_to_py(py, &value)?.unbind())
    }

    /// like `request`, but the response as a JSON string for logs (binary ->
    /// base64; see the core `json` module)
    fn request_json(
        &self,
        py: Python<'_>,
        opcode: u16,
        payload: &Bound<'_, PyAny>,
    ) -> PyResult<String> {
        let bytes = encode_value(&py_to_value(payload)?);
        let rt = self.rt.clone();
        let inner = self.inner.clone();
        let packet = py
            .allow_threads(move || rt.block_on(inner.request(opcode, &bytes)))
            .map_err(to_pyerr)?;
        let json = packet
            .json()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(json.to_string())
    }

    /// fire-and-forget; returns the seq number
    fn send(&self, opcode: u16, payload: &Bound<'_, PyAny>) -> PyResult<u16> {
        let bytes = encode_value(&py_to_value(payload)?);
        self.inner.send(opcode, &bytes).map_err(to_pyerr)
    }

    /// keepalive `interactive` flag (foreground/background hint)
    fn ping_interactive(&self) -> bool {
        self.inner.ping_interactive()
    }

    /// flip `interactive` on a live session; one ping goes out now so the server
    /// hears it right away
    fn set_ping_interactive(&self, interactive: bool) {
        self.inner.set_ping_interactive(interactive);
    }

    /// generic file upload to a CDN url. progress cb, if given, gets (sent, total).
    /// user_agent defaults to the session's handshake device.
    #[pyo3(signature = (url, data, filename, progress = None, user_agent = None))]
    fn upload_file(
        &self,
        py: Python<'_>,
        url: &str,
        data: Vec<u8>,
        filename: &str,
        progress: Option<PyObject>,
        user_agent: Option<String>,
    ) -> PyResult<PyObject> {
        let rt = self.rt.clone();
        let url = url.to_string();
        let filename = filename.to_string();
        let progress = py_progress(progress);
        let ua = user_agent.unwrap_or_else(|| self.inner.http_user_agent());
        let proxy = self.proxy.clone();
        let resp = py
            .allow_threads(move || {
                rt.block_on(kolibri_net::media::upload_file(
                    &url,
                    &data,
                    &filename,
                    false,
                    proxy.as_ref(),
                    progress,
                    &ua,
                ))
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        media_response(py, resp)
    }

    /// photo upload, multipart/form-data. photoToken comes back in the JSON body.
    #[pyo3(signature = (url, data, filename, progress = None, user_agent = None))]
    fn upload_photo(
        &self,
        py: Python<'_>,
        url: &str,
        data: Vec<u8>,
        filename: &str,
        progress: Option<PyObject>,
        user_agent: Option<String>,
    ) -> PyResult<PyObject> {
        let rt = self.rt.clone();
        let url = url.to_string();
        let filename = filename.to_string();
        let progress = py_progress(progress);
        let ua = user_agent.unwrap_or_else(|| self.inner.http_user_agent());
        let proxy = self.proxy.clone();
        let resp = py
            .allow_threads(move || {
                rt.block_on(kolibri_net::media::upload_photo(
                    &url,
                    &data,
                    &filename,
                    false,
                    proxy.as_ref(),
                    progress,
                    &ua,
                ))
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        media_response(py, resp)
    }

    /// video upload, parallel resumable chunks. progress gets (sent, total).
    #[pyo3(signature = (url, data, chunk_size = 2 * 1024 * 1024, concurrency = 4, progress = None))]
    fn upload_video(
        &self,
        py: Python<'_>,
        url: &str,
        data: Vec<u8>,
        chunk_size: usize,
        concurrency: usize,
        progress: Option<PyObject>,
    ) -> PyResult<bool> {
        let rt = self.rt.clone();
        let url = url.to_string();
        let progress = py_progress(progress);
        let proxy = self.proxy.clone();
        py.allow_threads(move || {
            rt.block_on(kolibri_net::media::upload_video(
                &url,
                data,
                chunk_size,
                concurrency,
                false,
                proxy,
                progress,
            ))
        })
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    /// next server push as {opcode, payload}, None on timeout
    #[pyo3(signature = (timeout_secs = None))]
    fn next_push(&self, py: Python<'_>, timeout_secs: Option<f64>) -> PyResult<Option<PyObject>> {
        let rt = self.rt.clone();
        let result = py.allow_threads(move || {
            let mut rx = self.push_rx.lock().unwrap();
            rt.block_on(async {
                match timeout_secs {
                    Some(t) => tokio::time::timeout(Duration::from_secs_f64(t), rx.recv())
                        .await
                        .ok()
                        .and_then(|r| r.ok()),
                    None => rx.recv().await.ok(),
                }
            })
        });

        match result {
            Some(packet) => {
                let dict = PyDict::new(py);
                dict.set_item("opcode", packet.opcode)?;
                let value = packet.value().unwrap_or(Value::Nil);
                dict.set_item("payload", value_to_py(py, &value)?)?;
                Ok(Some(dict.into_any().unbind()))
            }
            None => Ok(None),
        }
    }

    /// "disconnected" | "connecting" | "connected" | "online"
    fn state(&self) -> &'static str {
        match self.inner.state() {
            SessionState::Disconnected => "disconnected",
            SessionState::Connecting => "connecting",
            SessionState::Connected => "connected",
            SessionState::Online => "online",
        }
    }

    /// stops the session and disables auto-reconnect
    fn disconnect(&self) {
        self.inner.disconnect();
    }
}

/// wrap a python (sent, total) callable into a core progress cb. upload runs with
/// the GIL released, so the callback re-acquires it.
fn py_progress(progress: Option<PyObject>) -> Option<kolibri_net::media::ProgressFn> {
    progress.map(|cb| {
        let cb = std::sync::Arc::new(cb);
        std::sync::Arc::new(move |sent: u64, total: u64| {
            Python::with_gil(|py| {
                let _ = cb.call1(py, (sent, total));
            });
        }) as kolibri_net::media::ProgressFn
    })
}

fn wire_tap_from_callable(cb: PyObject) -> WireTap {
    let cb = Arc::new(cb);
    Arc::new(
        move |dir: Direction, cmd: u8, opcode: u16, seq: u16, payload: &[u8]| {
            let json = payload_to_json_string(payload);
            let cmd_label = cmd_label(dir, cmd);
            Python::with_gil(|py| {
                let _ = cb.call1(py, (dir.as_str(), cmd_label, opcode, seq, json));
            });
        },
    )
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

fn payload_to_json_string(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "null".to_string();
    }
    match rmpv::decode::read_value(&mut &payload[..]) {
        Ok(v) => kolibri_net::protocol::value_to_json(&v).to_string(),
        Err(_) => "null".to_string(),
    }
}

fn media_response(py: Python<'_>, resp: kolibri_net::media::HttpResponse) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("status", resp.status)?;
    dict.set_item("body", PyBytes::new(py, &resp.body))?;
    Ok(dict.into_any().unbind())
}

fn to_pyerr(e: TransportError) -> PyErr {
    match e {
        TransportError::Timeout => PyTimeoutError::new_err("request timed out"),
        other => PyRuntimeError::new_err(other.to_string()),
    }
}

fn encode_value(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, value).expect("in-memory msgpack write cannot fail");
    out
}

/// bool checked before int (python bool subclasses int)
fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        return Ok(Value::Nil);
    }
    if let Ok(b) = obj.downcast::<PyBool>() {
        return Ok(Value::Boolean(b.is_true()));
    }
    if let Ok(i) = obj.downcast::<PyInt>() {
        if let Ok(v) = i.extract::<i64>() {
            return Ok(Value::from(v));
        }
        if let Ok(v) = i.extract::<u64>() {
            return Ok(Value::from(v));
        }
        return Err(PyValueError::new_err("integer out of 64-bit range"));
    }
    if let Ok(f) = obj.downcast::<PyFloat>() {
        return Ok(Value::from(f.value()));
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        return Ok(Value::from(s.extract::<String>()?));
    }
    if let Ok(b) = obj.downcast::<PyBytes>() {
        return Ok(Value::Binary(b.as_bytes().to_vec()));
    }
    if let Ok(list) = obj.downcast::<PyList>() {
        let mut arr = Vec::with_capacity(list.len());
        for item in list.iter() {
            arr.push(py_to_value(&item)?);
        }
        return Ok(Value::Array(arr));
    }
    if let Ok(tuple) = obj.downcast::<PyTuple>() {
        let mut arr = Vec::with_capacity(tuple.len());
        for item in tuple.iter() {
            arr.push(py_to_value(&item)?);
        }
        return Ok(Value::Array(arr));
    }
    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut map = Vec::with_capacity(dict.len());
        for (k, v) in dict.iter() {
            map.push((py_to_value(&k)?, py_to_value(&v)?));
        }
        return Ok(Value::Map(map));
    }
    Err(PyTypeError::new_err(format!(
        "unsupported type for MessagePack: {}",
        obj.get_type().name()?
    )))
}

fn value_to_py<'py>(py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match value {
        Value::Nil => py.None().into_bound(py),
        Value::Boolean(b) => PyBool::new(py, *b).to_owned().into_any(),
        Value::Integer(i) => {
            if let Some(v) = i.as_i64() {
                v.into_pyobject(py).unwrap().into_any()
            } else if let Some(v) = i.as_u64() {
                v.into_pyobject(py).unwrap().into_any()
            } else {
                return Err(PyValueError::new_err("integer out of range"));
            }
        }
        Value::F32(f) => (*f as f64).into_pyobject(py).unwrap().into_any(),
        Value::F64(f) => (*f).into_pyobject(py).unwrap().into_any(),
        Value::String(s) => match s.as_str() {
            Some(text) => text.into_pyobject(py).unwrap().into_any(),
            None => PyBytes::new(py, s.as_bytes()).into_any(),
        },
        Value::Binary(b) => PyBytes::new(py, b).into_any(),
        Value::Array(arr) => {
            let list = PyList::empty(py);
            for v in arr {
                list.append(value_to_py(py, v)?)?;
            }
            list.into_any()
        }
        Value::Map(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(value_to_py(py, k)?, value_to_py(py, v)?)?;
            }
            dict.into_any()
        }
        Value::Ext(_, data) => PyBytes::new(py, data).into_any(),
    })
}

const DEFAULT_SIGNATURE_DIGEST: &str =
    "1684414033eb263e2c615f8b7df5ed8793850a07656304997fbf07e9e21e1e93";
const DEFAULT_SO_DIGEST: &str = "90e2fb8745b17b42a10182f8d8ac590e3fca5b311e2ce2d5144fa2c18cb3090d";
const DEFAULT_DEX_DIGEST: &str = "0a6265f6e5d8231b9cba641f8c40475e6f3baeb06ed41b804b9bf7307aa4214e";

fn hex_bytes(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

/// 96-byte anti-spoof fingerprint (authRequest `mode` / login `chatCacheFingerprint`).
/// signature/dex/so are raw digest bytes; omitted ones fall back to app defaults
#[pyfunction]
#[pyo3(signature = (calls_seed, device_id, signature = None, dex = None, so = None))]
fn auth_mode<'py>(
    py: Python<'py>,
    calls_seed: i64,
    device_id: &str,
    signature: Option<Vec<u8>>,
    dex: Option<Vec<u8>>,
    so: Option<Vec<u8>>,
) -> Bound<'py, PyBytes> {
    let signature = signature.unwrap_or_else(|| hex_bytes(DEFAULT_SIGNATURE_DIGEST));
    let dex = dex.unwrap_or_else(|| hex_bytes(DEFAULT_DEX_DIGEST));
    let so = so.unwrap_or_else(|| hex_bytes(DEFAULT_SO_DIGEST));
    let mode =
        kolibri_net::auth::chat_cache_fingerprint(&signature, &dex, &so, calls_seed, device_id);
    PyBytes::new(py, &mode)
}

fn json_to_py<'py>(py: Python<'py>, value: &serde_json::Value) -> PyResult<Bound<'py, PyAny>> {
    use serde_json::Value as J;
    Ok(match value {
        J::Null => py.None().into_bound(py),
        J::Bool(b) => PyBool::new(py, *b).to_owned().into_any(),
        J::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_pyobject(py).unwrap().into_any()
            } else if let Some(u) = n.as_u64() {
                u.into_pyobject(py).unwrap().into_any()
            } else {
                n.as_f64()
                    .unwrap_or(0.0)
                    .into_pyobject(py)
                    .unwrap()
                    .into_any()
            }
        }
        J::String(s) => s.as_str().into_pyobject(py).unwrap().into_any(),
        J::Array(a) => {
            let list = PyList::empty(py);
            for v in a {
                list.append(json_to_py(py, v)?)?;
            }
            list.into_any()
        }
        J::Object(o) => {
            let dict = PyDict::new(py);
            for (k, v) in o {
                dict.set_item(k, json_to_py(py, v)?)?;
            }
            dict.into_any()
        }
    })
}

fn py_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    use serde_json::Value as J;
    if obj.is_none() {
        return Ok(J::Null);
    }
    if let Ok(b) = obj.downcast::<PyBool>() {
        return Ok(J::Bool(b.is_true()));
    }
    if let Ok(i) = obj.downcast::<PyInt>() {
        if let Ok(v) = i.extract::<i64>() {
            return Ok(J::Number(v.into()));
        }
        if let Ok(v) = i.extract::<u64>() {
            return Ok(J::Number(v.into()));
        }
        return Err(PyValueError::new_err("integer out of 64-bit range"));
    }
    if let Ok(f) = obj.downcast::<PyFloat>() {
        return Ok(serde_json::Number::from_f64(f.value())
            .map(J::Number)
            .unwrap_or(J::Null));
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        return Ok(J::String(s.extract::<String>()?));
    }
    if let Ok(list) = obj.downcast::<PyList>() {
        let mut arr = Vec::with_capacity(list.len());
        for item in list.iter() {
            arr.push(py_to_json(&item)?);
        }
        return Ok(J::Array(arr));
    }
    if let Ok(tuple) = obj.downcast::<PyTuple>() {
        let mut arr = Vec::with_capacity(tuple.len());
        for item in tuple.iter() {
            arr.push(py_to_json(&item)?);
        }
        return Ok(J::Array(arr));
    }
    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in dict.iter() {
            let key = k
                .extract::<String>()
                .map_err(|_| PyTypeError::new_err("JSON object keys must be strings"))?;
            map.insert(key, py_to_json(&v)?);
        }
        return Ok(J::Object(map));
    }
    Err(PyTypeError::new_err(format!(
        "unsupported type for JSON: {}",
        obj.get_type().name()?
    )))
}

/// vcp call-params string => dict. with conversation_id, also includes ws2_url.
#[pyfunction]
#[pyo3(signature = (vcp, conversation_id = None))]
fn decode_vcp(
    py: Python<'_>,
    vcp: &str,
    conversation_id: Option<&str>,
) -> PyResult<Option<PyObject>> {
    let Some(p) = kolibri_net::calls::ConversationParams::decode(vcp) else {
        return Ok(None);
    };
    let dict = PyDict::new(py);
    dict.set_item("token", &p.token)?;
    dict.set_item("ws_endpoint", &p.ws_endpoint)?;
    dict.set_item("stun", &p.stun)?;
    dict.set_item("turn", p.turn.clone())?;
    dict.set_item("turn_user", &p.turn_user)?;
    dict.set_item("turn_password", &p.turn_password)?;
    dict.set_item("is_video", p.is_video)?;
    dict.set_item("expires_at", p.expires_at)?;
    dict.set_item("user_id", p.user_id())?;

    let ice = PyList::empty(py);
    for server in p.ice_servers() {
        let entry = PyDict::new(py);
        entry.set_item("urls", server.urls)?;
        entry.set_item("username", server.username)?;
        entry.set_item("credential", server.credential)?;
        ice.append(entry)?;
    }
    dict.set_item("ice_servers", ice)?;

    if let Some(cid) = conversation_id {
        let url = p.ws2_url(cid, &kolibri_net::calls::Ws2ClientInfo::default());
        dict.set_item("ws2_url", url)?;
    }
    Ok(Some(dict.into_any().unbind()))
}

/// ws2 call-signaling client. connects on construction; methods block on the
/// internal runtime.
#[pyclass]
struct CallSignaling {
    rt: Arc<Runtime>,
    inner: Arc<kolibri_net::calls::Ws2Signaling>,
    notif_rx: Mutex<broadcast::Receiver<serde_json::Value>>,
}

#[pymethods]
impl CallSignaling {
    #[new]
    #[pyo3(signature = (url, user_agent = None, proxy = None))]
    fn new(
        py: Python<'_>,
        url: &str,
        user_agent: Option<String>,
        proxy: Option<String>,
    ) -> PyResult<Self> {
        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
        );
        let proxy = match proxy {
            Some(url) => Some(ProxyConfig::parse(&url).map_err(PyValueError::new_err)?),
            None => None,
        };
        let url = url.to_string();
        let rt2 = rt.clone();
        let inner = py
            .allow_threads(move || {
                rt2.block_on(kolibri_net::calls::Ws2Signaling::connect_via(
                    &url,
                    user_agent.as_deref(),
                    proxy.as_ref(),
                ))
            })
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let inner = Arc::new(inner);
        let notif_rx = Mutex::new(inner.notifications());
        Ok(Self {
            rt,
            inner,
            notif_rx,
        })
    }

    fn accept_call(&self, py: Python<'_>) -> PyResult<PyObject> {
        self.run(py, |s| Box::pin(async move { s.accept_call().await }))
    }

    fn hangup(&self, py: Python<'_>, reason: &str) -> PyResult<PyObject> {
        let reason = reason.to_string();
        self.run(py, move |s| {
            Box::pin(async move { s.hangup(&reason).await })
        })
    }

    fn transmit_sdp(
        &self,
        py: Python<'_>,
        participant_id: i64,
        sdp_type: &str,
        sdp: &str,
    ) -> PyResult<PyObject> {
        let (t, d) = (sdp_type.to_string(), sdp.to_string());
        self.run(py, move |s| {
            Box::pin(async move { s.transmit_sdp(participant_id, &t, &d).await })
        })
    }

    fn transmit_candidate(
        &self,
        py: Python<'_>,
        participant_id: i64,
        candidate: &str,
        sdp_mid: &str,
        sdp_mline_index: i64,
    ) -> PyResult<PyObject> {
        let (c, m) = (candidate.to_string(), sdp_mid.to_string());
        self.run(py, move |s| {
            Box::pin(async move {
                s.transmit_candidate(participant_id, &c, &m, sdp_mline_index)
                    .await
            })
        })
    }

    fn change_media_settings(
        &self,
        py: Python<'_>,
        audio: bool,
        video: bool,
        screen: bool,
    ) -> PyResult<PyObject> {
        self.run(py, move |s| {
            Box::pin(async move { s.change_media_settings(audio, video, screen).await })
        })
    }

    /// raw command with a dict of extra fields
    fn send_command(
        &self,
        py: Python<'_>,
        command: &str,
        extra: &Bound<'_, PyAny>,
    ) -> PyResult<PyObject> {
        let extra = py_to_json(extra)?;
        let command = command.to_string();
        self.run(py, move |s| {
            Box::pin(async move { s.send_command(&command, extra).await })
        })
    }

    /// next ws2 notification as a dict, None on timeout
    #[pyo3(signature = (timeout_secs = None))]
    fn next_notification(
        &self,
        py: Python<'_>,
        timeout_secs: Option<f64>,
    ) -> PyResult<Option<PyObject>> {
        let rt = self.rt.clone();
        let result = py.allow_threads(move || {
            let mut rx = self.notif_rx.lock().unwrap();
            rt.block_on(async {
                match timeout_secs {
                    Some(t) => tokio::time::timeout(Duration::from_secs_f64(t), rx.recv())
                        .await
                        .ok()
                        .and_then(|r| r.ok()),
                    None => rx.recv().await.ok(),
                }
            })
        });
        match result {
            Some(v) => Ok(Some(json_to_py(py, &v)?.unbind())),
            None => Ok(None),
        }
    }

    fn is_connected(&self) -> bool {
        self.inner.is_connected()
    }

    fn close(&self) {
        self.inner.close();
    }
}

impl CallSignaling {
    fn run<F>(&self, py: Python<'_>, f: F) -> PyResult<PyObject>
    where
        F: FnOnce(
                Arc<kolibri_net::calls::Ws2Signaling>,
            ) -> std::pin::Pin<
                Box<
                    dyn std::future::Future<
                            Output = Result<serde_json::Value, kolibri_net::calls::CallError>,
                        > + Send,
                >,
            > + Send,
    {
        let rt = self.rt.clone();
        let inner = self.inner.clone();
        let value = py
            .allow_threads(move || rt.block_on(f(inner)))
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(json_to_py(py, &value)?.unbind())
    }
}

/// ws2 `connection` notification => {topology, is_sfu, participants, ice_servers},
/// plus `peer` when my_user_id is given
#[pyfunction]
#[pyo3(signature = (notification, my_user_id = None))]
fn parse_connection(
    py: Python<'_>,
    notification: &Bound<'_, PyAny>,
    my_user_id: Option<i64>,
) -> PyResult<PyObject> {
    let info = kolibri_net::calls::parse_connection(&py_to_json(notification)?);
    let dict = PyDict::new(py);
    dict.set_item("topology", &info.topology)?;
    dict.set_item("is_sfu", info.is_sfu())?;
    dict.set_item("participants", info.participants.clone())?;
    if let Some(uid) = my_user_id {
        dict.set_item("peer", info.peer_of(uid))?;
    }
    let ice = PyList::empty(py);
    for s in &info.ice_servers {
        let entry = PyDict::new(py);
        entry.set_item("urls", s.urls.clone())?;
        entry.set_item("username", &s.username)?;
        entry.set_item("credential", &s.credential)?;
        ice.append(entry)?;
    }
    dict.set_item("ice_servers", ice)?;
    Ok(dict.into_any().unbind())
}

/// ws2 `transmitted-data` notification => {kind: "sdp", type, sdp} or
/// {kind: "candidate", candidate, sdp_mid, sdp_mline_index}, or None
#[pyfunction]
fn parse_transmitted_data(
    py: Python<'_>,
    notification: &Bound<'_, PyAny>,
) -> PyResult<Option<PyObject>> {
    use kolibri_net::calls::TransmittedData;
    let dict = PyDict::new(py);
    match kolibri_net::calls::parse_transmitted_data(&py_to_json(notification)?) {
        Some(TransmittedData::Sdp { sdp_type, sdp }) => {
            dict.set_item("kind", "sdp")?;
            dict.set_item("type", sdp_type)?;
            dict.set_item("sdp", sdp)?;
        }
        Some(TransmittedData::Candidate {
            candidate,
            sdp_mid,
            sdp_mline_index,
        }) => {
            dict.set_item("kind", "candidate")?;
            dict.set_item("candidate", candidate)?;
            dict.set_item("sdp_mid", sdp_mid)?;
            dict.set_item("sdp_mline_index", sdp_mline_index)?;
        }
        None => return Ok(None),
    }
    Ok(Some(dict.into_any().unbind()))
}

#[pymodule]
fn kolibri(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Session>()?;
    m.add_class::<CallSignaling>()?;
    m.add_function(wrap_pyfunction!(auth_mode, m)?)?;
    m.add_function(wrap_pyfunction!(decode_vcp, m)?)?;
    m.add_function(wrap_pyfunction!(parse_connection, m)?)?;
    m.add_function(wrap_pyfunction!(parse_transmitted_data, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
