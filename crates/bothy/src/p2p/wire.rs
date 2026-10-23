//! Messages on the `bothy/1` data channel, as canonical CBOR maps.
//!
//! Every message is a hint: dropping, repeating or reordering them never changes what a
//! database converges to, because the remote stays the durable record.

use std::collections::BTreeMap;

use crate::codec::cbor::{decode_value, encode_value};
use crate::error::{Error, Result};
use crate::model::{DbId, DeviceId, Value};

/// Version of the channel protocol.
pub(crate) const VERSION: i64 = 1;

/// Label of the data channel.
pub(crate) const LABEL: &str = "bothy/1";

/// Segments up to this size travel inline; larger ones are only announced.
pub(crate) const INLINE_LIMIT: usize = 64 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Message {
    /// First message on a new channel: which database and device this is.
    Hello {
        db_id: DbId,
        device_id: DeviceId,
        session: u64,
    },
    /// The sender uploaded its segment `seq`. `bytes` are exactly the remote file's bytes
    /// (encrypted in an encrypted database), or `None` if the receiver should fetch it.
    SegmentUp { seq: u64, bytes: Option<Vec<u8>> },
}

fn bad(msg: &str) -> Error {
    Error::corrupt(format!("p2p message: {msg}"))
}

impl Message {
    pub(crate) fn encode(&self) -> Vec<u8> {
        let mut m = BTreeMap::new();
        m.insert("v".to_owned(), Value::Int(VERSION));
        match self {
            Message::Hello {
                db_id,
                device_id,
                session,
            } => {
                m.insert("t".into(), Value::Text("hello".into()));
                m.insert("db".into(), Value::Bytes(db_id.as_bytes().to_vec()));
                m.insert("dev".into(), Value::Bytes(device_id.as_bytes().to_vec()));
                m.insert(
                    "session".into(),
                    Value::Bytes(session.to_be_bytes().to_vec()),
                );
            }
            Message::SegmentUp { seq, bytes } => {
                m.insert("t".into(), Value::Text("seg".into()));
                m.insert("seq".into(), Value::Int(*seq as i64));
                if let Some(b) = bytes {
                    m.insert("bytes".into(), Value::Bytes(b.clone()));
                }
            }
        }
        encode_value(&Value::Map(m)).expect("message values always encode")
    }

    /// Parses a message. A newer protocol version or an unknown type is `FormatTooNew`, so
    /// the receiver can ignore it.
    pub(crate) fn decode(data: &[u8]) -> Result<Self> {
        let Value::Map(m) = decode_value(data)? else {
            return Err(bad("not a map"));
        };
        match m.get("v") {
            Some(Value::Int(VERSION)) => {}
            Some(Value::Int(v)) if *v > VERSION => {
                return Err(Error::format_too_new(format!("p2p protocol version {v}")));
            }
            _ => return Err(bad("v must be 1")),
        }
        let bytes16 = |k: &str| -> Result<[u8; 16]> {
            match m.get(k) {
                Some(Value::Bytes(b)) => b
                    .as_slice()
                    .try_into()
                    .map_err(|_| bad(&format!("{k} must be 16 bytes"))),
                _ => Err(bad(&format!("missing {k}"))),
            }
        };
        match m.get("t") {
            Some(Value::Text(t)) if t == "hello" => {
                let session = match m.get("session") {
                    Some(Value::Bytes(b)) if b.len() == 8 => {
                        u64::from_be_bytes(b.as_slice().try_into().expect("8 bytes"))
                    }
                    _ => return Err(bad("session must be 8 bytes")),
                };
                Ok(Message::Hello {
                    db_id: DbId::from_bytes(bytes16("db")?),
                    device_id: DeviceId::from_bytes(bytes16("dev")?),
                    session,
                })
            }
            Some(Value::Text(t)) if t == "seg" => {
                let seq = match m.get("seq") {
                    Some(Value::Int(s)) if *s > 0 => *s as u64,
                    _ => return Err(bad("seq must be a positive integer")),
                };
                let bytes = match m.get("bytes") {
                    None => None,
                    Some(Value::Bytes(b)) => Some(b.clone()),
                    Some(_) => return Err(bad("bytes must be a byte string")),
                };
                Ok(Message::SegmentUp { seq, bytes })
            }
            Some(Value::Text(t)) => Err(Error::format_too_new(format!("p2p message type {t}"))),
            _ => Err(bad("missing t")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;

    #[test]
    fn round_trips() {
        for m in [
            Message::Hello {
                db_id: DbId::from_bytes([1; 16]),
                device_id: DeviceId::from_bytes([2; 16]),
                session: u64::MAX,
            },
            Message::SegmentUp {
                seq: 7,
                bytes: Some(vec![1, 2, 3]),
            },
            Message::SegmentUp {
                seq: 1,
                bytes: None,
            },
        ] {
            assert_eq!(Message::decode(&m.encode()).unwrap(), m);
        }
    }

    #[test]
    fn rejects_bad_and_newer_messages() {
        let enc = |m: BTreeMap<String, Value>| encode_value(&Value::Map(m)).unwrap();
        let mut m = BTreeMap::new();
        m.insert("v".to_owned(), Value::Int(2));
        m.insert("t".to_owned(), Value::Text("seg".into()));
        assert_eq!(
            Message::decode(&enc(m.clone())).unwrap_err().kind(),
            ErrorKind::FormatTooNew
        );
        m.insert("v".to_owned(), Value::Int(1));
        m.insert("t".to_owned(), Value::Text("ping".into()));
        assert_eq!(
            Message::decode(&enc(m.clone())).unwrap_err().kind(),
            ErrorKind::FormatTooNew
        );
        m.insert("t".to_owned(), Value::Text("seg".into()));
        m.insert("seq".to_owned(), Value::Int(0));
        assert_eq!(
            Message::decode(&enc(m)).unwrap_err().kind(),
            ErrorKind::Corrupt
        );
        assert_eq!(
            Message::decode(b"\xff").unwrap_err().kind(),
            ErrorKind::Corrupt
        );
    }
}
