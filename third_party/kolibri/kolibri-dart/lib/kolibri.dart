import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter_rust_bridge/flutter_rust_bridge.dart'
    show RustStreamSink;
import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;

import 'src/rust/api/session.dart' hide authMode;
import 'src/rust/api/session.dart' as _rust;
import 'src/rust/frb_generated.dart';

export 'src/rust/api/session.dart'
    show
        KolibriSession,
        SessionOptions,
        HandshakeInfo,
        PushEvent,
        WireLogEvent,
        UploadEvent,
        UploadEvent_Progress,
        UploadEvent_Done,
        UploadEvent_Error;

export 'src/rust/api/calls.dart'
    show
        CallSignaling,
        CallParams,
        IceServer,
        ConnectionInfo,
        TransmittedData,
        decodeVcp,
        connectCallSignaling,
        parseConnection,
        parseTransmittedData;

bool _initialized = false;

/// Load the native library and init the bindings; call once. On Flutter omit
/// [libraryPath] (bundled library used); for `dart run` pass the built dylib path.
Future<void> initKolibri({String? libraryPath}) async {
  if (_initialized) return;
  await RustLib.init(
    externalLibrary:
        libraryPath == null ? null : ExternalLibrary.open(libraryPath),
  );
  _initialized = true;
}

SessionOptions _options({
  required String host,
  required int port,
  required String deviceId,
  required String instanceId,
  required String appVersion,
  required int buildNumber,
  required String deviceType,
  required String osVersion,
  required String timezone,
  required String screen,
  required String pushDeviceType,
  required String arch,
  required String locale,
  required String deviceName,
  required String deviceLocale,
  required int clientSessionId,
  required int pingIntervalSecs,
  required bool pingInteractive,
  required bool autoReconnect,
  required bool insecureTls,
  required String? proxy,
}) {
  return SessionOptions(
    host: host,
    port: port,
    deviceId: deviceId,
    instanceId: instanceId,
    appVersion: appVersion,
    buildNumber: buildNumber,
    deviceType: deviceType,
    osVersion: osVersion,
    timezone: timezone,
    screen: screen,
    pushDeviceType: pushDeviceType,
    arch: arch,
    locale: locale,
    deviceName: deviceName,
    deviceLocale: deviceLocale,
    clientSessionId: clientSessionId,
    pingIntervalSecs: BigInt.from(pingIntervalSecs),
    pingInteractive: pingInteractive,
    autoReconnect: autoReconnect,
    insecureTls: insecureTls,
    proxy: proxy,
  );
}

/// Create a session with sensible defaults; override any device field to spoof.
KolibriSession openSession({
  required String host,
  int port = 443,
  String deviceId = 'kolibri-dart',
  String instanceId = 'kolibri-dart',
  String appVersion = '26.20.2',
  int buildNumber = 6758,
  String deviceType = 'ANDROID',
  String osVersion = 'Android 14',
  String timezone = 'Europe/Moscow',
  String screen = '420dpi 420dpi 1080x2340',
  String pushDeviceType = 'GCM',
  String arch = 'arm64-v8a',
  String locale = 'ru',
  String deviceName = 'Dart',
  String deviceLocale = 'ru',
  int clientSessionId = 1700000000,
  int pingIntervalSecs = 30,
  bool pingInteractive = true,
  bool autoReconnect = true,
  bool insecureTls = false,
  String? proxy,
}) {
  return KolibriSession(
    options: _options(
      host: host,
      port: port,
      deviceId: deviceId,
      instanceId: instanceId,
      appVersion: appVersion,
      buildNumber: buildNumber,
      deviceType: deviceType,
      osVersion: osVersion,
      timezone: timezone,
      screen: screen,
      pushDeviceType: pushDeviceType,
      arch: arch,
      locale: locale,
      deviceName: deviceName,
      deviceLocale: deviceLocale,
      clientSessionId: clientSessionId,
      pingIntervalSecs: pingIntervalSecs,
      pingInteractive: pingInteractive,
      autoReconnect: autoReconnect,
      insecureTls: insecureTls,
      proxy: proxy,
    ),
  );
}

/// Like [openSession], but also returns a stream of every packet in both
/// directions (requests, pushes, handshake, ping) rendered for logging.
(KolibriSession, Stream<WireLogEvent>) openSessionWithWireLog({
  required String host,
  int port = 443,
  String deviceId = 'kolibri-dart',
  String instanceId = 'kolibri-dart',
  String appVersion = '26.20.2',
  int buildNumber = 6758,
  String deviceType = 'ANDROID',
  String osVersion = 'Android 14',
  String timezone = 'Europe/Moscow',
  String screen = '420dpi 420dpi 1080x2340',
  String pushDeviceType = 'GCM',
  String arch = 'arm64-v8a',
  String locale = 'ru',
  String deviceName = 'Dart',
  String deviceLocale = 'ru',
  int clientSessionId = 1700000000,
  int pingIntervalSecs = 30,
  bool pingInteractive = true,
  bool autoReconnect = true,
  bool insecureTls = false,
  String? proxy,
}) {
  final sink = RustStreamSink<WireLogEvent>();
  final session = KolibriSession(
    options: _options(
      host: host,
      port: port,
      deviceId: deviceId,
      instanceId: instanceId,
      appVersion: appVersion,
      buildNumber: buildNumber,
      deviceType: deviceType,
      osVersion: osVersion,
      timezone: timezone,
      screen: screen,
      pushDeviceType: pushDeviceType,
      arch: arch,
      locale: locale,
      deviceName: deviceName,
      deviceLocale: deviceLocale,
      clientSessionId: clientSessionId,
      pingIntervalSecs: pingIntervalSecs,
      pingInteractive: pingInteractive,
      autoReconnect: autoReconnect,
      insecureTls: insecureTls,
      proxy: proxy,
    ),
    wireLog: sink,
  );
  return (session, sink.stream);
}

/// Build a request from a Dart `Map`; the core does the msgpack. A `Uint8List`
/// value is sent as a binary field.
extension KolibriRequestMap on KolibriSession {
  Future<Map<String, dynamic>> requestMap(
    int opcode,
    Map<String, dynamic> payload,
  ) async {
    final out = await requestJson(
      opcode: opcode,
      jsonIn: jsonEncode(_escapeBinary(payload)),
    );
    return _asMap(out);
  }
}

/// A full response: packet command plus decoded payload. A server error is
/// reported here (isError/errorKey), not thrown.
class KolibriResponse {
  KolibriResponse({
    required this.cmd,
    required this.opcode,
    required this.payload,
    this.errorMessage,
    this.errorKey,
  });

  final int cmd;
  final int opcode;
  final Map<String, dynamic> payload;
  final String? errorMessage;
  final String? errorKey;

  bool get isOk => cmd == 1;
  bool get isNotFound => cmd == 2;
  bool get isError => cmd == 3;
}

extension KolibriRequestMapFull on KolibriSession {
  Future<KolibriResponse> requestMapFull(
    int opcode,
    Map<String, dynamic> payload,
  ) async {
    final r = await requestFull(
      opcode: opcode,
      jsonIn: jsonEncode(_escapeBinary(payload)),
    );
    return KolibriResponse(
      cmd: r.cmd,
      opcode: r.opcode,
      payload: _asMap(r.payloadJson),
      errorMessage: r.errorMessage,
      errorKey: r.errorKey,
    );
  }
}

/// The handshake payload decoded as a map (no msgpack package needed).
extension KolibriHandshakeMap on HandshakeInfo {
  Map<String, dynamic> get payloadMap => _asMap(payloadJson);
}

/// The push payload decoded as a map.
extension KolibriPushMap on PushEvent {
  Map<String, dynamic> get payloadMap => _asMap(payloadJson);
}

/// Server pushes with their payloads already decoded to maps.
extension KolibriPushesMap on KolibriSession {
  Stream<(int, Map<String, dynamic>)> pushesMap() =>
      pushes().map((e) => (e.opcode, e.payloadMap));
}

Map<String, dynamic> _asMap(String jsonStr) {
  final decoded = _unescapeBinary(jsonDecode(jsonStr));
  return decoded is Map ? Map<String, dynamic>.from(decoded) : <String, dynamic>{};
}

/// Inverse of [_escapeBinary]: `{"$bin":"<base64>"}` -> `Uint8List`.
dynamic _unescapeBinary(dynamic value) {
  if (value is Map) {
    if (value.length == 1 && value[r'$bin'] is String) {
      return base64.decode(value[r'$bin'] as String);
    }
    return value.map((k, v) => MapEntry(k, _unescapeBinary(v)));
  }
  if (value is List) {
    return value.map(_unescapeBinary).toList();
  }
  return value;
}

dynamic _escapeBinary(dynamic value) {
  if (value is Uint8List) {
    return {r'$bin': base64.encode(value)};
  }
  if (value is Map) {
    return value.map((k, v) => MapEntry(k, _escapeBinary(v)));
  }
  if (value is List) {
    return value.map(_escapeBinary).toList();
  }
  return value;
}

/// 96-byte anti-spoof fingerprint (authRequest `mode` / login `chatCacheFingerprint`).
/// signature/dex/so default to the known app values; override (raw bytes) per app build.
Uint8List authMode(
  int callsSeed,
  String deviceId, {
  List<int>? signature,
  List<int>? dex,
  List<int>? so,
}) {
  return _rust.authMode(
    signature: signature ?? _defaultSignatureDigest,
    dex: dex ?? _defaultDexDigest,
    so: so ?? _defaultSoDigest,
    callsSeed: callsSeed,
    deviceId: deviceId,
  );
}

final Uint8List _defaultSignatureDigest = _hex(
    '1684414033eb263e2c615f8b7df5ed8793850a07656304997fbf07e9e21e1e93');
final Uint8List _defaultSoDigest = _hex(
    '90e2fb8745b17b42a10182f8d8ac590e3fca5b311e2ce2d5144fa2c18cb3090d');
final Uint8List _defaultDexDigest = _hex(
    '0a6265f6e5d8231b9cba641f8c40475e6f3baeb06ed41b804b9bf7307aa4214e');

Uint8List _hex(String s) {
  final out = Uint8List(s.length ~/ 2);
  for (var i = 0; i < out.length; i++) {
    out[i] = int.parse(s.substring(i * 2, i * 2 + 2), radix: 16);
  }
  return out;
}
