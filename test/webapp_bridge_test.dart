import 'dart:convert';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:mayak/frontend/screens/webapp/web_app_bridge.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late List<(String, Map<String, dynamic>, bool)> sent;
  late int closeCalls;

  WebAppBridge buildBridge({
    bool privateChannel = false,
    String entryPoint = WebAppEntryPoint.webApp,
    BiometryAccessResolver? biometryAccessResolver,
    BiometryAuthenticator? biometryAuthenticator,
  }) {
    return WebAppBridge(
      botId: 777,
      entryPoint: entryPoint,
      privateChannel: privateChannel,
      contextResolver: () => null,
      viewportResolver: () => const Size(420, 800),
      onClose: () => closeCalls++,
      biometryAccessResolver: biometryAccessResolver,
      biometryAuthenticator: biometryAuthenticator,
      emitter: (method, payload, private) => sent.add((
        method,
        jsonDecode(payload) as Map<String, dynamic>,
        private,
      )),
    );
  }

  setUp(() {
    sent = [];
    closeCalls = 0;
  });

  test('reports the launch context it was created with', () async {
    final bridge = buildBridge(entryPoint: WebAppEntryPoint.inlineButton);

    await bridge.handleEvent(
      'WebAppGetLaunchContext',
      '{"requestId":"r1"}',
      false,
    );

    expect(sent, hasLength(1));
    expect(sent.first.$1, 'WebAppGetLaunchContext');
    expect(sent.first.$2, {
      'requestId': 'r1',
      'entryPoint': 'inline_button',
    });
  });

  test('answers viewport requests with the current webview size', () async {
    final bridge = buildBridge();

    await bridge.handleEvent(
      'WebAppGetViewportSize',
      '{"requestId":"r2"}',
      false,
    );

    expect(sent.first.$2['width'], 420);
    expect(sent.first.$2['height'], 800);
    expect(sent.first.$2['isStateStable'], isTrue);
  });

  test('rejects an unknown method with the client error code', () async {
    final bridge = buildBridge();

    await bridge.handleEvent('WebAppSomethingElse', '{"requestId":"r3"}', false);

    expect(sent.first.$2['error'], {
      'code': 'client.unsupported_method.unsupported_method',
    });
  });

  test('stays silent for methods that never get an answer', () async {
    final bridge = buildBridge();

    await bridge.handleEvent('WebAppReady', '{}', false);
    await bridge.handleEvent('WebAppStat', '{}', false);

    expect(sent, isEmpty);
  });

  test('drops gesture-gated methods until the user touches the page', () async {
    final bridge = buildBridge();

    await bridge.handleEvent(
      'WebAppShare',
      '{"requestId":"r4","text":"hi"}',
      false,
    );
    expect(sent, isEmpty);

    bridge.registerGesture();
    await bridge.handleEvent(
      'WebAppShare',
      '{"requestId":"r5"}',
      false,
    );

    expect(sent.single.$2['error'], {'code': 'client.web_app_share.invalid_request'});
  });

  test('ignores private-channel events when the channel is off', () async {
    final bridge = buildBridge();

    await bridge.handleEvent(
      'WebAppVerifyMobileId',
      '{"requestId":"r6","url":"https://example.test/verify"}',
      true,
    );

    expect(sent, isEmpty);
  });

  test('reports malformed payloads as a decode error', () async {
    final bridge = buildBridge();

    await bridge.handleEvent('WebAppGetViewportSize', 'not-json', false);

    expect(sent, isEmpty);
  });

  test('tracks the back button and closing behaviour the app asked for', () async {
    final bridge = buildBridge();

    expect(bridge.handlesBackButton, isFalse);
    expect(bridge.needsCloseConfirmation, isFalse);

    await bridge.handleEvent(
      'WebAppSetupBackButton',
      '{"isVisible":true}',
      false,
    );
    await bridge.handleEvent(
      'WebAppSetupClosingBehavior',
      '{"needConfirmation":true}',
      false,
    );

    expect(bridge.handlesBackButton, isTrue);
    expect(bridge.needsCloseConfirmation, isTrue);

    bridge.notifyBackPressed();
    expect(sent.single.$1, 'WebAppBackButtonPressed');
  });

  test('closes the screen when the app asks to', () async {
    final bridge = buildBridge();

    await bridge.handleEvent('WebAppClose', '{}', false);

    expect(closeCalls, 1);
  });

  test('echoes the screen capture behaviour back', () async {
    final bridge = buildBridge();

    await bridge.handleEvent(
      'WebAppSetupScreenCaptureBehavior',
      '{"requestId":"r7","isScreenCaptureEnabled":true}',
      false,
    );

    expect(sent.first.$2, {
      'requestId': 'r7',
      'isScreenCaptureEnabled': true,
    });
  });

  group('biometry request auth', () {
    const secureChannel = MethodChannel(
      'plugins.it_nomads.com/flutter_secure_storage',
    );

    test('fails and creates no token when the authenticator refuses', () async {
      SharedPreferences.setMockInitialValues({'active_account_id': '1'});
      final bridge = buildBridge(
        biometryAccessResolver: (accountId, botId) async => (true, true),
        biometryAuthenticator: (reason) async => false,
      );

      await bridge.handleEvent(
        'WebAppBiometryRequestAuth',
        '{"requestId":"bio1"}',
        false,
      );

      expect(sent, hasLength(1));
      expect(sent.first.$2['error'], isNotNull);
      expect(sent.first.$2.containsKey('token'), isFalse);
    });

    test('is refused without prior consent even if biometrics succeed', () async {
      SharedPreferences.setMockInitialValues({'active_account_id': '1'});
      final bridge = buildBridge(
        biometryAccessResolver: (accountId, botId) async => (false, false),
        biometryAuthenticator: (reason) async => true,
      );

      await bridge.handleEvent(
        'WebAppBiometryRequestAuth',
        '{"requestId":"bio2"}',
        false,
      );

      expect(sent.first.$2['error'], isNotNull);
      expect(sent.first.$2.containsKey('token'), isFalse);
    });

    test('mints a token only after a successful authenticator', () async {
      SharedPreferences.setMockInitialValues({'active_account_id': '1'});
      final written = <String>[];
      TestWidgetsFlutterBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(secureChannel, (call) async {
            if (call.method == 'write') {
              written.add(call.arguments['key'].toString());
            }
            return null;
          });
      final bridge = buildBridge(
        biometryAccessResolver: (accountId, botId) async => (true, true),
        biometryAuthenticator: (reason) async => true,
      );

      await bridge.handleEvent(
        'WebAppBiometryRequestAuth',
        '{"requestId":"bio3"}',
        false,
      );

      expect(sent.first.$2['status'], 'authorized');
      expect(sent.first.$2['token'], isNotNull);
      expect(written.any((key) => key.contains('biometry')), isTrue);

      TestWidgetsFlutterBinding.instance.defaultBinaryMessenger
          .setMockMethodCallHandler(secureChannel, null);
    });

    test('fails when nobody is logged in', () async {
      SharedPreferences.setMockInitialValues({});
      final bridge = buildBridge(
        biometryAccessResolver: (accountId, botId) async => (true, true),
        biometryAuthenticator: (reason) async => true,
      );

      await bridge.handleEvent(
        'WebAppBiometryRequestAuth',
        '{"requestId":"bio4"}',
        false,
      );

      expect(sent.first.$2['error'], isNotNull);
      expect(sent.first.$2.containsKey('token'), isFalse);
    });
  });

    test('answers NFC availability without pretending to support it', () async {

    final bridge = buildBridge();

    await bridge.handleEvent('WebAppNfcGetInfo', '{"requestId":"r8"}', false);
    await bridge.handleEvent(
      'WebAppNfcEmulateNfcTag',
      '{"requestId":"r9"}',
      false,
    );

    expect(sent[0].$2, {
      'requestId': 'r8',
      'available': false,
      'enabled': false,
    });
    expect(sent[1].$2['error'], {
      'code': 'client.nfc_emulate_nfc_tag.not_supported',
    });
  });
}
