---
title: Encryption
description: End-to-end encryption with a password, and what stays visible.
---

A database can be created **encrypted**. Everything it stores on Drive is then encrypted on
the device, before upload: operations, snapshots and device descriptions.

- **Ciphers:** XChaCha20-Poly1305 with a random 256-bit data key.
- **The data key** is wrapped with a key derived from the password by **Argon2id** (64 MiB,
  2 passes by default). Every device keeps its own wrapped copy in a key file on the remote.
- **Local reads and writes** never need the password. Syncing does.

```sh
bothy init --encrypt --remember   # first device
bothy join --remember             # other devices, same password
```

```python
db = bothy.Database.open(..., encrypt=True, password=pw)   # first device
db = bothy.Database.open(..., password=pw)                 # other devices
key = db.export_key()                                      # keep it to skip the password next time
```

```dart
final db = await Bothy.open(..., encrypt: true, password: password);
final key = await db.exportKey();   // store it in secure storage, open with key: next time
```

## Keeping the key

Typing the password on every start is tiresome, and deriving the key costs 64 MiB and about a
second on a slow board. Export the data key once and keep it in the platform's secure storage
(the system keyring, `flutter_secure_storage`, a file only the user can read). With the key,
no password is needed and nothing expensive runs.

## Changing the password

Changing the password re-wraps the data key. The old password stops working on every device,
and keys kept on devices keep working. It does **not** re-encrypt existing data, so someone who
already had the data key can still read it; full key rotation is planned.

## What stays visible to Google

File names (database, device and sequence numbers), file sizes and times, and segment headers
(time range and operation count). Collection names, document ids, field names and values are
encrypted. When devices exchange connection details for [direct channels](/concepts/p2p/), those
files are encrypted too, so IP addresses do not appear on Drive in plain text.

## A forgotten password cannot be recovered

There is no server and no recovery key. If every device has lost the key and nobody knows the
password, the data on Drive cannot be read. Each device's local database still has the data,
and a device can publish it again as a new encrypted database.
