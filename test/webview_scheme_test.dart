import 'package:flutter_test/flutter_test.dart';
import 'package:mayak/core/links/max_link.dart';
import 'package:mayak/core/utils/link_opener.dart';

void main() {
  group('leavesWebView', () {
    test('keeps page navigation inside the web view', () {
      for (final scheme in [
        'http',
        'https',
        'HTTPS',
        'about',
        'data',
        'blob',
      ]) {
        expect(leavesWebView(scheme), isFalse, reason: scheme);
      }
    });

    test('hands app schemes over to the app', () {
      for (final scheme in ['max', 'MAX', 'komet', 'tel', 'mailto', 'intent']) {
        expect(leavesWebView(scheme), isTrue, reason: scheme);
      }
    });

    test('treats a missing scheme as in-page', () {
      expect(leavesWebView(null), isFalse);
      expect(leavesWebView(''), isFalse);
    });

    test('javascript and file schemes leave the web view and get cancelled', () {
      expect(leavesWebView('javascript'), isTrue);
      expect(leavesWebView('file'), isTrue);
      expect(leavesWebView('FILE'), isTrue);
    });
  });

  group('webViewOriginMatches', () {
    test('foreign origins never match the launch origin', () {
      final uri = Uri.parse('https://evil.example.com/path');
      expect(webViewOriginMatches(uri, 'bot.example.com'), isFalse);
      expect(webViewOriginMatches(uri, null), isFalse);
      expect(webViewOriginMatches(uri, ''), isFalse);
    });

    test('the launch origin matches itself case-insensitively', () {
      final uri = Uri.parse('https://BOT.example.com/path');
      expect(webViewOriginMatches(uri, 'bot.example.com'), isTrue);
    });

    test('subdomains do not match the parent origin', () {
      final uri = Uri.parse('https://cdn.bot.example.com/x');
      expect(webViewOriginMatches(uri, 'bot.example.com'), isFalse);
    });
  });

  test('a max deep link from a web view resolves to in-app content', () {
    expect(MaxLink.parse('max://max.ru/somechannel'), isA<MaxContentLink>());
    expect(MaxLink.isMaxLink('max://max.ru/?cid=424242'), isTrue);
  });
}
