---
title: File format
description: The on-remote format, and how other languages can implement it.
---

Bothy's files on the remote follow a documented, versioned format, so any language can read
and write the same databases. The current version is `bothy_format = 1`.

```
<db_id>/manifest.json                                    written once by the creating device
<db_id>/devices/<device_id>/device.json                  one per device
<db_id>/devices/<device_id>/seg-<seq:016x>.bseg          immutable segments of operations
<db_id>/snapshots/<device_id>/snap-<hlc:016x>.bsnap      full-state snapshots
<db_id>/keys/<device_id>/key-<hlc:016x>.json             wrapped data keys (encrypted databases)
<db_id>/signaling/<device_id>/…                          optional, for direct channels
```

**Rules:**
- **Single writer:** every device writes only under its own id, and files are never
  overwritten.
- **Encoding:** values are canonical CBOR, and segments and snapshots may be zstd-compressed.
- **Forward compatibility:** readers ignore names they do not know.

**Conformance vectors:** in
[`spec/vectors`](https://github.com/bothy-sync/bothy/tree/main/spec/vectors). They cover the
HLC, values and operations, manifests, merging, segments, snapshots, encryption and signaling
files, so an implementation can check that it produces the same bytes and the same merged
state as the reference.

A format version, once released, never changes its reading rules; a change means a new
version.
