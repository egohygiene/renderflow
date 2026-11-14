//! Authored and unrun #430 portability cases. Synthetic receipts are declarations;
//! only the final case exercises the ordinary built-in body export when authorized.

#[path = "common/publication_fixture.rs"]
mod fixture;

use renderflow::evidence::portable::{self, ExportOptions, PortableIndex};
use renderflow::evidence::verification::{PinnedRunReceipt, VerificationLimits};
use renderflow::evidence::{ArtifactRole, RunState};
use renderflow::process::ProcessCancellationToken;
use renderflow::{EngineBuilder, ExecutionRequest, RunManifest};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};

const ORIGINAL: &[u8] = b"1\n00:00:01,000 --> 00:00:02,000\nhello\n\n";
const TEXT: &[u8] = b"hello\n";
fn token() -> ProcessCancellationToken {
    ProcessCancellationToken::new()
}
fn object(bytes: &[u8]) -> String {
    let digest = fixture::digest(bytes);
    format!("objects/sha256/{}/{digest}", &digest[..2])
}
fn put(root: &Path, bytes: &[u8]) -> String {
    let relative = object(bytes);
    let path = root.join(&relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
    relative
}
fn manifest(second: bool) -> RunManifest {
    let mut run = fixture::manifest(TEXT);
    if second {
        fixture::second_target(&mut run);
    }
    let mut source = run.artifact_manifest.artifacts[0].clone();
    source.artifact_id = "synthetic-original".into();
    source.role = "original".into();
    source.lifecycle = ArtifactRole::Source;
    source.locator = format!("artifact-store:{}", object(ORIGINAL));
    source.digest.value = fixture::digest(ORIGINAL);
    source.size_bytes = ORIGINAL.len() as u64;
    source.format = "srt".into();
    source.media_type = "application/x-subrip".into();
    source.sources.clear();
    source.metadata.clear();
    run.artifact_manifest.artifacts.push(source);
    run
}
fn pin(root: &Path, run: &RunManifest) -> PinnedRunReceipt {
    let bytes = serde_json::to_vec_pretty(run).unwrap();
    let path = root.join("selected-run.json");
    fs::write(&path, &bytes).unwrap();
    PinnedRunReceipt::read(
        &path,
        &fixture::digest(&bytes),
        &VerificationLimits::default(),
        token(),
    )
    .unwrap()
}
fn bundle(root: &Path) -> PathBuf {
    let path = root.join("original-bundle");
    fs::create_dir(&path).unwrap();
    fs::write(path.join("result.txt"), TEXT).unwrap();
    path
}
fn canonical() -> Vec<u8> {
    serde_json::to_vec(&json!({"schema_version":"renderflow.timed-transcript/v1",
        "source":{"sha256":fixture::digest(ORIGINAL),"format":"srt"},"overlap_policy":"preserve",
        "cues":[{"id":"1","start_ms":1000,"end_ms":2000,"text":"hello"}],"losses":[]}))
    .unwrap()
}
fn subtitle_reference(id: &str, digest: &str) -> serde_json::Value {
    json!({"schema_version":"renderflow.subtitle-artifact/v1","operation":"export",
        "input_artifact_id":id,"input_sha256":digest,"source":{"sha256":fixture::digest(ORIGINAL),"format":"srt"},
        "language":null,"overlap_policy":"preserve","canonical_transcript":{"artifact_id":id,"sha256":digest},
        "prior_losses":[],"export_losses":["timing","cue_ids","source_provenance"]})
}

#[test]
fn moved_bundle_keeps_canonical_evidence_without_original_bundle_or_store() {
    let root = tempfile::tempdir().unwrap();
    let source = bundle(root.path());
    let store = root.path().join("store");
    fs::create_dir(&store).unwrap();
    let canonical = canonical();
    let location = put(&store, &canonical);
    let mut run = manifest(false);
    let mut retained = run.artifact_manifest.artifacts[0].clone();
    retained.artifact_id = "synthetic-canonical".into();
    retained.lifecycle = ArtifactRole::Intermediate;
    retained.role = "canonical".into();
    retained.format = "timed_transcript_json".into();
    retained.media_type = "application/vnd.renderflow.timed-transcript+json".into();
    retained.locator = format!("artifact-store:{location}");
    retained.digest.value = fixture::digest(&canonical);
    retained.size_bytes = canonical.len() as u64;
    retained.metadata.clear();
    run.artifact_manifest.artifacts.push(retained);
    run.artifact_manifest.artifacts[0].sources = vec!["synthetic-canonical".into()];
    run.artifact_manifest.artifacts[0].metadata.insert(
        "renderflow.subtitle".into(),
        subtitle_reference("synthetic-canonical", &fixture::digest(&canonical)),
    );
    let receipt = pin(root.path(), &run);
    let output = root.path().join("portable");
    let result = portable::export(
        &receipt,
        &source,
        Some(&store),
        &output,
        &ExportOptions::default(),
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    let moved = root.path().join("moved");
    fs::rename(output, &moved).unwrap();
    fs::remove_dir_all(source).unwrap();
    fs::remove_dir_all(store).unwrap();
    let verified = portable::verify(
        &moved,
        result["index_sha256"].as_str().unwrap(),
        16384,
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    assert_eq!(verified.receipt().bytes(), receipt.bytes());
    assert_eq!(verified.receipt().manifest().state, RunState::Partial);
    assert_eq!(verified.publications().unwrap().len(), 1);
    assert_eq!(
        fs::read(moved.join(verified.index().records[2].path.as_ref().unwrap())).unwrap(),
        canonical
    );
    assert_eq!(
        verified.report().unwrap()["creative_approval_inferred"],
        false
    );
}

#[test]
fn required_canonical_source_is_included_even_when_other_originals_are_optional() {
    let root = tempfile::tempdir().unwrap();
    let source = bundle(root.path());
    let store = root.path().join("store");
    fs::create_dir(&store).unwrap();
    let canonical = canonical();
    let location = put(&store, &canonical);
    let mut run = manifest(false);
    let original = &mut run.artifact_manifest.artifacts[1];
    original.locator = format!("artifact-store:{location}");
    original.format = "timed_transcript_json".into();
    original.media_type = "application/vnd.renderflow.timed-transcript+json".into();
    original.digest.value = fixture::digest(&canonical);
    original.size_bytes = canonical.len() as u64;
    run.artifact_manifest.artifacts[0].metadata.insert(
        "renderflow.subtitle".into(),
        subtitle_reference("synthetic-original", &fixture::digest(&canonical)),
    );
    let receipt = pin(root.path(), &run);
    let output = root.path().join("portable");
    let result = portable::export(
        &receipt,
        &source,
        Some(&store),
        &output,
        &ExportOptions::default(),
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    let verified = portable::verify(
        &output,
        result["index_sha256"].as_str().unwrap(),
        16384,
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    assert!(!verified.index().include_sources);
    assert!(verified.index().records[1].path.is_some());
}

#[test]
fn remaining_sources_are_explicit_and_exported_copies_do_not_mutate_the_store() {
    let root = tempfile::tempdir().unwrap();
    let source = bundle(root.path());
    let store = root.path().join("store");
    fs::create_dir(&store).unwrap();
    let location = put(&store, ORIGINAL);
    let receipt = pin(root.path(), &manifest(false));
    let output = root.path().join("evidence");
    let result = portable::export(
        &receipt,
        &source,
        None,
        &output,
        &ExportOptions::default(),
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    let verified = portable::verify(
        &output,
        result["index_sha256"].as_str().unwrap(),
        16384,
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    assert!(verified.index().records[1].path.is_none());
    let full = root.path().join("full");
    let options = ExportOptions {
        include_sources: true,
        ..ExportOptions::default()
    };
    portable::export(
        &receipt,
        &source,
        Some(&store),
        &full,
        &options,
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    fs::write(full.join(location.clone()), b"changed exported copy").unwrap();
    assert_eq!(fs::read(store.join(location)).unwrap(), ORIGINAL);
}

#[test]
fn same_content_destinations_keep_distinct_native_views_with_one_object() {
    let root = tempfile::tempdir().unwrap();
    let source = bundle(root.path());
    fs::write(source.join("second.txt"), TEXT).unwrap();
    let receipt = pin(root.path(), &manifest(true));
    let output = root.path().join("portable");
    let result = portable::export(
        &receipt,
        &source,
        None,
        &output,
        &ExportOptions::default(),
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    assert_eq!(result["object_count"], 1);
    let verified = portable::verify(
        &output,
        result["index_sha256"].as_str().unwrap(),
        16384,
        &VerificationLimits::default(),
        token(),
    )
    .unwrap();
    let views = verified.publications().unwrap();
    assert_eq!(views.len(), 2);
    assert_ne!(
        views[0].native.artifact.locator,
        views[1].native.artifact.locator
    );
    assert_eq!(views[0].relative_payload, views[1].relative_payload);
}

#[test]
fn missing_canonical_inventory_or_store_never_creates_a_completed_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let source = bundle(root.path());
    let mut run = manifest(false);
    run.artifact_manifest.artifacts[0].metadata.insert(
        "renderflow.subtitle".into(),
        subtitle_reference("absent", &"a".repeat(64)),
    );
    let receipt = pin(root.path(), &run);
    let output = root.path().join("portable");
    assert!(portable::export(
        &receipt,
        &source,
        None,
        &output,
        &ExportOptions::default(),
        &VerificationLimits::default(),
        token()
    )
    .is_err());
    assert!(!output.exists());
    let mut run = manifest(false);
    let mut retained = run.artifact_manifest.artifacts[0].clone();
    retained.artifact_id = "synthetic-retained".into();
    retained.lifecycle = ArtifactRole::Intermediate;
    retained.locator = format!("artifact-store:{}", object(TEXT));
    retained.metadata.clear();
    run.artifact_manifest.artifacts.push(retained);
    let receipt = pin(root.path(), &run);
    assert!(portable::export(
        &receipt,
        &source,
        None,
        &output,
        &ExportOptions::default(),
        &VerificationLimits::default(),
        token()
    )
    .is_err());
    assert!(!output.exists());
}

#[test]
fn changed_receipt_payload_index_and_traversing_bindings_are_refused() {
    let root = tempfile::tempdir().unwrap();
    let source = bundle(root.path());
    let receipt = pin(root.path(), &manifest(false));
    let output = root.path().join("portable");
    let limits = VerificationLimits::default();
    let result = portable::export(
        &receipt,
        &source,
        None,
        &output,
        &ExportOptions::default(),
        &limits,
        token(),
    )
    .unwrap();
    let pin = result["index_sha256"].as_str().unwrap();
    fs::write(output.join("native/run.json"), b"tampered").unwrap();
    assert!(portable::verify(&output, pin, 16384, &limits, token()).is_err());
    fs::write(output.join("native/run.json"), receipt.bytes()).unwrap();
    fs::write(output.join(object(TEXT)), b"HELLO\n").unwrap();
    assert!(portable::verify(&output, pin, 16384, &limits, token()).is_err());
    fs::write(output.join(object(TEXT)), TEXT).unwrap();
    let mut index: PortableIndex =
        serde_json::from_slice(&fs::read(output.join("bundle.json")).unwrap()).unwrap();
    index.records[0].path = Some("../outside".into());
    let bytes = serde_json::to_vec(&index).unwrap();
    fs::write(output.join("bundle.json"), &bytes).unwrap();
    assert!(portable::verify(&output, &fixture::digest(&bytes), 16384, &limits, token()).is_err());
}

#[test]
fn cancellation_count_bounds_and_existing_destinations_leave_prior_content_intact() {
    let root = tempfile::tempdir().unwrap();
    let source = bundle(root.path());
    let receipt = pin(root.path(), &manifest(false));
    let output = root.path().join("portable");
    let cancelled = token();
    cancelled.cancel();
    assert!(portable::export(
        &receipt,
        &source,
        None,
        &output,
        &ExportOptions::default(),
        &VerificationLimits::default(),
        cancelled
    )
    .is_err());
    let options = ExportOptions {
        max_records: 1,
        ..ExportOptions::default()
    };
    assert!(portable::export(
        &receipt,
        &source,
        None,
        &output,
        &options,
        &VerificationLimits::default(),
        token()
    )
    .is_err());
    assert!(!output.exists());
    fs::create_dir(&output).unwrap();
    fs::write(output.join("keep"), b"prior").unwrap();
    assert!(portable::export(
        &receipt,
        &source,
        None,
        &output,
        &ExportOptions::default(),
        &VerificationLimits::default(),
        token()
    )
    .is_err());
    assert_eq!(fs::read(output.join("keep")).unwrap(), b"prior");
}

#[test]
fn native_only_fountain_sdk_export_keeps_its_unpublished_canonical_companion() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("original.fountain");
    fs::write(
        &input,
        include_bytes!("../../../examples/creative-forest/sources/original.fountain"),
    )
    .unwrap();
    let output = root.path().join("native");
    let config = json!({"schema":"renderflow/v2",
        "sources":[{"id":"script","path":input,"format":"fountain","detect":false}],
        "targets":{"exact":[{"id":"native-fdx","format":"fdx"}]},
        "execution":{"screenplay_body":{"allow_unqualified":true}},"output":{"bundle_root":output}});
    let config_path = root.path().join("renderflow.json");
    fs::write(&config_path, serde_json::to_vec(&config).unwrap()).unwrap();
    let result = EngineBuilder::new()
        .build()
        .unwrap()
        .execute(ExecutionRequest::from_path(config_path))
        .unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete);
    let path = Path::new(result.manifest_path.as_ref().unwrap());
    let bytes = fs::read(path).unwrap();
    let limits = VerificationLimits::default();
    let receipt = PinnedRunReceipt::read(path, &fixture::digest(&bytes), &limits, token()).unwrap();
    let portable = root.path().join("portable");
    let store = root.path().join(".renderflow/artifacts");
    let exported = portable::export(
        &receipt,
        &output,
        Some(&store),
        &portable,
        &ExportOptions::default(),
        &limits,
        token(),
    )
    .unwrap();
    fs::remove_dir_all(store).unwrap();
    fs::remove_dir_all(output).unwrap();
    fs::remove_file(input).unwrap();
    let verified = portable::verify(
        &portable,
        exported["index_sha256"].as_str().unwrap(),
        16384,
        &limits,
        token(),
    )
    .unwrap();
    assert_eq!(verified.publications().unwrap().len(), 1);
    assert!(verified
        .receipt()
        .manifest()
        .artifact_manifest
        .artifacts
        .iter()
        .enumerate()
        .any(|(index, record)| record.format == "screenplay_json"
            && verified.index().records[index].path.is_some()));
    assert_eq!(
        verified.publications().unwrap()[0].native.artifact.metadata["renderflow.screenplay_body"]
            ["review_required"],
        true
    );
}
