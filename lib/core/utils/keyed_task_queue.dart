import 'dart:async';

typedef KeyedTaskQueueErrorHandler = void Function(Object error);

class KeyedTaskQueue<K> {
  KeyedTaskQueue({this.onError});

  final KeyedTaskQueueErrorHandler? onError;

  final Map<K, Future<void>> _queues = {};

  int get pendingKeys => _queues.length;

  void enqueue(K key, Future<void> Function() task) {
    final previous = _queues.remove(key);
    final future = (previous ?? Future<void>.value())
        .then((_) => task())
        .catchError((Object e) {
          onError?.call(e);
        });
    _queues[key] = future;
    unawaited(
      future.whenComplete(() {
        if (identical(_queues[key], future)) _queues.remove(key);
      }),
    );
  }
}
