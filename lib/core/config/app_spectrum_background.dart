import 'package:flutter/foundation.dart';

import 'persisted_setting.dart';

class AppSpectrumBackground {
  static const prefKey = 'app_spectrum_background';
  static const bool defaultValue = false;

  static final _setting = PersistedSetting<bool>(
    prefKey: prefKey,
    defaultValue: defaultValue,
    read: (prefs, key) => prefs.getBool(key),
    write: (prefs, key, value) async {
      await prefs.setBool(key, value);
    },
  );

  static ValueNotifier<bool> get current => _setting.current;

  static bool get isEnabled => false;

  // Функция вырезана: спектр всегда выключен.
  static Future<bool> load() async => false;

  static Future<void> save(bool value) async {}
}
