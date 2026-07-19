import 'dart:io';

import 'package:kolibri/kolibri.dart';

Future<void> main() async {
  final libPath = Platform.environment['KOLIBRI_LIB'] ??
      'rust/target/debug/libkolibri_dart.dylib';
  await initKolibri(libraryPath: libPath);

  final server = await HttpServer.bind('127.0.0.1', 0);
  server.listen((req) async {
    await req.drain<void>();
    req.response.statusCode = 200;
    req.response.write('ok');
    await req.response.close();
  });

  final session = openSession(host: 'api.oneme.ru');
  final data = List<int>.filled(200000, 0x58);

  var progressCount = 0;
  await for (final event in session.uploadFile(
    url: 'http://127.0.0.1:${server.port}/up',
    data: data,
    filename: 'clip.bin',
  )) {
    switch (event) {
      case UploadEvent_Progress(:final sent, :final total):
        progressCount++;
        print('progress: $sent / $total');
      case UploadEvent_Done(:final status, :final body):
        print('done: status=$status body=${String.fromCharCodes(body)}');
      case UploadEvent_Error(:final message):
        print('error: $message');
    }
  }
  print('progress events: $progressCount');

  await server.close();
  session.disconnect();
}
