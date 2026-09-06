abstract final class UpdateConfig {
  static const String baseUrl = String.fromEnvironment(
    'MAYAK_UPDATE_BASE_URL',
    defaultValue: 'https://raw.githubusercontent.com/Davidgo134/Mayak/update',
  );

  static bool get isConfigured => _normalizedBase.isNotEmpty;

  static Uri get manifestUri => Uri.parse(
    '$_normalizedBase/latest.json',
  ).replace(queryParameters: {'t': _cacheBuster});

  static String get downloadsPage => _normalizedBase;

  static String get _normalizedBase {
    var url = baseUrl.trim();
    while (url.endsWith('/')) {
      url = url.substring(0, url.length - 1);
    }
    return url;
  }

  static String get _cacheBuster =>
      (DateTime.now().millisecondsSinceEpoch ~/ 60000).toString();
}
