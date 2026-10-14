# bothy

**Serverless, local-first sync engine that uses the user's own cloud storage as the transport.**

> ⚠️ **Early development.** The API and the on-remote format will change. Do not use it for
> real data yet.

In Scotland and northern England, a *bothy* is an unlocked mountain shelter that belongs to no
one: anyone may use it, and travellers leave notes and supplies for whoever comes next. Bothy
works the same way: there is no server, the data stays in the user's own cloud account, and
devices drop off and pick up files at a shared place.

## What it will do

- Every device keeps a local SQLite database as the source of truth; apps work offline.
- Devices exchange immutable, append-only files through the user's own Google Drive
  (`appDataFolder`). No backend, no accounts of our own.
- Changes are ordered with a hybrid logical clock and merged per field (last-writer-wins), so
  every device converges to the same state.
- A documented, versioned on-remote format (`SPEC.md`) so other languages can interoperate.
- Bindings for Flutter/Dart, Python and C, aimed at mobile, desktop and embedded Linux.

## What is in this release

Only the hybrid logical clock (`Hlc`, `HlcClock`) and the format version constant:

```rust
use bothy::{Hlc, HlcClock};

let mut clock = HlcClock::new();
let a = clock.now(1_700_000_000_000);
let b = clock.now(1_700_000_000_000);
assert!(b > a);
```

## Links

- Repository: <https://github.com/bothy-sync/bothy>
- CLI: [`bothy-cli`](https://crates.io/crates/bothy-cli)

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
