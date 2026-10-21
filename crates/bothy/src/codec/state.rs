//! Document state entries, shared by the canonical state dump and snapshots:
//! `[collection, doc_id, deleted_at: Stamp | null, [[field, Stamp, value | undefined], ...]]`.

use std::collections::BTreeMap;

use crate::Hlc;
use crate::codec::cbor::{Decoder, Encoder};
use crate::error::{Error, Result};
use crate::merge::{DocState, FieldState};
use crate::model::{CollectionName, DeviceId, DocId, FieldName, Stamp};

/// Writes a stamp as `[hlc, device_id]`.
pub fn encode_stamp(e: &mut Encoder, s: &Stamp) {
    e.array(2);
    e.uint(s.hlc.as_u64());
    e.bytes(s.device.as_bytes());
}

/// Reads a stamp.
pub fn decode_stamp(d: &mut Decoder<'_>) -> Result<Stamp> {
    if d.array()? != 2 {
        return Err(Error::corrupt("stamp: expected a 2-element array"));
    }
    let hlc = Hlc::from_u64(d.uint()?);
    let device: [u8; 16] = d
        .bytes()?
        .try_into()
        .map_err(|_| Error::corrupt("stamp: device id must be 16 bytes"))?;
    Ok(Stamp::new(hlc, DeviceId::from_bytes(device)))
}

/// Writes one document entry. Fields are written in byte order of their names.
pub fn encode_entry(
    e: &mut Encoder,
    collection: &CollectionName,
    doc_id: &DocId,
    state: &DocState,
) -> Result<()> {
    e.array(4);
    e.text(collection.as_str());
    e.text(doc_id.as_str());
    match &state.deleted_at {
        Some(s) => encode_stamp(e, s),
        None => e.null(),
    }
    e.array(state.fields.len());
    for (name, f) in &state.fields {
        e.array(3);
        e.text(name.as_str());
        encode_stamp(e, &f.stamp);
        match &f.value {
            Some(v) => e.value(v)?,
            None => e.undefined(),
        }
    }
    Ok(())
}

/// Reads one document entry.
pub fn decode_entry(d: &mut Decoder<'_>) -> Result<(CollectionName, DocId, DocState)> {
    if d.array()? != 4 {
        return Err(Error::corrupt("entry: expected a 4-element array"));
    }
    let collection = CollectionName::new(d.text()?).map_err(as_corrupt)?;
    let doc_id = DocId::new(d.text()?).map_err(as_corrupt)?;
    let deleted_at = if d.null_if_present()? {
        None
    } else {
        Some(decode_stamp(d)?)
    };
    let n = d.array()?;
    let mut fields = BTreeMap::new();
    let mut prev: Option<&str> = None;
    for _ in 0..n {
        if d.array()? != 3 {
            return Err(Error::corrupt("entry: field must be a 3-element array"));
        }
        let name = d.text()?;
        if prev.is_some_and(|p| p >= name) {
            return Err(Error::corrupt("entry: fields are not sorted or repeat"));
        }
        prev = Some(name);
        let stamp = decode_stamp(d)?;
        let value = if d.undefined_if_present()? {
            None
        } else {
            Some(d.value()?)
        };
        fields.insert(
            FieldName::new(name).map_err(as_corrupt)?,
            FieldState { stamp, value },
        );
    }
    Ok((collection, doc_id, DocState { deleted_at, fields }))
}

fn as_corrupt(e: Error) -> Error {
    Error::corrupt(format!("entry: {}", e.message()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Value;

    fn stamp(h: u64, d: u8) -> Stamp {
        Stamp::new(Hlc::from_u64(h), DeviceId::from_bytes([d; 16]))
    }

    #[test]
    fn entry_round_trip_with_unset_and_tombstone() {
        let mut state = DocState {
            deleted_at: Some(stamp(5, 1)),
            ..DocState::default()
        };
        state.fields.insert(
            FieldName::new("a").unwrap(),
            FieldState {
                stamp: stamp(7, 2),
                value: Some(Value::Int(1)),
            },
        );
        state.fields.insert(
            FieldName::new("b").unwrap(),
            FieldState {
                stamp: stamp(8, 2),
                value: None,
            },
        );
        let coll = CollectionName::new("notes").unwrap();
        let id = DocId::new("n1").unwrap();
        let mut e = Encoder::new();
        encode_entry(&mut e, &coll, &id, &state).unwrap();
        let bytes = e.into_bytes();
        let mut d = Decoder::new(&bytes);
        let (c2, id2, s2) = decode_entry(&mut d).unwrap();
        d.finish().unwrap();
        assert_eq!((c2, id2, s2), (coll, id, state));
    }

    #[test]
    fn undefined_is_rejected_as_a_nested_value() {
        // [["x", [1, h'00..'], [undefined]]] inside an entry is not an unset marker.
        let mut e = Encoder::new();
        e.array(4);
        e.text("c");
        e.text("d");
        e.null();
        e.array(1);
        e.array(3);
        e.text("x");
        encode_stamp(&mut e, &stamp(1, 1));
        e.array(1);
        e.undefined();
        let bytes = e.into_bytes();
        assert!(decode_entry(&mut Decoder::new(&bytes)).is_err());
    }
}
