---
title: Where the files live
description: The hidden app folder, a visible Drive folder, or a local directory.
---

A database lives in one **remote**, chosen when it is created:

| Remote | Where | Sign-in | Use when |
|---|---|---|---|
| `appdata` (default) | Drive's hidden per-app folder | browser only | computers and phones |
| `folder:<name>` | a visible `Bothy/<name>` folder in Drive | browser or device code | a headless board will join |
| `path:<dir>` | a local or shared directory | none | development, tests, LAN shares |

**appdata** is invisible in the Drive web interface and only reachable by your Cloud
project's clients. It needs the `drive.appdata` scope, which Google refuses in the device
code flow that headless boards use. So a database that a board will ever join must be a
**folder** database (`drive.file` scope).

In code:

- Rust: `DriveRemote::appdata`, `DriveRemote::folder`, `FolderRemote`.
- Python: `DriveRemote.appdata`, `DriveRemote.folder`, `FolderRemote`.
- Flutter: `BothyRemoteSpec.googleDriveAppData`, `.googleDriveFolder`, `.path`.

The files count against the user's Drive storage. A database of notes is usually a few
megabytes.
