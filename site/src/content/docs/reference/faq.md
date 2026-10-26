---
title: FAQ
description: Common questions.
---

### Do I need a server?

No. Devices sync through the user's Google Drive. For P2P, a public STUN server (Google's by
default) tells devices their public address; it never sees data.

### Who can read the data?

The user, through their Google account, and any client of your Google Cloud project that the
user signs in to. With encryption on, Google and anyone else with the files see only names,
sizes and times.

### What happens when two devices edit the same note offline?

Different fields both survive. For the same field the newer write wins, and the older value
goes to the conflict log. See [Offline edits and conflicts](/concepts/conflicts/).

### Can two Google accounts share a database?

Not yet. A database lives in one user's Drive.

### Is Bothy free?

Yes, MIT or Apache-2.0. Running it costs nothing: Drive storage is the user's, and Drive API
quota is free within Google's limits.

### Does it work with iCloud, OneDrive or Dropbox?

Not yet. The engine talks to storage through a small `RemoteStore` trait, so other backends
can be added; a second provider is on the roadmap.

### How big can a database get?

Tested up to 100,000 documents and 20 devices. See [Limits and quota](/concepts/limits/).

### I forgot the password.

If no device still has the key, the encrypted files cannot be read. Each device's local copy
still has the data. See [Encryption](/concepts/encryption/).
