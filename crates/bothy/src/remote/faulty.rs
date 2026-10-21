//! A fault-injecting wrapper for tests and simulations (feature `test-util`).

use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;

use crate::error::{Error, ErrorKind, Result};
use crate::remote::{ChangeBatch, PutOutcome, RemoteEntry, RemoteStore};

/// Probabilities (0.0 to 1.0) and delays of injected faults.
#[derive(Clone, Copy, Debug, Default)]
pub struct Faults {
    /// A call fails with `Offline` before reaching the store.
    pub offline: f64,
    /// A call fails with `RateLimited` before reaching the store.
    pub rate_limited: f64,
    /// `put_new` reaches the store, but the response is lost (`Offline`).
    pub lost_response: f64,
    /// New files stay out of `list` results for up to this many list calls.
    pub list_lag: u32,
}

/// Error message of a simulated crash; the caller should drop and reopen the database.
pub const CRASH: &str = "simulated crash";

/// Whether `e` is a simulated crash from [`FaultyRemote::crash_after`].
pub fn is_crash(e: &Error) -> bool {
    e.kind() == ErrorKind::Internal && e.message() == CRASH
}

struct State {
    rng: u64,
    hidden: HashMap<String, u32>,
    crash_in: Option<u32>,
}

/// Wraps a store and injects faults from a seeded generator, so runs are reproducible.
pub struct FaultyRemote<R> {
    inner: R,
    faults: Faults,
    enabled: AtomicBool,
    state: Mutex<State>,
}

impl<R: std::fmt::Debug> std::fmt::Debug for FaultyRemote<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FaultyRemote")
            .field("inner", &self.inner)
            .field("faults", &self.faults)
            .finish_non_exhaustive()
    }
}

enum Gate {
    Pass,
    Fail(Error),
    /// Run the call, then report a crash.
    CrashAfter,
}

impl<R: RemoteStore> FaultyRemote<R> {
    /// Wraps `inner` with the given faults and seed.
    pub fn new(inner: R, faults: Faults, seed: u64) -> Self {
        Self {
            inner,
            faults,
            enabled: AtomicBool::new(true),
            state: Mutex::new(State {
                rng: seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1,
                hidden: HashMap::new(),
                crash_in: None,
            }),
        }
    }

    /// Turns fault injection on or off (off: a plain pass-through, all files visible).
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::SeqCst);
        if !on {
            let mut s = self.state.lock().expect("not poisoned");
            s.hidden.clear();
            s.crash_in = None;
        }
    }

    /// After `calls` more calls, the next call completes and then reports [`CRASH`], as if the
    /// process died right after it.
    pub fn crash_after(&self, calls: u32) {
        self.state.lock().expect("not poisoned").crash_in = Some(calls);
    }

    /// Cancels a pending [`FaultyRemote::crash_after`].
    pub fn disarm(&self) {
        self.state.lock().expect("not poisoned").crash_in = None;
    }

    /// The wrapped store.
    pub fn inner(&self) -> &R {
        &self.inner
    }

    fn chance(s: &mut State, p: f64) -> bool {
        // xorshift64*
        s.rng ^= s.rng >> 12;
        s.rng ^= s.rng << 25;
        s.rng ^= s.rng >> 27;
        let x = s.rng.wrapping_mul(0x2545_f491_4f6c_dd1d);
        p > 0.0 && ((x >> 11) as f64 / (1u64 << 53) as f64) < p
    }

    fn gate(&self) -> Gate {
        if !self.enabled.load(Ordering::SeqCst) {
            return Gate::Pass;
        }
        let mut s = self.state.lock().expect("not poisoned");
        if let Some(n) = s.crash_in {
            if n == 0 {
                s.crash_in = None;
                return Gate::CrashAfter;
            }
            s.crash_in = Some(n - 1);
        }
        if Self::chance(&mut s, self.faults.offline) {
            return Gate::Fail(Error::new(ErrorKind::Offline, "injected: offline"));
        }
        if Self::chance(&mut s, self.faults.rate_limited) {
            return Gate::Fail(Error::new(ErrorKind::RateLimited, "injected: rate limited"));
        }
        Gate::Pass
    }

    fn crash() -> Error {
        Error::new(ErrorKind::Internal, CRASH)
    }
}

#[async_trait]
impl<R: RemoteStore> RemoteStore for FaultyRemote<R> {
    async fn list(&self, prefix: &str) -> Result<Vec<RemoteEntry>> {
        let gate = self.gate();
        if let Gate::Fail(e) = gate {
            return Err(e);
        }
        let mut entries = self.inner.list(prefix).await?;
        {
            let mut s = self.state.lock().expect("not poisoned");
            entries.retain(|e| !s.hidden.contains_key(&e.key));
            s.hidden.retain(|_, n| {
                *n = n.saturating_sub(1);
                *n > 0
            });
        }
        match gate {
            Gate::CrashAfter => Err(Self::crash()),
            _ => Ok(entries),
        }
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let gate = self.gate();
        if let Gate::Fail(e) = gate {
            return Err(e);
        }
        let out = self.inner.get(key).await?;
        match gate {
            Gate::CrashAfter => Err(Self::crash()),
            _ => Ok(out),
        }
    }

    async fn reserve(&self, key: &str) -> Result<Option<String>> {
        self.inner.reserve(key).await
    }

    async fn put_new(
        &self,
        key: &str,
        data: &[u8],
        reservation: Option<&str>,
    ) -> Result<PutOutcome> {
        let gate = self.gate();
        if let Gate::Fail(e) = gate {
            return Err(e);
        }
        let out = self.inner.put_new(key, data, reservation).await?;
        let lost = {
            let mut s = self.state.lock().expect("not poisoned");
            if out == PutOutcome::Created && self.enabled.load(Ordering::SeqCst) {
                let lag = self.faults.list_lag;
                if lag > 0 {
                    let n = 1 + (s.rng % lag as u64) as u32;
                    s.hidden.insert(key.to_owned(), n);
                }
            }
            self.enabled.load(Ordering::SeqCst) && Self::chance(&mut s, self.faults.lost_response)
        };
        match gate {
            Gate::CrashAfter => Err(Self::crash()),
            _ if lost => Err(Error::new(ErrorKind::Offline, "injected: response lost")),
            _ => Ok(out),
        }
    }

    async fn delete(&self, key: &str) -> Result<()> {
        if let Gate::Fail(e) = self.gate() {
            return Err(e);
        }
        self.inner.delete(key).await
    }

    async fn changes_since(&self, cursor: Option<&str>) -> Result<Option<ChangeBatch>> {
        // The wrapped feed is not delayed; delayed visibility is modelled through `list`.
        if let Gate::Fail(e) = self.gate() {
            return Err(e);
        }
        self.inner.changes_since(cursor).await
    }
}
