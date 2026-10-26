---
title: Direct channels (P2P)
description: Writes in about a second between devices that are online together.
---

Without a server there is no way for Google Drive to tell a device "something changed":
Drive's push notifications need a public web server. So devices poll, every 30 seconds by
default, and a write takes 15–30 s to show up elsewhere. That is fine for most data, but
not when someone sends a mission from a phone to a robot and waits for it to move.

**Direct channels** fix that for devices that are online at the same time.

## What happens

1. A device with P2P on writes a small "online" file to the remote.
2. Another device sees it at its next poll and writes a connection **offer**; the first one
   answers. The files carry WebRTC session descriptions and are encrypted in encrypted
   databases.
3. The two open a direct, encrypted WebRTC data channel. That takes 10–60 s, once.
4. From then on, right after a device uploads a segment it sends the same bytes over the
   channel, and the peer applies them at once.

Measured from a write until the other device sees it, over Drive:

| | p50 |
|---|---|
| Polling only | about 30 s |
| Direct channel, Mac → Android | 1.1–1.9 s |
| Direct channel, Android → Mac | 1.9 s |

Most of the remaining time is the Drive upload itself, which must finish before the bytes
are sent.

## Turning it on

```rust
let config = Config::new("notes.db", "com.example.notes").p2p(P2pConfig::new());
```

```python
db = bothy.Database.open(..., p2p=True)
```

```dart
final db = await Bothy.open(..., p2p: true);
```

```sh
bothy sync --watch --p2p
```

P2P runs while background sync runs. `p2p_status()` / `p2pStatus()` reports whether it is
looking for devices and the state of each peer.

## When there is no channel

Nothing is lost: sync goes on through Drive as before.

- **Durability:** the remote is always the durable record. Nothing travels over the channel
  before it is uploaded.
- **Channel loss:** a lost channel is noticed within about 20 s and retried (15 s after a
  failure, doubling up to 10 minutes). In tests, a phone back from a network outage reconnected
  within about a minute and a half.
- **What blocks a channel:** both devices must be online with background sync running; a phone
  app in the background has no channel. Strict corporate or carrier networks can block direct
  connections, and there is no relay server (TURN), so such devices keep polling.

## STUN

To connect across networks, each device asks a STUN server for its public address. The
default is Google's public server (`stun.l.google.com:19302`). It learns only the device's
address, never any data. Pass an empty list (`stun=[]`, `stunServers: const []`,
`--no-stun`) to stay on the local network.

On iOS, add `NSLocalNetworkUsageDescription` to `Info.plist`; a sandboxed macOS app needs the
`com.apple.security.network.server` entitlement.
