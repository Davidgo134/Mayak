import 'package:flutter/foundation.dart';

import 'persisted_setting.dart';

class AppPillGradient {
  static const prefKey = 'app_pill_gradient';

  static final _setting = PersistedSetting<bool>(
    prefKey: prefKey,
    defaultValue: false,
    read: (prefs, key) => prefs.getBool(key),
    write: (prefs, key, value) async {
      await prefs.setBool(key, value);
    },
  );

  static ValueNotifier<bool> get current => _setting.current;

  // Функция вырезана: градиент всегда выключен.
  static Future<bool> load() async => false;

  static Future<void> save(bool value) async {}
}
