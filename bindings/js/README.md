# @bothy-sync/core

**Format constants, TypeScript types and HLC helpers for
[Bothy](https://github.com/bothy-sync/bothy), a serverless, local-first sync engine that uses
your own Google Drive as the transport.**

> ⚠️ **Early development.** The format is a draft and will change.

Bothy's sync engine is written in Rust with bindings for Flutter, Python and C. This package
gives JavaScript and TypeScript code the shared pieces of the on-remote format: the format
version, the `manifest.json` types and helpers for the hybrid logical clock (HLC) timestamps
that order every change.

```js
import { parseHlc, hlcToParts, FORMAT_VERSION } from "@bothy-sync/core";

hlcToParts(parseHlc("018bcfe568000001")); // { physicalMs: 1700000000000, logical: 1 }
```

For JavaScript apps that need full sync today, see RxDB's Google Drive replication. Bothy
targets Flutter, native, Python and embedded devices, with a documented format.

License: MIT OR Apache-2.0
