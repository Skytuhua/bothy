---
title: Offline edits and conflicts
description: Per-field last-writer-wins, deletes that stick, and a log of what lost.
---

Devices edit while offline and merge later. Bothy merges **per top-level field**:

- **Different fields of the same document both survive.** The laptop changes `title` and the
  phone changes `done`: after sync the note has both changes.
- **The same field keeps the newest write**, by its HLC timestamp. If two writes have the
  same timestamp, the device id breaks the tie, so every device picks the same winner.
- **A delete wins over older edits.** An edit made before the delete, even if it syncs later,
  does not bring the document back. A write made *after* the delete does.

Deleted documents leave a small **tombstone**, so a late, older edit cannot resurrect them.
Tombstones are kept.

## The conflict log

When a write loses to a newer one, the losing value is not just dropped: it goes to a local
**conflict log** (up to 1,000 records by default), so your app can show "this was overwritten"
or offer to restore it.

```sh
bothy conflicts --since 2h
```

```python
for c in db.conflicts():
    print(c["collection"], c["id"], c["field"], c["loser_value_json"])
```

```dart
final lost = await db.conflicts(since: DateTime.now().subtract(Duration(hours: 2)));
```

## Clocks

Last-writer-wins needs sane clocks. The HLC follows the largest time it has seen, but limits
how far a remote timestamp may pull it ahead (one hour), so a device with a wrong clock
cannot win every conflict for days.

## When this is not enough

Per-field merging fits records: notes, tasks, settings, missions, readings. It does not merge
two people typing in the same paragraph. For collaborative text, store the text as a CRDT
(for example Automerge or Yjs) in a bytes field and merge it in the app; an optional CRDT
mode is on the roadmap.
