import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mayak/core/storage/token_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

const _secureChannel = MethodChannel(
  'plugins.it_nomads.com/flutter_secure_storage',
);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  tearDown(() {
    TestWidgetsFlutterBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_secureChannel, null);
  });

  test('readToken returns null when keystore read throws PlatformException', () async {
    SharedPreferences.setMockInitialValues({});
    TestWidgetsFlutterBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_secureChannel, (call) async {
          throw PlatformException(
            code: 'keystore_invalid_key',
            message: 'invalid keystore key',
          );
        });

    final token = await TokenStorage.readToken(42);

    expect(token, isNull);
  });

  test('readToken recovers legacy SharedPreferences token via migration', () async {
    SharedPreferences.setMockInitialValues({
      'auth_token_7': 'legacy-token-fixture',
    });
    final written = <String, dynamic>{};
    TestWidgetsFlutterBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_secureChannel, (call) async {
          if (call.method == 'write') {
            written[call.arguments['key']] = call.arguments['value'];
            return null;
          }
          return null;
        });

    final token = await TokenStorage.readToken(7);

    expect(token, 'legacy-token-fixture');
    expect(written['auth_token_7'], 'legacy-token-fixture');
    final prefs = await SharedPreferences.getInstance();
    expect(prefs.getString('auth_token_7'), isNull);
  });

  test('secureKeysWithPrefix returns empty list when keystore readAll fails', () async {
    SharedPreferences.setMockInitialValues({});
    TestWidgetsFlutterBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(_secureChannel, (call) async {
          throw PlatformException(code: 'keystore_error', message: 'unavailable');
        });

    final keys = await TokenStorage.secureKeysWithPrefix('auth_token_');

    expect(keys, isEmpty);
  });
}
