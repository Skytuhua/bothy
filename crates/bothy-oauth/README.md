# bothy-oauth

Google OAuth helpers for [Bothy](https://github.com/bothy-sync/bothy) apps that run their own
sign-in: the CLI, the Python binding and embedded agents. The core crate `bothy` never does
OAuth; it only asks an `AccessTokenProvider` for tokens.

- `loopback_login`: browser sign-in with a `127.0.0.1` redirect and PKCE (RFC 8252).
- `device_login`: the device flow for machines without a browser. Google allows only the
  `drive.file` scope there.
- `RefreshingTokens`: an `AccessTokenProvider` that refreshes and caches access tokens.
- `TokenStore`: keeps the grant in the OS keyring, or in a file readable only by the user.

```rust
use bothy_oauth::{Client, RefreshingTokens, TokenStore, device_login};

let client = Client::from_file("client_secret_tv.json")?;
let grant = device_login(&client, |code| {
    println!("Open {} and enter {}", code.verification_url, code.user_code);
})
.await?;
let store = TokenStore::file_only("agent/token.json");
store.save(&grant)?;
let tokens = std::sync::Arc::new(RefreshingTokens::new(client, &grant)?);
// bothy::remote::drive::DriveRemote::folder(tokens, "robot")
```

Use OAuth clients of your own Google Cloud project; every client of one project sees the same
Drive files.

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option.
