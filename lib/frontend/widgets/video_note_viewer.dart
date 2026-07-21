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

/// Opens a fullscreen, enlarged view of a video note (circle message).
/// Mirrors MAX: the circle simply scales up to fill the screen width, no
/// dark background, no scrubber ring drawn around it — instead you drag
/// horizontally anywhere on the circle to seek, like a native MAX/TG-style
/// video note viewer.
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
      barrierColor: Colors.transparent,
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
            scale: Tween<double>(begin: 0.8, end: 1.0).animate(curved),
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
  double _seekStartDx = 0;
  Duration _seekStartPosition = Duration.zero;
  Duration _seekPreviewPosition = Duration.zero;

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

  void _onSeekStart(DragStartDetails details) {
    final c = _controller;
    if (c == null || !c.value.isInitialized) return;
    Haptics.tap();
    setState(() {
      _seeking = true;
      _seekStartDx = details.globalPosition.dx;
      _seekStartPosition = c.value.position;
      _seekPreviewPosition = c.value.position;
    });
  }

  void _onSeekUpdate(DragUpdateDetails details, double trackWidth) {
    final c = _controller;
    if (!_seeking || c == null || !c.value.isInitialized) return;
    final duration = c.value.duration;
    if (duration.inMilliseconds <= 0) return;
    final dx = details.globalPosition.dx - _seekStartDx;
    // Full drag across the circle's diameter scrubs the full duration.
    final fraction = dx / trackWidth;
    final deltaMs = (duration.inMilliseconds * fraction).round();
    var targetMs = _seekStartPosition.inMilliseconds + deltaMs;
    targetMs = targetMs.clamp(0, duration.inMilliseconds);
    setState(() {
      _seekPreviewPosition = Duration(milliseconds: targetMs);
    });
  }

  void _onSeekEnd(DragEndDetails details) {
    final c = _controller;
    if (c != null && c.value.isInitialized) {
      c.seekTo(_seekPreviewPosition);
    }
    setState(() => _seeking = false);
  }

  @override
  Widget build(BuildContext context) {
    final screenSize = MediaQuery.of(context).size;
    // Scale up to fill the screen width, like MAX — bounded by height too
    // so it never overflows on very narrow/tall screens.
    final diameter = math.min(screenSize.width, screenSize.height * 0.9);
    final c = _controller;
    final ready = c != null && c.value.isInitialized;

    final position = _seeking
        ? _seekPreviewPosition
        : (ready ? c.value.position : Duration.zero);
    final duration = ready ? c.value.duration : Duration.zero;
    final progress = duration.inMilliseconds > 0
        ? (position.inMilliseconds / duration.inMilliseconds).clamp(0.0, 1.0)
        : 0.0;
    final dismissProgress = (_dismissOffset.abs() / 300).clamp(0.0, 1.0);
    final dismissScale = 1 - dismissProgress * 0.18;

    return Scaffold(
      backgroundColor: Colors.transparent,
      body: GestureDetector(
        onVerticalDragUpdate: (d) {
          if (_seeking) return;
          setState(() => _dismissOffset += d.delta.dy);
        },
        onVerticalDragEnd: (d) {
          if (_seeking) return;
          if (_dismissOffset.abs() > 110) {
            Navigator.of(context).pop();
          } else {
            setState(() => _dismissOffset = 0);
          }
        },
        child: Stack(
          children: [
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
                child: Transform.scale(
                  scale: dismissScale,
                  child: GestureDetector(
                    onTap: _togglePlay,
                    onHorizontalDragStart: _onSeekStart,
                    onHorizontalDragUpdate: (d) =>
                        _onSeekUpdate(d, diameter),
                    onHorizontalDragEnd: _onSeekEnd,
                    child: ClipOval(
                      child: SizedBox(
                        width: diameter,
                        height: diameter,
                        child: Stack(
                          alignment: Alignment.center,
                          children: [
                            ready
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
                            if (_loading)
                              const IgnorePointer(
                                child:
                                    SmallSpinner(size: 40, color: Colors.white),
                              ),
                            if (_error && !_loading)
                              const IgnorePointer(
                                child: Icon(
                                  Symbols.error,
                                  color: Colors.white54,
                                  size: 48,
                                ),
                              ),
                            if (ready && !c.value.isPlaying && !_seeking)
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
                            Positioned(
                              left: 0,
                              right: 0,
                              bottom: 0,
                              child: IgnorePointer(
                                child: Container(
                                  height: 4,
                                  decoration: BoxDecoration(
                                    gradient: LinearGradient(
                                      colors: [
                                        Colors.transparent,
                                        Colors.black.withValues(alpha: 0.35),
                                      ],
                                      begin: Alignment.topCenter,
                                      end: Alignment.bottomCenter,
                                    ),
                                  ),
                                ),
                              ),
                            ),
                            Positioned(
                              left: 0,
                              right: 0,
                              bottom: 0,
                              child: IgnorePointer(
                                child: FractionallySizedBox(
                                  alignment: Alignment.centerLeft,
                                  widthFactor: progress,
                                  child: Container(
                                    height: 3,
                                    color: Colors.white,
                                  ),
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
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
