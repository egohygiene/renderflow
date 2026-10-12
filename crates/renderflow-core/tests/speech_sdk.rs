//! Synthetic public-SDK refusals and side-effect boundaries for #428.
//! Authored only: these cases have not been compiled or executed. The declared
//! providers and model intentionally do not exist; no native process is needed.

use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use renderflow::checkpoint::RecoveryAction;
use renderflow::evidence::{RunState, StepState};
use renderflow::{
    CancellationToken, EngineBuilder, ExecutionRequest, ProgressEvent, ProgressReporter,
    RenderflowError,
};
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Default)]
struct Events(Mutex<Vec<ProgressEvent>>);

impl ProgressReporter for Events {
    fn on_event(&self, event: &ProgressEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

impl Events {
    fn has_speech_phase(&self) -> bool {
        self.0.lock().unwrap().iter().any(|event| {
            event.state.as_deref().is_some_and(|state| state.starts_with("speech."))
        })
    }
}

struct Fixture {
    directory: tempfile::TempDir,
    source: PathBuf,
    config: PathBuf,
    output: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("synthetic.wav");
        let config = directory.path().join("renderflow.yaml");
        let output = directory.path().join("dist");
        // One silent PCM16 sample at 16 kHz, mono. This is a structural fixture,
        // not a speech-recognition or model-quality qualification fixture.
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&38_u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16_u32.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&1_u16.to_le_bytes());
        wav.extend_from_slice(&16000_u32.to_le_bytes());
        wav.extend_from_slice(&32000_u32.to_le_bytes());
        wav.extend_from_slice(&2_u16.to_le_bytes());
        wav.extend_from_slice(&16_u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&2_u32.to_le_bytes());
        wav.extend_from_slice(&0_i16.to_le_bytes());
        fs::write(&source, &wav).unwrap();
        let spec = json!({
            "schema": "renderflow/v2",
            "sources": [{"id": "source.speech", "path": source, "format": "wav"}],
            "targets": {"exact": [{"id": "target.captions", "format": "vtt", "role": "captions", "requirement": "required"}]},
            "output": {"bundle_root": output, "naming_template": "{target.role}.{ext}", "collision": "error"},
            "execution": {
                "ai": "local_only",
                "speech": {
                    "source_sha256": format!("{:x}", Sha256::digest(&wav)),
                    "provider": {"path": directory.path().join("missing-whisper-cli"), "sha256": "a".repeat(64)},
                    "model": {"path": directory.path().join("missing-model.bin"), "sha256": "b".repeat(64), "id": "synthetic.model"}
                }
            }
        });
        fs::write(&config, serde_yaml_ng::to_string(&spec).unwrap()).unwrap();
        Self { directory, source, config, output }
    }

    fn state_directory(&self) -> PathBuf { self.directory.path().join(".renderflow") }
}

#[test]
fn dry_run_with_resume_needs_no_provider_files_and_publishes_no_bundle() {
    let fixture = Fixture::new();
    let events = Arc::new(Events::default());
    let engine = EngineBuilder::new().with_progress_reporter(events.clone()).build().unwrap();
    let result = engine.execute(
        ExecutionRequest::from_path(&fixture.config).dry_run(true).with_resume(true),
    ).unwrap();

    assert_eq!(result.run_manifest.state, RunState::Planned);
    assert!(result.run_manifest.steps.is_empty());
    assert!(result.manifest.artifacts.is_empty());
    assert!(!fixture.output.exists());
    assert!(!fixture.state_directory().exists());
    assert!(!events.has_speech_phase(), "dry-run must not enter native speech phases");
}

#[test]
fn pre_cancelled_sdk_request_returns_typed_cancellation_before_planning() {
    let fixture = Fixture::new();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    let engine = EngineBuilder::new().with_cancellation_token(cancellation).build().unwrap();
    let error = engine.execute(ExecutionRequest::from_path(&fixture.config)).unwrap_err();

    assert!(matches!(error, RenderflowError::Cancelled));
    assert!(!fixture.output.exists());
    assert!(!fixture.state_directory().exists());
}

#[test]
fn cancellation_after_resolve_records_cancelled_steps_without_entering_provider_phases() {
    let fixture = Fixture::new();
    let cancellation = CancellationToken::new();
    let events = Arc::new(Events::default());
    let engine = EngineBuilder::new()
        .with_cancellation_token(cancellation.clone())
        .with_progress_reporter(events.clone())
        .build().unwrap();
    let resolved = engine.resolve_execution(ExecutionRequest::from_path(&fixture.config)).unwrap();
    cancellation.cancel();
    let result = engine.execute_resolved(resolved, false).unwrap();

    assert_eq!(result.run_manifest.state, RunState::Cancelled);
    assert!(!result.run_manifest.steps.is_empty());
    assert!(result.run_manifest.steps.iter().all(|step| step.state == StepState::Cancelled));
    assert!(result.manifest.outputs.is_empty());
    assert!(result.manifest.artifacts.is_empty());
    assert!(!events.has_speech_phase());
    assert!(!fixture.output.join("captions.vtt").exists());
    assert!(!fixture.state_directory().exists());
}

#[test]
fn changed_original_source_is_refused_before_native_speech_execution() {
    let fixture = Fixture::new();
    let events = Arc::new(Events::default());
    let engine = EngineBuilder::new().with_progress_reporter(events.clone()).build().unwrap();
    let resolved = engine.resolve_execution(ExecutionRequest::from_path(&fixture.config)).unwrap();
    fs::write(&fixture.source, b"source changed after resolution").unwrap();
    let result = engine.execute_resolved(resolved, false).unwrap();

    assert_eq!(result.run_manifest.state, RunState::Failed);
    assert!(result.run_manifest.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == "execution.source_changed_after_intake"
    }));
    assert!(result.run_manifest.steps.is_empty());
    assert!(result.manifest.outputs.is_empty());
    assert!(!events.has_speech_phase());
    assert!(!fixture.output.join("captions.vtt").exists());
}

#[test]
fn resume_assessment_explicitly_requires_recomputation_without_opening_state_store() {
    let fixture = Fixture::new();
    let engine = EngineBuilder::new().build().unwrap();
    let assessment = engine.assess_resume(ExecutionRequest::from_path(&fixture.config)).unwrap();

    assert_eq!(assessment.overall.action, RecoveryAction::Recompute);
    assert_eq!(assessment.overall.reason_code, "speech.checkpoint_reuse_disabled");
    assert_eq!(assessment.decisions.len(), 1);
    assert!(assessment.overall.artifact_ids.is_empty());
    assert!(!fixture.output.exists());
    assert!(!fixture.state_directory().exists());
}
