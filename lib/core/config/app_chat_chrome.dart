import 'package:flutter/foundation.dart';

import '../../frontend/widgets/liquid_glass.dart';
import 'persisted_setting.dart';

enum ChatChromeStyle { color, blur, none, transparent, liquidGlass }

class ChatChromeMaterial {
  static bool isLiquid(ChatChromeStyle style) =>
      style == ChatChromeStyle.liquidGlass && LiquidGlass.isSupported;
}

class AppChatChrome {
  static const prefKey = 'app_chat_chrome';

  static final _setting = PersistedEnum<ChatChromeStyle>(
    prefKey: prefKey,
    defaultValue: ChatChromeStyle.transparent,
    encode: _encode,
    decode: _parse,
  );

  static ValueNotifier<ChatChromeStyle> get current => _setting.current;

  static ChatChromeStyle _parse(String? value) {
    final v = enumFromName(
      ChatChromeStyle.values,
      value,
      ChatChromeStyle.transparent,
    );
    // Доступны только Нет и Frost blur; прежние варианты мигрируют.
    return v == ChatChromeStyle.none ? v : ChatChromeStyle.transparent;
  }

  static String _encode(ChatChromeStyle value) => value.name;

  static Future<ChatChromeStyle> load() => _setting.load();

  static Future<void> save(ChatChromeStyle value) => _setting.save(value);
}
