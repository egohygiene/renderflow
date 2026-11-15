//! Explicit declarations only: parsing never probes tools or media.
use std::marker::PhantomData;
use std::path::Path;

use anyhow::{ensure, Result};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};

pub const PROFILE: &str = "pcm-delivery-ffmpeg-8.0.1-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tool {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioValidationPolicy {
    pub profile: String,
    pub ffmpeg: Tool,
    pub ffprobe: Tool,
}

fn object<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<T, D::Error> {
    struct Object<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for Object<T> {
        type Value = T;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("a native audio policy object")
        }
        fn visit_map<M: serde::de::MapAccess<'de>>(self, map: M) -> Result<T, M::Error> {
            T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(Object(PhantomData))
}
impl<'de> Deserialize<'de> for Tool {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            path: String,
            sha256: String,
        }
        let fields: Fields = object(deserializer)?;
        Ok(Self {
            path: fields.path,
            sha256: fields.sha256,
        })
    }
}
impl<'de> Deserialize<'de> for AudioValidationPolicy {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Fields {
            profile: String,
            ffmpeg: Tool,
            ffprobe: Tool,
        }
        let fields: Fields = object(deserializer)?;
        let policy = Self {
            profile: fields.profile,
            ffmpeg: fields.ffmpeg,
            ffprobe: fields.ffprobe,
        };
        policy.validate().map_err(serde::de::Error::custom)?;
        Ok(policy)
    }
}
impl AudioValidationPolicy {
    pub fn validate(&self) -> Result<()> {
        ensure!(self.profile == PROFILE, "audio.validation.policy.profile");
        for tool in [&self.ffmpeg, &self.ffprobe] {
            ensure!(
                Path::new(&tool.path).is_absolute()
                    && tool.path.len() <= 4096
                    && !tool.path.chars().any(char::is_control),
                "audio.validation.policy.path"
            );
            ensure!(
                tool.sha256.len() == 64
                    && tool
                        .sha256
                        .bytes()
                        .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)),
                "audio.validation.policy.sha256"
            );
        }
        Ok(())
    }
}
pub fn schema() -> Value {
    let tool = json!({"type":"object","additionalProperties":false,"required":["path","sha256"],"properties":{
        "path":{"type":"string","minLength":1,"maxLength":4096},"sha256":{"type":"string","pattern":"^[0-9a-f]{64}$"}}});
    json!({"type":"object","additionalProperties":false,"required":["profile","ffmpeg","ffprobe"],"properties":{
        "profile":{"const":PROFILE},"ffmpeg":tool,"ffprobe":tool}})
}
