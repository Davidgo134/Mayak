import 'dart:io';

import 'package:flutter/services.dart';

import '../utils/logger.dart';

class NativeVideoNoteRecorder {
  static const _channel = MethodChannel('ru.mayak.app/video_note');

  int? textureId;
  bool isFront = true;
  bool hasTorch = false;
  bool get isAvailable => Platform.isAndroid;

  Future<bool> init({bool front = true}) async {
    if (!isAvailable) return false;
    try {
      final res = await _channel.invokeMapMethod<String, dynamic>('init', {
        'front': front,
      });
      textureId = res?['textureId'] as int?;
      isFront = front;
      hasTorch = res?['hasTorch'] as bool? ?? false;
      return textureId != null;
    } catch (e) {
      logger.w('NativeVideoNoteRecorder.init: $e');
      return false;
    }
  }

  Future<bool> switchCamera() async {
    if (!isAvailable) return false;
    try {
      final res = await _channel.invokeMapMethod<String, dynamic>(
        'switchCamera',
      );
      final front = res?['isFront'] as bool?;
      if (front != null) isFront = front;
      hasTorch = res?['hasTorch'] as bool? ?? false;
      return front != null;
    } catch (e) {
      logger.w('NativeVideoNoteRecorder.switchCamera: $e');
      return false;
    }
  }

  /// Returns the ACTUAL torch state reported by the native side after the
  /// request, not just an echo of [on]. The native layer only turns the LED
  /// on when the current camera actually has flash hardware
  /// (`FLASH_INFO_AVAILABLE`) and is the back camera; on any other case (no
  /// hardware, front camera, apply failure) it reports back `false`. The
  /// Dart UI must reflect this real state instead of blindly toggling the
  /// icon.
  Future<bool> toggleTorch(bool on) async {
    if (!isAvailable) return false;
    try {
      final result = await _channel.invokeMethod<bool>('toggleTorch', {
        'on': on,
      });
      return result ?? false;
    } catch (e) {
      logger.w('NativeVideoNoteRecorder.toggleTorch: $e');
      return false;
    }
  }

  Future<bool> start() async {
    if (!isAvailable) return false;
    try {
      await _channel.invokeMethod('start');
      return true;
    } catch (e) {
      logger.w('NativeVideoNoteRecorder.start: $e');
      return false;
    }
  }

  Future<String?> stop() async {
    if (!isAvailable) return null;
    try {
      return await _channel.invokeMethod<String>('stop');
    } catch (e) {
      logger.w('NativeVideoNoteRecorder.stop: $e');
      return null;
    }
  }

  Future<void> dispose() async {
    if (!isAvailable) return;
    try {
      await _channel.invokeMethod('dispose');
    } catch (_) {}
    textureId = null;
  }
}
