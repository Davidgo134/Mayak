class ClientIdAllocator {
  ClientIdAllocator();

  static final ClientIdAllocator instance = ClientIdAllocator();

  int _lastMs = 0;

  int next() {
    final now = DateTime.now().millisecondsSinceEpoch;
    _lastMs = now > _lastMs ? now : _lastMs + 1;
    return _lastMs;
  }

  int nextNegative() => -next();
}
