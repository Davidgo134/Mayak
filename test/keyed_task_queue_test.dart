import 'dart:async';

import 'package:flutter_test/flutter_test.dart';
import 'package:mayak/core/utils/keyed_task_queue.dart';

void main() {
  test('a hanging task on one chat does not block pushes of another chat', () async {
    final queue = KeyedTaskQueue<int>();
    final releaseA = Completer<void>();
    var bApplied = false;

    queue.enqueue(1, () => releaseA.future);
    queue.enqueue(2, () async {
      bApplied = true;
    });

    await Future<void>.delayed(const Duration(milliseconds: 20));

    expect(bApplied, isTrue);

    releaseA.complete();
    await Future<void>.delayed(Duration.zero);
  });

  test('tasks on the same chat run strictly in order', () async {
    final queue = KeyedTaskQueue<int>();
    final log = <int>[];

    queue.enqueue(7, () async {
      await Future<void>.delayed(const Duration(milliseconds: 10));
      log.add(1);
    });
    queue.enqueue(7, () async {
      log.add(2);
    });

    await Future<void>.delayed(const Duration(milliseconds: 40));

    expect(log, [1, 2]);
  });

  test('a failing task does not break the chain for its chat', () async {
    Object? reported;
    final queue = KeyedTaskQueue<int>(
      onError: (e) => reported = e,
    );
    var after = false;

    queue.enqueue(3, () async {
      throw StateError('boom');
    });
    queue.enqueue(3, () async {
      after = true;
    });

    await Future<void>.delayed(const Duration(milliseconds: 20));

    expect(after, isTrue);
    expect(reported, isStateError);
  });

  test('finished queues are cleaned up', () async {
    final queue = KeyedTaskQueue<int>();

    queue.enqueue(9, () async {});
    await Future<void>.delayed(const Duration(milliseconds: 20));

    expect(queue.pendingKeys, 0);
  });
}
