//! Proxying the outbound connect. Both schemes hand back a plain TCP stream to
//! the real target, so TLS and everything above it stays the same.
//!
//! - HTTP CONNECT: send `CONNECT host:port`, wait for `200`, then it's a tunnel.
//! - SOCKS5 (RFC 1928 + RFC 1929 user/pass): greeting, optional auth, then a
//!   connect command carrying the target as a domain (proxy resolves it).

use std::io;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProxyKind {
    Http,
    Socks5,
}

/// An outbound proxy. `username`/`password`, if set, do auth — HTTP Basic or
/// SOCKS5 user/pass.
#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub kind: ProxyKind,
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl ProxyConfig {
    /// Parse `scheme://[user:pass@]host:port`. Schemes: `http`, `socks5`,
    /// `socks5h` (both socks variants pass the target as a domain name).
    pub fn parse(url: &str) -> Result<Self, String> {
        let (scheme, rest) = url
            .split_once("://")
            .ok_or_else(|| format!("proxy url has no scheme: {url}"))?;
        let kind = match scheme {
            "http" => ProxyKind::Http,
            "socks5" | "socks5h" => ProxyKind::Socks5,
            other => return Err(format!("unsupported proxy scheme: {other}")),
        };
        let (auth, authority) = match rest.rsplit_once('@') {
            Some((a, h)) => (Some(a), h),
            None => (None, rest),
        };
        let (username, password) = match auth {
            Some(a) => match a.split_once(':') {
                Some((u, p)) => (Some(u.to_string()), Some(p.to_string())),
                None => (Some(a.to_string()), None),
            },
            None => (None, None),
        };
        let (host, port) = authority
            .rsplit_once(':')
            .ok_or_else(|| format!("proxy url has no port: {url}"))?;
        let port: u16 = port
            .parse()
            .map_err(|_| format!("bad proxy port: {port}"))?;
        Ok(ProxyConfig {
            kind,
            host: host.to_string(),
            port,
            username,
            password,
        })
    }
}

/// Open a TCP stream to `(target_host, target_port)`, directly or through
/// `proxy`. `connect_timeout` covers the lot, proxy handshake included.
pub async fn connect_tcp(
    target_host: &str,
    target_port: u16,
    connect_timeout: Duration,
    proxy: Option<&ProxyConfig>,
) -> io::Result<TcpStream> {
    timeout(connect_timeout, async {
        match proxy {
            None => {
                let tcp = TcpStream::connect((target_host, target_port)).await?;
                tcp.set_nodelay(true).ok();
                Ok(tcp)
            }
            Some(p) => {
                let mut tcp = TcpStream::connect((p.host.as_str(), p.port)).await?;
                tcp.set_nodelay(true).ok();
                match p.kind {
                    ProxyKind::Http => http_connect(&mut tcp, target_host, target_port, p).await?,
                    ProxyKind::Socks5 => {
                        socks5_connect(&mut tcp, target_host, target_port, p).await?
                    }
                }
                Ok(tcp)
            }
        }
    })
    .await
    .unwrap_or_else(|_| {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "proxy connect timed out",
        ))
    })
}

async fn http_connect(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    proxy: &ProxyConfig,
) -> io::Result<()> {
    let mut req = format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n");
    if let Some(user) = &proxy.username {
        let pass = proxy.password.as_deref().unwrap_or("");
        let token = base64_encode(format!("{user}:{pass}").as_bytes());
        req.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
    }
    req.push_str("Proxy-Connection: keep-alive\r\n\r\n");
    stream.write_all(req.as_bytes()).await?;
    stream.flush().await?;

    let mut buf = Vec::with_capacity(256);
    let mut byte = [0u8; 1];
    loop {
        let n = stream.read(&mut byte).await?;
        if n == 0 {
            return Err(proxy_err("proxy closed the connection during CONNECT"));
        }
        buf.push(byte[0]);
        if buf.ends_with(b"\r\n\r\n") {
            break;
        }
        if buf.len() > 8192 {
            return Err(proxy_err("proxy CONNECT response too long"));
        }
    }

    let head = String::from_utf8_lossy(&buf);
    let status = head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(0);
    if status != 200 {
        return Err(proxy_err(&format!("proxy CONNECT failed: {status}")));
    }
    Ok(())
}

async fn socks5_connect(
    stream: &mut TcpStream,
    host: &str,
    port: u16,
    proxy: &ProxyConfig,
) -> io::Result<()> {
    let has_auth = proxy.username.is_some();
    if has_auth {
        stream.write_all(&[0x05, 0x02, 0x00, 0x02]).await?;
    } else {
        stream.write_all(&[0x05, 0x01, 0x00]).await?;
    }
    stream.flush().await?;

    let mut method = [0u8; 2];
    stream.read_exact(&mut method).await?;
    if method[0] != 0x05 {
        return Err(proxy_err("not a SOCKS5 proxy"));
    }
    match method[1] {
        0x00 => {}
        0x02 => socks5_userpass(stream, proxy).await?,
        0xFF => return Err(proxy_err("SOCKS5 proxy rejected auth methods")),
        other => return Err(proxy_err(&format!("SOCKS5 unexpected method {other}"))),
    }

    let host_bytes = host.as_bytes();
    if host_bytes.len() > 255 {
        return Err(proxy_err("SOCKS5 target host too long"));
    }
    let mut req = Vec::with_capacity(7 + host_bytes.len());
    req.extend_from_slice(&[0x05, 0x01, 0x00, 0x03]);
    req.push(host_bytes.len() as u8);
    req.extend_from_slice(host_bytes);
    req.extend_from_slice(&port.to_be_bytes());
    stream.write_all(&req).await?;
    stream.flush().await?;

    let mut head = [0u8; 4];
    stream.read_exact(&mut head).await?;
    if head[1] != 0x00 {
        return Err(proxy_err(&format!(
            "SOCKS5 connect failed (reply {})",
            head[1]
        )));
    }
    let skip = match head[3] {
        0x01 => 4,
        0x04 => 16,
        0x03 => {
            let mut len = [0u8; 1];
            stream.read_exact(&mut len).await?;
            len[0] as usize
        }
        other => return Err(proxy_err(&format!("SOCKS5 bad address type {other}"))),
    };
    let mut rest = vec![0u8; skip + 2];
    stream.read_exact(&mut rest).await?;
    Ok(())
}

async fn socks5_userpass(stream: &mut TcpStream, proxy: &ProxyConfig) -> io::Result<()> {
    let user = proxy.username.as_deref().unwrap_or("");
    let pass = proxy.password.as_deref().unwrap_or("");
    if user.len() > 255 || pass.len() > 255 {
        return Err(proxy_err("SOCKS5 credentials too long"));
    }
    let mut msg = Vec::with_capacity(3 + user.len() + pass.len());
    msg.push(0x01);
    msg.push(user.len() as u8);
    msg.extend_from_slice(user.as_bytes());
    msg.push(pass.len() as u8);
    msg.extend_from_slice(pass.as_bytes());
    stream.write_all(&msg).await?;
    stream.flush().await?;

    let mut reply = [0u8; 2];
    stream.read_exact(&mut reply).await?;
    if reply[1] != 0x00 {
        return Err(proxy_err("SOCKS5 auth rejected"));
    }
    Ok(())
}

fn proxy_err(msg: &str) -> io::Error {
    io::Error::other(msg.to_string())
}

fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(ALPHABET[((n >> 18) & 63) as usize] as char);
        out.push(ALPHABET[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{base64_encode, ProxyConfig, ProxyKind};

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"user:pass"), "dXNlcjpwYXNz");
    }

    #[test]
    fn parses_http_with_auth() {
        let p = ProxyConfig::parse("http://bob:secret@10.0.0.1:8080").unwrap();
        assert_eq!(p.kind, ProxyKind::Http);
        assert_eq!(p.host, "10.0.0.1");
        assert_eq!(p.port, 8080);
        assert_eq!(p.username.as_deref(), Some("bob"));
        assert_eq!(p.password.as_deref(), Some("secret"));
    }

    #[test]
    fn parses_socks5_no_auth() {
        let p = ProxyConfig::parse("socks5://127.0.0.1:1080").unwrap();
        assert_eq!(p.kind, ProxyKind::Socks5);
        assert_eq!(p.port, 1080);
        assert!(p.username.is_none());
    }

    #[test]
    fn rejects_bad_scheme_and_missing_port() {
        assert!(ProxyConfig::parse("ftp://x:1").is_err());
        assert!(ProxyConfig::parse("http://host").is_err());
    }
}
