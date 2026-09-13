import 'package:flutter/services.dart' show PlatformException;
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:shared_preferences/shared_preferences.dart';

class TokenStorage {
  static const _tokenPrefix = 'auth_token_';
  static const _activeAccountKey = 'active_account_id';

  static const _secure = FlutterSecureStorage(
    aOptions: AndroidOptions(
      resetOnError: true,
      encryptedSharedPreferences: true,
    ),
    iOptions: IOSOptions(
      accessibility: KeychainAccessibility.first_unlock_this_device,
      synchronizable: false,
    ),
    mOptions: MacOsOptions(usesDataProtectionKeychain: true),
  );

  static const int _duplicateKeychainItem = -25299;

  static bool _isDuplicateItem(PlatformException error) =>
      error.details == _duplicateKeychainItem ||
      (error.message?.contains('$_duplicateKeychainItem') ?? false);

  static Future<void> _delete(String key) async {
    try {
      await _secure.delete(key: key);
    } on PlatformException catch (_) {}
  }

  static Future<void> _write(String key, String value) async {
    try {
      await _secure.write(key: key, value: value);
    } on PlatformException catch (e) {
      await _delete(key);
      if (!_isDuplicateItem(e)) return;
      try {
        await _secure.write(key: key, value: value);
      } on PlatformException catch (_) {}
    }
  }

  static Future<void> writeSecure(String key, String value) =>
      _write(key, value);

  static Future<String?> readSecure(String key) async {
    try {
      return await _secure.read(key: key);
    } on PlatformException catch (_) {
      await _delete(key);
      return null;
    }
  }

  static Future<void> deleteSecure(String key) => _delete(key);

  static Future<List<String>> secureKeysWithPrefix(String prefix) async {
    try {
      final all = await _secure.readAll();
      return all.keys.where((key) => key.startsWith(prefix)).toList();
    } on PlatformException catch (_) {
      return [];
    }
  }

  static Future<void> saveToken(String token, int accountId) =>
      _write('$_tokenPrefix$accountId', token);

  static Future<String?> readToken(int accountId) async {
    final key = '$_tokenPrefix$accountId';
    final secured = await readSecure(key);
    if (secured != null) return secured;

    final prefs = await SharedPreferences.getInstance();
    final legacy = prefs.getString(key);
    if (legacy != null) {
      await _write(key, legacy);
      await prefs.remove(key);
      return legacy;
    }
    return null;
  }

  static Future<void> deleteToken(int accountId) async {
    await _delete('$_tokenPrefix$accountId');
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove('$_tokenPrefix$accountId');
  }

  static Future<void> setActiveAccount(int accountId) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setString(_activeAccountKey, accountId.toString());
  }

  static Future<void> clearActiveAccount() async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_activeAccountKey);
  }

  static Future<int?> getActiveAccountId() async {
    final prefs = await SharedPreferences.getInstance();
    final val = prefs.getString(_activeAccountKey);
    return val != null ? int.tryParse(val) : null;
  }

  static Future<String?> readActiveToken() async {
    final id = await getActiveAccountId();
    if (id == null) return null;
    return await readToken(id);
  }

  static Future<void> deleteAccount(int accountId) async {
    await deleteToken(accountId);
    final activeId = await getActiveAccountId();
    if (activeId == accountId) {
      final prefs = await SharedPreferences.getInstance();
      await prefs.remove(_activeAccountKey);
    }
  }
}
