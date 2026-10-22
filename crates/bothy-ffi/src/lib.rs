//! C ABI for Bothy (`include/bothy.h`).
//!
//! - **Handles:** a database is an opaque `BothyDb *` from `bothy_open`, freed by
//!   `bothy_close`.
//! - **Values:** documents and reports cross as JSON strings.
//! - **Returned strings:** owned by the caller; release them with `bothy_string_free`.
//! - **Errors:** failures return `NULL` or `-1`. `bothy_last_error` and
//!   `bothy_last_error_kind` describe the last failure on the calling thread.
//! - **Threads:** every function is safe to call from any thread.
//! - **Panics:** a panic inside the library is caught and reported as `BOTHY_INTERNAL`.
//!
//! The configuration is a JSON object:
//!
//! ```json
//! {"path": "robot.db", "app_id": "com.example.robot",
//!  "remote": {"folder": "/mnt/shared"},          // or {"drive_folder": "robot"}, {"drive_appdata": true}
//!  "password": "...", "encrypt": false,          // optional
//!  "key_hex": "64 hex chars",                     // optional, from bothy_export_key
//!  "device_name": "arm-1", "conflict_log_limit": 1000}
//! ```

use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use bothy::auth::AccessTokenProvider;
use bothy::blocking;
use bothy::remote::drive::DriveRemote;
use bothy::remote::{FolderRemote, RemoteStore};
use bothy::{Config, Error, ErrorKind, ListOptions};
use serde_json::{Value as J, json};

/// Error categories returned by `bothy_last_error_kind`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BothyErrorKind {
    /// No error.
    BothyOk = 0,
    /// The remote is unreachable.
    BothyOffline = 1,
    /// Sign in again (or the token callback returned nothing).
    BothyAuthRequired = 2,
    /// The remote asked to slow down.
    BothyRateLimited = 3,
    /// The remote storage is full.
    BothyQuotaExceeded = 4,
    /// Remote files disagree.
    BothyRemoteInconsistent = 5,
    /// Written by a newer version.
    BothyFormatTooNew = 6,
    /// The local database is newer than this library.
    BothySchemaTooNew = 7,
    /// The password or key is wrong.
    BothyWrongPassword = 8,
    /// The database is encrypted and locked.
    BothyKeyRequired = 9,
    /// A file failed validation.
    BothyCorrupt = 10,
    /// Another process has the database open.
    BothyDatabaseLocked = 11,
    /// A name, id, value or argument is invalid.
    BothyInvalidInput = 12,
    /// A bug or an unexpected failure.
    BothyInternal = 13,
}

fn kind_of(k: ErrorKind) -> BothyErrorKind {
    use BothyErrorKind as K;
    match k {
        ErrorKind::Offline => K::BothyOffline,
        ErrorKind::AuthRequired => K::BothyAuthRequired,
        ErrorKind::RateLimited => K::BothyRateLimited,
        ErrorKind::QuotaExceeded => K::BothyQuotaExceeded,
        ErrorKind::RemoteInconsistent => K::BothyRemoteInconsistent,
        ErrorKind::FormatTooNew => K::BothyFormatTooNew,
        ErrorKind::SchemaTooNew => K::BothySchemaTooNew,
        ErrorKind::WrongPassword => K::BothyWrongPassword,
        ErrorKind::KeyRequired => K::BothyKeyRequired,
        ErrorKind::Corrupt => K::BothyCorrupt,
        ErrorKind::DatabaseLocked => K::BothyDatabaseLocked,
        ErrorKind::InvalidInput => K::BothyInvalidInput,
        _ => K::BothyInternal,
    }
}

thread_local! {
    static LAST: RefCell<(BothyErrorKind, CString)> =
        RefCell::new((BothyErrorKind::BothyOk, CString::default()));
}

fn set_error(kind: BothyErrorKind, message: &str) {
    let msg = CString::new(message.replace('\0', " ")).unwrap_or_default();
    LAST.with(|l| *l.borrow_mut() = (kind, msg));
}

fn clear_error() {
    LAST.with(|l| *l.borrow_mut() = (BothyErrorKind::BothyOk, CString::default()));
}

fn invalid(m: &str) -> Error {
    Error::new(ErrorKind::InvalidInput, m.to_owned())
}

/// Runs `f`, records its error or panic, and maps the result for C.
fn guard<T>(fail: T, f: impl FnOnce() -> bothy::Result<T>) -> T {
    clear_error();
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => {
            set_error(kind_of(e.kind()), &e.to_string());
            fail
        }
        Err(_) => {
            set_error(BothyErrorKind::BothyInternal, "internal panic in bothy");
            fail
        }
    }
}

/// Borrows a C string argument.
///
/// # Safety
/// `p` is NULL or a valid NUL-terminated string that outlives the call.
unsafe fn arg<'a>(p: *const c_char, name: &str) -> bothy::Result<&'a str> {
    if p.is_null() {
        return Err(invalid(&format!("{name} is NULL")));
    }
    // SAFETY: the caller passes a valid NUL-terminated string.
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .map_err(|_| invalid(&format!("{name} is not UTF-8")))
}

fn out(s: String) -> *mut c_char {
    CString::new(s.replace('\0', "")).map_or(std::ptr::null_mut(), CString::into_raw)
}

/// An open database.
pub struct BothyDb {
    db: blocking::Database,
}

/// Fills `buf` (of `len` bytes) with a NUL-terminated access token and returns its length
/// without the NUL, or a negative value if there is none (signed out). `force_refresh` is true
/// after the previous token was rejected.
pub type BothyTokenFn = Option<
    unsafe extern "C" fn(
        ctx: *mut c_void,
        force_refresh: bool,
        buf: *mut c_char,
        len: usize,
    ) -> i64,
>;

struct CTokens {
    f: unsafe extern "C" fn(*mut c_void, bool, *mut c_char, usize) -> i64,
    ctx: usize,
    stale: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl AccessTokenProvider for CTokens {
    async fn access_token(&self) -> bothy::Result<String> {
        let refresh = self.stale.swap(false, std::sync::atomic::Ordering::SeqCst);
        let mut buf = vec![0u8; 4096];
        // SAFETY: the callback contract in bothy.h: it writes at most `len` bytes into `buf`.
        let n = unsafe {
            (self.f)(
                self.ctx as *mut c_void,
                refresh,
                buf.as_mut_ptr().cast(),
                buf.len(),
            )
        };
        if n <= 0 || n as usize >= buf.len() {
            return Err(Error::new(
                ErrorKind::AuthRequired,
                "the token callback returned no token",
            ));
        }
        buf.truncate(n as usize);
        String::from_utf8(buf).map_err(|_| invalid("the token is not UTF-8"))
    }

    async fn invalidate(&self, _rejected: &str) {
        self.stale.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

fn open(config: &str, tokens: Option<Arc<dyn AccessTokenProvider>>) -> bothy::Result<Box<BothyDb>> {
    let c: J =
        serde_json::from_str(config).map_err(|e| invalid(&format!("config is not JSON: {e}")))?;
    let s = |k: &str| c[k].as_str();
    let path = s("path").ok_or_else(|| invalid("config needs \"path\""))?;
    let app_id = s("app_id").ok_or_else(|| invalid("config needs \"app_id\""))?;
    let r = &c["remote"];
    let need_tokens = || {
        tokens
            .clone()
            .ok_or_else(|| invalid("a Drive remote needs bothy_open_with_tokens"))
    };
    let remote: Arc<dyn RemoteStore> = if let Some(dir) = r["folder"].as_str() {
        Arc::new(FolderRemote::new(dir)?)
    } else if let Some(name) = r["drive_folder"].as_str() {
        Arc::new(DriveRemote::folder(need_tokens()?, name)?)
    } else if r["drive_appdata"].as_bool() == Some(true) {
        Arc::new(DriveRemote::appdata(need_tokens()?)?)
    } else {
        return Err(invalid(
            "config \"remote\" needs \"folder\", \"drive_folder\" or \"drive_appdata\"",
        ));
    };
    let limit = match &c["conflict_log_limit"] {
        J::Null if c.get("conflict_log_limit").is_some() => None,
        J::Null => Some(1000),
        v => Some(
            v.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| invalid("bad conflict_log_limit"))?,
        ),
    };
    let mut config = Config::new(path, app_id).conflict_log(limit);
    if let Some(n) = s("device_name") {
        config = config.device_name(n);
    }
    let encrypt = c["encrypt"].as_bool().unwrap_or(false);
    config = match (s("password"), s("key_hex")) {
        (Some(p), _) if encrypt => config.encrypt(p),
        (Some(p), _) => config.password(p),
        (None, Some(hex)) => config.key(key_from_hex(hex)?),
        (None, None) if encrypt => return Err(invalid("encrypt needs a password")),
        (None, None) => config,
    };
    Ok(Box::new(BothyDb {
        db: blocking::Database::open(config, remote)?,
    }))
}

fn key_from_hex(hex: &str) -> bothy::Result<bothy::crypto::DataKey> {
    let bad = || invalid("key_hex must be 64 hex characters");
    if hex.len() != 64 {
        return Err(bad());
    }
    let mut k = [0u8; 32];
    for (i, b) in k.iter_mut().enumerate() {
        *b = u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|_| bad())?;
    }
    Ok(bothy::crypto::DataKey::from_bytes(k))
}

/// Opens (or creates) a local database with a folder remote. Returns NULL on error.
///
/// # Safety
/// `config_json` is a valid NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_open(config_json: *const c_char) -> *mut BothyDb {
    guard(std::ptr::null_mut(), || {
        // SAFETY: forwarded from the caller.
        let cfg = unsafe { arg(config_json, "config_json") }?;
        Ok(Box::into_raw(open(cfg, None)?))
    })
}

/// Like `bothy_open`, with Google Drive tokens from `token_fn` (called from library threads,
/// possibly concurrently with the caller; `ctx` is passed back unchanged).
///
/// # Safety
/// `config_json` is a valid NUL-terminated string; `token_fn` stays callable with `ctx` until
/// `bothy_close`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_open_with_tokens(
    config_json: *const c_char,
    token_fn: BothyTokenFn,
    ctx: *mut c_void,
) -> *mut BothyDb {
    guard(std::ptr::null_mut(), || {
        // SAFETY: forwarded from the caller.
        let cfg = unsafe { arg(config_json, "config_json") }?;
        let f = token_fn.ok_or_else(|| invalid("token_fn is NULL"))?;
        let tokens = Arc::new(CTokens {
            f,
            ctx: ctx as usize,
            stale: Default::default(),
        });
        Ok(Box::into_raw(open(cfg, Some(tokens))?))
    })
}

/// Borrows the database behind a handle.
///
/// # Safety
/// `db` is NULL or a live handle from `bothy_open`.
unsafe fn handle<'a>(db: *const BothyDb) -> bothy::Result<&'a BothyDb> {
    // SAFETY: the caller passes a live handle.
    unsafe { db.as_ref() }.ok_or_else(|| invalid("db is NULL"))
}

/// Attaches to the oldest database of the app on the remote, or creates one. Returns its id
/// (free with `bothy_string_free`), or NULL on error.
///
/// # Safety
/// `db` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_init_or_join(db: *mut BothyDb) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        // SAFETY: forwarded from the caller.
        let d = unsafe { handle(db) }?;
        Ok(out(d.db.init_or_join()?.to_hex()))
    })
}

/// Replaces a document with the JSON object `json`. Returns 1 if it changed, 0 if not, -1 on
/// error.
///
/// # Safety
/// `db` is a live handle; the strings are valid NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_put(
    db: *mut BothyDb,
    collection: *const c_char,
    id: *const c_char,
    json: *const c_char,
) -> i32 {
    guard(-1, || {
        // SAFETY: forwarded from the caller.
        let (d, c, i, j) = unsafe {
            (
                handle(db)?,
                arg(collection, "collection")?,
                arg(id, "id")?,
                arg(json, "json")?,
            )
        };
        let doc: J =
            serde_json::from_str(j).map_err(|e| invalid(&format!("not valid JSON: {e}")))?;
        Ok(i32::from(d.db.collection(c)?.put(i, doc)?))
    })
}

/// Sets the fields of the JSON object `json` on a document. Returns 1 if it changed, 0 if
/// not, -1 on error.
///
/// # Safety
/// As `bothy_put`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_patch(
    db: *mut BothyDb,
    collection: *const c_char,
    id: *const c_char,
    json: *const c_char,
) -> i32 {
    guard(-1, || {
        // SAFETY: forwarded from the caller.
        let (d, c, i, j) = unsafe {
            (
                handle(db)?,
                arg(collection, "collection")?,
                arg(id, "id")?,
                arg(json, "json")?,
            )
        };
        let doc: J =
            serde_json::from_str(j).map_err(|e| invalid(&format!("not valid JSON: {e}")))?;
        Ok(i32::from(d.db.collection(c)?.patch(i, doc, &[])?))
    })
}

/// The document as JSON, or NULL if it does not exist or on error (then
/// `bothy_last_error_kind` is not `BOTHY_OK`). Free with `bothy_string_free`.
///
/// # Safety
/// `db` is a live handle; the strings are valid NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_get(
    db: *mut BothyDb,
    collection: *const c_char,
    id: *const c_char,
) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        // SAFETY: forwarded from the caller.
        let (d, c, i) = unsafe { (handle(db)?, arg(collection, "collection")?, arg(id, "id")?) };
        Ok(d.db
            .collection(c)?
            .get(i)?
            .map_or(std::ptr::null_mut(), |v| out(v.to_json().to_string())))
    })
}

/// Up to `limit` documents in id order after the id `after` (may be NULL), as a JSON array
/// of `{"id": ..., "doc": {...}}`. Free with `bothy_string_free`; NULL on error.
///
/// # Safety
/// `db` is a live handle; `collection` is a valid string; `after` is NULL or a valid string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_list(
    db: *mut BothyDb,
    collection: *const c_char,
    limit: u32,
    after: *const c_char,
) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        // SAFETY: forwarded from the caller.
        let (d, c) = unsafe { (handle(db)?, arg(collection, "collection")?) };
        let mut opts = ListOptions::default().limit(limit as usize);
        if !after.is_null() {
            // SAFETY: non-NULL and valid per the contract.
            opts = opts.after(unsafe { arg(after, "after") }?);
        }
        let rows: Vec<J> =
            d.db.collection(c)?
                .list(opts)?
                .into_iter()
                .map(|(id, v)| json!({"id": id.as_str(), "doc": v.to_json()}))
                .collect();
        Ok(out(J::Array(rows).to_string()))
    })
}

/// Deletes a document. Returns 1 if it existed, 0 if not, -1 on error.
///
/// # Safety
/// `db` is a live handle; the strings are valid NUL-terminated strings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_delete(
    db: *mut BothyDb,
    collection: *const c_char,
    id: *const c_char,
) -> i32 {
    guard(-1, || {
        // SAFETY: forwarded from the caller.
        let (d, c, i) = unsafe { (handle(db)?, arg(collection, "collection")?, arg(id, "id")?) };
        Ok(i32::from(d.db.collection(c)?.delete(i)?))
    })
}

/// Pushes local changes and pulls remote ones. Returns a JSON report (free with
/// `bothy_string_free`), or NULL on error.
///
/// # Safety
/// `db` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_sync(db: *mut BothyDb) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        // SAFETY: forwarded from the caller.
        let r = unsafe { handle(db) }?.db.sync()?;
        Ok(out(json!({
            "pushed_ops": r.pushed_ops, "segments_up": r.segments_up,
            "segments_down": r.segments_down, "pulled_ops": r.pulled_ops,
            "changed_docs": r.changed_docs, "password_changed": r.password_changed,
            "skipped": r.skipped,
        })
        .to_string()))
    })
}

/// The 32-byte data key of an unlocked encrypted database as 64 hex characters (keep it
/// secret; pass it back as `key_hex`). Free with `bothy_string_free`; NULL on error.
///
/// # Safety
/// `db` is a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_export_key(db: *mut BothyDb) -> *mut c_char {
    guard(std::ptr::null_mut(), || {
        // SAFETY: forwarded from the caller.
        let key = unsafe { handle(db) }?.db.export_key()?;
        Ok(out(key
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()))
    })
}

/// The category of the last error on this thread (`BOTHY_OK` if the last call succeeded).
#[unsafe(no_mangle)]
pub extern "C" fn bothy_last_error_kind() -> BothyErrorKind {
    LAST.with(|l| l.borrow().0)
}

/// The message of the last error on this thread, or "" — valid until the next call on this
/// thread. Never contains tokens, passwords or keys.
#[unsafe(no_mangle)]
pub extern "C" fn bothy_last_error() -> *const c_char {
    LAST.with(|l| l.borrow().1.as_ptr())
}

/// Frees a string returned by this library. NULL is ignored.
///
/// # Safety
/// `s` is NULL or a string returned by this library, not freed yet.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_string_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: the string came from CString::into_raw in `out`.
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Closes the database and frees the handle. NULL is ignored.
///
/// # Safety
/// `db` is NULL or a live handle, not used afterwards.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bothy_close(db: *mut BothyDb) {
    if !db.is_null() {
        // SAFETY: the handle came from Box::into_raw in `bothy_open*`.
        let _ = catch_unwind(AssertUnwindSafe(|| drop(unsafe { Box::from_raw(db) })));
    }
}

/// The library version, e.g. "0.3.0". Static; do not free.
#[unsafe(no_mangle)]
pub extern "C" fn bothy_version() -> *const c_char {
    static V: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    V.as_ptr().cast()
}
