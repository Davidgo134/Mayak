import 'dart:async';
import 'dart:io';
import 'dart:typed_data';

import '../../core/media/gallery_source.dart' show GalleryItem;
import '../../core/protocol/packet.dart';
import '../../core/storage/app_database.dart';
import '../../core/storage/token_storage.dart';
import '../../core/utils/logger.dart';
import '../../models/attachment.dart';
import '../api.dart';
import 'chats.dart';
import 'messages.dart';
import 'upload_service.dart';

class OutboxService {
  OutboxService._();

  static final OutboxService instance = OutboxService._();

  static const int _maxSendAttempts = 5;

  Api? _api;
  MessagesModule? _messages;
  bool _flushing = false;
  final Map<String, int> _sendAttempts = {};

  void init(Api api, MessagesModule messages) {
    if (_api != null) return;
    _api = api;
    _messages = messages;
    api.stateStream.listen((state) {
      if (state == SessionState.online) unawaited(flush());
    });
    if (api.state == SessionState.online) unawaited(flush());
  }

  Future<void> flush() async {
    if (_flushing) return;
    final api = _api;
    final messages = _messages;
    if (api == null || messages == null) return;
    if (api.state != SessionState.online) return;

    _flushing = true;
    try {
      final accountId = await TokenStorage.getActiveAccountId();
      if (accountId == null) return;

      final rows = await AppDatabase.loadPendingMessages(accountId);
      for (final row in rows) {
        if (api.state != SessionState.online) break;
        final pending = CachedMessage.fromDbRow(row);

        final attachment = _retryableAttachment(pending);
        if (attachment != null) {
          await _flushMedia(accountId, pending, attachment);
          continue;
        }

        final remoteFile = _remoteFileAttachment(pending);
        if (remoteFile != null) {
          await _flushRemoteFile(accountId, pending, remoteFile);
          continue;
        }

        final text = pending.text;
        if (text == null || text.isEmpty) continue;

        final payload = pending.payload;
        final replyToMessageId = _replyIdFromPayload(payload);
        final replySourceChatId = _replySourceChatIdFromPayload(payload);
        final elements = _elementsFromPayload(payload);

        try {
          final actualId = await messages.sendMessage(
            accountId,
            pending.chatId,
            text,
            replyToMessageId: replyToMessageId,
            replySourceChatId: replySourceChatId,
            elements: elements,
          );
          _sendAttempts.remove(pending.id);
          final sent = CachedMessage(
            id: actualId.isNotEmpty ? actualId : pending.id,
            accountId: accountId,
            chatId: pending.chatId,
            senderId: accountId,
            text: text,
            time: pending.time,
            status: 'sent',
            payload: payload,
          );
          await AppDatabase.saveMessages([sent.toDbRow()]);
          if (sent.id != pending.id) {
            await AppDatabase.deleteMessage(
              accountId,
              pending.chatId,
              pending.id,
            );
          }
          chats.emitMessageSent(pending.chatId, pending.id, sent);
          await chats.applyOutgoing(
            accountId,
            pending.chatId,
            messageId: sent.id,
            time: sent.time,
            text: text,
            status: 'sent',
            elements: elements.isEmpty ? null : elements,
          );
        } catch (e) {
          final attempts = _sendAttempts.update(
            pending.id,
            (v) => v + 1,
            ifAbsent: () => 1,
          );
          if (!isPermanentSendFailure(e) && attempts < _maxSendAttempts) {
            logger.w('Outbox: отправка ${pending.id} не удалась: $e');
            continue;
          }
          logger.w('Outbox: ${pending.id} отклонено сервером: $e');
          final failed = pending.copyWith(status: 'error');
          await AppDatabase.saveMessages([failed.toDbRow()]);
          chats.emitMessageSent(pending.chatId, pending.id, failed);
          await chats.applyOutgoing(
            accountId,
            pending.chatId,
            messageId: failed.id,
            time: failed.time,
            text: text,
            status: 'error',
            elements: elements.isEmpty ? null : elements,
          );
        }
      }
    } catch (e) {
      logger.e('Outbox flush: $e');
    } finally {
      _flushing = false;
    }
  }

  FileAttachment? _remoteFileAttachment(CachedMessage msg) {
    for (final att in msg.attachments ?? const <MessageAttachment>[]) {
      if (att is! FileAttachment) continue;
      if (att.localPath != null) return null;
      if ((att.fileId ?? 0) != 0 || (att.fileToken ?? '').isNotEmpty) {
        return att;
      }
    }
    return null;
  }

  MessageAttachment? _retryableAttachment(CachedMessage msg) {
    for (final att in msg.attachments ?? const <MessageAttachment>[]) {
      final path = _attachmentLocalPath(att);
      if (path != null) return att;
    }
    return null;
  }

  String? _attachmentLocalPath(MessageAttachment att) {
    return switch (att) {
      PhotoAttachment a => a.localPath,
      VideoAttachment a => a.localPath,
      AudioAttachment a => a.localPath,
      FileAttachment a => a.localPath,
      _ => null,
    };
  }

  Future<void> _flushMedia(
    int accountId,
    CachedMessage pending,
    MessageAttachment attachment,
  ) async {
    if (UploadService.instance.job(pending.id) != null) return;
    final path = _attachmentLocalPath(attachment);
    if (path == null || !File(path).existsSync()) {
      await _markMediaFailed(accountId, pending, permanent: true);
      return;
    }
    final sending = pending.copyWith(status: 'sending');
    await AppDatabase.saveMessages([sending.toDbRow()]);
    if (UploadService.instance.job(pending.id) != null) return;
    chats.emitMessageSent(pending.chatId, pending.id, sending);
    await _dispatchMedia(accountId, sending, attachment);
    final sent = UploadService.instance.completedFor(pending.id);
    if (sent != null) {
      _sendAttempts.remove(pending.id);
      chats.emitMessageSent(pending.chatId, pending.id, sent);
      await chats.reconcileLastMessage(accountId, pending.chatId);
      return;
    }
    final attempts = _sendAttempts.update(
      pending.id,
      (v) => v + 1,
      ifAbsent: () => 1,
    );
    await _markMediaFailed(
      accountId,
      pending,
      permanent: attempts >= _maxSendAttempts,
    );
  }

  Future<void> _flushRemoteFile(
    int accountId,
    CachedMessage pending,
    FileAttachment att,
  ) async {
    final messages = _messages;
    if (messages == null) return;
    final sending = pending.copyWith(status: 'sending');
    await AppDatabase.saveMessages([sending.toDbRow()]);
    chats.emitMessageSent(pending.chatId, pending.id, sending);
    try {
      final actualId = await messages.sendFileMessage(
        pending.chatId,
        att.fileId ?? 0,
        token: att.fileToken,
      );
      _sendAttempts.remove(pending.id);
      final sent = CachedMessage(
        id: (actualId != null && actualId.isNotEmpty) ? actualId : pending.id,
        accountId: accountId,
        chatId: pending.chatId,
        senderId: accountId,
        text: pending.text,
        time: pending.time,
        status: 'sent',
        payload: pending.payload,
        attachments: pending.attachments,
      );
      await AppDatabase.saveMessages([sent.toDbRow()]);
      if (sent.id != pending.id) {
        await AppDatabase.deleteMessage(accountId, pending.chatId, pending.id);
      }
      chats.emitMessageSent(pending.chatId, pending.id, sent);
      await chats.applyOutgoing(
        accountId,
        pending.chatId,
        messageId: sent.id,
        time: sent.time,
        text: pending.text ?? '',
        status: 'sent',
      );
    } catch (e) {
      final attempts = _sendAttempts.update(
        pending.id,
        (v) => v + 1,
        ifAbsent: () => 1,
      );
      await _markMediaFailed(
        accountId,
        pending,
        permanent: isPermanentSendFailure(e) || attempts >= _maxSendAttempts,
      );
    }
  }

  Future<void> _markMediaFailed(
    int accountId,
    CachedMessage pending, {
    required bool permanent,
  }) async {
    final failed = pending.copyWith(status: permanent ? 'error' : 'pending');
    await AppDatabase.saveMessages([failed.toDbRow()]);
    chats.emitMessageSent(pending.chatId, pending.id, failed);
    await chats.applyOutgoing(
      accountId,
      pending.chatId,
      messageId: pending.id,
      time: pending.time,
      text: pending.text ?? '',
      status: permanent ? 'error' : 'pending',
    );
  }

  Future<void> _dispatchMedia(
    int accountId,
    CachedMessage msg,
    MessageAttachment att,
  ) async {
    final path = _attachmentLocalPath(att);
    if (path == null) return;
    if (att is PhotoAttachment) {
      final jobs = <({File file, GalleryItem? item})>[
        for (final a
            in (msg.attachments ?? const <MessageAttachment>[])
                .whereType<PhotoAttachment>())
          if (a.localPath != null && File(a.localPath!).existsSync())
            (file: File(a.localPath!), item: null),
      ];
      await UploadService.instance.sendPhotos(
        accountId: accountId,
        chatId: msg.chatId,
        tempId: msg.id,
        jobs: jobs,
        caption: msg.text ?? '',
        placeholder: msg,
      );
    } else if (att is VideoAttachment) {
      if (att.isNote) {
        await UploadService.instance.sendVideoNote(
          accountId: accountId,
          chatId: msg.chatId,
          tempId: msg.id,
          file: File(path),
          durationMs: att.duration ?? 0,
          placeholder: msg,
        );
      } else {
        await UploadService.instance.sendVideo(
          accountId: accountId,
          chatId: msg.chatId,
          tempId: msg.id,
          file: File(path),
          caption: msg.text ?? '',
          placeholder: msg,
        );
      }
    } else if (att is AudioAttachment) {
      final wave = att.waveform == null
          ? Uint8List(0)
          : Uint8List.fromList(att.waveform!.codeUnits);
      await UploadService.instance.sendVoice(
        accountId: accountId,
        chatId: msg.chatId,
        tempId: msg.id,
        file: File(path),
        durationMs: att.duration ?? 0,
        wave: wave,
        placeholder: msg,
      );
    } else if (att is FileAttachment) {
      final file = File(path);
      await UploadService.instance.sendFile(
        accountId: accountId,
        chatId: msg.chatId,
        tempId: msg.id,
        source: file,
        filename: att.name ?? 'file',
        size: att.size ?? file.lengthSync(),
        placeholder: msg,
      );
    }
  }

  int? _replyIdFromPayload(Map<String, dynamic>? payload) {
    if (payload == null) return null;
    final link = payload['link'];
    if (link is! Map) return null;
    if ((link['type'] as String?)?.toUpperCase() != 'REPLY') return null;
    final msg = link['message'];
    if (msg is Map) {
      final id = msg['id'];
      if (id is int) return id;
      if (id != null) return int.tryParse(id.toString());
    }
    final mid = link['messageId'];
    if (mid is int) return mid;
    if (mid != null) return int.tryParse(mid.toString());
    return null;
  }

  int? _replySourceChatIdFromPayload(Map<String, dynamic>? payload) {
    if (payload == null) return null;
    final link = payload['link'];
    if (link is! Map) return null;
    if ((link['type'] as String?)?.toUpperCase() != 'REPLY') return null;
    final chatId = link['chatId'];
    if (chatId is int) return chatId;
    if (chatId != null) return int.tryParse(chatId.toString());
    return null;
  }

  List<Map<String, dynamic>> _elementsFromPayload(
    Map<String, dynamic>? payload,
  ) {
    final raw = payload?['elements'];
    if (raw is! List) return const [];
    return raw
        .whereType<Map>()
        .map((e) => Map<String, dynamic>.from(e))
        .toList();
  }
}
