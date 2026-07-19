import sys
import kolibri

dev = "spoof-0001"
phone = sys.argv[1] if len(sys.argv) > 1 else "+70000000000"
s = kolibri.Session("api.oneme.ru", 443, device_id=dev, device_name="Google Pixel 8 Pro")
seed = s.connect()["calls_seed"]
mode = kolibri.auth_mode(seed, dev)
r = s.request(17, {"phone": phone, "type": "START_AUTH", "language": "ru", "mode": mode})
print("token:", r["token"])
