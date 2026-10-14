# bothy (Python)

**Serverless, local-first sync engine that uses your own Google Drive as the transport.**

> ⚠️ **Early development.** This release only contains the format version and helpers for
> Bothy's hybrid logical clock timestamps. The sync engine will arrive as native bindings to
> the Rust core.

Bothy lets apps sync structured data between devices (desktop, phone, embedded Linux boards
such as a Raspberry Pi) without running a server: each device keeps a local SQLite database,
and devices exchange immutable files through the user's own Google Drive.

A typical embedded use: a robot or sensor board receives tasks and configuration from a phone
app and sends logs back, with no server or MQTT broker in between.

```python
from bothy import Hlc, FORMAT_VERSION

h = Hlc.from_hex("018bcfe568000001")
print(h.physical_ms, h.logical)   # 1700000000000 1
```

- Repository: https://github.com/bothy-sync/bothy
- License: MIT OR Apache-2.0
