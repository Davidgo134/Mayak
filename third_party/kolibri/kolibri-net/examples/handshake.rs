//! Runs the sessionInit handshake against production and disconnects. No SMS,
//! no account action.
//!
//!     cargo run --example handshake

use std::time::Duration;

use kolibri_net::{ClientConfig, HandshakeConfig, Session, SessionConfig, UserAgent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let handshake = HandshakeConfig {
        instance_id: "i1e9c0de-0000-4000-8000-kolibri0001".to_string(),
        device_id: "d1e9c0de-0000-4000-8000-kolibri0001".to_string(),
        client_session_id: 1_700_000_000,
        user_agent: UserAgent {
            device_type: "ANDROID".to_string(),
            app_version: "26.20.2".to_string(),
            os_version: "Android 14".to_string(),
            timezone: "Europe/Moscow".to_string(),
            screen: "420dpi 420dpi 1080x2340".to_string(),
            push_device_type: "GCM".to_string(),
            arch: "arm64-v8a".to_string(),
            locale: "ru".to_string(),
            build_number: 6758,
            device_name: "Xiaomi 23127PN0CG".to_string(),
            device_locale: "ru".to_string(),
        },
    };

    let mut config = SessionConfig::new(ClientConfig::new("api.oneme.ru", 443), handshake);
    config.auto_reconnect = false;

    println!("→ connecting to api.oneme.ru:443 …");
    let session = Session::new(config);

    let info = tokio::time::timeout(Duration::from_secs(20), session.connect()).await??;

    println!("✓ handshake OK");
    println!("  callsSeed   = {:?}", info.calls_seed);
    println!("  device_name = {:?}", info.device_name);
    println!("  full payload = {}", info.payload);

    session.disconnect();
    Ok(())
}
