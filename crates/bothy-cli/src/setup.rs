//! `bothy setup google`: the checks behind the walk-through that creates a Google Cloud project
//! and OAuth clients. The prompts live in `commands.rs`; everything here is pure and tested.

use std::path::{Path, PathBuf};

use serde_json::Value as J;

/// What kind of OAuth client a downloaded JSON file is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientKind {
    /// "Desktop app": browser sign-in with a loopback redirect.
    Desktop,
    /// "TVs and Limited Input devices": device code sign-in.
    Device,
    /// "Web application".
    Web,
    /// Android or iOS (no secret; for apps, not for the CLI).
    Mobile,
}

impl ClientKind {
    pub fn label(self) -> &'static str {
        match self {
            ClientKind::Desktop => "Desktop app",
            ClientKind::Device => "TVs and Limited Input devices",
            ClientKind::Web => "Web application",
            ClientKind::Mobile => "Android or iOS",
        }
    }
}

/// The parts of a client file the walk-through checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientFile {
    pub kind: ClientKind,
    pub id: String,
    pub project_id: Option<String>,
}

/// Reads a client JSON file as downloaded from the Cloud console.
///
/// Desktop and TV clients are both `installed`; only the desktop one has `redirect_uris`.
/// Android and iOS clients have no secret.
pub fn inspect(bytes: &[u8]) -> Result<ClientFile, String> {
    let j: J = serde_json::from_slice(bytes)
        .map_err(|_| "this is not a JSON file from the Cloud console".to_owned())?;
    let (inner, web) = match (j.get("installed"), j.get("web")) {
        (Some(c), _) => (c, false),
        (None, Some(c)) => (c, true),
        _ => {
            return Err(
                "this JSON is not an OAuth client file (no \"installed\" or \"web\")".into(),
            );
        }
    };
    let id = inner["client_id"]
        .as_str()
        .filter(|s| s.ends_with(".apps.googleusercontent.com"))
        .ok_or("the file has no client id ending in .apps.googleusercontent.com")?
        .to_owned();
    let has_secret = inner["client_secret"]
        .as_str()
        .is_some_and(|s| !s.is_empty());
    let kind = if web {
        ClientKind::Web
    } else if !has_secret {
        ClientKind::Mobile
    } else if inner.get("redirect_uris").is_some() {
        ClientKind::Desktop
    } else {
        ClientKind::Device
    };
    Ok(ClientFile {
        kind,
        id,
        project_id: inner["project_id"].as_str().map(str::to_owned),
    })
}

/// Checks that `file` is a client of `want` kind from `project`, with an explanation if not.
pub fn expect(file: &ClientFile, want: ClientKind, project: &str) -> Result<(), String> {
    if file.kind != want {
        return Err(format!(
            "this is a \"{}\" client; create one of type \"{}\" instead",
            file.kind.label(),
            want.label()
        ));
    }
    match &file.project_id {
        Some(p) if p != project => Err(format!(
            "this client belongs to project {p}, not {project}; all devices of a database must use one project"
        )),
        _ => Ok(()),
    }
}

/// Google's rules for project ids: 6–30 characters, lowercase letters, digits and hyphens,
/// starting with a letter and not ending with a hyphen.
pub fn check_project_id(id: &str) -> Result<(), String> {
    let ok = (6..=30).contains(&id.len())
        && id.starts_with(|c: char| c.is_ascii_lowercase())
        && !id.ends_with('-')
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "{id:?} is not a project id (6–30 lowercase letters, digits or hyphens, starting with a letter); copy it from the console's project picker"
        ))
    }
}

/// Cloud console pages of the walk-through.
#[derive(Clone, Copy, Debug)]
pub enum Page {
    CreateProject,
    DriveApi,
    Branding,
    Audience,
    DataAccess,
    CreateClient,
}

pub fn console_url(page: Page, project: &str) -> String {
    let base = "https://console.cloud.google.com";
    match page {
        Page::CreateProject => format!("{base}/projectcreate"),
        Page::DriveApi => format!("{base}/apis/library/drive.googleapis.com?project={project}"),
        Page::Branding => format!("{base}/auth/branding?project={project}"),
        Page::Audience => format!("{base}/auth/audience?project={project}"),
        Page::DataAccess => format!("{base}/auth/scopes?project={project}"),
        Page::CreateClient => format!("{base}/auth/clients/create?project={project}"),
    }
}

/// The newest `client_secret_*.json` in `dir` (where browsers save the download).
pub fn newest_client_file(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| {
            let n = e.file_name();
            let n = n.to_string_lossy();
            n.starts_with("client_secret") && n.ends_with(".json")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// What a failed Drive check means for someone setting up, if we know.
pub fn explain_drive_error(message: &str, project: &str) -> Option<String> {
    if message.contains("accessNotConfigured") {
        Some(format!(
            "the Google Drive API is not enabled in project {project}: enable it at {}",
            console_url(Page::DriveApi, project)
        ))
    } else if message.contains("lacks the Drive scope") {
        Some(format!(
            "the sign-in did not grant the Drive scopes: add drive.appdata and drive.file at {}, then run `bothy auth login` again",
            console_url(Page::DataAccess, project)
        ))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "123-abc.apps.googleusercontent.com";

    fn file(json: &str) -> Result<ClientFile, String> {
        inspect(json.as_bytes())
    }

    #[test]
    fn tells_client_kinds_apart() {
        let desktop = format!(
            r#"{{"installed":{{"client_id":"{ID}","client_secret":"s","project_id":"my-sync-1","redirect_uris":["http://localhost"]}}}}"#
        );
        let tv = format!(
            r#"{{"installed":{{"client_id":"{ID}","client_secret":"s","project_id":"my-sync-1"}}}}"#
        );
        let web = format!(r#"{{"web":{{"client_id":"{ID}","client_secret":"s"}}}}"#);
        let android = format!(r#"{{"installed":{{"client_id":"{ID}","project_id":"x"}}}}"#);
        assert_eq!(file(&desktop).unwrap().kind, ClientKind::Desktop);
        assert_eq!(file(&tv).unwrap().kind, ClientKind::Device);
        assert_eq!(file(&web).unwrap().kind, ClientKind::Web);
        assert_eq!(file(&android).unwrap().kind, ClientKind::Mobile);
        assert_eq!(
            file(&desktop).unwrap().project_id.as_deref(),
            Some("my-sync-1")
        );
        assert!(file("[]").is_err());
        assert!(file("not json").is_err());
        assert!(file(r#"{"installed":{"client_id":"nope","client_secret":"s"}}"#).is_err());
    }

    #[test]
    fn expects_the_right_kind_and_project() {
        let f = ClientFile {
            kind: ClientKind::Web,
            id: ID.into(),
            project_id: Some("my-sync-1".into()),
        };
        let e = expect(&f, ClientKind::Desktop, "my-sync-1").unwrap_err();
        assert!(
            e.contains("Web application") && e.contains("Desktop app"),
            "{e}"
        );
        let f = ClientFile {
            kind: ClientKind::Desktop,
            ..f
        };
        assert!(expect(&f, ClientKind::Desktop, "my-sync-1").is_ok());
        let e = expect(&f, ClientKind::Desktop, "other-proj").unwrap_err();
        assert!(e.contains("my-sync-1"), "{e}");
    }

    #[test]
    fn project_ids_follow_google_rules() {
        for ok in ["my-sync-1", "abcdef", "a23456789012345678901234567890"] {
            assert!(check_project_id(ok).is_ok(), "{ok}");
        }
        for bad in ["abc", "My-Sync", "1abcdef", "abcdef-", "abc_def", ""] {
            assert!(check_project_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn urls_and_errors_name_the_project() {
        assert!(console_url(Page::DriveApi, "p-123456").ends_with("?project=p-123456"));
        let e = explain_drive_error(
            "Google Drive refused the request (accessNotConfigured)",
            "p-123456",
        )
        .unwrap();
        assert!(e.contains("drive.googleapis.com?project=p-123456"), "{e}");
        assert!(explain_drive_error("cannot reach Google Drive", "p").is_none());
    }

    #[test]
    fn finds_the_newest_download() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(newest_client_file(dir.path()), None);
        std::fs::write(dir.path().join("client_secret_old.json"), "{}").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.path().join("client_secret_new.json"), "{}").unwrap();
        std::fs::write(dir.path().join("other.json"), "{}").unwrap();
        assert_eq!(
            newest_client_file(dir.path()).unwrap().file_name().unwrap(),
            "client_secret_new.json"
        );
    }
}
