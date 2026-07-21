import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:material_symbols_icons/symbols.dart';
import 'package:video_player/video_player.dart';
import 'package:komet/main.dart';

import '../../../../backend/modules/messages.dart';
import '../../../../core/utils/haptics.dart';
import '../../../../core/utils/media_cache.dart';
import '../../../../core/utils/logger.dart';
import '../../../../models/attachment.dart';
import '../../small_spinner.dart';
import '../../round_video_pip.dart';

/// Round video message bubble, Telegram-Android style:
/// - Tap expands the circle in place (no separate fullscreen route).
/// - A top bar appears above it: play/pause on the left, speed toggle
///   and a close (X) button on the right. Close = fully stop & collapse.
/// - Scrolling the bubble off-screen while playing hands the controller
///   off to the global floating PiP mini player instead of stopping it.
/// - Dragging along the ring edge scrubs; the thumb dot only shows while
///   paused, matching Telegram's behavior.
class VideoNoteBubble extends StatefulWidget {
  final VideoAttachment attachment;
  final String messageId;
  final int chatId;
  final ColorScheme cs;
  final Color? textColor;

  const VideoNoteBubble({
    super.key,
    required this.attachment,
    required this.messageId,
    required this.chatId,
    required this.cs,
    this.textColor,
  });

  @override
  State<VideoNoteBubble> createState() => _VideoNoteBubbleState();
}

class _VideoNoteBubbleState extends State<VideoNoteBubble>
    with SingleTickerProviderStateMixin {
  static const double _collapsedSize = 210;
  static const double _expandedSize = 260;

  bool _expanded = false;
  bool _opening = false;
  bool _error = false;
  VideoPlayerController? _controller;
  double _speed = 1.0;

  bool _seeking = false;
  double _seekProgress = 0;

  bool _transcriptionVisible = false;
  String? _transcriptionText;
  bool _transcriptionLoading = false;
  StreamSubscription<TranscriptionResult>? _transcriptionSub;
  late final AnimationController _transcriptionIconAnim = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 900),
  );

  @override
  void initState() {
    super.initState();
    _transcriptionSub = TranscriptionCache.updates.listen((result) {
      if (result.messageId != widget.messageId) return;
      if (!mounted) return;
      setState(() {
        _transcriptionLoading = false;
        _transcriptionIconAnim.stop();
        if (result.status == 1) {
          _transcriptionText = (result.text == null || result.text!.isEmpty)
              ? 'не удалось распознать текст'
              : result.text;
          _transcriptionVisible = true;
        } else if (result.status != 0) {
          _transcriptionText = 'ошибка транскрибации';
          _transcriptionVisible = true;
        }
      });
    });
  }

  @override
  void dispose() {
    _transcriptionSub?.cancel();
    _transcriptionIconAnim.dispose();
    final c = _controller;
    if (c != null &&
        RoundVideoPipController.instance.state.value?.controller != c) {
      c.removeListener(_onTick);
      c.dispose();
    }
    roundVideoPanelState.value = null;
    super.dispose();
  }

  static Uint8List? _previewBytes(String? data) {
    if (data == null) return null;
    const marker = 'base64,';
    final idx = data.indexOf(marker);
    if (idx < 0) return null;
    try {
      return base64Decode(data.substring(idx + marker.length));
    } catch (_) {
      return null;
    }
  }

  void _onTick() {
    if (mounted && !_seeking) setState(() {});
  }

  Future<void> _toggleExpand() async {
    if (_expanded) {
      _togglePlay();
      return;
    }
    Haptics.tap();
    setState(() {
      _expanded = true;
      _opening = true;
    });
    try {
      final a = widget.attachment;
      final videoId = a.videoId;
      final token = a.videoToken;
      if (videoId == null || token == null) {
        setState(() {
          _error = true;
          _opening = false;
        });
        return;
      }
      final cacheName = 'videonote_$videoId.mp4';
      var file = await MediaCache.existing(cacheName);
      if (file == null) {
        final url = await messagesModule.getVideoUrl(
          messageId: widget.messageId,
          chatId: widget.chatId,
          token: token,
          videoId: videoId,
        );
        if (url == null) throw Exception('no_url');
        file = await MediaCache.getOrDownload(cacheName, url);
        if (file == null) throw Exception('download');
      }
      if (!mounted) return;
      final c = VideoPlayerController.file(file);
      _controller = c;
      await c.initialize();
      if (!mounted) {
        c.dispose();
        return;
      }
      await c.setLooping(true);
      c.addListener(_onTick);
      c.play();
      setState(() {
        _opening = false;
      });
      _publishPanelState();
    } catch (e) {
      logger.w('VideoNoteBubble._toggleExpand: $e');
      if (mounted) {
        setState(() {
          _opening = false;
          _error = true;
        });
      }
    }
  }

  void _publishPanelState() {
    if (!_expanded) {
      roundVideoPanelState.value = null;
      return;
    }
    final c = _controller;
    roundVideoPanelState.value = RoundVideoPanelState(
      isPlaying: c?.value.isPlaying ?? false,
      speed: _speed,
      onTogglePlay: _togglePlay,
      onCycleSpeed: _cycleSpeed,
      onClose: _closeExpanded,
    );
  }

  void _togglePlay() {
    final c = _controller;
    if (c == null || !c.value.isInitialized) return;
    Haptics.tap();
    setState(() => c.value.isPlaying ? c.pause() : c.play());
    _publishPanelState();
  }

  void _cycleSpeed() {
    final c = _controller;
    if (c == null || !c.value.isInitialized) return;
    Haptics.tap();
    const speeds = [1.0, 1.5, 2.0];
    final idx = speeds.indexOf(_speed);
    final next = speeds[(idx + 1) % speeds.length];
    setState(() => _speed = next);
    c.setPlaybackSpeed(next);
    _publishPanelState();
  }

  /// Close (X) button: fully stops playback and collapses back to the
  /// small in-feed circle. No PiP is created — this is a hard stop,
  /// distinct from scrolling away (which hands off to PiP).
  void _closeExpanded() {
    Haptics.tap();
    final c = _controller;
    c?.pause();
    c?.removeListener(_onTick);
    c?.dispose();
    setState(() {
      _controller = null;
      _expanded = false;
      _seeking = false;
    });
    _publishPanelState();
  }

  /// Hands the currently playing controller off to the global floating
  /// PiP mini player (Telegram's PipRoundVideoView behavior) instead of
  /// stopping it, e.g. when the bubble scrolls off-screen or the user
  /// navigates to another screen while it's still playing.
  void handOffToPip() {
    final c = _controller;
    if (c == null || !c.value.isInitialized || !c.value.isPlaying) return;
    RoundVideoPipController.instance.activate(
      PipData(
        controller: c,
        messageId: widget.messageId,
        chatId: widget.chatId,
        onDisposeIfOwned: () {
          c.removeListener(_onTick);
          c.dispose();
        },
        onExpand: (context) {
          RoundVideoPipController.instance.clear();
        },
      ),
    );
    setState(() {
      _controller = null;
      _expanded = false;
    });
    _publishPanelState();
  }

  double _angleToProgress(Offset local, double size) {
    final center = Offset(size / 2, size / 2);
    final d = local - center;
    var angle = math.atan2(d.dy, d.dx) + math.pi / 2;
    if (angle < 0) angle += 2 * math.pi;
    return (angle / (2 * math.pi)).clamp(0.0, 1.0);
  }

  bool _nearRingEdge(Offset local, double size) {
    final center = Offset(size / 2, size / 2);
    final dist = (local - center).distance;
    final radius = size / 2;
    return dist > radius - 28;
  }

  void _onRingPanStart(DragStartDetails details, double size) {
    final c = _controller;
    if (c == null || !c.value.isInitialized) return;
    if (!_nearRingEdge(details.localPosition, size)) return;
    Haptics.tap();
    setState(() {
      _seeking = true;
      _seekProgress = _angleToProgress(details.localPosition, size);
    });
  }

  void _onRingPanUpdate(DragUpdateDetails details, double size) {
    if (!_seeking) return;
    setState(() {
      _seekProgress = _angleToProgress(details.localPosition, size);
    });
  }

  void _onRingPanEnd(DragEndDetails details) {
    final c = _controller;
    if (c != null &&
        c.value.isInitialized &&
        c.value.duration.inMilliseconds > 0) {
      final target = Duration(
        milliseconds: (c.value.duration.inMilliseconds * _seekProgress)
            .round(),
      );
      c.seekTo(target);
    }
    setState(() => _seeking = false);
  }

  String _formatTime(Duration d) {
    final totalSeconds = d.inSeconds;
    final minutes = totalSeconds ~/ 60;
    final seconds = totalSeconds % 60;
    return '$minutes:${seconds.toString().padLeft(2, '0')}';
  }

  Future<void> _requestTranscription() async {
    final videoId = widget.attachment.videoId;
    if (videoId == null) return;

    if (_transcriptionVisible && _transcriptionText != null) {
      setState(() => _transcriptionVisible = false);
      return;
    }

    if (TranscriptionCache.has(widget.messageId)) {
      final cached = TranscriptionCache.get(widget.messageId)!;
      setState(() {
        _transcriptionText = cached.text ?? 'не удалось распознать текст';
        _transcriptionVisible = true;
      });
      return;
    }

    Haptics.tap();
    setState(() => _transcriptionLoading = true);
    _transcriptionIconAnim.repeat();

    try {
      final result = await messagesModule.requestTranscription(
        widget.chatId,
        int.tryParse(widget.messageId) ?? 0,
        videoId,
      );

      TranscriptionCache.put(widget.messageId, result);

      if (!mounted) return;
      setState(() {
        _transcriptionLoading = false;
        _transcriptionIconAnim.stop();
        if (result.status == 1) {
          _transcriptionText = (result.text == null || result.text!.isEmpty)
              ? 'не удалось распознать текст'
              : result.text;
          _transcriptionVisible = true;
        } else if (result.status == 0) {
          _transcriptionText = 'транскрибация...';
          _transcriptionVisible = true;
        } else {
          _transcriptionText = 'ошибка транскрибации';
          _transcriptionVisible = true;
        }
      });
    } catch (e) {
      if (!mounted) return;
      setState(() {
        _transcriptionLoading = false;
        _transcriptionIconAnim.stop();
        _transcriptionText = 'ошибка транскрибации';
        _transcriptionVisible = true;
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    final a = widget.attachment;
    final preview = _previewBytes(a.previewData);
    final textColor = widget.textColor ?? widget.cs.onSurface;
    final c = _controller;
    final ready = c != null && c.value.isInitialized;
    final size = _expanded ? _expandedSize : _collapsedSize;

    double progress = 0;
    if (ready && c.value.duration.inMilliseconds > 0) {
      progress =
          c.value.position.inMilliseconds / c.value.duration.inMilliseconds;
    }
    final shownProgress = _seeking ? _seekProgress : progress.clamp(0.0, 1.0);
    final duration = ready ? c.value.duration : Duration.zero;
    final elapsed = ready
        ? (_seeking
              ? Duration(
                  milliseconds:
                      (duration.inMilliseconds * _seekProgress).round(),
                )
              : c.value.position)
        : Duration.zero;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        AnimatedContainer(
          duration: const Duration(milliseconds: 260),
          curve: Curves.easeOutCubic,
          width: size + (_expanded ? 36 : 0),
          height: size + (_expanded ? 36 : 0),
          child: Stack(
            alignment: Alignment.center,
            clipBehavior: Clip.none,
            children: [
              if (_expanded)
                GestureDetector(
                  behavior: HitTestBehavior.opaque,
                  onPanStart: (d) => _onRingPanStart(d, size + 36),
                  onPanUpdate: (d) => _onRingPanUpdate(d, size + 36),
                  onPanEnd: _onRingPanEnd,
                  child: SizedBox(
                    width: size + 36,
                    height: size + 36,
                    child: CustomPaint(
                      size: Size(size + 36, size + 36),
                      painter: _RingPainter(
                        progress: shownProgress,
                        showThumb: ready && !c!.value.isPlaying,
                      ),
                    ),
                  ),
                ),
              GestureDetector(
                onTap: _toggleExpand,
                child: AnimatedContainer(
                  duration: const Duration(milliseconds: 260),
                  curve: Curves.easeOutCubic,
                  width: size,
                  height: size,
                  child: ClipOval(
                    child: SizedBox(
                      width: size,
                      height: size,
                      child: ready
                          ? FittedBox(
                              fit: BoxFit.cover,
                              clipBehavior: Clip.hardEdge,
                              child: SizedBox(
                                width: c!.value.size.width,
                                height: c.value.size.height,
                                child: VideoPlayer(c),
                              ),
                            )
                          : (preview != null
                                ? Image.memory(
                                    preview,
                                    fit: BoxFit.cover,
                                    gaplessPlayback: true,
                                  )
                                : Container(
                                    color: widget.cs.surfaceContainerHighest,
                                  )),
                    ),
                  ),
                ),
              ),
              if (_opening)
                const IgnorePointer(
                  child: SmallSpinner(size: 40, color: Colors.white),
                ),
              if (_error && !_opening)
                const IgnorePointer(
                  child: Icon(Symbols.error, color: Colors.white54, size: 48),
                ),
              if (!_expanded && !_opening)
                IgnorePointer(
                  child: Container(
                    width: 52,
                    height: 52,
                    decoration: const BoxDecoration(
                      color: Colors.black45,
                      shape: BoxShape.circle,
                    ),
                    child: const Icon(
                      Symbols.play_arrow,
                      color: Colors.white,
                      size: 30,
                    ),
                  ),
                ),
              if (_expanded && ready && !c!.value.isPlaying)
                IgnorePointer(
                  child: Container(
                    width: 64,
                    height: 64,
                    decoration: const BoxDecoration(
                      color: Colors.black38,
                      shape: BoxShape.circle,
                    ),
                    child: const Icon(
                      Symbols.play_arrow,
                      color: Colors.white,
                      size: 34,
                    ),
                  ),
                ),
              if (_expanded && ready)
                Positioned(
                  bottom: 4,
                  left: 8,
                  child: Text(
                    _formatTime(elapsed),
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      shadows: [Shadow(color: Colors.black54, blurRadius: 4)],
                    ),
                  ),
                ),
              if (_expanded && ready)
                Positioned(
                  bottom: 4,
                  right: 8,
                  child: Text(
                    _formatTime(duration),
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                      shadows: [Shadow(color: Colors.black54, blurRadius: 4)],
                    ),
                  ),
                ),
              if (!_expanded && widget.attachment.videoId != null)
                Positioned(
                  right: 8,
                  bottom: 8,
                  child: GestureDetector(
                    onTap: _requestTranscription,
                    child: SizedBox(
                      width: 28,
                      height: 32,
                      child: Center(
                        child: _transcriptionLoading
                            ? RotationTransition(
                                turns: _transcriptionIconAnim,
                                child: Icon(
                                  Symbols.graphic_eq,
                                  color: Colors.white.withValues(alpha: 0.9),
                                  size: 16,
                                  shadows: const [
                                    Shadow(
                                      color: Colors.black54,
                                      blurRadius: 4,
                                    ),
                                  ],
                                ),
                              )
                            : Text(
                                'Т',
                                style: TextStyle(
                                  color: _transcriptionVisible
                                      ? widget.cs.primary
                                      : Colors.white.withValues(alpha: 0.9),
                                  fontSize: 15,
                                  fontWeight: FontWeight.w700,
                                  shadows: const [
                                    Shadow(
                                      color: Colors.black54,
                                      blurRadius: 4,
                                    ),
                                  ],
                                ),
                              ),
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
        AnimatedSize(
          duration: const Duration(milliseconds: 260),
          curve: Curves.easeOutCubic,
          alignment: Alignment.topLeft,
          child: _transcriptionVisible
              ? Container(
                  key: const ValueKey('transcription'),
                  margin: const EdgeInsets.only(top: 8),
                  width: _collapsedSize,
                  padding: const EdgeInsets.symmetric(
                    horizontal: 12,
                    vertical: 10,
                  ),
                  decoration: BoxDecoration(
                    color: widget.cs.surfaceContainerHighest.withValues(
                      alpha: 0.6,
                    ),
                    borderRadius: BorderRadius.circular(14),
                  ),
                  child: AnimatedSwitcher(
                    duration: const Duration(milliseconds: 200),
                    child: Text(
                      _transcriptionText ?? '',
                      key: ValueKey(_transcriptionText),
                      style: TextStyle(
                        color: textColor.withValues(alpha: 0.85),
                        fontSize: 13,
                        height: 1.35,
                      ),
                    ),
                  ),
                )
              : const SizedBox(
                  key: ValueKey('no-transcription'),
                  width: _collapsedSize,
                  height: 0,
                ),
        ),
      ],
    );
  }
}

class _RingPainter extends CustomPainter {
  final double progress;
  final bool showThumb;

  _RingPainter({required this.progress, required this.showThumb});

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = size.width / 2 - 4;

    final trackPaint = Paint()
      ..color = Colors.white.withValues(alpha: 0.3)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3;
    canvas.drawCircle(center, radius, trackPaint);

    final progressPaint = Paint()
      ..color = Colors.white
      ..style = PaintingStyle.stroke
      ..strokeWidth = 3
      ..strokeCap = StrokeCap.round;
    canvas.drawArc(
      Rect.fromCircle(center: center, radius: radius),
      -math.pi / 2,
      2 * math.pi * progress,
      false,
      progressPaint,
    );

    if (showThumb) {
      final thumbAngle = -math.pi / 2 + 2 * math.pi * progress;
      final thumbCenter = Offset(
        center.dx + radius * math.cos(thumbAngle),
        center.dy + radius * math.sin(thumbAngle),
      );
      final thumbShadowPaint = Paint()
        ..color = Colors.black.withValues(alpha: 0.35)
        ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 3);
      canvas.drawCircle(thumbCenter, 9, thumbShadowPaint);
      final thumbPaint = Paint()..color = Colors.white;
      canvas.drawCircle(thumbCenter, 8, thumbPaint);
    }
  }

  @override
  bool shouldRepaint(covariant _RingPainter oldDelegate) =>
      oldDelegate.progress != progress || oldDelegate.showThumb != showThumb;
}
