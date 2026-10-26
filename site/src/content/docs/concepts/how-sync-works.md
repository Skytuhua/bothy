---
title: How sync works
description: Immutable segments on the remote, a local database on every device.
---

Every device has a **local SQLite database**, the source of truth on that device. Reads and
writes go there and return at once, online or not. Sync moves changes between devices
through a **remote**: a folder in the user's Google Drive, or any shared directory.

## Writing

A write becomes an **operation**: "set these fields of document `n1` in `notes`", "unset a
field", or "delete the document". Each operation carries a timestamp from a **hybrid logical
clock** (HLC): wall-clock milliseconds plus a counter. It orders events even when device
clocks disagree a little, and a device's clock cannot jump more than an hour ahead of what it
has seen.

Operations wait in a local outbox. When the device syncs, it packs them into a **segment**: a
compressed (zstd) CBOR file, numbered per device:

```
<db>/devices/<device>/seg-0000000000000007.bseg
```

Segments are **immutable** and every device writes only under its own name, so devices never
overwrite each other's files and need no locks. An interrupted upload is retried with
exactly the same bytes.

## Reading

To pull, a device lists what is new on the remote (Drive's change feed, or a directory
listing), downloads the segments it has not applied yet and merges them into its database.
Applying a segment twice changes nothing, and segments can arrive in any order.

## Joining and catching up

A device that joins, or comes back after a long time, does not replay every segment from the
beginning. Devices regularly write a **snapshot** of the whole state, and a newcomer starts
from the most complete one and applies only the segments after it. Old segments that a
snapshot covers are deleted after a grace period (30 days by default).

## When sync happens

With background sync (`start_auto_sync`, `startAutoSync`, `bothy sync --watch`):

- **after local writes:** once writing has paused for 2 s, and at most 10 s after the first
  unsynced write;
- **otherwise:** a poll every 30 s (5 minutes for background and embedded use);
- **after failures:** retries with an exponential backoff up to 15 minutes;
- **with a direct channel open:** after 100 ms, and the peer applies the segment at once
  (see [Direct channels](/concepts/p2p/)).

You can always call `sync()` yourself.

## Same result everywhere

Merging is deterministic: when every device has seen the same segments, every device holds
byte-for-byte the same state. A seeded simulation with crashes, lost responses, clock skew,
rate limits and restored copies checks this on 500 scenarios in CI.
