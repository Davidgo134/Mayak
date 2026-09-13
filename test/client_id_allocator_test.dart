import 'package:flutter_test/flutter_test.dart';
import 'package:mayak/core/protocol/client_id_allocator.dart';

void main() {
  test('allocates 20 unique monotonic ids in a tight loop without sleep', () {
    final allocator = ClientIdAllocator();
    final ids = List<int>.generate(20, (_) => allocator.next());

    expect(ids.toSet().length, 20);
    for (var i = 1; i < ids.length; i++) {
      expect(ids[i], greaterThan(ids[i - 1]));
    }
  });

  test('negative ids keep the sign and epoch-milliseconds magnitude', () {
    final allocator = ClientIdAllocator();
    final before = DateTime.now().millisecondsSinceEpoch;
    final ids = List<int>.generate(20, (_) => allocator.nextNegative());
    final after = DateTime.now().millisecondsSinceEpoch;

    expect(ids.every((id) => id < 0), isTrue);
    expect(ids.toSet().length, 20);
    for (final id in ids) {
      expect(-id, inInclusiveRange(before, after + 60));
    }
  });

  test('ids never move backwards in time between bursts', () {
    final allocator = ClientIdAllocator();
    final first = allocator.next();
    final second = allocator.next();

    expect(second, greaterThan(first));
    expect(first, greaterThan(0));
  });
}
