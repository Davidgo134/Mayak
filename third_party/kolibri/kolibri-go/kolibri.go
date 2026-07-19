// Package kolibri is a Go binding for the kolibri messaging-protocol core,
// linking the Rust library over cgo. A Session owns a tokio runtime; every call
// blocks until it completes.
package kolibri

/*
#cgo CFLAGS: -I${SRCDIR}
#cgo LDFLAGS: -L${SRCDIR}/rust/target/release -lkolibri_go
#cgo darwin LDFLAGS: -framework CoreFoundation -framework Security
#include <stdlib.h>
#include "kolibri.h"

extern void goWireTrampoline(void *user, char *direction, char *cmd,
                             uint16_t opcode, uint16_t seq, char *json);
static KWireCb kolibriWireCb(void) { return (KWireCb)goWireTrampoline; }
*/
import "C"

import (
	"encoding/base64"
	"encoding/json"
	"errors"
	"runtime/cgo"
	"strings"
	"time"
	"unsafe"
)

// Session states, as returned by (*Session).State.
const (
	Disconnected = 0
	Connecting   = 1
	Connected    = 2
	Online       = 3
)

// WireEvent is one tapped packet, delivered to the Config.OnWire callback.
type WireEvent struct {
	Direction string // "out" | "in"
	Cmd       string // "request" | "ok" | "not_found" | "error" | "push"
	Opcode    uint16
	Seq       uint16
	JSON      string // payload rendered as JSON (lossy; binary -> base64)
}

// Config holds the device fields (which feed the sessionInit handshake) and
// connection options. Use DefaultConfig for sensible defaults.
type Config struct {
	Host            string
	Port            uint16
	DeviceID        string
	InstanceID      string
	AppVersion      string
	BuildNumber     int64
	DeviceType      string
	OSVersion       string
	Timezone        string
	Screen          string
	PushDeviceType  string
	Arch            string
	Locale          string
	DeviceName      string
	DeviceLocale    string
	ClientSessionID int64
	PingIntervalSec uint64
	PingInteractive bool
	AutoReconnect   bool
	InsecureTLS     bool
	Proxy           string // "scheme://[user:pass@]host:port", or "" for none
	OnWire          func(WireEvent)
}

// DefaultConfig returns a Config for host with defaults matching the reference
// client; override any field before calling Open.
func DefaultConfig(host string) Config {
	return Config{
		Host:            host,
		Port:            443,
		DeviceID:        "kolibri-go",
		InstanceID:      "kolibri-go",
		AppVersion:      "26.20.2",
		BuildNumber:     6758,
		DeviceType:      "ANDROID",
		OSVersion:       "Android 14",
		Timezone:        "Europe/Moscow",
		Screen:          "420dpi 420dpi 1080x2340",
		PushDeviceType:  "GCM",
		Arch:            "arm64-v8a",
		Locale:          "ru",
		DeviceName:      "Go",
		DeviceLocale:    "ru",
		ClientSessionID: 1700000000,
		PingIntervalSec: 30,
		PingInteractive: true,
		AutoReconnect:   true,
	}
}

// Session is a live protocol session backed by the Rust core.
type Session struct {
	ptr      *C.KSession
	handle   cgo.Handle     // wire callback registration, if any
	wireCell unsafe.Pointer // C cell holding the handle token, freed on Close
}

//export goWireTrampoline
func goWireTrampoline(user unsafe.Pointer, dir, cmd *C.char, opcode, seq C.uint16_t, jsonStr *C.char) {
	cb := cgo.Handle(uintptr(*(*C.uintptr_t)(user))).Value().(func(WireEvent))
	cb(WireEvent{
		Direction: C.GoString(dir),
		Cmd:       C.GoString(cmd),
		Opcode:    uint16(opcode),
		Seq:       uint16(seq),
		JSON:      C.GoString(jsonStr),
	})
}

// Open builds a session from cfg. If cfg.OnWire is set, every packet in both
// directions is reported to it.
func Open(cfg Config) (*Session, error) {
	var frees []unsafe.Pointer
	cs := func(s string) *C.char {
		p := C.CString(s)
		frees = append(frees, unsafe.Pointer(p))
		return p
	}
	defer func() {
		for _, p := range frees {
			C.free(p)
		}
	}()

	cconf := C.KConfig{
		host:               cs(cfg.Host),
		port:               C.uint16_t(cfg.Port),
		device_id:          cs(cfg.DeviceID),
		instance_id:        cs(cfg.InstanceID),
		app_version:        cs(cfg.AppVersion),
		build_number:       C.int64_t(cfg.BuildNumber),
		device_type:        cs(cfg.DeviceType),
		os_version:         cs(cfg.OSVersion),
		timezone:           cs(cfg.Timezone),
		screen:             cs(cfg.Screen),
		push_device_type:   cs(cfg.PushDeviceType),
		arch:               cs(cfg.Arch),
		locale:             cs(cfg.Locale),
		device_name:        cs(cfg.DeviceName),
		device_locale:      cs(cfg.DeviceLocale),
		client_session_id:  C.int64_t(cfg.ClientSessionID),
		ping_interval_secs: C.uint64_t(cfg.PingIntervalSec),
		ping_interactive:   C.bool(cfg.PingInteractive),
		auto_reconnect:     C.bool(cfg.AutoReconnect),
		insecure_tls:       C.bool(cfg.InsecureTLS),
		proxy:              cs(cfg.Proxy),
	}

	var wireCb C.KWireCb
	var wireUser unsafe.Pointer
	var handle cgo.Handle
	if cfg.OnWire != nil {
		handle = cgo.NewHandle(cfg.OnWire)
		wireCb = C.kolibriWireCb()
		cell := (*C.uintptr_t)(C.malloc(C.size_t(unsafe.Sizeof(C.uintptr_t(0)))))
		*cell = C.uintptr_t(handle)
		wireUser = unsafe.Pointer(cell)
	}

	var out *C.KSession
	if e := C.kolibri_session_new(&cconf, wireCb, wireUser, &out); e != nil {
		if handle != 0 {
			handle.Delete()
		}
		if wireUser != nil {
			C.free(wireUser)
		}
		return nil, takeErr(e)
	}
	return &Session{ptr: out, handle: handle, wireCell: wireUser}, nil
}

// Connect runs the sessionInit handshake and returns the decoded handshake
// payload as a map.
func (s *Session) Connect() (map[string]any, error) {
	var out *C.char
	if e := C.kolibri_session_connect_json(s.ptr, &out); e != nil {
		return nil, takeErr(e)
	}
	return jsonToMap(takeString(out))
}

// ConnectRaw is Connect returning the raw msgpack payload instead of a map.
func (s *Session) ConnectRaw() ([]byte, error) {
	var out C.KBytes
	if e := C.kolibri_session_connect(s.ptr, &out); e != nil {
		return nil, takeErr(e)
	}
	return takeBytes(out), nil
}

// Request sends opcode with a msgpack payload and returns the response payload.
func (s *Session) Request(opcode uint16, payload []byte) ([]byte, error) {
	var out C.KBytes
	if e := C.kolibri_session_request(s.ptr, C.uint16_t(opcode), bytesPtr(payload), C.size_t(len(payload)), &out); e != nil {
		return nil, takeErr(e)
	}
	return takeBytes(out), nil
}

// RequestJSON sends a JSON payload and gets the response as JSON, no msgpack lib
// needed. {"$bin":"<base64>"} in the request is a binary field; binary in the
// response comes back as base64.
func (s *Session) RequestJSON(opcode uint16, jsonIn string) (string, error) {
	cj := C.CString(jsonIn)
	defer C.free(unsafe.Pointer(cj))
	var out *C.char
	if e := C.kolibri_session_request_json(s.ptr, C.uint16_t(opcode), cj, &out); e != nil {
		return "", takeErr(e)
	}
	return takeString(out), nil
}

// RequestMap builds the request from a Go map and returns the response as a map;
// the core does the msgpack. A []byte in the map is sent as a binary field.
func (s *Session) RequestMap(opcode uint16, in map[string]any) (map[string]any, error) {
	body, err := json.Marshal(escapeBinary(in))
	if err != nil {
		return nil, err
	}
	out, err := s.RequestJSON(opcode, string(body))
	if err != nil {
		return nil, err
	}
	return jsonToMap(out)
}

// jsonToMap decodes a JSON object string into a map; "" and "null" give an
// empty map (a request that returns no body). Numbers decode as json.Number so
// large int64 values (like callsSeed) keep full precision — a plain decode into
// `any` would turn them into lossy float64. A {"$bin":"<base64>"} object is
// turned back into a []byte.
func jsonToMap(s string) (map[string]any, error) {
	if s == "" || s == "null" {
		return map[string]any{}, nil
	}
	dec := json.NewDecoder(strings.NewReader(s))
	dec.UseNumber()
	var m map[string]any
	if err := dec.Decode(&m); err != nil {
		return nil, err
	}
	if unescaped, ok := unescapeBinary(m).(map[string]any); ok {
		return unescaped, nil
	}
	return m, nil
}

// unescapeBinary is the inverse of escapeBinary: {"$bin":"<base64>"} -> []byte.
func unescapeBinary(v any) any {
	switch t := v.(type) {
	case map[string]any:
		if len(t) == 1 {
			if b64, ok := t["$bin"].(string); ok {
				if raw, err := base64.StdEncoding.DecodeString(b64); err == nil {
					return raw
				}
			}
		}
		m := make(map[string]any, len(t))
		for k, val := range t {
			m[k] = unescapeBinary(val)
		}
		return m
	case []any:
		a := make([]any, len(t))
		for i, val := range t {
			a[i] = unescapeBinary(val)
		}
		return a
	default:
		return v
	}
}

// escapeBinary turns []byte into {"$bin":"<base64>"} so it survives json.Marshal
// as binary — plain json would flatten it to an indistinguishable base64 string.
func escapeBinary(v any) any {
	switch t := v.(type) {
	case []byte:
		return map[string]string{"$bin": base64.StdEncoding.EncodeToString(t)}
	case map[string]any:
		m := make(map[string]any, len(t))
		for k, val := range t {
			m[k] = escapeBinary(val)
		}
		return m
	case []any:
		a := make([]any, len(t))
		for i, val := range t {
			a[i] = escapeBinary(val)
		}
		return a
	default:
		return v
	}
}

// Send is fire-and-forget; it returns the assigned seq.
func (s *Session) Send(opcode uint16, payload []byte) (uint16, error) {
	var seq C.uint16_t
	if e := C.kolibri_session_send(s.ptr, C.uint16_t(opcode), bytesPtr(payload), C.size_t(len(payload)), &seq); e != nil {
		return 0, takeErr(e)
	}
	return uint16(seq), nil
}

// NextPush waits for the next server push and returns its opcode and decoded
// payload map. A negative timeout blocks forever; ok is false on timeout.
func (s *Session) NextPush(timeout time.Duration) (opcode uint16, payload map[string]any, ok bool, err error) {
	var op C.uint16_t
	var out *C.char
	var got C.bool
	if e := C.kolibri_session_next_push_json(s.ptr, pushTimeout(timeout), &op, &out, &got); e != nil {
		return 0, nil, false, takeErr(e)
	}
	if !bool(got) {
		return 0, nil, false, nil
	}
	m, err := jsonToMap(takeString(out))
	return uint16(op), m, true, err
}

// NextPushRaw is NextPush returning the raw msgpack payload instead of a map.
func (s *Session) NextPushRaw(timeout time.Duration) (opcode uint16, payload []byte, ok bool, err error) {
	var op C.uint16_t
	var out C.KBytes
	var got C.bool
	if e := C.kolibri_session_next_push(s.ptr, pushTimeout(timeout), &op, &out, &got); e != nil {
		return 0, nil, false, takeErr(e)
	}
	if !bool(got) {
		return 0, nil, false, nil
	}
	return uint16(op), takeBytes(out), true, nil
}

func pushTimeout(timeout time.Duration) C.int64_t {
	if timeout < 0 {
		return C.int64_t(-1)
	}
	return C.int64_t(timeout.Milliseconds())
}

// State returns the current session state (Disconnected/Connecting/Connected/Online).
func (s *Session) State() int { return int(C.kolibri_session_state(s.ptr)) }

// PingInteractive reports the current keepalive interactive flag.
func (s *Session) PingInteractive() bool { return bool(C.kolibri_session_ping_interactive(s.ptr)) }

// SetPingInteractive flips the keepalive interactive flag on the live session;
// one ping goes out immediately.
func (s *Session) SetPingInteractive(v bool) {
	C.kolibri_session_set_ping_interactive(s.ptr, C.bool(v))
}

// UserAgent returns the media HTTP User-Agent derived from the handshake device.
func (s *Session) UserAgent() string { return takeString(C.kolibri_session_user_agent(s.ptr)) }

// UploadFile POSTs data to a CDN url (single request) and returns (status, body).
func (s *Session) UploadFile(url string, data []byte, filename string) (uint16, []byte, error) {
	curl := C.CString(url)
	defer C.free(unsafe.Pointer(curl))
	cname := C.CString(filename)
	defer C.free(unsafe.Pointer(cname))
	var status C.uint16_t
	var body C.KBytes
	if e := C.kolibri_upload_file(s.ptr, curl, bytesPtr(data), C.size_t(len(data)), cname, &status, &body); e != nil {
		return 0, nil, takeErr(e)
	}
	return uint16(status), takeBytes(body), nil
}

// UploadPhoto uploads data as multipart/form-data and returns (status, body).
func (s *Session) UploadPhoto(url string, data []byte, filename string) (uint16, []byte, error) {
	curl := C.CString(url)
	defer C.free(unsafe.Pointer(curl))
	cname := C.CString(filename)
	defer C.free(unsafe.Pointer(cname))
	var status C.uint16_t
	var body C.KBytes
	if e := C.kolibri_upload_photo(s.ptr, curl, bytesPtr(data), C.size_t(len(data)), cname, &status, &body); e != nil {
		return 0, nil, takeErr(e)
	}
	return uint16(status), takeBytes(body), nil
}

// UploadVideo uploads data in parallel resumable chunks. It returns true on success.
func (s *Session) UploadVideo(url string, data []byte, chunkSize, concurrency int) (bool, error) {
	curl := C.CString(url)
	defer C.free(unsafe.Pointer(curl))
	var ok C.bool
	if e := C.kolibri_upload_video(s.ptr, curl, bytesPtr(data), C.size_t(len(data)), C.size_t(chunkSize), C.size_t(concurrency), &ok); e != nil {
		return false, takeErr(e)
	}
	return bool(ok), nil
}

// Disconnect stops the session and disables auto-reconnect.
func (s *Session) Disconnect() { C.kolibri_session_disconnect(s.ptr) }

// Close disconnects and frees the session. Use it (e.g. with defer) when done.
func (s *Session) Close() {
	if s.ptr != nil {
		C.kolibri_session_free(s.ptr)
		s.ptr = nil
	}
	if s.handle != 0 {
		s.handle.Delete()
		s.handle = 0
	}
	if s.wireCell != nil {
		C.free(s.wireCell)
		s.wireCell = nil
	}
}

// Known reference-client digests for the anti-spoof fingerprint; override in
// AuthMode if they change.
var (
	DefaultSignatureDigest = mustHex("1684414033eb263e2c615f8b7df5ed8793850a07656304997fbf07e9e21e1e93")
	DefaultDexDigest       = mustHex("0a6265f6e5d8231b9cba641f8c40475e6f3baeb06ed41b804b9bf7307aa4214e")
	DefaultSoDigest        = mustHex("90e2fb8745b17b42a10182f8d8ac590e3fca5b311e2ce2d5144fa2c18cb3090d")
)

// AuthMode builds the 96-byte anti-spoof fingerprint (authRequest `mode` /
// login `chatCacheFingerprint`). Pass nil digests to use the defaults above.
func AuthMode(callsSeed int64, deviceID string, signature, dex, so []byte) []byte {
	if signature == nil {
		signature = DefaultSignatureDigest
	}
	if dex == nil {
		dex = DefaultDexDigest
	}
	if so == nil {
		so = DefaultSoDigest
	}
	cdev := C.CString(deviceID)
	defer C.free(unsafe.Pointer(cdev))
	var out C.KBytes
	C.kolibri_auth_mode(
		bytesPtr(signature), C.size_t(len(signature)),
		bytesPtr(dex), C.size_t(len(dex)),
		bytesPtr(so), C.size_t(len(so)),
		C.int64_t(callsSeed), cdev, &out)
	return takeBytes(out)
}

func bytesPtr(b []byte) *C.uint8_t {
	if len(b) == 0 {
		return nil
	}
	return (*C.uint8_t)(unsafe.Pointer(&b[0]))
}

func takeBytes(b C.KBytes) []byte {
	if b.ptr == nil || b.len == 0 {
		C.kolibri_bytes_free(b)
		return []byte{}
	}
	out := C.GoBytes(unsafe.Pointer(b.ptr), C.int(b.len))
	C.kolibri_bytes_free(b)
	return out
}

func takeString(s *C.char) string {
	if s == nil {
		return ""
	}
	out := C.GoString(s)
	C.kolibri_string_free(s)
	return out
}

func takeErr(e *C.char) error {
	return errors.New(takeString(e))
}

func mustHex(s string) []byte {
	out := make([]byte, len(s)/2)
	for i := range out {
		var b byte
		for j := 0; j < 2; j++ {
			c := s[i*2+j]
			switch {
			case c >= '0' && c <= '9':
				b = b<<4 | (c - '0')
			case c >= 'a' && c <= 'f':
				b = b<<4 | (c - 'a' + 10)
			}
		}
		out[i] = b
	}
	return out
}
