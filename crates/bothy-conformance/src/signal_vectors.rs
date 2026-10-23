//! Vectors for signaling files (`signaling.json`): key names, documents, and sealed documents
//! with a fixed data key and nonces.

use std::path::Path;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use bothy::codec::keys::{KeyKind, LogicalKey, SignalKind};
use bothy::codec::signal::{Description, Signal};
use bothy::crypto::{self, DataKey};
use bothy::{ErrorKind, Hlc};
use serde_json::{Value as J, json};

use crate::generate::{DB, DEV_A, DEV_B};
use crate::typed::{Res, hex, s, unhex};

const DEK: [u8; 32] = [0x42; 32];
const HLC: u64 = 0x018bcfe568000001;

fn cases() -> Vec<(LogicalKey, Signal)> {
    let hlc = Hlc::from_u64(HLC);
    let offer = Description {
        session: 0x0102030405060708,
        to_session: 0x1112131415161718,
        sdp: "v=0\r\no=- 1 1 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n".into(),
    };
    let answer = Description {
        session: 0x1112131415161718,
        to_session: 0x0102030405060708,
        sdp: "v=0\r\no=- 2 1 IN IP4 0.0.0.0\r\ns=-\r\nt=0 0\r\n".into(),
    };
    vec![
        (
            LogicalKey::signal(DB, DEV_B, SignalKind::Online, hlc),
            Signal::Online {
                session: 0x1112131415161718,
            },
        ),
        (
            LogicalKey::signal(DB, DEV_A, SignalKind::Offer(DEV_B), hlc),
            Signal::Description(offer),
        ),
        (
            LogicalKey::signal(DB, DEV_B, SignalKind::Answer(DEV_A), hlc),
            Signal::Description(answer),
        ),
    ]
}

fn kind_name(k: SignalKind) -> (&'static str, Option<String>) {
    match k {
        SignalKind::Online => ("online", None),
        SignalKind::Offer(p) => ("offer", Some(p.to_hex())),
        SignalKind::Answer(p) => ("answer", Some(p.to_hex())),
    }
}

fn sealed(key: &str, plain: &[u8], n: u8) -> J {
    let ct = crypto::encrypt_with_nonce(&DEK, &[n; 24], plain, key.as_bytes()).unwrap();
    json!({"sealed": "xchacha20poly1305", "nonce": B64.encode([n; 24]), "ct": B64.encode(ct)})
}

fn signaling_json() -> J {
    let mut valid = Vec::new();
    for (i, (key, doc)) in cases().into_iter().enumerate() {
        let KeyKind::Signal(device, kind, hlc) = key.kind else {
            unreachable!()
        };
        let (kind, peer) = kind_name(kind);
        let name = key.to_string();
        let plain = doc.to_json(&key);
        valid.push(json!({
            "key": name,
            "device_id": device.to_hex(),
            "kind": kind,
            "peer": peer,
            "hlc": hlc.to_hex(),
            "json": String::from_utf8(plain.clone()).unwrap(),
            "sealed": sealed(&name, &plain, 0x10 + i as u8),
        }));
    }
    let dev = DEV_A.to_hex();
    let peer = DEV_B.to_hex();
    let db = DB.to_hex();
    let invalid_keys = [
        format!("{db}/signaling/{dev}/offer.json"),
        format!("{db}/signaling/{dev}/online-{HLC:016x}.txt"),
        format!("{db}/signaling/{dev}/online-{:015x}.json", HLC),
        format!("{db}/signaling/{dev}/offer-{HLC:016x}.json"),
        format!("{db}/signaling/{dev}/offer-{peer}-{HLC:016x}-x.json"),
        format!("{db}/signaling/{dev}/hello-{HLC:016x}.json"),
        format!("{db}/signaling/{dev}/x/online-{HLC:016x}.json"),
    ];
    let offer_key = format!("{db}/signaling/{dev}/offer-{peer}-{HLC:016x}.json");
    let doc = |extra: J| {
        let mut base = json!({"v": 1, "device_id": dev, "to": peer, "session": "0102030405060708",
                              "to_session": "1112131415161718", "sdp": "v=0\r\n"});
        for (k, v) in extra.as_object().unwrap() {
            base[k] = v.clone();
        }
        serde_json::to_string(&base).unwrap()
    };
    json!({
        "about": "Signaling files for the P2P channel. A key parses to device, kind, peer and HLC; \
                  a document parses (after opening, if sealed with the data key and aad = its key) \
                  and re-encodes to the same JSON.",
        "dek": hex(&DEK),
        "valid": valid,
        "invalid_keys": invalid_keys,
        "invalid_documents": [
            {"key": offer_key, "json": doc(json!({"to": dev})), "error": "corrupt"},
            {"key": offer_key, "json": doc(json!({"device_id": peer})), "error": "corrupt"},
            {"key": offer_key, "json": doc(json!({"sdp": ""})), "error": "corrupt"},
            {"key": offer_key, "json": doc(json!({"session": "0102"})), "error": "corrupt"},
            {"key": offer_key, "json": doc(json!({"v": 2})), "error": "format_too_new"},
            {"key": offer_key, "json": "[]", "error": "corrupt"},
        ],
    })
}

/// Signaling vector files.
pub fn all() -> Vec<(String, J)> {
    vec![("signaling.json".into(), signaling_json())]
}

fn error_name(k: ErrorKind) -> &'static str {
    match k {
        ErrorKind::Corrupt => "corrupt",
        ErrorKind::FormatTooNew => "format_too_new",
        _ => "other",
    }
}

/// Checks `signaling.json` under `dir`.
pub fn check(dir: &Path) -> Res<usize> {
    let path = dir.join("signaling.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let j: J = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let dek = DataKey::from_bytes(
        unhex(s(&j, "dek")?)?
            .try_into()
            .map_err(|_| "dek must be 32 bytes")?,
    );
    let mut n = 0;
    for v in j["valid"].as_array().ok_or("valid must be an array")? {
        let name = s(v, "key")?;
        let key = LogicalKey::parse(name).ok_or_else(|| format!("{name}: does not parse"))?;
        let KeyKind::Signal(device, kind, hlc) = key.kind else {
            return Err(format!("{name}: not a signaling key"));
        };
        let (k, peer) = kind_name(kind);
        if device.to_hex() != s(v, "device_id")?
            || k != s(v, "kind")?
            || peer.as_deref() != v["peer"].as_str()
            || hlc.to_hex() != s(v, "hlc")?
        {
            return Err(format!("{name}: parts differ"));
        }
        if key.to_string() != name {
            return Err(format!("{name}: does not format back"));
        }
        let plain = s(v, "json")?.as_bytes();
        let doc = Signal::from_json(&key, plain).map_err(|e| format!("{name}: {e}"))?;
        if doc.to_json(&key) != plain {
            return Err(format!("{name}: does not re-encode to the same JSON"));
        }
        let opened = crypto::open_json(&dek, name, &serde_json::to_vec(&v["sealed"]).unwrap())
            .map_err(|e| format!("{name}: {e}"))?;
        if opened != plain {
            return Err(format!("{name}: sealed document opens to other bytes"));
        }
        n += 3;
    }
    for k in j["invalid_keys"]
        .as_array()
        .ok_or("invalid_keys must be an array")?
    {
        let k = k.as_str().ok_or("invalid keys are strings")?;
        if LogicalKey::parse(k).is_some() {
            return Err(format!("{k}: should not parse"));
        }
        n += 1;
    }
    for d in j["invalid_documents"]
        .as_array()
        .ok_or("invalid_documents must be an array")?
    {
        let key = LogicalKey::parse(s(d, "key")?).ok_or("invalid document key does not parse")?;
        let got = match Signal::from_json(&key, s(d, "json")?.as_bytes()) {
            Ok(_) => "ok",
            Err(e) => error_name(e.kind()),
        };
        if got != s(d, "error")? {
            return Err(format!(
                "{}: expected {}, got {got}",
                s(d, "json")?,
                s(d, "error")?
            ));
        }
        n += 1;
    }
    Ok(n)
}
