//! Conversion between [`Value`] and JSON.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::error::{Error, Result};
use crate::model::Value;

impl Value {
    /// Converts JSON to a value. Integers that fit `i64` become `Int`, other numbers `Float`.
    pub fn from_json(json: serde_json::Value) -> Result<Value> {
        let v = from_json(json)?;
        v.validate()?;
        Ok(v)
    }

    /// Converts the value to JSON. Bytes become base64 strings, so this is lossy for bytes.
    pub fn to_json(&self) -> serde_json::Value {
        use serde_json::Value as J;
        match self {
            Value::Null => J::Null,
            Value::Bool(b) => J::Bool(*b),
            Value::Int(i) => J::from(*i),
            Value::Float(f) => serde_json::Number::from_f64(*f).map_or(J::Null, J::Number),
            Value::Text(s) => J::String(s.clone()),
            Value::Bytes(b) => J::String(STANDARD.encode(b)),
            Value::Array(items) => J::Array(items.iter().map(Value::to_json).collect()),
            Value::Map(m) => J::Object(m.iter().map(|(k, v)| (k.clone(), v.to_json())).collect()),
        }
    }
}

fn from_json(json: serde_json::Value) -> Result<Value> {
    use serde_json::Value as J;
    Ok(match json {
        J::Null => Value::Null,
        J::Bool(b) => Value::Bool(b),
        J::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if n.is_u64() {
                return Err(Error::invalid_input("integer is larger than i64::MAX"));
            } else {
                Value::float(
                    n.as_f64()
                        .ok_or_else(|| Error::invalid_input("bad number"))?,
                )?
            }
        }
        J::String(s) => Value::Text(s),
        J::Array(items) => Value::Array(items.into_iter().map(from_json).collect::<Result<_>>()?),
        J::Object(m) => Value::Map(
            m.into_iter()
                .map(|(k, v)| Ok((k, from_json(v)?)))
                .collect::<Result<_>>()?,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_keep_int_and_float_apart() {
        assert_eq!(Value::from_json(json!(1)).unwrap(), Value::Int(1));
        assert_eq!(Value::from_json(json!(1.0)).unwrap(), Value::Float(1.0));
        assert!(Value::from_json(json!(u64::MAX)).is_err());
    }

    #[test]
    fn round_trip_without_bytes() {
        let j = json!({"title": "Hi", "n": -3, "x": 0.5, "tags": ["a", null, true], "o": {}});
        assert_eq!(Value::from_json(j.clone()).unwrap().to_json(), j);
    }

    #[test]
    fn bytes_render_as_base64() {
        assert_eq!(Value::Bytes(vec![0, 255]).to_json(), json!("AP8="));
    }
}
