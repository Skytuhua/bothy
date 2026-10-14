# bothy (Dart / Flutter)

**Serverless, local-first sync engine that uses the user's own Google Drive as the transport.**

> ⚠️ **Early development.** This release only contains the format version and Bothy's hybrid
> logical clock (`Hlc`) type. The sync engine will arrive as bindings to the Rust core
> (via `flutter_rust_bridge`).

Bothy lets Flutter apps sync structured data between phones, desktops and embedded devices
without a server of their own: every device keeps a local SQLite database and works offline,
and devices exchange immutable files through the user's own Google Drive (`appDataFolder`).
Concurrent edits to different fields merge automatically.

```dart
import 'package:bothy/bothy.dart';

final hlc = Hlc.parse('018bcfe568000001');
print(hlc.physicalMs); // 1700000000000
```

- Repository: https://github.com/bothy-sync/bothy
- License: MIT OR Apache-2.0 (see `LICENSE`)
