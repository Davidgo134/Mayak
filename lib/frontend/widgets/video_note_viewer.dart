import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:material_symbols_icons/symbols.dart';
import 'package:video_player/video_player.dart';
import 'package:komet/main.dart';

import '../../core/utils/haptics.dart';
import '../../core/utils/logger.dart';
import '../../core/utils/media_cache.dart';
import '../../models/attachment.dart';
import 'small_spinner.dart';

/// Opens a fullscreen, enlarged view of a video note (circle message) with
/// a circular scrubber ring around it for seeking — mirrors the "killer
/// feature" from Telegram/MAX where tapping a video note blows it up and
/// lets you drag around a round progress ring to seek, plus tap to
/// play/pause.
Future<void> openVideoNoteViewer(
  BuildContext context, {
  required VideoAttachment attachment,
  required String messageId,
  required int chatId,
}) {
  Haptics.tap();
  return Navigator.of(context).push(
    PageRouteBuilder<void>(
      opaque: false,
      barrierColor: Colors.black,
      transitionDuration: const Duration(milliseconds: 260),
      reverseTransitionDuration: const Duration(milliseconds: 200),
      pageBuilder: (_, __, ___) => VideoNoteViewer(
        attachment: attachment,
        messageId: messageId,
        chatId: chatId,
      ),
      transitionsBuilder: (_, animation, __, child) {
        final curved = CurvedAnimation(
          parent: animation,
          curve: Curves.easeOutCubic,
        );
        return FadeTransition(
          opacity: curved,
          child: ScaleTransition(
            scale: Tween<double>(begin: 0.82, end: 1.0).animate(curved),
            child: child,
          ),
        );
      },
    ),
  );
}

class VideoNoteViewer extends StatefulWidget {
  final VideoAttachment attachment;
  final String messageId;
  final int chatId;

  const VideoNoteViewer({
    super.key,
    required this.attachment,
    required this.messageId,
    required this.chatId,
  });

  @override
  State<VideoNoteViewer> createState() => _VideoNoteViewerState();
}

class _VideoNoteViewerState extends State<VideoNoteViewer> {
  VideoPlayerController? _controller;
  bool _loading = true;
  bool _error = false;
  bool _seeking = false;
  double _seekProgress = 0;
  double _dismissOffset = 0;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    final a = widget.attachment;
    final videoId = a.videoId;
    final token = a.videoToken;
    if (videoId == null || token == null) {
      if (mounted) {
        setState(() {
          _loading = false;
          _error = true;
        });
      }
      return;
    }
    try {
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
      setState(() => _loading = false);
    } catch (e) {
      logger.w('VideoNoteViewer._load: $e');
      if (mounted) {
        setState(() {
          _loading = false;
          _error = true;
        });
      }
    }
  }

  void _onTick() {
    if (mounted && !_seeking) setState(() {});
  }

  @override
  void dispose() {
    _controller?.removeListener(_onTick);
    _controller?.dispose();
    super.dispose();
  }

  void _togglePlay() {
    final c = _controller;
    if (c == null || !c.value.isInitialized) return;
    Haptics.tap();
    setState(() => c.value.isPlaying ? c.pause() : c.play());
  }

  double _angleToProgress(Offset local, double size) {
    final center = Offset(size / 2, size / 2);
    final d = local - center;
    var angle = math.atan2(d.dy, d.dx) + math.pi / 2;
    if (angle < 0) angle += 2 * math.pi;
    return (angle / (2 * math.pi)).clamp(0.0, 1.0);
  }

  void _onRingPanStart(DragStartDetails details, double size) {
    final c = _controller;
    if (c == null || !c.value.isInitialized) return;
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

  @override
  Widget build(BuildContext context) {
    final screenSize = MediaQuery.of(context).size;
    final diameter = math.min(screenSize.width, screenSize.height) * 0.7;
    final ringSize = diameter + 44;
    final c = _controller;
    final ready = c != null && c.value.isInitialized;

    double progress = 0;
    if (ready && c.value.duration.inMilliseconds > 0) {
      progress =
          c.value.position.inMilliseconds / c.value.duration.inMilliseconds;
    }
    final shownProgress = _seeking ? _seekProgress : progress.clamp(0.0, 1.0);
    final dismissProgress = (_dismissOffset.abs() / 300).clamp(0.0, 1.0);

    return Scaffold(
      backgroundColor: Colors.transparent,
      body: GestureDetector(
        onVerticalDragUpdate: (d) =>
            setState(() => _dismissOffset += d.delta.dy),
        onVerticalDragEnd: (d) {
          if (_dismissOffset.abs() > 110) {
            Navigator.of(context).pop();
          } else {
            setState(() => _dismissOffset = 0);
          }
        },
        child: Stack(
          children: [
            Positioned.fill(
              child: Container(
                color:
                    Colors.black.withValues(alpha: 1 - dismissProgress * 0.5),
              ),
            ),
            SafeArea(
              child: Align(
                alignment: Alignment.topRight,
                child: Padding(
                  padding: const EdgeInsets.all(8),
                  child: IconButton(
                    icon: const Icon(Symbols.close, color: Colors.white),
                    onPressed: () => Navigator.of(context).pop(),
                  ),
                ),
              ),
            ),
            Center(
              child: Transform.translate(
                offset: Offset(0, _dismissOffset),
                child: SizedBox(
                  width: ringSize,
                  height: ringSize,
                  child: Stack(
                    alignment: Alignment.center,
                    children: [
                      GestureDetector(
                        onPanStart: (d) => _onRingPanStart(d, ringSize),
                        onPanUpdate: (d) => _onRingPanUpdate(d, ringSize),
                        onPanEnd: _onRingPanEnd,
                        child: CustomPaint(
                          size: Size(ringSize, ringSize),
                          painter: _RingPainter(progress: shownProgress),
                        ),
                      ),
                      GestureDetector(
                        onTap: _togglePlay,
                        child: ClipOval(
                          child: SizedBox(
                            width: diameter,
                            height: diameter,
                            child: ready
                                ? FittedBox(
                                    fit: BoxFit.cover,
                                    clipBehavior: Clip.hardEdge,
                                    child: SizedBox(
                                      width: c.value.size.width,
                                      height: c.value.size.height,
                                      child: VideoPlayer(c),
                                    ),
                                  )
                                : Container(color: Colors.black),
                          ),
                        ),
                      ),
                      if (_loading)
                        const IgnorePointer(
                          child: SmallSpinner(size: 40, color: Colors.white),
                        ),
                      if (_error && !_loading)
                        const IgnorePointer(
                          child: Icon(
                            Symbols.error,
                            color: Colors.white54,
                            size: 48,
                          ),
                        ),
                      if (ready && !c.value.isPlaying)
                        IgnorePointer(
                          child: Container(
                            width: 64,
                            height: 64,
                            decoration: const BoxDecoration(
                              color: Colors.black45,
                              shape: BoxShape.circle,
                            ),
                            child: const Icon(
                              Symbols.play_arrow,
                              color: Colors.white,
                              size: 36,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _RingPainter extends CustomPainter {
  final double progress;

  const _RingPainter({required this.progress});

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = size.width / 2 - 6;

    final base = Paint()
      ..color = Colors.white24
      ..style = PaintingStyle.stroke
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round;
    canvas.drawCircle(center, radius, base);

    final progressPaint = Paint()
      ..color = Colors.white
      ..style = PaintingStyle.stroke
      ..strokeWidth = 4
      ..strokeCap = StrokeCap.round;
    final sweep = 2 * math.pi * progress.clamp(0.0, 1.0);
    canvas.drawArc(
      Rect.fromCircle(center: center, radius: radius),
      -math.pi / 2,
      sweep,
      false,
      progressPaint,
    );

    final angle = -math.pi / 2 + sweep;
    final thumbCenter =
        center + Offset(math.cos(angle), math.sin(angle)) * radius;
    canvas.drawCircle(thumbCenter, 7, Paint()..color = Colors.white);
    canvas.drawCircle(
      thumbCenter,
      7,
      Paint()
        ..color = Colors.black26
        ..style = PaintingStyle.stroke
        ..strokeWidth = 1.5,
    );
  }

  @override
  bool shouldRepaint(_RingPainter old) => old.progress != progress;
}
