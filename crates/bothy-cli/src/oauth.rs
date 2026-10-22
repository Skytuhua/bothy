//! Google OAuth for the CLI, on top of `bothy-oauth`: shows the prompts and opens the browser.
//! Tokens and secrets are never printed.

pub use bothy_oauth::{Client, Grant, SCOPE_APPDATA, SCOPE_FILE};

use crate::fail::CliResult;

/// Signs in through the browser (loopback redirect with PKCE).
pub async fn loopback_login(client: &Client, scopes: &[&str]) -> CliResult<Grant> {
    Ok(bothy_oauth::loopback_login(client, scopes, |url| {
        eprintln!("Opening the browser to sign in. If it does not open, visit:\n\n  {url}\n");
        let _ = open::that_detached(url);
    })
    .await?)
}

/// Signs in on a machine without a browser: show a code, the user approves on another device.
pub async fn device_login(client: &Client) -> CliResult<Grant> {
    Ok(bothy_oauth::device_login(client, |code| {
        eprintln!(
            "On a phone or computer, open {} and enter the code:\n\n  {}\n\nWaiting for approval...",
            code.verification_url, code.user_code
        );
    })
    .await?)
}

/// Revokes a refresh token at Google.
pub async fn revoke(refresh_token: &str) -> CliResult<()> {
    Ok(bothy_oauth::revoke(refresh_token).await?)
}
