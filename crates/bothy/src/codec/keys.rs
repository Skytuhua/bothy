//! Logical keys: backend-independent names of remote files.
//!
//! ```text
//! <db_id>/manifest.json
//! <db_id>/devices/<device_id>/device.json
//! <db_id>/devices/<device_id>/seg-<seq:016x>.bseg
//! <db_id>/snapshots/<device_id>/snap-<hlc:016x>.bsnap
//! <db_id>/keys/<device_id>/key-<hlc:016x>.json
//! <db_id>/signaling/<device_id>/online-<hlc:016x>.json
//! <db_id>/signaling/<device_id>/offer-<peer_id>-<hlc:016x>.json
//! <db_id>/signaling/<device_id>/answer-<peer_id>-<hlc:016x>.json
//! ```

use std::fmt;

use crate::Hlc;
use crate::model::{DbId, DeviceId};

/// What a logical key points to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum KeyKind {
    /// `manifest.json`.
    Manifest,
    /// A device's `device.json`.
    Device(DeviceId),
    /// A segment.
    Segment(DeviceId, u64),
    /// A snapshot.
    Snapshot(DeviceId, Hlc),
    /// A key file of an encrypted database.
    KeyFile(DeviceId, Hlc),
    /// A signaling file for the P2P channel (see [`crate::codec::signal`]).
    Signal(DeviceId, SignalKind, Hlc),
}

/// Kind of a signaling file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignalKind {
    /// `online-<hlc>.json`: the device takes P2P connections.
    Online,
    /// `offer-<peer>-<hlc>.json`: a session description offered to `peer`.
    Offer(DeviceId),
    /// `answer-<peer>-<hlc>.json`: a session description answering `peer`'s offer.
    Answer(DeviceId),
}

/// A parsed logical key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LogicalKey {
    /// Database the file belongs to.
    pub db_id: DbId,
    /// The file.
    pub kind: KeyKind,
}

impl LogicalKey {
    /// Key of `manifest.json`.
    pub const fn manifest(db_id: DbId) -> Self {
        Self {
            db_id,
            kind: KeyKind::Manifest,
        }
    }

    /// Key of a device's `device.json`.
    pub const fn device(db_id: DbId, device: DeviceId) -> Self {
        Self {
            db_id,
            kind: KeyKind::Device(device),
        }
    }

    /// Key of a segment.
    pub const fn segment(db_id: DbId, device: DeviceId, seq: u64) -> Self {
        Self {
            db_id,
            kind: KeyKind::Segment(device, seq),
        }
    }

    /// Key of a snapshot.
    pub const fn snapshot(db_id: DbId, device: DeviceId, hlc: Hlc) -> Self {
        Self {
            db_id,
            kind: KeyKind::Snapshot(device, hlc),
        }
    }

    /// Key of a key file.
    pub const fn key_file(db_id: DbId, device: DeviceId, hlc: Hlc) -> Self {
        Self {
            db_id,
            kind: KeyKind::KeyFile(device, hlc),
        }
    }

    /// Key of a signaling file.
    pub const fn signal(db_id: DbId, device: DeviceId, kind: SignalKind, hlc: Hlc) -> Self {
        Self {
            db_id,
            kind: KeyKind::Signal(device, kind, hlc),
        }
    }

    /// The device that owns the file, if any (the manifest has none).
    pub fn owner(&self) -> Option<DeviceId> {
        match self.kind {
            KeyKind::Manifest => None,
            KeyKind::Device(d)
            | KeyKind::Segment(d, _)
            | KeyKind::Snapshot(d, _)
            | KeyKind::KeyFile(d, _)
            | KeyKind::Signal(d, _, _) => Some(d),
        }
    }

    /// Parses a key. Returns `None` for anything that does not match a known pattern exactly;
    /// readers ignore such files.
    pub fn parse(s: &str) -> Option<Self> {
        let mut parts = s.split('/');
        let db_id: DbId = parts.next()?.parse().ok()?;
        let kind = match (parts.next()?, parts.next(), parts.next(), parts.next()) {
            ("manifest.json", None, None, None) => KeyKind::Manifest,
            ("devices", Some(dev), Some(file), None) => {
                let dev: DeviceId = dev.parse().ok()?;
                if file == "device.json" {
                    KeyKind::Device(dev)
                } else {
                    let seq = hex16(file.strip_prefix("seg-")?.strip_suffix(".bseg")?)?;
                    if seq == 0 {
                        return None;
                    }
                    KeyKind::Segment(dev, seq)
                }
            }
            ("snapshots", Some(dev), Some(file), None) => {
                let dev: DeviceId = dev.parse().ok()?;
                let hlc = hex16(file.strip_prefix("snap-")?.strip_suffix(".bsnap")?)?;
                KeyKind::Snapshot(dev, Hlc::from_u64(hlc))
            }
            ("keys", Some(dev), Some(file), None) => {
                let dev: DeviceId = dev.parse().ok()?;
                let hlc = hex16(file.strip_prefix("key-")?.strip_suffix(".json")?)?;
                KeyKind::KeyFile(dev, Hlc::from_u64(hlc))
            }
            ("signaling", Some(dev), Some(file), None) => {
                let dev: DeviceId = dev.parse().ok()?;
                let stem = file.strip_suffix(".json")?;
                let (kind, hlc) = if let Some(h) = stem.strip_prefix("online-") {
                    (SignalKind::Online, h)
                } else if let Some(rest) = stem.strip_prefix("offer-") {
                    let (peer, h) = peer_and_hlc(rest)?;
                    (SignalKind::Offer(peer), h)
                } else {
                    let (peer, h) = peer_and_hlc(stem.strip_prefix("answer-")?)?;
                    (SignalKind::Answer(peer), h)
                };
                KeyKind::Signal(dev, kind, Hlc::from_u64(hex16(hlc)?))
            }
            _ => return None,
        };
        Some(Self { db_id, kind })
    }
}

/// `<peer_id>-<hlc>` of an offer or answer file name.
fn peer_and_hlc(s: &str) -> Option<(DeviceId, &str)> {
    let (peer, hlc) = s.split_once('-')?;
    Some((peer.parse().ok()?, hlc))
}

fn hex16(s: &str) -> Option<u64> {
    let ok = s.len() == 16 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    ok.then(|| u64::from_str_radix(s, 16).ok()).flatten()
}

impl fmt::Display for LogicalKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = self.db_id;
        match self.kind {
            KeyKind::Manifest => write!(f, "{db}/manifest.json"),
            KeyKind::Device(d) => write!(f, "{db}/devices/{d}/device.json"),
            KeyKind::Segment(d, seq) => write!(f, "{db}/devices/{d}/seg-{seq:016x}.bseg"),
            KeyKind::Snapshot(d, hlc) => write!(f, "{db}/snapshots/{d}/snap-{hlc}.bsnap"),
            KeyKind::KeyFile(d, hlc) => write!(f, "{db}/keys/{d}/key-{hlc}.json"),
            KeyKind::Signal(d, SignalKind::Online, hlc) => {
                write!(f, "{db}/signaling/{d}/online-{hlc}.json")
            }
            KeyKind::Signal(d, SignalKind::Offer(p), hlc) => {
                write!(f, "{db}/signaling/{d}/offer-{p}-{hlc}.json")
            }
            KeyKind::Signal(d, SignalKind::Answer(p), hlc) => {
                write!(f, "{db}/signaling/{d}/answer-{p}-{hlc}.json")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DB: &str = "000102030405060708090a0b0c0d0e0f";
    const DEV: &str = "f0f1f2f3f4f5f6f7f8f9fafbfcfdfeff";
    const PEER: &str = "a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1";

    #[test]
    fn round_trip_all_kinds() {
        for s in [
            format!("{DB}/manifest.json"),
            format!("{DB}/devices/{DEV}/device.json"),
            format!("{DB}/devices/{DEV}/seg-000000000000002a.bseg"),
            format!("{DB}/snapshots/{DEV}/snap-018bcfe568000001.bsnap"),
            format!("{DB}/keys/{DEV}/key-018bcfe568000001.json"),
            format!("{DB}/signaling/{DEV}/online-018bcfe568000001.json"),
            format!("{DB}/signaling/{DEV}/offer-{PEER}-018bcfe568000001.json"),
            format!("{DB}/signaling/{DEV}/answer-{PEER}-018bcfe568000001.json"),
        ] {
            let k = LogicalKey::parse(&s).unwrap_or_else(|| panic!("{s}"));
            assert_eq!(k.to_string(), s);
        }
        let k =
            LogicalKey::parse(&format!("{DB}/devices/{DEV}/seg-000000000000002a.bseg")).unwrap();
        assert_eq!(k.kind, KeyKind::Segment(DEV.parse().unwrap(), 42));
        let k = LogicalKey::parse(&format!(
            "{DB}/signaling/{DEV}/offer-{PEER}-018bcfe568000001.json"
        ))
        .unwrap();
        assert_eq!(
            k.kind,
            KeyKind::Signal(
                DEV.parse().unwrap(),
                SignalKind::Offer(PEER.parse().unwrap()),
                Hlc::from_u64(0x018bcfe568000001)
            )
        );
        assert_eq!(k.owner(), Some(DEV.parse().unwrap()));
    }

    #[test]
    fn unknown_or_sloppy_keys_are_ignored() {
        for s in [
            String::new(),
            DB.to_string(),
            format!("{DB}/manifest.json/x"),
            format!("{DB}/devices/{DEV}/seg-2a.bseg"),
            format!("{DB}/devices/{DEV}/seg-000000000000002A.bseg"),
            format!("{DB}/devices/{DEV}/seg-0000000000000000.bseg"),
            format!("{DB}/devices/{DEV}/notes.txt"),
            format!("{DB}/signaling/{DEV}/offer.json"),
            format!("{DB}/signaling/{DEV}/online-018bcfe568000001.txt"),
            format!("{DB}/signaling/{DEV}/online-18bcfe568000001.json"),
            format!("{DB}/signaling/{DEV}/offer-018bcfe568000001.json"),
            format!("{DB}/signaling/{DEV}/offer-{PEER}-018bcfe568000001-x.json"),
            format!("{DB}/signaling/{DEV}/hello-018bcfe568000001.json"),
            format!("{DB}/signaling/{DEV}/x/online-018bcfe568000001.json"),
            format!("{}/manifest.json", DB.to_uppercase()),
        ] {
            assert_eq!(LogicalKey::parse(&s), None, "{s}");
        }
    }
}
