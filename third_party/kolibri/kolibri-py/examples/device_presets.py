"""Realistic Android device profiles for the sessionInit handshake spoof.

Values (model, Android version, screen) are real device fingerprints; keep
device_type ANDROID (IOS returns no callsSeed). app_version/build_number are the
real oneme release. Timezone/locale default to RU to stay consistent with a
Russian account - a Moscow number reporting Asia/Tokyo looks off.

Pick ONE profile per account and keep it stable across runs (the login token is
tied to a device_id; don't randomize every connect). Use a fixed device_id:

    import secrets; DEVICE_ID = secrets.token_hex(8)   # 16 hex chars (Android ID)

    import kolibri
    from device_presets import random_device
    dev = random_device()
    s = kolibri.Session("api.oneme.ru", 443, device_id=DEVICE_ID, **dev)
"""

import hashlib
import random

# (model, Android version, screen) - real fingerprints, RU-popular first.
_DEVICES = [
    ("Xiaomi 13 Pro", "Android 13", "xxhdpi 460dpi 1440x3200"),
    ("Redmi Note 12 Pro", "Android 13", "xxhdpi 395dpi 1080x2400"),
    ("Redmi Note 11 Pro", "Android 12", "xxhdpi 420dpi 1080x2400"),
    ("POCO X5 Pro", "Android 12", "xxhdpi 395dpi 1080x2400"),
    ("Samsung Galaxy S24 Ultra", "Android 14", "xxhdpi 450dpi 1440x3120"),
    ("Samsung Galaxy S23", "Android 14", "xxhdpi 425dpi 1080x2340"),
    ("Samsung Galaxy A54", "Android 14", "xxhdpi 400dpi 1080x2340"),
    ("Samsung Galaxy A52", "Android 13", "xxhdpi 410dpi 1080x2400"),
    ("Realme GT Master Edition", "Android 13", "xxhdpi 400dpi 1080x2400"),
    ("realme 11 Pro", "Android 13", "xxhdpi 400dpi 1080x2412"),
    ("HONOR 90", "Android 13", "xxhdpi 400dpi 1200x2664"),
    ("HONOR X9b", "Android 13", "xxhdpi 400dpi 1200x2652"),
    ("TECNO CAMON 20", "Android 13", "xhdpi 320dpi 1080x2400"),
    ("OnePlus 12", "Android 14", "xxhdpi 450dpi 1440x3168"),
    ("Google Pixel 8", "Android 14", "xxhdpi 420dpi 1080x2400"),
    ("Nothing Phone (2)", "Android 14", "xxhdpi 400dpi 1080x2412"),
]

# Real Russian timezones.
_TIMEZONES = [
    "Europe/Moscow",
    "Europe/Samara",
    "Asia/Yekaterinburg",
    "Asia/Novosibirsk",
    "Asia/Krasnoyarsk",
]


def random_device(locale: str = "ru") -> dict:
    name, os_version, screen = random.choice(_DEVICES)
    return _profile(name, os_version, screen, random.choice(_TIMEZONES), locale)


def by_name(model: str, locale: str = "ru") -> dict:
    for name, os_version, screen in _DEVICES:
        if name.lower() == model.lower():
            return _profile(name, os_version, screen, random.choice(_TIMEZONES), locale)
    raise KeyError(f"no preset for {model!r}")


def stable_device(device_id: str, locale: str = "ru") -> dict:
    """Deterministic profile for a device_id - same id always maps to the same
    device, so the spoofed identity stays consistent across runs."""
    h = int(hashlib.sha256(device_id.encode()).hexdigest(), 16)
    name, os_version, screen = _DEVICES[h % len(_DEVICES)]
    tz = _TIMEZONES[(h // len(_DEVICES)) % len(_TIMEZONES)]
    return _profile(name, os_version, screen, tz, locale)


def _profile(name, os_version, screen, timezone, locale) -> dict:
    return dict(
        device_type="ANDROID",
        device_name=name,
        os_version=os_version,
        screen=screen,
        arch="arm64-v8a",
        timezone=timezone,
        locale=locale,
        device_locale=locale,
        push_device_type="GCM",
        app_version="26.20.2",
        build_number=6758,
    )
