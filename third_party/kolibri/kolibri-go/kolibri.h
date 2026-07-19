#ifndef KOLIBRI_H
#define KOLIBRI_H

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* A byte buffer owned by the library; free with kolibri_bytes_free. */
typedef struct {
  uint8_t *ptr;
  size_t len;
} KBytes;

/* device + connection options. String fields are NUL-terminated UTF-8;
 * `proxy` may be NULL/empty for a direct connection. */
typedef struct {
  const char *host;
  uint16_t port;
  const char *device_id;
  const char *instance_id;
  const char *app_version;
  int64_t build_number;
  const char *device_type;
  const char *os_version;
  const char *timezone;
  const char *screen;
  const char *push_device_type;
  const char *arch;
  const char *locale;
  const char *device_name;
  const char *device_locale;
  int64_t client_session_id;
  uint64_t ping_interval_secs;
  bool ping_interactive;
  bool auto_reconnect;
  bool insecure_tls;
  const char *proxy;
} KConfig;

typedef struct KSession KSession;

/* wire-tap callback: one call per packet each direction. */
typedef void (*KWireCb)(void *user, const char *direction, const char *cmd,
                        uint16_t opcode, uint16_t seq, const char *json);

/* Fallible calls return NULL on success, or an owned error string to free with
 * kolibri_string_free. Results go into out-params. */

void kolibri_bytes_free(KBytes b);
void kolibri_string_free(char *s);

char *kolibri_session_new(const KConfig *cfg, KWireCb wire_cb, void *wire_user,
                          KSession **out);
char *kolibri_session_connect(KSession *h, KBytes *out);
char *kolibri_session_connect_json(KSession *h, char **out);
char *kolibri_session_request(KSession *h, uint16_t opcode,
                              const uint8_t *payload, size_t len, KBytes *out);
char *kolibri_session_request_json(KSession *h, uint16_t opcode,
                                   const char *json_in, char **out);
char *kolibri_session_send(KSession *h, uint16_t opcode, const uint8_t *payload,
                           size_t len, uint16_t *out_seq);
char *kolibri_session_next_push(KSession *h, int64_t timeout_ms,
                                uint16_t *out_opcode, KBytes *out_payload,
                                bool *out_got);
char *kolibri_session_next_push_json(KSession *h, int64_t timeout_ms,
                                     uint16_t *out_opcode, char **out_json,
                                     bool *out_got);
int kolibri_session_state(KSession *h);
bool kolibri_session_ping_interactive(KSession *h);
void kolibri_session_set_ping_interactive(KSession *h, bool interactive);
char *kolibri_session_user_agent(KSession *h);
void kolibri_session_disconnect(KSession *h);
void kolibri_session_free(KSession *h);

char *kolibri_upload_file(KSession *h, const char *url, const uint8_t *data,
                          size_t len, const char *filename, uint16_t *out_status,
                          KBytes *out_body);
char *kolibri_upload_photo(KSession *h, const char *url, const uint8_t *data,
                           size_t len, const char *filename,
                           uint16_t *out_status, KBytes *out_body);
char *kolibri_upload_video(KSession *h, const char *url, const uint8_t *data,
                           size_t len, size_t chunk_size, size_t concurrency,
                           bool *out_ok);

char *kolibri_auth_mode(const uint8_t *signature, size_t signature_len,
                        const uint8_t *dex, size_t dex_len, const uint8_t *so,
                        size_t so_len, int64_t calls_seed, const char *device_id,
                        KBytes *out);

/* calls: ws2 signaling. Methods return response JSON in *out_json. */
typedef struct KCall KCall;

char *kolibri_decode_vcp(const char *vcp, const char *conversation_id,
                         bool *out_got, char **out_json);
char *kolibri_parse_connection(const char *notification, int64_t my_user_id,
                               bool has_user_id, char **out_json);
char *kolibri_parse_transmitted_data(const char *notification, bool *out_got,
                                     char **out_json);

char *kolibri_call_connect(const char *url, const char *user_agent,
                           const char *proxy, KCall **out);
char *kolibri_call_accept(KCall *h, char **out_json);
char *kolibri_call_hangup(KCall *h, const char *reason, char **out_json);
char *kolibri_call_transmit_sdp(KCall *h, int64_t participant_id,
                                const char *sdp_type, const char *sdp,
                                char **out_json);
char *kolibri_call_transmit_candidate(KCall *h, int64_t participant_id,
                                      const char *candidate, const char *sdp_mid,
                                      int64_t sdp_mline_index, char **out_json);
char *kolibri_call_change_media(KCall *h, bool audio, bool video, bool screen,
                                char **out_json);
char *kolibri_call_send_command(KCall *h, const char *command,
                                const char *extra_json, char **out_json);
char *kolibri_call_next_notification(KCall *h, int64_t timeout_ms,
                                     char **out_json, bool *out_got);
bool kolibri_call_is_connected(KCall *h);
void kolibri_call_close(KCall *h);

#ifdef __cplusplus
}
#endif

#endif /* KOLIBRI_H */
