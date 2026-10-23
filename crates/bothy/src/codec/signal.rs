//! Signaling files: how two online devices exchange session descriptions for the P2P channel
//! through the remote, with no server (`<db_id>/signaling/<device_id>/…`).
//!
//! A device that takes P2P connections writes `online-<hlc>.json` and rewrites it every
//! [`ONLINE_REFRESH_MS`]. For each online peer with a larger device id, it writes an offer to that
//! peer's session; the peer answers. The P2P channel only carries hints, so these files are never
//! needed for correctness, and readers that do not know them ignore them.
//!
//! Lifetimes are counted from the physical part of the HLC in the file name. In an encrypted
//! database the files are sealed like `device.json` (`aad` = the logical key).

use serde_json::{Map, Value as J};

use super::keys::{KeyKind, LogicalKey, SignalKind};
use crate::Hlc;
use crate::error::{Error, Result};
use crate::model::DeviceId;

/// Version of the signaling documents written and read by this build.
pub const VERSION: u64 = 1;

/// An `online` file is valid for this long after its HLC.
pub const ONLINE_TTL_MS: u64 = 15 * 60 * 1000;

/// A device rewrites its `online` file this often while it takes connections.
pub const ONLINE_REFRESH_MS: u64 = 10 * 60 * 1000;

/// An offer or answer is valid for this long after its HLC.
pub const DESCRIPTION_TTL_MS: u64 = 2 * 60 * 1000;

/// Any device may delete another device's signaling file this long after its HLC (an exception
/// to the single-writer rule, so files of devices that vanished do not pile up).
pub const STALE_AFTER_MS: u64 = 60 * 60 * 1000;

/// Longest accepted session description.
pub const MAX_SDP_BYTES: usize = 16 * 1024;

/// Contents of a signaling file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Signal {
    /// The device takes P2P connections in session `session`.
    Online {
        /// Random id the device picks each time it starts taking connections.
        session: u64,
    },
    /// An offer or an answer (the file name says which, and to whom).
    Description(Description),
}

/// A session description sent to one peer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description {
    /// The writer's session.
    pub session: u64,
    /// The peer's session this description is for.
    pub to_session: u64,
    /// SDP with all ICE candidates (no trickle).
    pub sdp: String,
}

/// Wall time (ms) at which a signaling file stops being valid.
pub fn expires_ms(kind: SignalKind, hlc: Hlc) -> u64 {
    let ttl = match kind {
        SignalKind::Online => ONLINE_TTL_MS,
        SignalKind::Offer(_) | SignalKind::Answer(_) => DESCRIPTION_TTL_MS,
    };
    hlc.physical_ms().saturating_add(ttl)
}

/// Whether any device may delete the file at wall time `now_ms`.
pub fn is_stale(hlc: Hlc, now_ms: u64) -> bool {
    now_ms > hlc.physical_ms().saturating_add(STALE_AFTER_MS)
}

fn bad(msg: &str) -> Error {
    Error::corrupt(format!("signaling file: {msg}"))
}

impl Signal {
    /// The JSON bytes to upload (before sealing) for the file at `key`.
    pub fn to_json(&self, key: &LogicalKey) -> Vec<u8> {
        let KeyKind::Signal(device, kind, _) = key.kind else {
            panic!("{key} is not a signaling key");
        };
        let mut m = Map::new();
        m.insert("v".into(), J::from(VERSION));
        m.insert("device_id".into(), J::String(device.to_hex()));
        match (self, kind) {
            (Signal::Online { session }, SignalKind::Online) => {
                m.insert("session".into(), J::String(hex(*session)));
            }
            (Signal::Description(d), SignalKind::Offer(to) | SignalKind::Answer(to)) => {
                m.insert("to".into(), J::String(to.to_hex()));
                m.insert("session".into(), J::String(hex(d.session)));
                m.insert("to_session".into(), J::String(hex(d.to_session)));
                m.insert("sdp".into(), J::String(d.sdp.clone()));
            }
            _ => panic!("{key} does not match the document"),
        }
        serde_json::to_vec(&J::Object(m)).expect("JSON values always serialize")
    }

    /// Parses a signaling file (after opening it, if sealed). The device ids must match `key`.
    /// A newer document version is `FormatTooNew`; callers skip such files.
    pub fn from_json(key: &LogicalKey, bytes: &[u8]) -> Result<Self> {
        let KeyKind::Signal(device, kind, _) = key.kind else {
            return Err(Error::invalid_input(format!(
                "{key} is not a signaling key"
            )));
        };
        let obj = match serde_json::from_slice::<J>(bytes) {
            Ok(J::Object(m)) => m,
            _ => return Err(bad("not a JSON object")),
        };
        match obj.get("v").and_then(J::as_u64) {
            Some(VERSION) => {}
            Some(v) if v > VERSION => {
                return Err(Error::format_too_new(format!("signaling file version {v}")));
            }
            _ => return Err(bad("v must be 1")),
        }
        let id = |name: &str| -> Result<DeviceId> {
            obj.get(name)
                .and_then(J::as_str)
                .and_then(|s| s.parse().ok())
                .ok_or_else(|| bad(&format!("{name} must be 32 lowercase hex characters")))
        };
        let session = |name: &str| -> Result<u64> {
            obj.get(name)
                .and_then(J::as_str)
                .and_then(parse_hex16)
                .ok_or_else(|| bad(&format!("{name} must be 16 lowercase hex characters")))
        };
        if id("device_id")? != device {
            return Err(bad("device_id does not match the key"));
        }
        match kind {
            SignalKind::Online => Ok(Signal::Online {
                session: session("session")?,
            }),
            SignalKind::Offer(to) | SignalKind::Answer(to) => {
                if id("to")? != to {
                    return Err(bad("to does not match the key"));
                }
                let sdp = obj
                    .get("sdp")
                    .and_then(J::as_str)
                    .filter(|s| !s.is_empty() && s.len() <= MAX_SDP_BYTES)
                    .ok_or_else(|| bad("sdp must be a non-empty string of at most 16 KiB"))?;
                Ok(Signal::Description(Description {
                    session: session("session")?,
                    to_session: session("to_session")?,
                    sdp: sdp.to_owned(),
                }))
            }
        }
    }
}

fn hex(v: u64) -> String {
    format!("{v:016x}")
}

fn parse_hex16(s: &str) -> Option<u64> {
    let ok = s.len() == 16 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    ok.then(|| u64::from_str_radix(s, 16).ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;
    use crate::model::DbId;

    const DB: &str = "000102030405060708090a0b0c0d0e0f";
    const A: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";
    const B: &str = "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2";
    const HLC: Hlc = Hlc::from_u64(0x018bcfe568000001);

    fn key(kind: SignalKind) -> LogicalKey {
        LogicalKey::signal(DB.parse::<DbId>().unwrap(), A.parse().unwrap(), kind, HLC)
    }

    fn offer() -> (LogicalKey, Signal) {
        let d = Description {
            session: 0x0102030405060708,
            to_session: 0x1112131415161718,
            sdp: "v=0\r\no=- 1 1 IN IP4 0.0.0.0\r\n".into(),
        };
        (
            key(SignalKind::Offer(B.parse().unwrap())),
            Signal::Description(d),
        )
    }

    #[test]
    fn round_trips() {
        let online = Signal::Online { session: 42 };
        let k = key(SignalKind::Online);
        assert_eq!(Signal::from_json(&k, &online.to_json(&k)).unwrap(), online);
        let (k, s) = offer();
        let json = s.to_json(&k);
        assert_eq!(Signal::from_json(&k, &json).unwrap(), s);
        let v: J = serde_json::from_slice(&json).unwrap();
        assert_eq!(v["to"], B);
        assert_eq!(v["session"], "0102030405060708");
    }

    #[test]
    fn rejects_mismatches_and_newer_versions() {
        let (k, s) = offer();
        let mut v: J = serde_json::from_slice(&s.to_json(&k)).unwrap();
        let check = |v: &J, kind: ErrorKind| {
            let err = Signal::from_json(&k, &serde_json::to_vec(v).unwrap()).unwrap_err();
            assert_eq!(err.kind(), kind, "{v}");
        };
        let good = v.clone();
        v["to"] = J::String(A.into());
        check(&v, ErrorKind::Corrupt);
        let mut v = good.clone();
        v["device_id"] = J::String(B.into());
        check(&v, ErrorKind::Corrupt);
        let mut v = good.clone();
        v["sdp"] = J::String(String::new());
        check(&v, ErrorKind::Corrupt);
        let mut v = good.clone();
        v["session"] = J::String("0102".into());
        check(&v, ErrorKind::Corrupt);
        let mut v = good.clone();
        v["v"] = J::from(2);
        check(&v, ErrorKind::FormatTooNew);
        assert_eq!(
            Signal::from_json(&key(SignalKind::Answer(B.parse().unwrap())), b"[]")
                .unwrap_err()
                .kind(),
            ErrorKind::Corrupt
        );
    }

    #[test]
    fn lifetimes_count_from_the_hlc() {
        let t = HLC.physical_ms();
        assert_eq!(expires_ms(SignalKind::Online, HLC), t + ONLINE_TTL_MS);
        assert_eq!(
            expires_ms(SignalKind::Offer(B.parse().unwrap()), HLC),
            t + DESCRIPTION_TTL_MS
        );
        assert!(!is_stale(HLC, t + STALE_AFTER_MS));
        assert!(is_stale(HLC, t + STALE_AFTER_MS + 1));
        const { assert!(ONLINE_REFRESH_MS < ONLINE_TTL_MS) };
    }
}
