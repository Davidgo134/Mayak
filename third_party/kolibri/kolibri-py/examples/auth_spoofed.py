import sys
import kolibri

AUTH_REQUEST = 17

PROFILES = {
    "pixel8": dict(
        device_id="spoof-pixel8-0001",
        device_type="ANDROID",
        device_name="Google Pixel 8 Pro",
        os_version="Android 14",
        arch="arm64-v8a",
        screen="480dpi 480dpi 1344x2992",
        timezone="Europe/Moscow",
        locale="ru",
        device_locale="ru",
    ),
    "samsung_s24": dict(
        device_id="spoof-s24-0002",
        device_type="ANDROID",
        device_name="Samsung SM-S928B",
        os_version="Android 14",
        arch="arm64-v8a",
        screen="500dpi 500dpi 1440x3120",
        timezone="Asia/Yekaterinburg",
        locale="ru",
        device_locale="ru",
    ),
}

PROFILE = PROFILES["pixel8"]

if len(sys.argv) < 2:
    sys.exit("usage: python examples/auth_spoofed.py +7XXXXXXXXXX")

phone = "+" + "".join(c for c in sys.argv[1] if c.isdigit())

s = kolibri.Session(
    "api.oneme.ru",
    443,
    app_version="26.20.2",
    build_number=6758,
    **PROFILE,
)

info = s.connect()
print(f"handshake ok as {PROFILE['device_name']!r} -> {s.state()}")
print("calls_seed:", info["calls_seed"])
if info["calls_seed"] is None:
    sys.exit("no callsSeed returned — cannot build fingerprint (is device_type ANDROID?)")

mode = kolibri.auth_mode(info["calls_seed"], PROFILE["device_id"])

resp = s.request(AUTH_REQUEST, {
    "phone": phone,
    "type": "START_AUTH",
    "language": "ru",
    "mode": mode,
})
print("✓ code sent to", phone)
print("  temp token:", resp["token"][:32], "…")

s.disconnect()
