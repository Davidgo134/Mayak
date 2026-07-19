package kolibri

// #include <stdlib.h>
// #include "kolibri.h"
import "C"

import (
	"encoding/json"
	"time"
	"unsafe"
)

// IceServer is a STUN/TURN server in the shape a WebRTC stack expects.
type IceServer struct {
	URLs       []string `json:"urls"`
	Username   *string  `json:"username"`
	Credential *string  `json:"credential"`
}

// CallParams are the decoded call params (vcp): endpoints, ICE servers, and the
// ws2 connect url (when DecodeVCP is given a conversation id).
type CallParams struct {
	Token        string      `json:"token"`
	WsEndpoint   string      `json:"ws_endpoint"`
	Stun         *string     `json:"stun"`
	Turn         []string    `json:"turn"`
	TurnUser     *string     `json:"turn_user"`
	TurnPassword *string     `json:"turn_password"`
	IsVideo      bool        `json:"is_video"`
	ExpiresAt    *int64      `json:"expires_at"`
	UserID       int64       `json:"user_id"`
	IceServers   []IceServer `json:"ice_servers"`
	Ws2URL       string      `json:"ws2_url"`
}

// ConnectionInfo is a parsed ws2 `connection` notification.
type ConnectionInfo struct {
	Topology     string      `json:"topology"`
	IsSFU        bool        `json:"is_sfu"`
	Participants []int64     `json:"participants"`
	Peer         *int64      `json:"peer"`
	IceServers   []IceServer `json:"ice_servers"`
}

// TransmittedData is a parsed ws2 `transmitted-data` notification: an SDP
// (Kind=="sdp") or an ICE candidate (Kind=="candidate").
type TransmittedData struct {
	Kind          string `json:"kind"`
	Type          string `json:"type"`
	SDP           string `json:"sdp"`
	Candidate     string `json:"candidate"`
	SdpMid        string `json:"sdp_mid"`
	SdpMlineIndex int64  `json:"sdp_mline_index"`
}

// DecodeVCP decodes a vcp call-params string. Pass a conversation id to also get
// Ws2URL; pass "" to skip it. Returns nil if the vcp can't be decoded.
func DecodeVCP(vcp, conversationID string) (*CallParams, error) {
	cvcp := C.CString(vcp)
	defer C.free(unsafe.Pointer(cvcp))
	ccid := C.CString(conversationID)
	defer C.free(unsafe.Pointer(ccid))
	var got C.bool
	var out *C.char
	if e := C.kolibri_decode_vcp(cvcp, ccid, &got, &out); e != nil {
		return nil, takeErr(e)
	}
	if !bool(got) {
		return nil, nil
	}
	var p CallParams
	if err := json.Unmarshal([]byte(takeString(out)), &p); err != nil {
		return nil, err
	}
	return &p, nil
}

// ParseConnection parses a ws2 `connection` notification (raw JSON). Pass your
// own calls user id to get Peer filled; pass nil to skip it.
func ParseConnection(notificationJSON string, myUserID *int64) (*ConnectionInfo, error) {
	cn := C.CString(notificationJSON)
	defer C.free(unsafe.Pointer(cn))
	var uid C.int64_t
	has := C.bool(false)
	if myUserID != nil {
		uid = C.int64_t(*myUserID)
		has = C.bool(true)
	}
	var out *C.char
	if e := C.kolibri_parse_connection(cn, uid, has, &out); e != nil {
		return nil, takeErr(e)
	}
	var info ConnectionInfo
	if err := json.Unmarshal([]byte(takeString(out)), &info); err != nil {
		return nil, err
	}
	return &info, nil
}

// ParseTransmittedData parses a ws2 `transmitted-data` notification (raw JSON).
// Returns nil when it carries neither an SDP nor a candidate.
func ParseTransmittedData(notificationJSON string) (*TransmittedData, error) {
	cn := C.CString(notificationJSON)
	defer C.free(unsafe.Pointer(cn))
	var got C.bool
	var out *C.char
	if e := C.kolibri_parse_transmitted_data(cn, &got, &out); e != nil {
		return nil, takeErr(e)
	}
	if !bool(got) {
		return nil, nil
	}
	var d TransmittedData
	if err := json.Unmarshal([]byte(takeString(out)), &d); err != nil {
		return nil, err
	}
	return &d, nil
}

// Call is a ws2 signaling client; it connects on ConnectCall and blocks on its
// own runtime. Signaling only — the WebRTC media stack stays in your app.
type Call struct {
	ptr *C.KCall
}

// ConnectCall opens a ws2 signaling connection. userAgent and proxy may be "".
func ConnectCall(url, userAgent, proxy string) (*Call, error) {
	curl := C.CString(url)
	defer C.free(unsafe.Pointer(curl))
	cua := C.CString(userAgent)
	defer C.free(unsafe.Pointer(cua))
	cpx := C.CString(proxy)
	defer C.free(unsafe.Pointer(cpx))
	var out *C.KCall
	if e := C.kolibri_call_connect(curl, cua, cpx, &out); e != nil {
		return nil, takeErr(e)
	}
	return &Call{ptr: out}, nil
}

// Accept accepts the incoming call; returns the response JSON.
func (c *Call) Accept() (string, error) {
	var out *C.char
	if e := C.kolibri_call_accept(c.ptr, &out); e != nil {
		return "", takeErr(e)
	}
	return takeString(out), nil
}

// Hangup ends the call with a reason; returns the response JSON.
func (c *Call) Hangup(reason string) (string, error) {
	cr := C.CString(reason)
	defer C.free(unsafe.Pointer(cr))
	var out *C.char
	if e := C.kolibri_call_hangup(c.ptr, cr, &out); e != nil {
		return "", takeErr(e)
	}
	return takeString(out), nil
}

// TransmitSDP sends an SDP offer/answer to a participant; returns response JSON.
func (c *Call) TransmitSDP(participantID int64, sdpType, sdp string) (string, error) {
	ct := C.CString(sdpType)
	defer C.free(unsafe.Pointer(ct))
	cs := C.CString(sdp)
	defer C.free(unsafe.Pointer(cs))
	var out *C.char
	if e := C.kolibri_call_transmit_sdp(c.ptr, C.int64_t(participantID), ct, cs, &out); e != nil {
		return "", takeErr(e)
	}
	return takeString(out), nil
}

// TransmitCandidate sends an ICE candidate to a participant; returns response JSON.
func (c *Call) TransmitCandidate(participantID int64, candidate, sdpMid string, sdpMlineIndex int64) (string, error) {
	cc := C.CString(candidate)
	defer C.free(unsafe.Pointer(cc))
	cm := C.CString(sdpMid)
	defer C.free(unsafe.Pointer(cm))
	var out *C.char
	if e := C.kolibri_call_transmit_candidate(c.ptr, C.int64_t(participantID), cc, cm, C.int64_t(sdpMlineIndex), &out); e != nil {
		return "", takeErr(e)
	}
	return takeString(out), nil
}

// ChangeMedia updates the audio/video/screen flags; returns response JSON.
func (c *Call) ChangeMedia(audio, video, screen bool) (string, error) {
	var out *C.char
	if e := C.kolibri_call_change_media(c.ptr, C.bool(audio), C.bool(video), C.bool(screen), &out); e != nil {
		return "", takeErr(e)
	}
	return takeString(out), nil
}

// SendCommand sends a raw command with a JSON object of extra fields ("" for
// none); returns the response JSON.
func (c *Call) SendCommand(command, extraJSON string) (string, error) {
	cc := C.CString(command)
	defer C.free(unsafe.Pointer(cc))
	ce := C.CString(extraJSON)
	defer C.free(unsafe.Pointer(ce))
	var out *C.char
	if e := C.kolibri_call_send_command(c.ptr, cc, ce, &out); e != nil {
		return "", takeErr(e)
	}
	return takeString(out), nil
}

// NextNotification waits for the next ws2 notification (raw JSON). A negative
// timeout blocks forever; ok is false on timeout.
func (c *Call) NextNotification(timeout time.Duration) (notification string, ok bool, err error) {
	ms := C.int64_t(-1)
	if timeout >= 0 {
		ms = C.int64_t(timeout.Milliseconds())
	}
	var out *C.char
	var got C.bool
	if e := C.kolibri_call_next_notification(c.ptr, ms, &out, &got); e != nil {
		return "", false, takeErr(e)
	}
	if !bool(got) {
		return "", false, nil
	}
	return takeString(out), true, nil
}

// IsConnected reports whether the ws2 socket is still up.
func (c *Call) IsConnected() bool { return bool(C.kolibri_call_is_connected(c.ptr)) }

// Close hangs up the ws2 socket and frees the client.
func (c *Call) Close() {
	if c.ptr != nil {
		C.kolibri_call_close(c.ptr)
		c.ptr = nil
	}
}
