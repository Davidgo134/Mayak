import 'dart:async';
import 'dart:io';
import 'dart:ui' as ui;

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:material_symbols_icons/symbols.dart';

import '../../../../core/media/native_video_note_recorder.dart';
import '../../../../core/utils/haptics.dart';
import '../../../../core/utils/logger.dart';
import '../../../widgets/custom_notification.dart';
import 'voice_record_controller.dart';

class VideoNoteController {
  VideoNoteController({
    required this.contextOf,
    required this.isMounted,
    required this.onRecorded,
    required this.formatElapsed,
  });

  final BuildContext Function() contextOf;
  final bool Function() isMounted;
  final Future<void> Function(File file, int durationMs) onRecorded;
  final String Function(int ms) formatElapsed;

  final NativeVideoNoteRecorder _rec = NativeVideoNoteRecorder();
  final ValueNotifier<bool> _videoNoteMode = ValueNotifier(false);
  final ValueNotifier<int?> _textureId = ValueNotifier(null);
  final ValueNotifier<bool> _camReady = ValueNotifier(false);
  final ValueNotifier<bool> _isRecording = ValueNotifier(false);
  final ValueNotifier<int> _elapsedMs = ValueNotifier(0);
  final ValueNotifier<double> _cancelDrag = ValueNotifier(0);
  final ValueNotifier<bool> _isFrontCamera = ValueNotifier(true);
  final ValueNotifier<bool> _locked = ValueNotifier(false);
  final ValueNotifier<double> _lockDrag = ValueNotifier(0);
  final ValueNotifier<bool> _switchingCamera = ValueNotifier(false);
  final ValueNotifier<bool> _torchOn = ValueNotifier(false);
  final ValueNotifier<bool> _isPaused = ValueNotifier(false);
  static const double _lockThreshold = 90;
  final Stopwatch _stopwatch = Stopwatch();
  Timer? _timer;
  bool _cancelled = false;
  bool _stopRequested = false;
  OverlayEntry? _overlay;

  ValueListenable<bool> get videoNoteMode => _videoNoteMode;
  ValueListenable<bool> get camReady => _camReady;
  ValueListenable<bool> get isRecording => _isRecording;
  ValueListenable<bool> get isFrontCamera => _isFrontCamera;
  ValueListenable<bool> get switchingCamera => _switchingCamera;
  ValueListenable<bool> get locked => _locked;
  ValueListenable<double> get lockDrag => _lockDrag;

  Future<void> switchCamera() async {
    if (_rec.textureId == null) return;
    if (_switchingCamera.value) return;
    _switchingCamera.value = true;
    try {
      // Таймаут 3с чтобы не вешать UI если нативная сторона не отвечает
      final ok = await _rec.switchCamera().timeout(
        const Duration(seconds: 3),
        onTimeout: () => false,
      );
      if (ok) {
        _isFrontCamera.value = _rec.isFront;
        Haptics.tap();
      }
    } catch (e) {
      logger.w('switchCamera: $e');
    } finally {
      _switchingCamera.value = false;
    }
  }

  Future<void> toggleTorch() async {
    try {
      final newState = !_torchOn.value;
      await _rec.toggleTorch(newState);
      _torchOn.value = newState;
      Haptics.tap();
    } catch (e) {
      logger.w('toggleTorch: $e');
    }
  }

  Future<void> toggleMode() async {
    final toVideo = !_videoNoteMode.value;
    _videoNoteMode.value = toVideo;
    Haptics.tap();
    if (toVideo) {
      await _initCamera();
    } else {
      await _disposeCamera();
    }
  }

  Future<void> _initCamera({bool isFront = true}) async {
    if (_rec.textureId != null) return;
    if (!_rec.isAvailable) {
      if (isMounted()) showCustomNotification(contextOf(), 'Камера недоступна');
      return;
    }
    try {
      final ok = await _rec.init(front: isFront);
      if (!ok) {
        if (isMounted()) {
          showCustomNotification(contextOf(), 'Камера недоступна');
        }
        return;
      }
      if (!isMounted() || !_videoNoteMode.value) {
        await _disposeCamera();
        return;
      }
      _isFrontCamera.value = _rec.isFront;
      _textureId.value = _rec.textureId;
      _camReady.value = true;
    } catch (e) {
      logger.w('initNoteCamera: $e');
      if (isMounted()) showCustomNotification(contextOf(), 'Камера недоступна');
    }
  }

  Future<void> _disposeCamera() async {
    _camReady.value = false;
    _textureId.value = null;
    await _rec.dispose();
  }

  Future<void> startWithCamera({required bool isFront}) async {
    if (_isRecording.value) return;
    _stopRequested = false;
    _locked.value = true;

    if (_rec.textureId == null) {
      await _initCamera(isFront: isFront);
    } else {
      if (isFront != _isFrontCamera.value) {
        await switchCamera();
      }
    }

    try {
      final ok = await _rec.start();
      if (!ok) {
        _isRecording.value = false;
        return;
      }
      if (!isMounted()) {
        await _rec.stop();
        return;
      }
      _stopwatch
        ..reset()
        ..start();
      _elapsedMs.value = 0;
      _cancelDrag.value = 0;
      _lockDrag.value = 0;
      _cancelled = false;
      _isPaused.value = false;
      _isRecording.value = true;
      FocusManager.instance.primaryFocus?.unfocus();
      Haptics.send();
      _timer = Timer.periodic(const Duration(milliseconds: 100), (_) {
        if (!_isPaused.value) _elapsedMs.value = _stopwatch.elapsedMilliseconds;
      });
      _showOverlay();
      if (_stopRequested) {
        _stopRequested = false;
        await stop(cancel: false);
      }
    } catch (e) {
      logger.w('startNoteRecording: $e');
      _isRecording.value = false;
    }
  }

  Future<void> start() async {
    if (_isRecording.value) return;
    _stopRequested = false;
    if (_rec.textureId == null) {
      await _initCamera();
      return;
    }
    try {
      final ok = await _rec.start();
      if (!ok) {
        _isRecording.value = false;
        return;
      }
      if (!isMounted()) {
        await _rec.stop();
        return;
      }
      _stopwatch
        ..reset()
        ..start();
      _elapsedMs.value = 0;
      _cancelDrag.value = 0;
      _locked.value = false;
      _lockDrag.value = 0;
      _cancelled = false;
      _isPaused.value = false;
      _isRecording.value = true;
      FocusManager.instance.primaryFocus?.unfocus();
      Haptics.send();
      _timer = Timer.periodic(const Duration(milliseconds: 100), (_) {
        if (!_isPaused.value) _elapsedMs.value = _stopwatch.elapsedMilliseconds;
      });
      _showOverlay();
      if (_stopRequested) {
        _stopRequested = false;
        await stop(cancel: false);
      }
    } catch (e) {
      logger.w('startNoteRecording: $e');
      _isRecording.value = false;
    }
  }

  void handleDrag(Offset offsetFromOrigin) {
    if (!_isRecording.value || _locked.value) return;

    final lock = (-offsetFromOrigin.dy / _lockThreshold).clamp(0.0, 1.0);
    _lockDrag.value = lock;
    if (lock >= 1.0) {
      _locked.value = true;
      _lockDrag.value = 0;
      _cancelDrag.value = 0;
      Haptics.send();
      return;
    }

    final drag = (-offsetFromOrigin.dx / VoiceRecordController.cancelThreshold)
        .clamp(0.0, 1.0);
    _cancelDrag.value = drag;
    if (drag >= 1.0 && !_cancelled) {
      _cancelled = true;
      Haptics.error();
      stop(cancel: true);
    }
  }

  void handleEnd() {
    if (_locked.value) return;
    stop(cancel: false);
  }

  Future<void> stop({required bool cancel}) async {
    if (!_isRecording.value) {
      _stopRequested = true;
      return;
    }
    _timer?.cancel();
    _timer = null;
    _stopwatch.stop();
    final elapsed = _stopwatch.elapsedMilliseconds;
    _isRecording.value = false;
    _isPaused.value = false;
    _torchOn.value = false;
    _cancelDrag.value = 0;
    _locked.value = false;
    _lockDrag.value = 0;
    _hideOverlay();

    final path = await _rec.stop();

    final shouldCancel =
        cancel || _cancelled || elapsed < VoiceRecordController.minMs;
    if (shouldCancel || path == null) {
      if (path != null) {
        try {
          await File(path).delete();
        } catch (_) {}
      }
      return;
    }

    await onRecorded(File(path), elapsed);
  }

  void _showOverlay() {
    _overlay?.remove();
    _overlay = OverlayEntry(
      builder: (context) {
        final texId = _textureId.value;
        final cs = Theme.of(context).colorScheme;
        final mq = MediaQuery.of(context);
        final screenWidth = mq.size.width;
        final circleSize = screenWidth - 32.0;
        return Positioned.fill(
          child: Container(
            color: Colors.black.withValues(alpha: 0.55),
            child: SafeArea(
              child: Column(
                mainAxisSize: MainAxisSize.max,
                mainAxisAlignment: MainAxisAlignment.end,
                children: [
                  // Кружок занимает всё свободное пространство сверху
                  Expanded(
                    child: Align(
                      alignment: Alignment.center,
                      child: Stack(
                        clipBehavior: Clip.none,
                        alignment: Alignment.center,
                        children: [
                          // Превью камеры
                          ClipOval(
                            child: SizedBox(
                              width: circleSize,
                              height: circleSize,
                              child: texId != null
                                  ? Texture(textureId: texId)
                                  : Container(color: Colors.black),
                            ),
                          ),
                          // Arc progress
                          ValueListenableBuilder<int>(
                            valueListenable: _elapsedMs,
                            builder: (context, ms, _) {
                              final progress = (ms / 60000).clamp(0.0, 1.0);
                              return SizedBox(
                                width: circleSize + 6,
                                height: circleSize + 6,
                                child: CircularProgressIndicator(
                                  value: progress,
                                  strokeWidth: 3,
                                  color: Colors.white,
                                  backgroundColor:
                                      Colors.white.withValues(alpha: 0.2),
                                ),
                              );
                            },
                          ),
                        ],
                      ),
                    ),
                  ),

                  const SizedBox(height: 12),

                  // Ряд: flip|torch pill слева  |  кнопка паузы справа
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    child: Row(
                      mainAxisAlignment: MainAxisAlignment.spaceBetween,
                      children: [
                        // Pill: flip | torch
                        ValueListenableBuilder<bool>(
                          valueListenable: _switchingCamera,
                          builder: (context, switching, _) =>
                              ValueListenableBuilder<bool>(
                            valueListenable: _torchOn,
                            builder: (context, torchOn, _) =>
                                _buildPill(cs, switching, torchOn),
                          ),
                        ),
                        // Кнопка паузы — справа, ровно над кнопкой send
                        ValueListenableBuilder<bool>(
                          valueListenable: _isPaused,
                          builder: (context, paused, _) => Material(
                            color: cs.surfaceContainerHigh,
                            shape: const CircleBorder(),
                            child: InkWell(
                              customBorder: const CircleBorder(),
                              onTap: _togglePause,
                              child: Padding(
                                padding: const EdgeInsets.all(12),
                                child: Icon(
                                  paused
                                      ? Symbols.play_arrow
                                      : Symbols.pause,
                                  color: cs.onSurface,
                                  size: 22,
                                ),
                              ),
                            ),
                          ),
                        ),
                      ],
                    ),
                  ),

                  const SizedBox(height: 8),

                  // Action bar: send | ОТМЕНА | таймер
                  Padding(
                    padding: const EdgeInsets.only(
                        left: 16, right: 16, bottom: 16),
                    child: _buildActionBar(cs),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
    final overlay = Overlay.of(contextOf(), rootOverlay: true);
    overlay.insert(_overlay!);
  }

  Widget _buildPill(ColorScheme cs, bool switching, bool torchOn) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(20),
      child: BackdropFilter(
        filter: ui.ImageFilter.blur(sigmaX: 12, sigmaY: 12),
        child: Container(
          decoration: BoxDecoration(
            color: cs.surface.withValues(alpha: 0.35),
            borderRadius: BorderRadius.circular(20),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              // Flip
              InkWell(
                onTap: switching ? null : switchCamera,
                borderRadius: const BorderRadius.horizontal(
                    left: Radius.circular(20)),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                      horizontal: 16, vertical: 10),
                  child: switching
                      ? SizedBox(
                          width: 18,
                          height: 18,
                          child: CircularProgressIndicator(
                            strokeWidth: 2,
                            color: cs.onSurface,
                          ),
                        )
                      : Icon(Symbols.flip_camera_ios,
                          color: cs.onSurface, size: 20),
                ),
              ),
              Container(
                width: 1,
                height: 28,
                color: cs.outline.withValues(alpha: 0.4),
              ),
              // Torch
              InkWell(
                onTap: toggleTorch,
                borderRadius: const BorderRadius.horizontal(
                    right: Radius.circular(20)),
                child: Padding(
                  padding: const EdgeInsets.symmetric(
                      horizontal: 16, vertical: 10),
                  child: Icon(
                    torchOn
                        ? Symbols.flashlight_on
                        : Symbols.flashlight_off,
                    color: torchOn ? cs.primary : cs.onSurface,
                    size: 20,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  void _togglePause() {
    if (!_isRecording.value) return;
    final pausing = !_isPaused.value;
    if (pausing) {
      _stopwatch.stop();
    } else {
      _stopwatch.start();
    }
    _isPaused.value = pausing;
    Haptics.tap();
  }

  /// Action bar: [send] [ОТМЕНА] [● таймер]
  /// send слева, ОТМЕНА по центру, таймер справа — как в Telegram Beta.
  Widget _buildActionBar(ColorScheme cs) {
    return Container(
      constraints: const BoxConstraints(minWidth: 300),
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
      decoration: BoxDecoration(
        color: cs.surfaceContainerHighest.withValues(alpha: 0.92),
        borderRadius: BorderRadius.circular(28),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        children: [
          // Send — слева
          Material(
            color: cs.primary,
            shape: const CircleBorder(),
            child: InkWell(
              customBorder: const CircleBorder(),
              onTap: () => stop(cancel: false),
              child: Padding(
                padding: const EdgeInsets.all(10),
                child: Icon(Symbols.send, color: cs.onPrimary, size: 20),
              ),
            ),
          ),
          const SizedBox(width: 8),
          // ОТМЕНА — по центру
          Material(
            color: Colors.transparent,
            child: InkWell(
              borderRadius: BorderRadius.circular(20),
              onTap: () => stop(cancel: true),
              child: Padding(
                padding: const EdgeInsets.symmetric(
                    horizontal: 14, vertical: 10),
                child: Text(
                  'ОТМЕНА',
                  style: TextStyle(
                    color: cs.onSurface.withValues(alpha: 0.7),
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                    letterSpacing: 0.3,
                  ),
                ),
              ),
            ),
          ),
          const SizedBox(width: 16),
          // Таймер — справа
          ValueListenableBuilder<int>(
            valueListenable: _elapsedMs,
            builder: (context, ms, _) => Text(
              formatElapsed(ms),
              style: TextStyle(
                color: cs.onSurface,
                fontSize: 15,
                fontFeatures: [ui.FontFeature.tabularFigures()],
              ),
            ),
          ),
          const SizedBox(width: 6),
          // Красная точка рядом с таймером
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(
              color: cs.error,
              shape: BoxShape.circle,
            ),
          ),
        ],
      ),
    );
  }

  void _hideOverlay() {
    _overlay?.remove();
    _overlay = null;
  }

  void dispose() {
    _timer?.cancel();
    _overlay?.remove();
    _rec.dispose();
    _textureId.dispose();
    _videoNoteMode.dispose();
    _camReady.dispose();
    _isRecording.dispose();
    _elapsedMs.dispose();
    _cancelDrag.dispose();
    _isFrontCamera.dispose();
    _locked.dispose();
    _lockDrag.dispose();
    _torchOn.dispose();
    _isPaused.dispose();
  }
}
