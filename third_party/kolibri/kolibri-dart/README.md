# kolibri (Dart)

Dart / Flutter bindings for [`kolibri-net`](../kolibri-net) via
[`flutter_rust_bridge`](https://github.com/fzyzcjy/flutter_rust_bridge) (v2).
Async Rust maps to Dart `Future`s, server pushes to a Dart `Stream`. Payloads
cross the boundary as raw MessagePack bytes (`Uint8List`) — decode them on the
Dart side (e.g. with `msgpack_dart`), matching the "bytes in, bytes out" core.

## Layout

```
kolibri-dart/
├── rust/                 Rust crate (flutter_rust_bridge API over kolibri-net)
│   └── src/api/session.rs
├── lib/
│   ├── kolibri.dart      hand-written ergonomic wrapper (defaults + init)
│   └── src/rust/         generated bindings (flutter_rust_bridge_codegen)
├── example/handshake.dart
└── flutter_rust_bridge.yaml
```

## Build

```bash
dart pub get
flutter_rust_bridge_codegen generate            # regenerate bindings after Rust API changes
dart run build_runner build --delete-conflicting-outputs  # freezed classes for UploadEvent
cargo build --manifest-path rust/Cargo.toml     # build the native library
```

## Run (pure Dart)

```bash
dart run example/handshake.dart              # loads rust/target/debug/libkolibri_dart.dylib
```

## Usage

```dart
import 'package:kolibri/kolibri.dart';

await initKolibri(libraryPath: '.../libkolibri_dart.dylib'); // omit on Flutter
final s = openSession(host: 'api.oneme.ru');                 // override device fields to spoof

final info = await s.connect();               // sessionInit handshake
print(info.callsSeed);

final respBytes = await s.request(opcode: 64, payload: msgpackBytes); // Uint8List → Uint8List
s.pushes().listen((p) => print('push ${p.opcode}'));         // Stream<PushEvent>

// media upload → Stream<UploadEvent> (Progress → Done/Error)
await for (final e in s.uploadFile(url: cdnUrl, data: bytes, filename: 'v.mp4')) {
  switch (e) {
    case UploadEvent_Progress(:final sent, :final total): /* … */
    case UploadEvent_Done(:final status, :final body): /* … */
    case UploadEvent_Error(:final message): /* … */
  }
}
s.disconnect();
```

## Flutter integration

For a real Flutter app, build the native library for each target ABI
(`cargo-ndk` for Android, an xcframework for iOS, the platform `.so`/`.dll`/`.dylib`
for desktop) and let `flutter_rust_bridge`'s default loader find the bundled
library — then `initKolibri()` with no `libraryPath`.
