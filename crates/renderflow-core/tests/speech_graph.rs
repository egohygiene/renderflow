//! Planning-only SDK coverage for #428, authored but intentionally unrun.
//! Fixtures are original synthetic bytes. No test executes a model/provider.

use std::fs;
use std::path::PathBuf;

use renderflow::speech::policy::SpeechPolicy;
use renderflow::{EngineBuilder, PlanRequest};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const MP4_HEADER: &[u8] = b"\x00\x00\x00\x18ftypisom\x00\x00\x00\x00isomiso2";

struct Fixture {
    directory: tempfile::TempDir,
    source: PathBuf,
    config: PathBuf,
    output: PathBuf,
    bytes: Vec<u8>,
    spec: Value,
}

impl Fixture {
    fn new(format: &str, bytes: &[u8], targets: &[&str]) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join(format!("source.{format}"));
        let config = directory.path().join("renderflow.yaml");
        let output = directory.path().join("dist");
        fs::write(&source, bytes).unwrap();
        let spec = json!({
            "schema": "renderflow/v2",
            "sources": [{
                "id": "source.speech",
                "path": source,
                "format": format,
                "sha256": format!("{:x}", Sha256::digest(bytes))
            }],
            "targets": {"exact": targets.iter().map(|format| json!({
                "id": format!("target.{format}"),
                "format": format,
                "role": format,
                "requirement": "required"
            })).collect::<Vec<_>>()},
            "output": {
                "bundle_root": output,
                "naming_template": "{target.role}.{ext}",
                "collision": "error"
            },
            "execution": {
                "ai": "local_only",
                "speech": {
                    "source_sha256": format!("{:x}", Sha256::digest(bytes)),
                    "provider": {
                        "path": directory.path().join("missing-whisper-cli"),
                        "sha256": "a".repeat(64)
                    },
                    "model": {
                        "path": directory.path().join("missing-model.bin"),
                        "sha256": "b".repeat(64),
                        "id": "synthetic-planning-model"
                    }
                }
            }
        });
        Self { directory, source, config, output, bytes: bytes.to_vec(), spec }
    }

    fn wav(targets: &[&str]) -> Self {
        Self::new("wav", &synthetic_wav(), targets)
    }

    fn with_audio_preparation(&mut self) {
        self.spec["execution"]["speech"]["audio"] = json!({
            "stream_index": 2,
            "ffprobe": {
                "path": self.directory.path().join("missing-ffprobe"),
                "sha256": "c".repeat(64)
            },
            "ffmpeg": {
                "path": self.directory.path().join("missing-ffmpeg"),
                "sha256": "d".repeat(64)
            }
        });
    }

    fn write_config(&self) {
        fs::write(&self.config, serde_yaml_ng::to_string(&self.spec).unwrap()).unwrap();
    }

    fn assert_untouched(&self) {
        assert!(!self.output.exists(), "planning must not publish a bundle");
        assert_eq!(fs::read(&self.source).unwrap(), self.bytes);
        for name in ["missing-whisper-cli", "missing-model.bin", "missing-ffprobe", "missing-ffmpeg"] {
            assert!(!self.directory.path().join(name).exists());
        }
    }

    fn assert_refused(&self) {
        self.write_config();
        let engine = EngineBuilder::new().build().unwrap();
        let error = engine.plan(PlanRequest::from_path(&self.config)).unwrap_err();
        assert!(!error.to_string().is_empty());
        self.assert_untouched();
    }
}

fn synthetic_wav() -> Vec<u8> {
    // One second of silence; enough structure for ordinary source intake.
    // Silence is not an accuracy fixture and is never sent to a recognizer.
    let data_bytes = 32_000_u32;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(&32_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_bytes.to_le_bytes());
    bytes.resize(44 + data_bytes as usize, 0);
    bytes
}

fn assert_invalid_policy(value: Value) {
    if let Ok(policy) = serde_json::from_value::<SpeechPolicy>(value) {
        assert!(policy.validate().is_err(), "malformed speech policy passed validation");
    }
}

#[test]
fn speech_fanout_shares_one_recognition_edge_without_touching_tools_or_models() {
    let fixture = Fixture::wav(&["timed_transcript_json", "srt", "vtt"]);
    fixture.write_config();
    let engine = EngineBuilder::new().build().unwrap();
    let plan = engine.plan(PlanRequest::from_path(&fixture.config)).unwrap();
    assert_eq!(plan.source, "wav");
    assert_eq!(plan.edges.len(), 3);
    let speech_edges = plan.edges.iter()
        .filter(|edge| edge.evidence.get("transform_id").map(String::as_str) == Some("speech.transcribe"))
        .collect::<Vec<_>>();
    assert_eq!(speech_edges.len(), 1, "one ASR run must feed every derivative");
    let speech = speech_edges[0];
    assert_eq!(speech.from, "wav");
    assert_eq!(speech.to, "timed_transcript_json");
    assert_eq!(speech.provider_id.as_deref(), Some("adapter.speech.whisper_cpp"));
    assert_eq!(speech.capability_id.as_deref(), Some("speech.transcribe"));
    assert!(speech.required_provider_ids.is_empty());
    for target in ["srt", "vtt"] {
        let edge = plan.edges.iter().find(|edge| edge.to == target).unwrap();
        assert_eq!(edge.from, "timed_transcript_json");
        assert_eq!(edge.evidence["transform_id"], format!("builtin.subtitle.timed_transcript_json.{target}"));
    }
    fixture.assert_untouched();
}

#[test]
fn explicit_ai_allow_can_plan_the_same_local_speech_route() {
    let mut fixture = Fixture::wav(&["timed_transcript_json"]);
    fixture.spec["execution"]["ai"] = json!("allow");
    fixture.write_config();
    let engine = EngineBuilder::new().build().unwrap();
    let plan = engine.plan(PlanRequest::from_path(&fixture.config)).unwrap();
    assert_eq!(plan.edges.len(), 1);
    assert_eq!(plan.edges[0].provider_id.as_deref(), Some("adapter.speech.whisper_cpp"));
    fixture.assert_untouched();
}

#[test]
fn speech_is_never_selected_without_explicit_speech_policy() {
    let mut fixture = Fixture::wav(&["timed_transcript_json"]);
    fixture.spec["execution"].as_object_mut().unwrap().remove("speech");
    fixture.assert_refused();
}

#[test]
fn default_and_explicit_ai_denial_block_speech() {
    for explicit in [false, true] {
        let mut fixture = Fixture::wav(&["timed_transcript_json"]);
        if explicit {
            fixture.spec["execution"]["ai"] = json!("deny");
        } else {
            fixture.spec["execution"].as_object_mut().unwrap().remove("ai");
        }
        fixture.assert_refused();
    }
}

#[test]
fn deterministic_requirement_refuses_unqualified_asr() {
    let mut fixture = Fixture::wav(&["timed_transcript_json"]);
    fixture.spec["execution"]["requirements"] = json!({"deterministic": true});
    fixture.assert_refused();
}

#[test]
fn speech_and_subtitle_export_transform_denials_are_respected() {
    for transform in ["speech.transcribe", "builtin.subtitle.timed_transcript_json.srt"] {
        let mut fixture = Fixture::wav(&["srt"]);
        fixture.spec["execution"]["transforms"] = json!({"deny": [transform]});
        fixture.assert_refused();
    }
}

#[test]
fn speech_provider_denial_blocks_the_route() {
    let mut fixture = Fixture::wav(&["timed_transcript_json"]);
    fixture.spec["execution"]["tools"] = json!({"deny": ["adapter.speech.whisper_cpp"]});
    fixture.assert_refused();
}

#[test]
fn container_speech_requires_explicit_audio_selection() {
    let fixture = Fixture::new("mp4", MP4_HEADER, &["timed_transcript_json"]);
    fixture.assert_refused();
}

#[test]
fn selected_container_audio_declares_both_preparation_dependencies() {
    let mut fixture = Fixture::new("mp4", MP4_HEADER, &["srt", "vtt"]);
    fixture.with_audio_preparation();
    fixture.write_config();
    let engine = EngineBuilder::new().build().unwrap();
    let plan = engine.plan(PlanRequest::from_path(&fixture.config)).unwrap();
    assert_eq!(plan.edges.len(), 3);
    let speech = plan.edges.iter().find(|edge| edge.from == "mp4").unwrap();
    assert_eq!(speech.to, "timed_transcript_json");
    assert_eq!(speech.provider_id.as_deref(), Some("adapter.speech.whisper_cpp"));
    assert_eq!(speech.required_provider_ids.len(), 2);
    assert!(speech.required_provider_ids.iter().any(|provider| provider == "tool.ffmpeg"));
    assert!(speech.required_provider_ids.iter().any(|provider| provider == "tool.ffprobe"));
    fixture.assert_untouched();
}

#[test]
fn either_preparation_provider_can_be_denied_independently() {
    for provider in ["tool.ffmpeg", "tool.ffprobe"] {
        let mut fixture = Fixture::new("mp4", MP4_HEADER, &["timed_transcript_json"]);
        fixture.with_audio_preparation();
        fixture.spec["execution"]["tools"] = json!({"deny": [provider]});
        fixture.assert_refused();
    }
}

#[test]
fn speech_and_embedded_subtitle_policies_cannot_be_combined() {
    let mut fixture = Fixture::new("mp4", MP4_HEADER, &["timed_transcript_json"]);
    fixture.with_audio_preparation();
    fixture.spec["execution"]["subtitles"] = json!({"stream_index": 3});
    fixture.assert_refused();
}

#[test]
fn speech_source_pin_must_match_the_actual_intake_digest() {
    let mut fixture = Fixture::wav(&["timed_transcript_json"]);
    fixture.spec["execution"]["speech"]["source_sha256"] = json!("e".repeat(64));
    fixture.assert_refused();
}

#[test]
fn speech_policy_does_not_enable_unbounded_text_or_media_targets() {
    for target in ["txt", "mp3"] {
        let fixture = Fixture::wav(&[target]);
        fixture.assert_refused();
    }
}

#[test]
fn malformed_or_relative_speech_identities_are_rejected_before_planning() {
    let fixture = Fixture::wav(&["timed_transcript_json"]);
    let policy = fixture.spec["execution"]["speech"].clone();
    for pointer in ["/source_sha256", "/provider/sha256", "/model/sha256"] {
        let mut malformed = policy.clone();
        *malformed.pointer_mut(pointer).unwrap() = json!("not-a-sha256");
        assert_invalid_policy(malformed);
    }
    for pointer in ["/provider/path", "/model/path"] {
        let mut relative = policy.clone();
        *relative.pointer_mut(pointer).unwrap() = json!("relative-file");
        assert_invalid_policy(relative);
    }
    let mut missing = policy.clone();
    missing.as_object_mut().unwrap().remove("source_sha256");
    assert!(serde_json::from_value::<SpeechPolicy>(missing).is_err());
    let mut unknown = policy;
    unknown["auto_download"] = json!(true);
    assert!(serde_json::from_value::<SpeechPolicy>(unknown).is_err());
}

#[test]
fn explicit_speech_options_and_audio_limits_are_bounded() {
    let mut fixture = Fixture::new("mp4", MP4_HEADER, &["timed_transcript_json"]);
    fixture.with_audio_preparation();
    let policy = fixture.spec["execution"]["speech"].clone();
    for options in [
        json!({"language": "en"}),
        json!({"language": "en", "threads": 0, "timeout_seconds": 900}),
        json!({"language": "en", "threads": 65, "timeout_seconds": 900}),
        json!({"language": "en", "threads": 4, "timeout_seconds": 0}),
        json!({"language": "en", "threads": 4, "timeout_seconds": 3601}),
        json!({"language": "English", "threads": 4, "timeout_seconds": 900}),
    ] {
        let mut malformed = policy.clone();
        malformed["options"] = options;
        assert_invalid_policy(malformed);
    }
    for stream in [json!(-1), json!(1.5), json!("2")] {
        let mut malformed = policy.clone();
        malformed["audio"]["stream_index"] = stream;
        assert!(serde_json::from_value::<SpeechPolicy>(malformed).is_err());
    }
    let mut zero_timeout = policy.clone();
    zero_timeout["audio"]["timeout_seconds"] = json!(0);
    assert_invalid_policy(zero_timeout);
    let mut relative = policy;
    relative["audio"]["ffmpeg"]["path"] = json!("ffmpeg");
    assert_invalid_policy(relative);
}

#[test]
fn omitted_options_have_explicit_serialized_defaults() {
    let mut fixture = Fixture::new("mp4", MP4_HEADER, &["timed_transcript_json"]);
    fixture.with_audio_preparation();
    let policy = serde_json::from_value::<SpeechPolicy>(fixture.spec["execution"]["speech"].clone()).unwrap();
    let serialized = serde_json::to_value(policy).unwrap();
    assert_eq!(serialized["options"], json!({"language": "en", "threads": 4, "timeout_seconds": 900}));
    assert_eq!(serialized["audio"]["timeout_seconds"], 300);
    assert_eq!(serialized["audio"]["stream_index"], 2);
    fixture.assert_untouched();
}

#[test]
fn speech_policy_requires_objects_and_rejects_duplicate_fields() {
    let fixture = Fixture::wav(&["timed_transcript_json"]);
    let policy = fixture.spec["execution"]["speech"].clone();
    assert!(serde_json::from_value::<SpeechPolicy>(json!([])).is_err());
    for field in ["provider", "model", "options", "audio"] {
        let mut malformed = policy.clone();
        malformed[field] = json!([]);
        assert!(serde_json::from_value::<SpeechPolicy>(malformed).is_err(), "accepted positional {field}");
    }
    let serialized = serde_json::to_string(&policy).unwrap();
    let duplicate = format!("{{\"source_sha256\":\"{}\",{}", "a".repeat(64), &serialized[1..]);
    assert!(serde_json::from_str::<SpeechPolicy>(&duplicate).is_err());
}
