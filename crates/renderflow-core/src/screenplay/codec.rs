use std::fmt;
use std::io::{self, Write};

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use super::{invalid, validate, Screenplay, ScreenplayError, MAX_INPUT_BYTES};

/// A JSON value visitor that refuses duplicate keys at every depth, including
/// opaque extensions. Parsing directly into Value would silently keep one.
struct UniqueValue(Value);

impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON value without duplicate object keys")
            }
            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> { Ok(UniqueValue(Value::Bool(value))) }
            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> { Ok(UniqueValue(Value::Number(value.into()))) }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> { Ok(UniqueValue(Value::Number(value.into()))) }
            fn visit_f64<E: serde::de::Error>(self, _value: f64) -> Result<Self::Value, E> {
                Err(E::custom("JSON numbers must be lexical integers fitting i64/u64; retain other numeric tokens as strings"))
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> { Ok(UniqueValue(Value::String(value.to_owned()))) }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> { Ok(UniqueValue(Value::String(value))) }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> { Ok(UniqueValue(Value::Null)) }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> { Ok(UniqueValue(Value::Null)) }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(UniqueValue(value)) = sequence.next_element()? { values.push(value); }
                Ok(UniqueValue(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut object: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some(key) = object.next_key::<String>()? {
                    if values.contains_key(&key) { return Err(serde::de::Error::custom(format!("duplicate JSON key: {key}"))); }
                    let UniqueValue(value) = object.next_value()?;
                    values.insert(key, value);
                }
                Ok(UniqueValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

struct BoundedWriter {
    bytes: Option<Vec<u8>>,
    count: usize,
    limit: usize,
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.count.checked_add(bytes.len()).ok_or_else(|| io::Error::other("encoded byte limit exceeded"))?;
        if count > self.limit { return Err(io::Error::other("encoded byte limit exceeded")); }
        if let Some(output) = &mut self.bytes { output.extend_from_slice(bytes); }
        self.count = count;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

pub(super) fn bounded_encoded_len<T: Serialize>(value: &T, limit: usize) -> Result<usize, serde_json::Error> {
    let mut writer = BoundedWriter { bytes: None, count: 0, limit };
    serde_json::to_writer(&mut writer, value)?;
    Ok(writer.count)
}

fn require_object_shape(value: &Value) -> Result<(), ScreenplayError> {
    let root = value.as_object().ok_or_else(|| invalid("canonical screenplay JSON must be an object"))?;
    if !root.get("source").is_some_and(Value::is_object) {
        return Err(invalid("source must be an object"));
    }
    for field in ["title_page", "elements", "dual_dialogue", "losses"] {
        let values = root.get(field).and_then(Value::as_array).ok_or_else(|| invalid(format!("{field} must be an array")))?;
        if values.iter().any(|value| !value.is_object()) {
            return Err(invalid(format!("every {field} entry must be an object")));
        }
    }
    Ok(())
}

/// Strict bounded canonical JSON only. No native import or source probing.
pub fn parse_json(bytes: &[u8]) -> Result<Screenplay, ScreenplayError> {
    if bytes.len() > MAX_INPUT_BYTES { return Err(ScreenplayError::InputTooLarge); }
    let input = std::str::from_utf8(bytes)?;
    let UniqueValue(value) = serde_json::from_str(input)?;
    require_object_shape(&value)?;
    let screenplay: Screenplay = serde_json::from_value(value)?;
    validate(&screenplay)?;
    Ok(screenplay)
}

/// Deterministic pretty JSON with LF and preserved semantic array order.
pub fn render_json(screenplay: &Screenplay) -> Result<Vec<u8>, ScreenplayError> {
    validate(screenplay)?;
    let mut writer = BoundedWriter { bytes: Some(Vec::new()), count: 0, limit: MAX_INPUT_BYTES };
    serde_json::to_writer_pretty(&mut writer, screenplay).map_err(|_| ScreenplayError::OutputTooLarge)?;
    writer.write_all(b"\n").map_err(|_| ScreenplayError::OutputTooLarge)?;
    Ok(writer.bytes.unwrap_or_default())
}
