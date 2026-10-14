# bothy-cli

**Command-line tool for [Bothy](https://github.com/bothy-sync/bothy), a serverless, local-first
sync engine that uses your own Google Drive as the transport.**

> ⚠️ **Early development.** This release only prints its version and help. The commands below
> are planned, not implemented.

```sh
cargo install bothy-cli   # installs the `bothy` binary
bothy --version
```

Planned commands:

```
bothy auth login [--device]     # browser or device-code sign-in (for headless boards)
bothy init | join               # create or join a database in your Drive
bothy put <collection> <id> <json>
bothy get <collection> <id>
bothy list <collection>
bothy sync [--watch]
bothy status
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
