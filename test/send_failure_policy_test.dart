import 'package:flutter_test/flutter_test.dart';
import 'package:mayak/core/protocol/packet.dart';

void main() {
  test('a server-side rejection stays retryable and keeps the message pending', () {
    final error = PacketError('server error', errorKey: 'internal.error');
    expect(isPermanentSendFailure(error), isFalse);
  });

  test('not.ready stays retryable', () {
    final error = PacketError('not ready', errorKey: 'chat.not.ready');
    expect(isPermanentSendFailure(error), isFalse);
  });

  test('errors without an error key stay retryable', () {
    expect(isPermanentSendFailure(const PacketError('no key')), isFalse);
  });

  test('plain exceptions and session expiry stay retryable', () {
    expect(isPermanentSendFailure(Exception('offline')), isFalse);
    expect(
      isPermanentSendFailure(const SessionExpiredException('token')),
      isFalse,
    );
  });
}
