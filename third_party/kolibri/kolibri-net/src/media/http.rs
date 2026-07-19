use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{MediaError, ProgressFn};
use crate::transport::proxy::{connect_tcp, ProxyConfig};
use crate::transport::tls::build_connector;

pub(crate) struct ParsedUrl {
    pub https: bool,
    pub host: String,
    pub port: u16,
    pub path: String,
}

impl ParsedUrl {
    pub(crate) fn parse(url: &str) -> Result<Self, MediaError> {
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| MediaError::Url(format!("no scheme: {url}")))?;
        let https = match scheme {
            "https" => true,
            "http" => false,
            other => return Err(MediaError::Url(format!("unsupported scheme: {other}"))),
        };
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, "/"),
        };
        let (host, port) = match authority.rsplit_once(':') {
            Some((h, p)) if p.parse::<u16>().is_ok() => (h.to_string(), p.parse().unwrap()),
            _ => (authority.to_string(), if https { 443 } else { 80 }),
        };
        Ok(ParsedUrl {
            https,
            host,
            port,
            path: if path.is_empty() {
                "/".to_string()
            } else {
                path.to_string()
            },
        })
    }
}

/// status code + dechunked body bytes
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

// fresh connection per request, one request/response. no general HTTP client:
// the CDN wants exact header shapes.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn request(
    url: &ParsedUrl,
    method: &str,
    headers: &[(&str, String)],
    body: &[u8],
    insecure: bool,
    proxy: Option<&ProxyConfig>,
    timeout: Duration,
    progress: Option<&ProgressFn>,
    progress_total: u64,
) -> Result<HttpResponse, MediaError> {
    let head = build_head(method, &url.path, headers);
    let tcp = connect_tcp(&url.host, url.port, timeout, proxy).await?;

    if url.https {
        let connector = build_connector(insecure).map_err(|e| MediaError::Tls(e.to_string()))?;
        let name = rustls::pki_types::ServerName::try_from(url.host.clone())
            .map_err(|e| MediaError::Tls(e.to_string()))?;
        let tls = connector
            .connect(name, tcp)
            .await
            .map_err(|e| MediaError::Tls(e.to_string()))?;
        exchange(tls, &head, body, timeout, progress, progress_total).await
    } else {
        exchange(tcp, &head, body, timeout, progress, progress_total).await
    }
}

// like [`request`], but the body is streamed: `prefix` bytes, then `body_len`
// bytes pulled from `reader` (e.g. a file, off disk — never fully in RAM), then
// `suffix` bytes. Caller sets Content-Length to prefix+body_len+suffix.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn request_streaming<R: AsyncReadExt + Unpin>(
    url: &ParsedUrl,
    method: &str,
    headers: &[(&str, String)],
    prefix: &[u8],
    reader: R,
    body_len: u64,
    suffix: &[u8],
    insecure: bool,
    proxy: Option<&ProxyConfig>,
    timeout: Duration,
    progress: Option<&ProgressFn>,
    progress_total: u64,
) -> Result<HttpResponse, MediaError> {
    let head = build_head(method, &url.path, headers);
    let tcp = connect_tcp(&url.host, url.port, timeout, proxy).await?;

    if url.https {
        let connector = build_connector(insecure).map_err(|e| MediaError::Tls(e.to_string()))?;
        let name = rustls::pki_types::ServerName::try_from(url.host.clone())
            .map_err(|e| MediaError::Tls(e.to_string()))?;
        let tls = connector
            .connect(name, tcp)
            .await
            .map_err(|e| MediaError::Tls(e.to_string()))?;
        exchange_streaming(
            tls,
            &head,
            prefix,
            reader,
            body_len,
            suffix,
            timeout,
            progress,
            progress_total,
        )
        .await
    } else {
        exchange_streaming(
            tcp,
            &head,
            prefix,
            reader,
            body_len,
            suffix,
            timeout,
            progress,
            progress_total,
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
async fn exchange_streaming<S, R>(
    mut stream: S,
    head: &[u8],
    prefix: &[u8],
    mut reader: R,
    body_len: u64,
    suffix: &[u8],
    timeout: Duration,
    progress: Option<&ProgressFn>,
    progress_total: u64,
) -> Result<HttpResponse, MediaError>
where
    S: AsyncReadExt + AsyncWriteExt + Unpin,
    R: AsyncReadExt + Unpin,
{
    stream.write_all(head).await?;

    let mut sent = 0u64;
    let report = |sent: u64| {
        if let Some(cb) = progress {
            cb(sent, progress_total.max(sent));
        }
    };

    if !prefix.is_empty() {
        stream.write_all(prefix).await?;
        sent += prefix.len() as u64;
        report(sent);
    }

    let mut remaining = body_len;
    let mut buf = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let want = (buf.len() as u64).min(remaining) as usize;
        let n = reader.read(&mut buf[..want]).await?;
        if n == 0 {
            break;
        }
        stream.write_all(&buf[..n]).await?;
        sent += n as u64;
        remaining -= n as u64;
        report(sent);
    }

    if !suffix.is_empty() {
        stream.write_all(suffix).await?;
        sent += suffix.len() as u64;
        report(sent);
    }
    stream.flush().await?;

    read_response(&mut stream, timeout).await
}

fn build_head(method: &str, path: &str, headers: &[(&str, String)]) -> Vec<u8> {
    let mut s = format!("{method} {path} HTTP/1.1\r\n");
    for (k, v) in headers {
        s.push_str(k);
        s.push_str(": ");
        s.push_str(v);
        s.push_str("\r\n");
    }
    s.push_str("\r\n");
    s.into_bytes()
}

async fn exchange<S: AsyncReadExt + AsyncWriteExt + Unpin>(
    mut stream: S,
    head: &[u8],
    body: &[u8],
    timeout: Duration,
    progress: Option<&ProgressFn>,
    progress_total: u64,
) -> Result<HttpResponse, MediaError> {
    stream.write_all(head).await?;

    let mut sent = 0u64;
    for chunk in body.chunks(64 * 1024) {
        stream.write_all(chunk).await?;
        sent += chunk.len() as u64;
        if let Some(cb) = progress {
            cb(sent, progress_total.max(sent));
        }
    }
    stream.flush().await?;

    read_response(&mut stream, timeout).await
}

async fn read_response<S: AsyncReadExt + Unpin>(
    stream: &mut S,
    timeout: Duration,
) -> Result<HttpResponse, MediaError> {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 16 * 1024];
    loop {
        let n = tokio::time::timeout(timeout, stream.read(&mut tmp))
            .await
            .map_err(|_| MediaError::Timeout)??;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(resp) = try_parse(&buf, false)? {
            return Ok(resp);
        }
    }
    try_parse(&buf, true)?.ok_or(MediaError::Incomplete)
}

fn try_parse(buf: &[u8], at_close: bool) -> Result<Option<HttpResponse>, MediaError> {
    let Some(header_end) = find_subslice(buf, b"\r\n\r\n").map(|p| p + 4) else {
        return Ok(None);
    };
    let header_str = String::from_utf8_lossy(&buf[..header_end]);
    let mut lines = header_str.split("\r\n");
    let status = lines
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);

    let mut content_length: Option<usize> = None;
    let mut chunked = false;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            let val = v.trim();
            if key == "content-length" {
                content_length = val.parse().ok();
            } else if key == "transfer-encoding" && val.to_ascii_lowercase().contains("chunked") {
                chunked = true;
            }
        }
    }

    let body_bytes = &buf[header_end..];
    if chunked {
        if !at_close && find_subslice(body_bytes, b"0\r\n\r\n").is_none() {
            return Ok(None);
        }
        return Ok(Some(HttpResponse {
            status,
            body: decode_chunked(body_bytes),
        }));
    }
    if let Some(cl) = content_length {
        if !at_close && body_bytes.len() < cl {
            return Ok(None);
        }
        let end = cl.min(body_bytes.len());
        return Ok(Some(HttpResponse {
            status,
            body: body_bytes[..end].to_vec(),
        }));
    }
    if at_close {
        Ok(Some(HttpResponse {
            status,
            body: body_bytes.to_vec(),
        }))
    } else {
        Ok(None)
    }
}

fn decode_chunked(body: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let Some(line_end) = find_subslice(&body[i..], b"\r\n").map(|p| i + p) else {
            break;
        };
        let size_str = String::from_utf8_lossy(&body[i..line_end]);
        let size_hex = size_str.split(';').next().unwrap_or("").trim();
        if size_hex.is_empty() {
            i = line_end + 2;
            continue;
        }
        let Ok(size) = usize::from_str_radix(size_hex, 16) else {
            break;
        };
        if size == 0 {
            break;
        }
        let data_start = line_end + 2;
        if data_start + size > body.len() {
            break;
        }
        out.extend_from_slice(&body[data_start..data_start + size]);
        i = data_start + size;
        if i + 2 <= body.len() && &body[i..i + 2] == b"\r\n" {
            i += 2;
        }
    }
    out
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
