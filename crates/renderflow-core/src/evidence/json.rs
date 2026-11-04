//! Shared duplicate-key refusal, moved unchanged from the speech decoder.

use std::fmt;
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};

/// Shared bounded JSON object decoding for adapters. The caller selects its
/// byte cap; duplicate keys are refused at every nested map before typed
/// deserialization of the original bytes. This is structural decoding only.
pub fn object<T:serde::de::DeserializeOwned>(bytes:&[u8],max_bytes:usize)->anyhow::Result<T> {
    anyhow::ensure!(max_bytes>0 && max_bytes<=128*1024*1024 && bytes.len()<=max_bytes,"JSON object byte bound exceeded");
    let value:StrictValue=serde_json::from_slice(bytes)?;
    anyhow::ensure!(value.0.is_object(),"JSON request must be an object");
    Ok(serde_json::from_slice(bytes)?)
}

/// serde_json::Value alone silently overwrites duplicate keys. Inspect every
/// map, including ignored extension metadata, while retaining JSON's ordinary
/// recursion limit and the caller's byte cap.
pub(crate) struct StrictValue(pub(crate) Value);

impl<'de> Deserialize<'de> for StrictValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct StrictVisitor;
        impl<'de> Visitor<'de> for StrictVisitor {
            type Value = StrictValue;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON value without duplicate object keys")
            }

            fn visit_bool<E: de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Bool(value)))
            }

            fn visit_i64<E: de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Number(value.into())))
            }

            fn visit_u64<E: de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Number(value.into())))
            }

            fn visit_f64<E: de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value).map(|number| StrictValue(Value::Number(number)))
                    .ok_or_else(|| E::custom("nonfinite JSON number"))
            }

            fn visit_str<E: de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::String(value.to_owned())))
            }

            fn visit_string<E: de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::String(value)))
            }

            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Null))
            }

            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictValue(Value::Null))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(StrictValue(value)) = sequence.next_element()? {
                    values.push(value);
                }
                Ok(StrictValue(Value::Array(values)))
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate JSON object key"));
                    }
                    let StrictValue(value) = map.next_value()?;
                    values.insert(key, value);
                }
                Ok(StrictValue(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(StrictVisitor)
    }
}

