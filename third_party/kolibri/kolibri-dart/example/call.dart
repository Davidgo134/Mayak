import 'dart:convert';
import 'dart:io';

import 'package:kolibri/kolibri.dart';

Future<void> main() async {
  final lib = Platform.environment['KOLIBRI_LIB'] ??
      'rust/target/debug/libkolibri_dart.dylib';
  await initKolibri(libraryPath: lib);

  var gotPong = false;
  final server = await HttpServer.bind('127.0.0.1', 0);
  server.listen((req) async {
    final ws = await WebSocketTransformer.upgrade(req);
    ws.add('ping');
    ws.listen((msg) {
      if (msg == 'pong') {
        gotPong = true;
        return;
      }
      final v = jsonDecode(msg as String);
      ws.add(jsonEncode(
          {'sequence': v['sequence'], 'response': v['command'], 'type': 'response'}));
      if (v['command'] == 'accept-call') {
        ws.add(jsonEncode(
            {'type': 'notification', 'notification': 'connection', 'topology': 'P2P'}));
      }
    });
  });

  final sig = await connectCallSignaling(url: 'ws://127.0.0.1:${server.port}/ws2');

  final notifs = <String>[];
  final sub = sig.notifications().listen(notifs.add);

  print('accept-call : ${await sig.acceptCall()}');
  await sig.transmitSdp(participantId: 42, sdpType: 'offer', sdp: 'v=0...');

  await Future<void>.delayed(const Duration(milliseconds: 300));
  print('notification: ${notifs.isNotEmpty ? notifs.first : "(none)"}');
  print('got pong    : $gotPong');
  print('connected   : ${sig.isConnected()}');

  await sub.cancel();
  sig.close();
  await server.close(force: true);
  exit(0);
}
