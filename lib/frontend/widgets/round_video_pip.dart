import 'package:flutter/material.dart';
import 'package:material_symbols_icons/symbols.dart';
import 'package:video_player/video_player.dart';

/// Global controller for the floating round-video "PiP" mini player.
///
/// Mirrors Telegram's PipRoundVideoView: when the user scrolls a round
/// video message off-screen (or navigates away) while it's playing, the
/// circle shrinks into a small draggable bubble that floats above every
/// screen in the app until it's tapped (re-expand) or explicitly closed.
class RoundVideoPipController {
  RoundVideoPipController._();
  static final RoundVideoPipController instance = RoundVideoPipController._();

  final ValueNotifier<PipData?> state = ValueNotifier<PipData?>(null);

  void activate(PipData data) {
    final current = state.value;
    if (current != null && current.controller != data.controller) {
      current.onDisposeIfOwned?.call();
    }
    state.value = data;
  }

  void clear({bool disposeController = false}) {
    final current = state.value;
    if (current != null && disposeController) {
      current.onDisposeIfOwned?.call();
    }
    state.value = null;
  }
}

class PipData {
  final VideoPlayerController controller;
  final String messageId;
  final int chatId;
  final VoidCallback? onDisposeIfOwned;
  final void Function(BuildContext context) onExpand;

  const PipData({
    required this.controller,
    required this.messageId,
    required this.chatId,
    required this.onExpand,
    this.onDisposeIfOwned,
  });
}

/// Floating draggable mini circle. Insert once near the root of the app
/// (e.g. in MaterialApp's builder) so it renders above every screen.
class RoundVideoPipOverlay extends StatefulWidget {
  const RoundVideoPipOverlay({super.key});

  @override
  State<RoundVideoPipOverlay> createState() => _RoundVideoPipOverlayState();
}

class _RoundVideoPipOverlayState extends State<RoundVideoPipOverlay> {
  static const double _size = 92;
  Offset? _offset;

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<PipData?>(
      valueListenable: RoundVideoPipController.instance.state,
      builder: (context, data, _) {
        if (data == null) return const SizedBox.shrink();
        final screenSize = MediaQuery.of(context).size;
        final safe = MediaQuery.of(context).padding;
        _offset ??= Offset(
          screenSize.width - _size - 16,
          screenSize.height - _size - safe.bottom - 120,
        );
        final maxX = screenSize.width - _size;
        final maxY = screenSize.height - _size;

        return Positioned(
          left: _offset!.dx,
          top: _offset!.dy,
          child: GestureDetector(
            onPanUpdate: (d) {
              setState(() {
                final next = _offset! + d.delta;
                _offset = Offset(
                  next.dx.clamp(0.0, maxX),
                  next.dy.clamp(0.0, maxY),
                );
              });
            },
            onTap: () => data.onExpand(context),
            child: SizedBox(
              width: _size,
              height: _size,
              child: Stack(
                clipBehavior: Clip.none,
                children: [
                  DecoratedBox(
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      boxShadow: [
                        BoxShadow(
                          color: Colors.black.withValues(alpha: 0.35),
                          blurRadius: 12,
                          offset: const Offset(0, 4),
                        ),
                      ],
                    ),
                    child: ClipOval(
                      child: SizedBox(
                        width: _size,
                        height: _size,
                        child: data.controller.value.isInitialized
                            ? FittedBox(
                                fit: BoxFit.cover,
                                clipBehavior: Clip.hardEdge,
                                child: SizedBox(
                                  width: data.controller.value.size.width,
                                  height: data.controller.value.size.height,
                                  child: VideoPlayer(data.controller),
                                ),
                              )
                            : Container(color: Colors.black),
                      ),
                    ),
                  ),
                  Positioned(
                    right: -2,
                    top: -2,
                    child: GestureDetector(
                      onTap: () {
                        data.controller.pause();
                        RoundVideoPipController.instance.clear(
                          disposeController: true,
                        );
                      },
                      child: Container(
                        width: 22,
                        height: 22,
                        decoration: const BoxDecoration(
                          color: Colors.black87,
                          shape: BoxShape.circle,
                        ),
                        child: const Icon(
                          Symbols.close,
                          color: Colors.white,
                          size: 14,
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        );
      },
    );
  }
}
