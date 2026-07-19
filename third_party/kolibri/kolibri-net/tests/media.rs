//! Media uploader tests against a local mock HTTP server (plain HTTP, no TLS)
//! exercising the request shaping, response parsing, and parallel chunking.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use kolibri_net::media::{upload_file, upload_file_path, upload_video, upload_video_path};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Read one HTTP/1.1 request; return (method, content_range, body).
async fn read_request(stream: &mut TcpStream) -> (String, Option<String>, Vec<u8>) {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    // Read until headers complete.
    let header_end = loop {
        let n = stream.read(&mut tmp).await.unwrap();
        if n == 0 {
            return (String::new(), None, Vec::new());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(p) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
    };
    let header_str = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = header_str.split("\r\n");
    let method = lines
        .next()
        .and_then(|l| l.split_whitespace().next())
        .unwrap_or("")
        .to_string();

    let mut content_length = 0usize;
    let mut content_range = None;
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim().to_ascii_lowercase();
            if key == "content-length" {
                content_length = v.trim().parse().unwrap_or(0);
            } else if key == "content-range" {
                content_range = Some(v.trim().to_string());
            }
        }
    }

    let mut body = buf[header_end..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp).await.unwrap();
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }
    (method, content_range, body)
}

#[tokio::test]
async fn upload_file_posts_body_with_content_range() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (method, range, body) = read_request(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
        stream.flush().await.unwrap();
        (method, range, body)
    });

    let data = vec![0xAB; 5000];
    let url = format!("http://127.0.0.1:{}/upload", addr.port());
    let resp = upload_file(&url, &data, "clip.bin", false, None, None, "test-ua")
        .await
        .unwrap();

    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"ok");

    let (method, range, body) = server.await.unwrap();
    assert_eq!(method, "POST");
    assert_eq!(range.as_deref(), Some("bytes 0-4999/5000"));
    assert_eq!(body, data);
}

#[tokio::test]
async fn upload_file_path_streams_body_off_disk() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let (method, range, body) = read_request(&mut stream).await;
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok")
            .await
            .unwrap();
        stream.flush().await.unwrap();
        (method, range, body)
    });

    let data = vec![0xCD; 5000];
    let path = std::env::temp_dir().join(format!("kolibri_upload_{}.bin", addr.port()));
    std::fs::write(&path, &data).unwrap();
    let url = format!("http://127.0.0.1:{}/upload", addr.port());
    let resp = upload_file_path(
        &url,
        path.to_str().unwrap(),
        "clip.bin",
        None,
        None,
        false,
        None,
        None,
        "test-ua",
    )
    .await
    .unwrap();

    assert_eq!(resp.status, 200);
    assert_eq!(resp.body, b"ok");

    let (method, range, body) = server.await.unwrap();
    assert_eq!(method, "POST");
    assert_eq!(range.as_deref(), Some("bytes 0-4999/5000"));
    assert_eq!(body, data);
    let _ = std::fs::remove_file(&path);
}

#[tokio::test]
async fn upload_video_chunks_in_parallel_and_covers_all_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let received = Arc::new(AtomicUsize::new(0));
    let posts = Arc::new(AtomicUsize::new(0));
    let received2 = received.clone();
    let posts2 = posts.clone();

    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let received = received2.clone();
            let posts = posts2.clone();
            tokio::spawn(async move {
                let (method, _range, body) = read_request(&mut stream).await;
                let resp: &[u8] = if method == "GET" {
                    // resume offset 0
                    b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n0"
                } else {
                    received.fetch_add(body.len(), Ordering::SeqCst);
                    posts.fetch_add(1, Ordering::SeqCst);
                    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
                };
                stream.write_all(resp).await.ok();
                stream.flush().await.ok();
            });
        }
    });

    let total = 5 * 1024 * 1024; // 5 MB
    let data = vec![0x7F; total];
    let url = format!("http://127.0.0.1:{}/video", addr.port());

    let ok = upload_video(&url, data, 2 * 1024 * 1024, 4, false, None, None)
        .await
        .unwrap();

    assert!(ok);
    // 5 MB / 2 MB chunk = 3 chunks, all bytes delivered.
    assert_eq!(posts.load(Ordering::SeqCst), 3);
    assert_eq!(received.load(Ordering::SeqCst), total);

    server.abort();
    let _ = tokio::time::timeout(Duration::from_millis(100), server).await;
}

#[tokio::test]
async fn upload_video_path_reads_chunks_off_disk() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let received = Arc::new(AtomicUsize::new(0));
    let posts = Arc::new(AtomicUsize::new(0));
    let received2 = received.clone();
    let posts2 = posts.clone();

    let server = tokio::spawn(async move {
        loop {
            let (mut stream, _) = match listener.accept().await {
                Ok(v) => v,
                Err(_) => break,
            };
            let received = received2.clone();
            let posts = posts2.clone();
            tokio::spawn(async move {
                let (method, _range, body) = read_request(&mut stream).await;
                let resp: &[u8] = if method == "GET" {
                    b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n0"
                } else {
                    received.fetch_add(body.len(), Ordering::SeqCst);
                    posts.fetch_add(1, Ordering::SeqCst);
                    b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n"
                };
                stream.write_all(resp).await.ok();
                stream.flush().await.ok();
            });
        }
    });

    let total = 5 * 1024 * 1024; // 5 MB
    let data = vec![0x3C; total];
    let path = std::env::temp_dir().join(format!("kolibri_video_{}.bin", addr.port()));
    std::fs::write(&path, &data).unwrap();
    let url = format!("http://127.0.0.1:{}/video", addr.port());

    let ok = upload_video_path(
        &url,
        path.to_str().unwrap(),
        2 * 1024 * 1024,
        4,
        false,
        None,
        None,
    )
    .await
    .unwrap();

    assert!(ok);
    assert_eq!(posts.load(Ordering::SeqCst), 3);
    assert_eq!(received.load(Ordering::SeqCst), total);

    server.abort();
    let _ = tokio::time::timeout(Duration::from_millis(100), server).await;
    let _ = std::fs::remove_file(&path);
}
