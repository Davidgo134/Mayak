//! Hand-rolled HTTP(S) client for CDN uploads (reuses the transport's tokio +
//! rustls). CDN wants exact request shapes: single-POST files, multipart photos,
//! resumable parallel-chunk video. Control plane (upload URL, send message) stays
//! on the main protocol socket via [`crate::transport`].

mod http;
mod upload;

use std::sync::Arc;
use thiserror::Error;

pub use http::HttpResponse;
pub use upload::{
    content_type_for_filename, upload_file, upload_file_path, upload_photo, upload_photo_path,
    upload_video, upload_video_path,
};

/// `(bytes_sent, total_bytes)`
pub type ProgressFn = Arc<dyn Fn(u64, u64) + Send + Sync>;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("invalid url: {0}")]
    Url(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("tls error: {0}")]
    Tls(String),
    #[error("http status {0}")]
    Http(u16),
    #[error("request timed out")]
    Timeout,
    #[error("incomplete http response")]
    Incomplete,
}
