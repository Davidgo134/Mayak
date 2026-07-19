"""Answering music bot: logs in, waits for an incoming call, auto-accepts, and
plays an audio track into it.

Signaling goes through kolibri (main socket + ws2); the WebRTC media is aiortc.
This is a P2P (1:1) callee - it answers the caller's offer and streams the track.

    KOLIBRI_TRACK=song.opus KOLIBRI_PHONE=+7... python examples/call_bot.py
    # a LOGIN token is printed on first run; export KOLIBRI_LOGIN_TOKEN to skip SMS

Then call this account from another device to hear the track. Turn VPN off - it
breaks the UDP media path.
"""

import asyncio
import os
import sys

import kolibri
from device_presets import stable_device
from aiortc import (
    RTCConfiguration,
    RTCIceServer,
    RTCPeerConnection,
    RTCSessionDescription,
)
from aiortc.contrib.media import MediaBlackhole, MediaPlayer
from aiortc.sdp import candidate_from_sdp

HOST = "api.oneme.ru"
TRACK = os.environ.get("KOLIBRI_TRACK", os.path.join(os.path.dirname(__file__), "track.wav"))
IDENTITY_FILE = os.path.join(os.path.dirname(__file__), ".bot_identity.json")
NOTIF_CALL_START = 137
AUTH_REQUEST, AUTH, LOGIN = 17, 18, 19


def log(*a):
    print("[bot]", *a, flush=True)


def load_identity() -> dict:
    """Stable device identity in the real client format: device_id = 16 hex chars
    (Android ID), instance_id = UUID. Persisted so the login token stays valid."""
    import json
    import secrets
    import uuid

    env_id = os.environ.get("KOLIBRI_DEVICE_ID")
    if os.path.exists(IDENTITY_FILE):
        with open(IDENTITY_FILE) as f:
            ident = json.load(f)
    else:
        ident = {"device_id": secrets.token_hex(8), "instance_id": str(uuid.uuid4())}
        with open(IDENTITY_FILE, "w") as f:
            json.dump(ident, f)
    if env_id:
        ident["device_id"] = env_id
    return ident


def login(session, device_id: str):
    info = session.connect()
    seed = info["calls_seed"]
    log("online, calls_seed", seed)

    token = os.environ.get("KOLIBRI_LOGIN_TOKEN")
    if not token:
        phone = os.environ.get("KOLIBRI_PHONE") or input("bot phone (+7...): ").strip()
        mode = kolibri.auth_mode(seed, device_id)
        r = session.request(AUTH_REQUEST, {"phone": phone, "type": "START_AUTH",
                                           "language": "ru", "mode": mode})
        code = input("SMS code: ").strip()
        v = session.request(AUTH, {"token": r["token"], "verifyCode": code,
                                   "authTokenType": "CHECK_CODE"})
        token = v["tokenAttrs"]["LOGIN"]["token"]
        log("login token (export KOLIBRI_LOGIN_TOKEN to skip SMS next time):")
        log("  " + token)

    mode = kolibri.auth_mode(seed, device_id)
    session.request(LOGIN, {
        "token": token,
        "interactive": True,
        "exp": {"chatsCountGroups": bytes([0x0B, 0x32])},
        "chatCacheFingerprint": mode,
        "presenceSync": -1,
        "chatsSync": -1,
    })
    log("logged in")


def ice_servers(entries) -> list:
    servers = []
    for s in entries:
        kw = {"urls": s["urls"]}
        if s.get("username"):
            kw["username"] = s["username"]
        if s.get("credential"):
            kw["credential"] = s["credential"]
        servers.append(RTCIceServer(**kw))
    return servers


def to_candidate(candidate, sdp_mid, sdp_mline_index):
    if candidate.startswith("candidate:"):
        candidate = candidate[len("candidate:"):]
    cand = candidate_from_sdp(candidate)
    cand.sdpMid = sdp_mid
    cand.sdpMLineIndex = sdp_mline_index
    return cand


def local_candidates(sdp):
    """(candidate, sdpMid, sdpMLineIndex) tuples from our answer, for trickling."""
    out = []
    mid = None
    mline = -1
    for line in sdp.splitlines():
        if line.startswith("m="):
            mline += 1
            mid = None
        elif line.startswith("a=mid:"):
            mid = line[len("a=mid:"):]
        elif line.startswith("a=candidate:"):
            out.append((line[2:], mid or "0", max(mline, 0)))
    return out


async def handle_call(sig, params):
    my_calls_id = params["user_id"]
    peer_id = None
    pc = None
    blackhole = MediaBlackhole()

    send_video = os.environ.get("KOLIBRI_SEND_VIDEO") == "1"
    player = MediaPlayer(TRACK)
    has_audio = player.audio is not None
    has_video = send_video and player.video is not None

    def build_pc(servers):
        nonlocal pc
        pc = RTCPeerConnection(RTCConfiguration(iceServers=ice_servers(servers)))

        @pc.on("connectionstatechange")
        async def _():
            log("pc:", pc.connectionState)

        @pc.on("track")
        def _(track):
            blackhole.addTrack(track)

        if has_audio:
            pc.addTrack(player.audio)
        if has_video:
            pc.addTrack(player.video)

    await asyncio.to_thread(sig.accept_call)
    await asyncio.to_thread(sig.change_media_settings, has_audio, has_video, False)
    log("accepted; playing", os.path.basename(TRACK))

    while True:
        n = await asyncio.to_thread(sig.next_notification, 60.0)
        if n is None:
            if not sig.is_connected():
                break
            continue
        name = n.get("notification")

        if name == "connection":
            info = kolibri.parse_connection(n, my_user_id=my_calls_id)
            if info["peer"] is not None:
                peer_id = info["peer"]
            build_pc(info["ice_servers"])
            await blackhole.start()
            log("connection: peer", peer_id, "topology", info["topology"])
        elif name == "transmitted-data":
            td = kolibri.parse_transmitted_data(n)
            if td is None:
                continue
            if td["kind"] == "sdp" and td["type"] == "offer":
                if pc is None:
                    build_pc(params["ice_servers"])
                    await blackhole.start()
                await pc.setRemoteDescription(RTCSessionDescription(td["sdp"], "offer"))
                answer = await pc.createAnswer()
                await pc.setLocalDescription(answer)
                await asyncio.to_thread(sig.transmit_sdp, peer_id, "answer", pc.localDescription.sdp)
                for cand, mid, mline in local_candidates(pc.localDescription.sdp):
                    await asyncio.to_thread(sig.transmit_candidate, peer_id, cand, mid, mline)
                log("answered")
            elif td["kind"] == "candidate" and pc is not None:
                try:
                    await pc.addIceCandidate(
                        to_candidate(td["candidate"], td["sdp_mid"], td["sdp_mline_index"])
                    )
                except Exception as e:
                    log("bad candidate:", e)
        elif name in ("hungup", "closed-conversation"):
            log("call ended:", name)
            break
        elif name == "topology-changed" and (n.get("conversation") or {}).get("topology") == "SERVER":
            log("migrated to SFU - a P2P bot can't follow")
            break

    await blackhole.stop()
    if pc is not None:
        await pc.close()
    sig.close()


def _quiet_stun_errors(loop, context):
    # aioice keeps retrying STUN after pc.close(); the transport is gone so it
    # spams 'NoneType has no sendto'. harmless post-hangup noise.
    msg = str(context.get("message", ""))
    if "datagram transport" in msg or "sendto" in repr(context.get("exception")):
        return
    loop.default_exception_handler(context)


async def main():
    import random

    asyncio.get_running_loop().set_exception_handler(_quiet_stun_errors)

    ident = load_identity()
    device_id = ident["device_id"]
    device = stable_device(device_id)
    log("device:", device["device_name"], device["os_version"], device["timezone"])
    session = kolibri.Session(
        HOST, 443,
        device_id=device_id,
        instance_id=ident["instance_id"],
        client_session_id=random.randint(1, 0x7FFFFFFF),
        **device,
    )
    await asyncio.to_thread(login, session, device_id)

    log("waiting for an incoming call... (call this account now)")
    while True:
        push = await asyncio.to_thread(session.next_push, 3600.0)
        if push is None or push["opcode"] != NOTIF_CALL_START:
            continue
        payload = push["payload"]
        log("incoming call from", payload.get("callerId"), "type", payload.get("type"))
        params = kolibri.decode_vcp(payload.get("vcp"), payload.get("conversationId"))
        if params is None:
            log("could not decode vcp")
            continue
        sig = await asyncio.to_thread(kolibri.CallSignaling, params["ws2_url"])
        await handle_call(sig, params)
        log("ready for the next call")


if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        sys.exit(0)
