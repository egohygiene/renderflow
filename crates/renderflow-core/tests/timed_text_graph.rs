//! Synthetic SDK/graph integration coverage for #427.
//! These tests are authored but unrun. They use no native subtitle providers.

use std::fs;
use std::path::PathBuf;

use renderflow::evidence::{ArtifactEvidence, ArtifactRole, RunManifest, RunState, StepState};
use renderflow::{EngineBuilder, ExecutionRequest, PlanRequest};
use renderflow::timed_text::{self, TimedTextFormat, TimedTextLoss};
use serde_json::json;
use sha2::{Digest, Sha256};

const SRT: &[u8] = include_bytes!("../../../tests/fixtures/timed-text/plain-unicode.srt");
const WEBVTT: &[u8] = include_bytes!("../../../tests/fixtures/timed-text/identified.vtt");

struct Fixture {
    _directory: tempfile::TempDir,
    source: PathBuf,
    config: PathBuf,
    output: PathBuf,
}

impl Fixture {
    fn new(format: &str, bytes: &[u8], targets: &[(&str, &str)], policy: Option<serde_json::Value>) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let extension = if format == "timed_transcript_json" { "timed-transcript.json" } else { format };
        let source = directory.path().join(format!("source.{extension}"));
        let config = directory.path().join("renderflow.yaml");
        let output = directory.path().join("dist");
        fs::write(&source, bytes).unwrap();
        let mut spec = json!({
            "schema": "renderflow/v2",
            "sources": [{
                "id": "source.captions",
                "path": source,
                "format": format,
                "sha256": format!("{:x}", Sha256::digest(bytes))
            }],
            "targets": {"exact": targets.iter().map(|(format, role)| json!({
                "id": format!("target.{role}"),
                "format": format,
                "role": role,
                "requirement": "required"
            })).collect::<Vec<_>>()},
            "output": {
                "bundle_root": output,
                "naming_template": "{target.role}.{ext}",
                "collision": "error"
            }
        });
        if let Some(policy) = policy {
            spec["execution"] = json!({"subtitles": policy});
        }
        fs::write(&config, serde_yaml_ng::to_string(&spec).unwrap()).unwrap();
        Self { _directory: directory, source, config, output }
    }
}

fn produced<'a>(manifest: &'a RunManifest, format: &str) -> &'a ArtifactEvidence {
    let matches = manifest.artifact_manifest.artifacts.iter()
        .filter(|artifact| artifact.format == format && artifact.lifecycle != ArtifactRole::Source)
        .collect::<Vec<_>>();
    assert_eq!(matches.len(), 1, "expected one produced {format} artifact");
    matches[0]
}

#[test]
fn sdk_shares_canonical_transcript_and_preserves_native_and_flow_lineage() {
    let fixture = Fixture::new(
        "srt",
        SRT,
        &[("timed_transcript_json", "transcript"), ("vtt", "subtitles")],
        Some(json!({})),
    );
    let engine = EngineBuilder::new().build().unwrap();
    let plan = engine.plan(PlanRequest::from_path(&fixture.config)).unwrap();
    assert_eq!(plan.edges.len(), 2);
    assert!(plan.edges.iter().any(|edge| edge.from == "srt" && edge.to == "timed_transcript_json"));
    assert!(plan.edges.iter().any(|edge| edge.from == "timed_transcript_json" && edge.to == "vtt"));
    assert!(!fixture.output.exists(), "planning must not publish the bundle");

    let result = engine.execute(ExecutionRequest::from_path(&fixture.config)).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete, "{:?}", result.run_manifest.diagnostics);
    assert_eq!(result.run_manifest.steps.len(), 2);
    assert!(result.run_manifest.steps.iter().all(|step| step.state == StepState::Complete));
    assert_eq!(result.manifest.outputs.len(), 2);
    let source = result.manifest.artifacts.iter()
        .find(|artifact| artifact.lifecycle == ArtifactRole::Source).unwrap();
    let canonical = produced(&result.run_manifest, "timed_transcript_json");
    let subtitles = produced(&result.run_manifest, "vtt");
    assert_eq!(canonical.sources, vec![source.artifact_id.clone()]);
    assert_eq!(subtitles.sources, vec![canonical.artifact_id.clone()]);

    let json_bytes = fs::read(fixture.output.join("transcript.timed-transcript.json")).unwrap();
    let transcript = timed_text::parse(&json_bytes, TimedTextFormat::Json).unwrap();
    assert_eq!(transcript.source.sha256, source.digest.value);
    assert_eq!(transcript.cues[0].id, "7");
    assert_eq!(transcript.cues[0].start_ms, 1250);
    let vtt_bytes = fs::read(fixture.output.join("subtitles.vtt")).unwrap();
    assert_eq!(
        timed_text::parse(&vtt_bytes, TimedTextFormat::WebVtt).unwrap().cues,
        transcript.cues
    );
    assert_eq!(fs::read(&fixture.source).unwrap(), SRT);

    let metadata = &subtitles.metadata["renderflow.subtitle"];
    assert_eq!(metadata["schema_version"], "renderflow.subtitle-artifact/v1");
    assert_eq!(metadata["operation"], "export");
    assert_eq!(metadata["input_artifact_id"], canonical.artifact_id);
    assert_eq!(metadata["input_sha256"], canonical.digest.value);
    assert_eq!(metadata["source"]["sha256"], source.digest.value);
    assert_eq!(metadata["canonical_transcript"]["artifact_id"], canonical.artifact_id);
    assert_eq!(metadata["canonical_transcript"]["sha256"], canonical.digest.value);
    let losses = metadata["export_losses"].as_array().unwrap();
    assert!(losses.contains(&json!("source_provenance")));
    assert!(losses.contains(&json!("overlap_policy")));
    assert!(!losses.contains(&json!("timing")));
    assert!(!losses.contains(&json!("cue_ids")));
    assert_eq!(canonical.metadata["renderflow.subtitle"]["export_losses"], json!([]));

    let flow = result.run_manifest.flow_artifacts_v1();
    let flow_source = flow.iter().find(|artifact| artifact.digest == source.digest).unwrap();
    let flow_canonical = flow.iter().find(|artifact| artifact.digest == canonical.digest).unwrap();
    let flow_subtitles = flow.iter().find(|artifact| artifact.digest == subtitles.digest).unwrap();
    assert_eq!(flow_canonical.sources, vec![flow_source.artifact_id.clone()]);
    assert_eq!(flow_subtitles.sources, vec![flow_canonical.artifact_id.clone()]);
    assert!(flow.iter().all(|artifact| artifact.schema_version == "flow.artifact/v1"));
    let portable = serde_json::to_value(flow_subtitles).unwrap();
    assert!(portable.get("metadata").is_none(), "native metadata must not extend Flow v1");
    assert!(portable.get("export_losses").is_none());
}

#[test]
fn webvtt_to_srt_keeps_the_canonical_intermediate_and_discloses_identifier_loss() {
    let fixture = Fixture::new("vtt", WEBVTT, &[("srt", "subtitles")], Some(json!({})));
    let engine = EngineBuilder::new().build().unwrap();
    let result = engine.execute(ExecutionRequest::from_path(&fixture.config)).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete, "{:?}", result.run_manifest.diagnostics);
    let canonical = produced(&result.run_manifest, "timed_transcript_json");
    let subtitles = produced(&result.run_manifest, "srt");
    assert_eq!(canonical.lifecycle, ArtifactRole::Intermediate);
    assert_eq!(subtitles.sources, vec![canonical.artifact_id.clone()]);
    assert!(subtitles.metadata["renderflow.subtitle"]["export_losses"]
        .as_array().unwrap().contains(&json!("cue_ids")));
    let exported = timed_text::parse(
        &fs::read(fixture.output.join("subtitles.srt")).unwrap(),
        TimedTextFormat::Srt,
    ).unwrap();
    let original = timed_text::parse(WEBVTT, TimedTextFormat::WebVtt).unwrap();
    assert_eq!(exported.cues[0].id, "1");
    assert_eq!(original.cues[0].id, "opening");
    assert_eq!(exported.cues[0].text, original.cues[0].text);
    assert_eq!(exported.cues[0].start_ms, original.cues[0].start_ms);
    assert_eq!(exported.cues[0].end_ms, original.cues[0].end_ms);
    assert_eq!(fs::read(&fixture.source).unwrap(), WEBVTT);
}

#[test]
fn canonical_json_source_exports_without_confusing_claimed_origin_with_observed_input() {
    let mut transcript = timed_text::parse(WEBVTT, TimedTextFormat::WebVtt).unwrap();
    // The originating media is intentionally absent. Its supplied identity is
    // retained as a claim, while the graph observes the actual JSON input hash.
    transcript.source.sha256 = "a".repeat(64);
    transcript.language = Some("mul".to_owned());
    transcript.cues[0].speaker = Some("Synthetic narrator".to_owned());
    transcript.losses = vec![TimedTextLoss::Language];
    let canonical_bytes = timed_text::render(&transcript, TimedTextFormat::Json).unwrap();
    let fixture = Fixture::new(
        "timed_transcript_json",
        &canonical_bytes,
        &[("srt", "captions"), ("vtt", "subtitles")],
        Some(json!({})),
    );
    let engine = EngineBuilder::new().build().unwrap();
    let plan = engine.plan(PlanRequest::from_path(&fixture.config)).unwrap();
    assert_eq!(plan.source, "timed_transcript_json");
    assert_eq!(plan.edges.len(), 2);
    assert!(plan.edges.iter().all(|edge| edge.from == "timed_transcript_json"));
    let result = engine.execute(ExecutionRequest::from_path(&fixture.config)).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete, "{:?}", result.run_manifest.diagnostics);
    let source = result.manifest.artifacts.iter()
        .find(|artifact| artifact.lifecycle == ArtifactRole::Source).unwrap();
    assert_eq!(source.format, "timed_transcript_json");
    assert_eq!(source.digest.value, format!("{:x}", Sha256::digest(&canonical_bytes)));
    assert_ne!(source.digest.value, transcript.source.sha256);

    for format in ["srt", "vtt"] {
        let output = produced(&result.run_manifest, format);
        assert_eq!(output.sources, vec![source.artifact_id.clone()]);
        let evidence = &output.metadata["renderflow.subtitle"];
        assert_eq!(evidence["input_artifact_id"], source.artifact_id);
        assert_eq!(evidence["input_sha256"], source.digest.value);
        assert_eq!(evidence["canonical_transcript"]["artifact_id"], source.artifact_id);
        assert_eq!(evidence["canonical_transcript"]["sha256"], source.digest.value);
        assert_eq!(evidence["source"], serde_json::to_value(&transcript.source).unwrap());
        assert_eq!(evidence["language"], "mul");
        assert_eq!(evidence["prior_losses"], json!(["language"]));
        let losses = evidence["export_losses"].as_array().unwrap();
        for expected in ["speakers", "language", "source_provenance", "prior_loss_evidence"] {
            assert!(losses.contains(&json!(expected)), "missing {format} loss: {expected}");
        }
    }
    let retained_bytes = fs::read(&fixture.source).unwrap();
    assert_eq!(retained_bytes, canonical_bytes);
    assert_eq!(timed_text::parse(&retained_bytes, TimedTextFormat::Json).unwrap(), transcript);
    let exported = timed_text::parse(
        &fs::read(fixture.output.join("subtitles.vtt")).unwrap(),
        TimedTextFormat::WebVtt,
    ).unwrap();
    assert_eq!(exported.cues[0].id, transcript.cues[0].id);
    assert_eq!(exported.cues[0].text, transcript.cues[0].text);
    assert_eq!(exported.cues[0].start_ms, transcript.cues[0].start_ms);
    assert_eq!(exported.cues[0].end_ms, transcript.cues[0].end_ms);
    assert_eq!(exported.cues[0].speaker, None);
}

#[test]
fn subtitle_graph_execution_is_opt_in() {
    let fixture = Fixture::new("srt", SRT, &[("vtt", "subtitles")], None);
    let engine = EngineBuilder::new().build().unwrap();
    let error = engine.plan(PlanRequest::from_path(&fixture.config)).unwrap_err();
    assert!(format!("{:#}", anyhow::Error::new(error)).contains("no policy-allowed transformation path"));
    assert!(!fixture.output.exists());
    assert_eq!(fs::read(&fixture.source).unwrap(), SRT);
}

#[test]
fn standalone_subtitle_planning_refuses_a_container_stream_index() {
    let fixture = Fixture::new(
        "srt", SRT, &[("vtt", "subtitles")], Some(json!({"stream_index": 2})),
    );
    let engine = EngineBuilder::new().build().unwrap();
    let error = engine.plan(PlanRequest::from_path(&fixture.config)).unwrap_err();
    assert!(format!("{:#}", anyhow::Error::new(error)).contains("subtitle.stream_index.unexpected"));
    assert!(!fixture.output.exists());
    assert_eq!(fs::read(&fixture.source).unwrap(), SRT);
}

#[test]
fn media_planning_refuses_to_guess_a_subtitle_stream() {
    // An original minimal ISO-BMFF header is sufficient for intake recognition.
    // It is deliberately not playable media and must never reach FFprobe/FFmpeg.
    let header = b"\x00\x00\x00\x18ftypisom\x00\x00\x00\x00isomiso2";
    let fixture = Fixture::new(
        "mp4", header, &[("timed_transcript_json", "transcript")], Some(json!({})),
    );
    let engine = EngineBuilder::new().build().unwrap();
    let error = engine.plan(PlanRequest::from_path(&fixture.config)).unwrap_err();
    assert!(format!("{:#}", anyhow::Error::new(error)).contains("subtitle.stream_index.required"));
    assert!(!fixture.output.exists());
    assert_eq!(fs::read(&fixture.source).unwrap(), header);
}
