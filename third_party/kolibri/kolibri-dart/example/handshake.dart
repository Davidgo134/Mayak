import 'dart:io';

import 'package:kolibri/kolibri.dart';

Future<void> main() async {
  final libPath = Platform.environment['KOLIBRI_LIB'] ??
      'rust/target/debug/libkolibri_dart.dylib';
  await initKolibri(libraryPath: libPath);

  final session = openSession(host: 'api.oneme.ru');
  print('state: ${session.state()}');

  final info = await session.connect();
  print('state: ${session.state()}');
  print('calls_seed  : ${info.callsSeed}');
  print('device_name : ${info.deviceName}');
  print('payload     : ${info.payload.length} bytes of msgpack');

  session.disconnect();
  print('state: ${session.state()}');
}
