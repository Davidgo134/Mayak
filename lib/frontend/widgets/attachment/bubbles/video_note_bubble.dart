import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:material_symbols_icons/symbols.dart';
import 'package:komet/main.dart';

import '../../../../backend/modules/messages.dart';
import '../../../../core/utils/haptics.dart';
import '../../../../models/attachment.dart';
import '../../small_spinner.dart';
import '../../video_note_viewer.dart';

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
  static const double _size = 210;
  bool _opening = false;

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

  Future<void> _openViewer() async {
    if (_opening) return;
    setState(() => _opening = true);
    try {
      await openVideoNoteViewer(
        context,
        attachment: widget.attachment,
        messageId: widget.messageId,
        chatId: widget.chatId,
      );
    } finally {
      if (mounted) setState(() => _opening = false);
    }
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

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        SizedBox(
          width: _size,
          height: _size,
          child: Stack(
            alignment: Alignment.center,
            clipBehavior: Clip.none,
            children: [
              GestureDetector(
                onTap: _openViewer,
                child: ClipOval(
                  child: SizedBox(
                    width: _size,
                    height: _size,
                    child: preview != null
                        ? Image.memory(
                            preview,
                            fit: BoxFit.cover,
                            gaplessPlayback: true,
                          )
                        : Container(color: widget.cs.surfaceContainerHighest),
                  ),
                ),
              ),
              IgnorePointer(
                child: Container(
                  width: 52,
                  height: 52,
                  decoration: const BoxDecoration(
                    color: Colors.black45,
                    shape: BoxShape.circle,
                  ),
                  child: _opening
                      ? const Padding(
                          padding: EdgeInsets.all(14),
                          child: SmallSpinner(size: 36, color: Colors.white),
                        )
                      : const Icon(
                          Symbols.play_arrow,
                          color: Colors.white,
                          size: 30,
                        ),
                ),
              ),
              if (widget.attachment.videoId != null)
                Positioned(
                  right: 4,
                  bottom: 4,
                  child: GestureDetector(
                    onTap: _requestTranscription,
                    child: AnimatedContainer(
                      duration: const Duration(milliseconds: 200),
                      curve: Curves.easeOut,
                      width: 34,
                      height: 34,
                      decoration: BoxDecoration(
                        color: _transcriptionVisible
                            ? widget.cs.primary
                            : Colors.black54,
                        shape: BoxShape.circle,
                        border: Border.all(
                          color: Colors.white.withValues(alpha: 0.25),
                          width: 1,
                        ),
                      ),
                      child: Center(
                        child: _transcriptionLoading
                            ? RotationTransition(
                                turns: _transcriptionIconAnim,
                                child: const Icon(
                                  Symbols.graphic_eq,
                                  color: Colors.white,
                                  size: 16,
                                ),
                              )
                            : Text(
                                'Т',
                                style: TextStyle(
                                  color: _transcriptionVisible
                                      ? widget.cs.onPrimary
                                      : Colors.white,
                                  fontSize: 14,
                                  fontWeight: FontWeight.w700,
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
                  width: _size,
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
                  width: _size,
                  height: 0,
                ),
        ),
      ],
    );
  }
}
