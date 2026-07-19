"""sessionInit handshake against the real server, no SMS.

    python examples/handshake.py
"""

import kolibri

s = kolibri.Session("api.oneme.ru", 443)
print("state:", s.state())

info = s.connect()
print("state:", s.state())
print("calls_seed :", info["calls_seed"])
print("location   :", info["payload"].get("location"))
print("countries  :", len(info["payload"].get("reg-country-code", [])))

s.disconnect()
print("state:", s.state())
