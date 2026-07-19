//! WARNING: sends a REAL SMS to the given number via api.oneme.ru.
//!
//!     cargo run --example auth_request -- +7XXXXXXXXXX
//!
//! Override device identity with KOLIBRI_DEVICE_ID / KOLIBRI_INSTANCE_ID for a
//! fresh install fingerprint.

use std::time::Duration;

use kolibri_net::protocol::opcodes;
use kolibri_net::{ClientConfig, HandshakeConfig, Session, SessionConfig, UserAgent};
use rmpv::Value;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncBufReadExt, BufReader};

const HOST: &str = "api.oneme.ru";
const PORT: u16 = 443;
const APP_VERSION: &str = "26.20.2";
const BUILD_NUMBER: i64 = 6758;

// APK signature / dex / so digests for the anti-spoof `mode` fingerprint
// (from ChatCacheFingerprint)
const SIGNATURE_DIGEST: &str = "1684414033eb263e2c615f8b7df5ed8793850a07656304997fbf07e9e21e1e93";
const SO_DIGEST: &str = "90e2fb8745b17b42a10182f8d8ac590e3fca5b311e2ce2d5144fa2c18cb3090d";
const DEX_DIGEST: &str = "0a6265f6e5d8231b9cba641f8c40475e6f3baeb06ed41b804b9bf7307aa4214e";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let phone = match std::env::args().nth(1) {
        Some(p) => normalize_phone(&p),
        None => {
            eprintln!("usage: cargo run --example auth_request -- +7XXXXXXXXXX");
            std::process::exit(2);
        }
    };

    let device_id = std::env::var("KOLIBRI_DEVICE_ID")
        .unwrap_or_else(|_| "d1e9c0de-0000-4000-8000-kolibri0001".to_string());
    let instance_id = std::env::var("KOLIBRI_INSTANCE_ID")
        .unwrap_or_else(|_| "i1e9c0de-0000-4000-8000-kolibri0001".to_string());

    let handshake = HandshakeConfig {
        instance_id,
        device_id: device_id.clone(),
        client_session_id: 1_700_000_000,
        user_agent: UserAgent {
            device_type: "ANDROID".to_string(),
            app_version: APP_VERSION.to_string(),
            os_version: "Android 14".to_string(),
            timezone: "Europe/Moscow".to_string(),
            screen: "420dpi 420dpi 1080x2340".to_string(),
            push_device_type: "GCM".to_string(),
            arch: "arm64-v8a".to_string(),
            locale: "ru".to_string(),
            build_number: BUILD_NUMBER,
            device_name: "Xiaomi 23127PN0CG".to_string(),
            device_locale: "ru".to_string(),
        },
    };

    let mut config = SessionConfig::new(ClientConfig::new(HOST, PORT), handshake);
    config.ping_interval = Duration::from_secs(10);
    config.auto_reconnect = false;

    let session = Session::new(config);

    println!("→ connecting to {HOST}:{PORT} …");
    let info = session.connect().await?;
    println!(
        "✓ online. callsSeed={:?} device_name={:?}",
        info.calls_seed, info.device_name
    );

    let calls_seed = info
        .calls_seed
        .ok_or("server did not return callsSeed in handshake")?;

    let mode = compute_mode(calls_seed, &device_id);
    let request = Value::Map(vec![
        (Value::from("phone"), Value::from(phone.clone())),
        (Value::from("type"), Value::from("START_AUTH")),
        (Value::from("language"), Value::from("ru")),
        (Value::from("mode"), Value::Binary(mode)),
    ]);

    println!("→ requesting OTP code for {} …", mask_phone(&phone));
    let response = session
        .request(opcodes::AUTH_REQUEST, &encode(&request))
        .await?;

    if !response.is_ok() {
        return Err(format!("authRequest not ok (cmd={})", response.cmd).into());
    }
    let payload = response.value()?;
    let token = map_str(&payload, "token").ok_or("no token in authRequest response")?;
    println!("✓ code sent. temp token = {token}");

    print!("Enter the SMS code (blank to skip): ");
    use std::io::Write;
    std::io::stdout().flush().ok();
    let mut line = String::new();
    BufReader::new(tokio::io::stdin())
        .read_line(&mut line)
        .await?;
    let code = line.trim();
    if code.is_empty() {
        println!("skipped verification.");
        return Ok(());
    }

    let verify = Value::Map(vec![
        (Value::from("token"), Value::from(token)),
        (Value::from("verifyCode"), Value::from(code)),
        (Value::from("authTokenType"), Value::from("CHECK_CODE")),
    ]);
    println!("→ verifying code …");
    let vresp = session.request(opcodes::AUTH, &encode(&verify)).await?;
    let vpayload = vresp.value()?;
    println!("✓ verify response (cmd={}): {vpayload}", vresp.cmd);

    Ok(())
}

fn normalize_phone(phone: &str) -> String {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    format!("+{digits}")
}

fn mask_phone(phone: &str) -> String {
    if phone.len() <= 5 {
        return "***".to_string();
    }
    format!("{}***{}", &phone[..3], &phone[phone.len() - 2..])
}

// three SHA-256 hashes of (digest || int64_be(callsSeed) || utf8(deviceId)),
// concatenated to 96 bytes
fn compute_mode(calls_seed: i64, device_id: &str) -> Vec<u8> {
    let seed = calls_seed.to_be_bytes();
    let dev = device_id.as_bytes();
    let mut out = Vec::with_capacity(96);
    out.extend(sha256_of(&[&hex(SIGNATURE_DIGEST), &seed, dev]));
    out.extend(sha256_of(&[&hex(DEX_DIGEST), &seed, dev]));
    out.extend(sha256_of(&[&hex(SO_DIGEST), &seed, dev]));
    out
}

fn sha256_of(parts: &[&[u8]]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    for p in parts {
        hasher.update(p);
    }
    hasher.finalize().to_vec()
}

fn hex(s: &str) -> Vec<u8> {
    (0..s.len() / 2)
        .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
        .collect()
}

fn encode(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, value).unwrap();
    out
}

fn map_str(value: &Value, key: &str) -> Option<String> {
    value
        .as_map()?
        .iter()
        .find(|(k, _)| k.as_str() == Some(key))
        .and_then(|(_, v)| v.as_str().map(|s| s.to_string()))
}
