import 'dart:io';
import 'package:kolibri/kolibri.dart';

Future<void> main() async {
  final lib = Platform.environment['KOLIBRI_LIB'] ??
      'rust/target/debug/libkolibri_dart.dylib';
  await initKolibri(libraryPath: lib);
  final m = authMode(5091188991553007784, 'd1e9c0de-0000-4000-8000-kolibri0001');
  print(m.map((b) => b.toRadixString(16).padLeft(2, '0')).join());
}
