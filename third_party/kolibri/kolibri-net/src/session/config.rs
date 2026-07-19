use std::time::Duration;

use crate::transport::ClientConfig;

/// `userAgent` sub-map of the sessionInit handshake. Host supplies the device
/// values; field names/nesting live here so every client sends the same shape.
#[derive(Debug, Clone)]
pub struct UserAgent {
    pub device_type: String,
    pub app_version: String,
    pub os_version: String,
    pub timezone: String,
    pub screen: String,
    pub push_device_type: String,
    pub arch: String,
    pub locale: String,
    pub build_number: i64,
    pub device_name: String,
    pub device_locale: String,
}

impl UserAgent {
    /// CDN/HTTP User-Agent for media uploads, from the same device fields sent in
    /// the handshake (opcode 6) so both agree.
    /// e.g. `OKMessages/26.20.2 (Android 14; Google Pixel 8; xxhdpi 420dpi 1080x2400)`
    pub fn http_user_agent(&self) -> String {
        format!(
            "OKMessages/{} ({}; {}; {})",
            self.app_version, self.os_version, self.device_name, self.screen
        )
    }
}

/// inputs for the `sessionInit` (opcode 6) payload.
#[derive(Debug, Clone)]
pub struct HandshakeConfig {
    pub instance_id: String,
    pub device_id: String,
    pub client_session_id: i64,
    pub user_agent: UserAgent,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub client: ClientConfig,
    pub handshake: HandshakeConfig,
    /// keepalive ping interval once online
    pub ping_interval: Duration,
    /// `interactive` flag in the ping payload (false = ghost/offline mode)
    pub ping_interactive: bool,
    pub auto_reconnect: bool,
}

impl SessionConfig {
    pub fn new(client: ClientConfig, handshake: HandshakeConfig) -> Self {
        Self {
            client,
            handshake,
            ping_interval: Duration::from_secs(30),
            ping_interactive: true,
            auto_reconnect: true,
        }
    }
}
