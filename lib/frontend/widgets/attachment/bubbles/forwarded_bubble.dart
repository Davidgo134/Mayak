import 'dart:convert';
import 'dart:typed_data';

import 'package:cached_network_image/cached_network_image.dart';
import 'package:flutter/material.dart';
import 'package:material_symbols_icons/symbols.dart';

import '../../../../backend/modules/messages.dart';
import '../../../../models/attachment.dart';
import 'bubble_context.dart';
import 'contact_bubble.dart';
import 'file_bubble.dart';
import 'photo_bubble.dart';
import 'sticker_bubble.dart';
import 'video_bubble.dart';

Widget _forwardedHeader(
  BubbleContext ctx,
  ForwardedMessageAttachment forwarded,
) {
  final headerColor = ctx.dim;
  final displaySender =
      forwarded.originalSenderName ??
      ContactCache.get(forwarded.originalSenderId) ??
      forwarded.originalSenderId.toString();
  final senderAvatar =
      forwarded.originalSenderAvatar ??
      ContactCache.getAvatar(forwarded.originalSenderId);
  return Padding(
    padding: const EdgeInsets.only(left: 8, top: 8, right: 8),
    child: Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(Symbols.forward, size: 14, color: headerColor),
        const SizedBox(width: 4),
        if (senderAvatar != null && senderAvatar.isNotEmpty)
          CircleAvatar(
            radius: 10,
            backgroundImage: CachedNetworkImageProvider(
              senderAvatar,
              maxWidth: 96,
              maxHeight: 96,
            ),
            backgroundColor: ctx.cs.primaryContainer,
          )
        else
          CircleAvatar(
            radius: 10,
            backgroundColor: ctx.cs.primaryContainer,
            child: Text(
              displaySender.isNotEmpty ? displaySender[0].toUpperCase() : '?',
              style: TextStyle(fontSize: 9, color: ctx.cs.onPrimaryContainer),
            ),
          ),
        const SizedBox(width: 6),
        Flexible(
          child: Text(
            displaySender,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: headerColor,
              fontSize: 12,
              fontWeight: FontWeight.w500,
            ),
          ),
        ),
      ],
    ),
  );
}

/// Wraps forwarded content so tapping it navigates to the original message
/// in the chat it was forwarded from, instead of (or in addition to) any
/// action the inner content widget already performs.
Widget _wrapForwardOriginTap(
  BubbleContext ctx,
  ForwardedMessageAttachment forwarded,
  Widget child,
) {
  final onTap = ctx.onForwardOriginTap;
  final originChatId = forwarded.originalChatId;
  if (onTap == null || originChatId == null) return child;
  return GestureDetector(
    behavior: HitTestBehavior.opaque,
    onTap: () => onTap(
      originChatId,
      forwarded.originalMessageId,
      forwarded.originalTime,
      forwarded.originalSenderName,
      forwarded.originalSenderAvatar,
    ),
    child: child,
  );
}

/// Static (non-interactive) preview of a forwarded video note.
/// Unlike [VideoNoteBubble], this never plays inline — tapping it always
/// jumps to the original circle message, matching the request to open the
/// source chat/message rather than starting inline playback here.
class _ForwardedVideoNotePreview extends StatelessWidget {
  static const double _size = 210;

  final VideoAttachment video;
  final ColorScheme cs;

  const _ForwardedVideoNotePreview({required this.video, required this.cs});

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

  @override
  Widget build(BuildContext context) {
    final preview = _previewBytes(video.previewData);
    return SizedBox(
      width: _size,
      height: _size,
      child: Stack(
        alignment: Alignment.center,
        children: [
          ClipOval(
            child: SizedBox(
              width: _size,
              height: _size,
              child: preview != null
                  ? Image.memory(
                      preview,
                      fit: BoxFit.cover,
                      gaplessPlayback: true,
                    )
                  : Container(color: cs.surfaceContainerHighest),
            ),
          ),
          Container(
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
        ],
      ),
    );
  }
}

class ForwardedPhotoBubble extends StatelessWidget {
  final BubbleContext ctx;
  final ForwardedMessageAttachment forwarded;
  final List<PhotoAttachment> photos;

  const ForwardedPhotoBubble({
    super.key,
    required this.ctx,
    required this.forwarded,
    required this.photos,
  });

  @override
  Widget build(BuildContext context) {
    final message = ctx.message;
    final hasCaption = message.text != null && message.text!.isNotEmpty;

    return _wrapForwardOriginTap(
      ctx,
      forwarded,
      Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          _forwardedHeader(ctx, forwarded),
          const SizedBox(height: 4),
          if (hasCaption) ...[
            Padding(
              padding: const EdgeInsets.only(left: 8),
              child: Text(
                message.text ?? '',
                style: TextStyle(color: ctx.text, fontSize: 16, height: 1.3),
              ),
            ),
            const SizedBox(height: 6),
          ],
          PhotoBubble(ctx: ctx, photos: photos),
        ],
      ),
    );
  }
}

class ForwardedGenericBubble extends StatelessWidget {
  final BubbleContext ctx;
  final ForwardedMessageAttachment forwarded;
  final List<MessageAttachment> attachments;

  const ForwardedGenericBubble({
    super.key,
    required this.ctx,
    required this.forwarded,
    required this.attachments,
  });

  @override
  Widget build(BuildContext context) {
    return _wrapForwardOriginTap(
      ctx,
      forwarded,
      IntrinsicWidth(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          mainAxisSize: MainAxisSize.min,
          children: [
            _forwardedHeader(ctx, forwarded),
            const SizedBox(height: 4),
            ...attachments.map((a) {
              if (a is FileAttachment) {
                return FileBubble(ctx: ctx, file: a, fill: true);
              }
              if (a is StickerAttachment) {
                return StickerBubble(ctx: ctx, sticker: a);
              }
              if (a is VideoAttachment && a.isNote) {
                return _ForwardedVideoNotePreview(video: a, cs: ctx.cs);
              }
              if (a is VideoAttachment) {
                return VideoBubble(ctx: ctx, video: a);
              }
              return const SizedBox.shrink();
            }),
          ],
        ),
      ),
    );
  }
}

class ForwardedStickerBubble extends StatelessWidget {
  final BubbleContext ctx;
  final ForwardedMessageAttachment forwarded;
  final MessageAttachment sticker;

  const ForwardedStickerBubble({
    super.key,
    required this.ctx,
    required this.forwarded,
    required this.sticker,
  });

  @override
  Widget build(BuildContext context) {
    return _wrapForwardOriginTap(
      ctx,
      forwarded,
      Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          _forwardedHeader(ctx, forwarded),
          const SizedBox(height: 4),
          StickerBubble(ctx: ctx, sticker: sticker),
        ],
      ),
    );
  }
}

class ForwardedContactBubble extends StatelessWidget {
  final BubbleContext ctx;
  final ForwardedMessageAttachment forwarded;

  const ForwardedContactBubble({
    super.key,
    required this.ctx,
    required this.forwarded,
  });

  @override
  Widget build(BuildContext context) {
    final contact = forwarded.originalContact!;

    return _wrapForwardOriginTap(
      ctx,
      forwarded,
      Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          _forwardedHeader(ctx, forwarded),
          const SizedBox(height: 4),
          buildContactCard(
            ctx,
            firstName: contact.firstName,
            lastName: contact.lastName,
            name: contact.name,
            photoUrl: contact.photoUrl ?? contact.baseUrl,
            phoneNumber: contact.phoneNumber,
          ),
        ],
      ),
    );
  }
}
