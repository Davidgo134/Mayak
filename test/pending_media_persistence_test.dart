import 'package:flutter_test/flutter_test.dart';
import 'package:mayak/backend/modules/messages.dart';
import 'package:mayak/models/attachment.dart';

void main() {
  CachedMessage restore(CachedMessage message) =>
      CachedMessage.fromDbRow(message.toDbRow());

  CachedMessage pending(MessageAttachment attachment) => CachedMessage(
    id: 'temp_fixture',
    accountId: 101,
    chatId: 202,
    senderId: 101,
    time: 303,
    status: 'pending',
    attachments: [attachment],
  );

  test('persists an offline photo local path for outbox recovery', () {
    final restored = restore(
      pending(const PhotoAttachment(localPath: '/fixture/photo.jpg')),
    );

    expect(restored.attachments, hasLength(1));
    expect((restored.attachments!.single as PhotoAttachment).localPath,
        '/fixture/photo.jpg');
  });

  test('persists an offline video note local path and media metadata', () {
    final restored = restore(
      pending(
        const VideoAttachment(
          localPath: '/fixture/note.mp4',
          duration: 1234,
          videoType: 1,
        ),
      ),
    );

    final attachment = restored.attachments!.single as VideoAttachment;
    expect(attachment.localPath, '/fixture/note.mp4');
    expect(attachment.duration, 1234);
    expect(attachment.videoType, 1);
  });

  test('persists an offline voice local path, duration, and waveform', () {
    final restored = restore(
      pending(
        const AudioAttachment(
          localPath: '/fixture/voice.ogg',
          duration: 4567,
          waveform: 'fixture-wave',
        ),
      ),
    );

    final attachment = restored.attachments!.single as AudioAttachment;
    expect(attachment.localPath, '/fixture/voice.ogg');
    expect(attachment.duration, 4567);
    expect(attachment.waveform, 'fixture-wave');
  });

  test('persists an offline file local path and metadata', () {
    final restored = restore(
      pending(
        const FileAttachment(
          localPath: '/fixture/file.bin',
          name: 'file.bin',
          size: 2048,
        ),
      ),
    );

    final attachment = restored.attachments!.single as FileAttachment;
    expect(attachment.localPath, '/fixture/file.bin');
    expect(attachment.name, 'file.bin');
    expect(attachment.size, 2048);
  });
}
