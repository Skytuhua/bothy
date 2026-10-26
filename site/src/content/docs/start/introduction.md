---
title: What is Bothy?
description: A sync engine for apps that have no server of their own.
---

Bothy is a sync engine for applications that have **no server of their own**. Your app keeps
its data in a local database on each device, and Bothy moves changes between the devices
through files in the user's own cloud storage, today Google Drive.

In Scotland and northern England, a *bothy* is an unlocked mountain shelter that belongs to
no one: anyone may use it, and travellers leave notes and supplies for whoever comes next.
Bothy works the same way. There is no server; devices drop off and pick up files at a
shared place that the user owns.

## Who it is for

- **Small and personal apps** that need sync across a user's devices but don't want to run,
  secure and pay for a backend: note takers, task lists, journals, trackers, settings.
- **Privacy-minded apps**: the data never leaves the user's own Google account, and it can
  be end-to-end encrypted with a password only the user knows.
- **Robots and embedded boards**: a Raspberry Pi class board takes missions from a phone and
  sends logs back, with no MQTT broker or server in between, even after days offline.

## What you get

- **A local database per device** (SQLite). Reads and writes never wait for the network.
- **Collections of JSON documents**, with `put`, `patch`, `get`, `list`, `delete` and a
  change stream.
- **Deterministic merging.** Hybrid logical clocks and per-field last-writer-wins: every
  device converges to the same state, and a delete is never undone by an older edit.
  Overwritten values are kept in a conflict log.
- **Background sync** that is quick after local writes and polls the remote otherwise.
- **Optional end-to-end encryption**, with password changes that reach every device.
- **Direct channels** between devices that are online at the same time.

## What it is not

- Not a real-time collaborative text editor: concurrent edits of the *same field* keep the
  newest one, they are not merged character by character.
- Not for sharing between different Google accounts. One database belongs to one user.
- Not a server database: there is no query engine across users, and one Google Cloud
  project's Drive quota is shared by all of its users (see [Limits and quota](/concepts/limits/)).

## How it compares

| | Bothy | Sync engines with a server (PowerSync, ElectricSQL, Replicache…) | CRDT libraries (Automerge, Yjs) |
|---|---|---|---|
| Server you run | none | yes | you bring the transport |
| Where data lives | the user's Drive | your backend | wherever you put it |
| Offline-first | yes | yes | yes |
| Merge | per field, last writer wins | varies | character and structure level |
| Platforms | Rust, Flutter, Python, C, CLI | mostly web and mobile | many languages |

Ready? [Start in 15 minutes](/start/quick-start/).
