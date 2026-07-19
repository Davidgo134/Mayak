//! Verify an OTP code using the temp token from the `auth_request` example.
//!
//!     cargo run --example auth_verify -- <TOKEN> <CODE>

use std::time::Duration;

use kolibri_net::protocol::opcodes;
use kolibri_net::{ClientConfig, HandshakeConfig, Session, SessionConfig, UserAgent};
use rmpv::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let token = args.next().ok_or("usage: auth_verify -- <TOKEN> <CODE>")?;
    let code = args.next().ok_or("usage: auth_verify -- <TOKEN> <CODE>")?;

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

    println!("→ connecting …");
    let session = Session::new(config);
    tokio::time::timeout(Duration::from_secs(20), session.connect()).await??;
    println!("✓ online, verifying code …");

    let verify = Value::Map(vec![
        (Value::from("token"), Value::from(token)),
        (Value::from("verifyCode"), Value::from(code)),
        (Value::from("authTokenType"), Value::from("CHECK_CODE")),
    ]);
    let mut buf = Vec::new();
    rmpv::encode::write_value(&mut buf, &verify).unwrap();

    let resp = session.request(opcodes::AUTH, &buf).await?;
    println!("✓ response (cmd={}):\n{}", resp.cmd, resp.value()?);

    session.disconnect();
    Ok(())
}
