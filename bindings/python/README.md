# bothy (Python)

**Serverless, local-first sync engine that uses your own Google Drive as the transport.**

> ⚠️ **Early release.** The API may still change between minor versions.

Bothy lets programs sync structured data between devices without running a server. The
devices can be desktops, phones (through the Flutter package) or embedded Linux boards such
as a Raspberry Pi. Each device keeps a local SQLite database and works offline; devices
exchange immutable files through the user's own Google Drive or a shared folder.

Concurrent edits to different fields merge. Optionally, everything stored on Drive is
end-to-end encrypted.

A typical embedded setup: a robot gets missions from a phone app and sends logs back, with no
server or MQTT broker in between.

```python
import bothy

db = bothy.Database.open("robot.db", app_id="com.example.robot",
                         remote=bothy.FolderRemote("/mnt/shared/robot"))
db.init_or_join()

missions = db.collection("missions")
missions.put("m1", {"goal": "dock", "speed": 0.5})
missions.patch("m1", {"state": "accepted"})
print(missions.get("m1"))

db.start_auto_sync()                      # or db.sync() when you choose
for change in db.changes(timeout=30):     # local and remote changes
    print(change.collection, change.id, "from another device" if change.remote else "")
db.close()
```

The API is synchronous and thread-safe, and calls release the GIL. Wheels are abi3, for
CPython 3.9 or newer on Linux (x86_64, aarch64, armv7), macOS and Windows.

## Google Drive

Bothy never signs in by itself. Use OAuth clients of your own Google Cloud project; every
client of one project sees the same Drive files.

```python
import bothy, bothy.auth

# A board without a browser: the device flow (a "TVs and Limited Input devices" client).
grant = bothy.auth.device_login("client_tv.json")   # prints a URL and a code to enter on a phone
grant.save("token.json")                            # readable only by you

tokens = bothy.auth.TokenProvider("client_tv.json", bothy.auth.Grant.load("token.json"))
db = bothy.Database.open("robot.db", app_id="com.example.robot",
                         remote=bothy.DriveRemote.folder("robot", tokens))
```

- **Remote type:** the device flow only grants the `drive.file` scope, so headless devices
  use `DriveRemote.folder` (a visible `Bothy/<name>` folder).
- **Desktops** can use `bothy.auth.loopback_login` with a "Desktop app" client, and
  `DriveRemote.appdata` (the hidden app folder).
- **Your own tokens:** any callable `(force_refresh: bool) -> str` works as `tokens`.

## Encryption

```python
db = bothy.Database.open(..., encrypt=True, password=pw)   # the device that creates it
db = bothy.Database.open(..., password=pw)                 # devices that join
key = db.export_key()                                      # keep it to skip the password
db = bothy.Database.open(..., key=key)
```

- **Wrong password:** raises `bothy.WrongPasswordError`.
- **Locked database:** opened without a secret, an encrypted database is locked. Local reads
  and writes work; `sync()` raises `bothy.KeyRequiredError` until `unlock()`.
- **Embedded boards:** keep the key instead of the password. Unlocking with a password costs
  64 MiB of memory once; syncing with the key does not.

## Errors

Every engine error is a `bothy.BothyError`. Each error kind has its own subclass, for
example `OfflineError`, `AuthRequiredError`, `WrongPasswordError`, `KeyRequiredError` and
`InvalidInputError`.

## Building from source

```sh
pip install maturin
maturin develop            # needs a Rust toolchain
python -m unittest discover -s tests
```

- Repository: https://github.com/bothy-sync/bothy
- License: MIT OR Apache-2.0
