//! Authored, UNRUN integration regressions for ordinary composition builds (#406).
//!
//! These exercise the shared CLI/SDK boundary with repository-owned fixtures.
//! They have not been compiled or executed and do not qualify rendering fidelity,
//! native tooling, platform behavior, release readiness, or Flow acceptance.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use clap::Parser;
use renderflow::app::run_cli;
use renderflow::cli::Cli;
use renderflow::composition::{CompositionBranchOutcome, CompositionRun, PdfSettings};
use renderflow::evidence::{sha256_serialized, ArtifactRole, RunState};
use renderflow::graph::execution_plan::ForestTargetOutcome;
use renderflow::graph::ForestBranchState;
use renderflow::optimization::OptimizationMode;
use renderflow::planning::build::{execute, resolve};
use renderflow::planning::PlanningRequest;
use renderflow::{CancellationToken, EngineBuilder, ExecutionRequest, InspectionRequest, PlanRequest,
    RenderflowError, RunManifest};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    package: PathBuf,
    config: PathBuf,
    output: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let package = root.join("package");
        copy_tree(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/page-composition/raster-composition"),
            &package,
        );
        let fixture = Self {
            config: root.join("renderflow.yaml"),
            output: root.join("bundle"),
            package,
            root,
            _directory: directory,
        };
        fixture.write_spec(&json!({
            "schema": "renderflow/v2",
            "sources": [{"id": "page", "kind": "composition", "path": "package/page.composition.json"}],
            "targets": {"exact": [{"format": "transcript_json"}]},
            "output": {"bundle_root": "bundle"}
        }));
        fixture
    }

    fn spec(&self) -> Value {
        serde_yaml_ng::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap()
    }

    fn write_spec(&self, spec: &Value) {
        fs::write(&self.config, serde_yaml_ng::to_string(spec).unwrap()).unwrap();
    }

    fn request(&self) -> PlanningRequest { PlanningRequest::from_path(&self.config) }

    fn read_json<T: serde::de::DeserializeOwned>(&self, filename: &str) -> T {
        serde_json::from_slice(&fs::read(self.output.join(filename)).unwrap()).unwrap()
    }
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

// Include directories so an empty cache/state directory is also a side effect.
fn snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn collect(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            let key = path.strip_prefix(root).unwrap().to_path_buf();
            if entry.file_type().unwrap().is_dir() {
                files.insert(key, None);
                collect(root, &path, files);
            } else {
                files.insert(key, Some(fs::read(path).unwrap()));
            }
        }
    }
    let mut files = BTreeMap::new();
    collect(root, root, &mut files);
    files
}

fn digest(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

#[test]
fn omitted_composition_policy_selects_declared_locales_and_config_relative_paths() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.root);
    let cwd = std::env::current_dir().unwrap();
    let resolved = resolve(fixture.request()).unwrap();
    let forest = resolved.plan().artifact_forest.as_ref().unwrap();

    assert!(resolved.is_composition());
    assert_eq!(resolved.source_path(), fixture.package.join("page.composition.json"));
    assert_eq!(Path::new(&resolved.spec().output.bundle_root), fixture.output);
    assert_eq!(forest.branches.len(), 2);
    assert!(forest.branches.iter().all(|branch| branch.state == ForestBranchState::Selected));
    assert_eq!(forest.branches.iter().map(|branch| branch.options["locale"].as_str().unwrap())
        .collect::<BTreeSet<_>>(), BTreeSet::from(["de", "en"]));
    assert_eq!(forest.branches.iter().map(|branch| &branch.target_id).collect::<BTreeSet<_>>().len(), 2);
    assert_eq!(resolved.predicted_output_paths().unwrap().into_iter().collect::<BTreeSet<_>>(),
        BTreeSet::from([
            fixture.output.join("outputs/de/page.transcript.json"),
            fixture.output.join("outputs/en/page.transcript.json"),
        ]));
    let profile = EngineBuilder::new().build().unwrap()
        .inspect(InspectionRequest::from_path(&fixture.config)).unwrap();
    assert_eq!(Path::new(&profile.input_path), resolved.source_path());
    assert_eq!(profile.targets, vec!["transcript_json"]);
    assert_eq!(std::env::current_dir().unwrap(), cwd, "never change process-global cwd");
    assert_eq!(snapshot(&fixture.root), before);
}

#[test]
fn cli_graph_plan_and_sdk_provider_plan_share_native_transcript_override() {
    let fixture = Fixture::new();
    let exported = fixture.root.join("graph-plan.json");
    let config = fixture.config.to_str().unwrap();
    let export = exported.to_str().unwrap();
    let cli = Cli::try_parse_from([
        "renderflow", "graph", "plan", "--config", config,
        "--target", "transcript_text", "--format", "json", "--export", export,
    ]).unwrap();
    run_cli(cli).unwrap();
    let cli_plan: Value = serde_json::from_slice(&fs::read(exported).unwrap()).unwrap();
    let engine = EngineBuilder::new().build().unwrap();
    let provider = engine.provider_plan(PlanRequest::from_path(&fixture.config)
        .with_target("transcript_text")).unwrap();
    let resolved = resolve(fixture.request().with_target("transcript_text")).unwrap();

    assert_eq!(cli_plan, serde_json::to_value(&provider.plan).unwrap());
    assert_eq!(cli_plan, serde_json::to_value(resolved.plan()).unwrap());
    assert_eq!(provider.digest, sha256_serialized(resolved.plan()).unwrap());
    assert!(resolved.predicted_output_paths().unwrap().iter()
        .all(|path| path.ends_with("page.transcript.txt")));
    run_cli(Cli::try_parse_from([
        "renderflow", "build", "--config", config,
        "--target", "transcript_text", "--dry-run",
    ]).unwrap()).unwrap();
    assert!(!fixture.output.exists());
    assert!(!fixture.root.join(".renderflow").exists());
}

#[test]
fn dry_run_neither_creates_cache_nor_claims_predicted_paths_as_publications() {
    let fixture = Fixture::new();
    let mut spec = fixture.spec();
    spec["execution"] = json!({"composition": {"cache_directory": "terminal-cache"}});
    fixture.write_spec(&spec);
    let before = snapshot(&fixture.root);
    let resolved = resolve(fixture.request()).unwrap();
    assert_eq!(resolved.spec().execution.composition.as_ref().unwrap().cache_directory.as_deref(),
        Some(fixture.root.join("terminal-cache").to_str().unwrap()));
    let plan = serde_json::to_value(resolved.plan()).unwrap();
    let result = execute(resolved, true).unwrap();

    assert_eq!(result.run_manifest.state, RunState::Planned);
    assert_eq!(serde_json::to_value(&result.plan).unwrap(), plan);
    assert_eq!(result.outputs.len(), 2);
    assert!(result.manifest_path.is_none());
    assert!(result.run_manifest.steps.is_empty());
    assert!(result.run_manifest.artifact_manifest.artifacts.is_empty());
    assert_eq!(result.run_manifest.artifact_manifest.outputs.iter().map(String::as_str)
        .collect::<BTreeSet<_>>(), BTreeSet::from([
            "outputs/de/page.transcript.json", "outputs/en/page.transcript.json",
        ]));
    let forest = result.run_manifest.artifact_forest.as_ref().unwrap();
    assert!(forest.produced_artifacts.is_empty());
    assert!(forest.execution.as_ref().unwrap().targets.iter().all(|target|
        target.outcome == ForestTargetOutcome::Planned && target.locator.is_none()));
    assert_eq!(snapshot(&fixture.root), before);
}

#[test]
fn sdk_execution_uses_frozen_content_and_spec_after_resolution() {
    let fixture = Fixture::new();
    let engine = EngineBuilder::new().build().unwrap();
    let content_path = fixture.package.join("content/en.json");
    let original = fs::read(&content_path).unwrap();
    let resolved = engine.resolve_execution(ExecutionRequest::from_path(&fixture.config)).unwrap();
    let plan_digest = sha256_serialized(resolved.plan()).unwrap();
    fs::write(&content_path, b"changed after resolve; not valid JSON").unwrap();
    fs::write(&fixture.config, b"invalid: [changed after resolve").unwrap();
    let result = engine.execute_resolved(resolved, false).unwrap();

    assert_eq!(result.run_manifest.state, RunState::Complete);
    assert_eq!(result.run_manifest.execution_plan_digest, plan_digest);
    assert_eq!(fs::read(fixture.output.join("inputs").join(digest(&original))).unwrap(), original);
    let transcript = fs::read_to_string(fixture.output.join("outputs/en/page.transcript.json")).unwrap();
    assert!(transcript.contains("Shared art."));
    assert!(!transcript.contains("changed after resolve"));
    assert_eq!(fs::read(&content_path).unwrap(), b"changed after resolve; not valid JSON");
    assert_eq!(fs::read(&fixture.config).unwrap(), b"invalid: [changed after resolve");
}

#[test]
fn build_receipt_binds_exact_provider_plan_and_preserves_native_publication_joins() {
    let fixture = Fixture::new();
    let engine = EngineBuilder::new().build().unwrap();
    let provider = engine.provider_plan(PlanRequest::from_path(&fixture.config)).unwrap();
    let source_digest = sha256_serialized(resolve(fixture.request()).unwrap().spec()).unwrap();
    let sources_before = snapshot(&fixture.package);
    let result = engine.execute(ExecutionRequest::from_path(&fixture.config)).unwrap();
    let native: CompositionRun = fixture.read_json("composition-run.json");
    let native_common: RunManifest = fixture.read_json("run-manifest.json");
    let build: RunManifest = fixture.read_json("renderflow-run.json");
    let plan_bytes = fs::read(fixture.output.join("build-plan.json")).unwrap();

    assert_eq!(build, result.run_manifest);
    assert_eq!(build.state, RunState::Complete);
    assert_eq!(build.execution_plan_digest, provider.digest);
    assert_eq!(build.execution_plan_digest.value, digest(&plan_bytes));
    assert_eq!(plan_bytes, serde_json::to_vec(&provider.plan).unwrap());
    assert_eq!(build.source_spec_digest, source_digest);
    assert_eq!(native_common, native.run_manifest().unwrap());
    assert_eq!(native_common.execution_plan_digest.value,
        digest(&fs::read(fixture.output.join("execution-plan.json")).unwrap()));
    assert_eq!(build.artifact_manifest.artifacts, native.artifact_manifest.artifacts);
    assert_eq!(build.artifact_manifest.outputs, native.artifact_manifest.outputs);
    assert_eq!(build.artifact_manifest.run_id, native.artifact_manifest.run_id);
    assert_eq!(Path::new(&build.artifact_manifest.output_dir), fixture.output);
    assert_eq!(native.artifact_manifest.output_dir, ".");
    assert_eq!(build.steps, native_common.steps);
    assert_eq!(build.run_id, native.run_id);
    let forest = build.artifact_forest.as_ref().unwrap();
    let native_forest = native_common.artifact_forest.as_ref().unwrap();
    assert_eq!(forest.branches, native_forest.branches);
    assert_eq!(forest.execution, native_forest.execution);
    assert_eq!(forest.produced_artifacts, native_forest.produced_artifacts);
    assert_eq!(forest.policy["build"]["native_execution_plan_digest"],
        serde_json::to_value(&native_common.execution_plan_digest).unwrap());
    assert_eq!(forest.policy["build"]["native_forest_identity"], native_forest.identity);
    for row in &forest.execution.as_ref().unwrap().targets {
        assert_eq!(row.outcome, ForestTargetOutcome::Published);
        let record = &build.artifact_manifest.artifacts[row.artifact_evidence_index.unwrap()];
        assert_eq!(row.artifact_id.as_deref(), Some(record.artifact_id.as_str()));
        assert_eq!(row.locator.as_deref(), Some(record.locator.as_str()));
        assert_ne!(record.lifecycle, ArtifactRole::Source);
        let path = fixture.output.join(record.locator.strip_prefix("bundle:").unwrap());
        assert!(path.is_file());
    }
    assert_eq!(snapshot(&fixture.package), sources_before);
}

#[test]
fn unsupported_generic_overrides_and_publication_knobs_fail_before_writes() {
    let fixture = Fixture::new();
    let before = snapshot(&fixture.root);
    for request in [
        fixture.request().with_all_reachable(),
        fixture.request().with_profile("everything"),
        fixture.request().with_exclude("format:pdf").unwrap(),
        fixture.request().with_optimization(OptimizationMode::Speed),
    ] {
        let error = resolve(request).err().expect("unsupported override must be explicit");
        assert!(format!("{error:#}").contains("composition.build.overrides"));
    }
    assert_eq!(snapshot(&fixture.root), before);

    let original = fixture.spec();
    for (pointer, value) in [
        ("/output/naming_template", json!("custom/{ext}")),
        ("/output/collision", json!("overwrite")),
    ] {
        let mut spec = original.clone();
        let field = pointer.rsplit('/').next().unwrap();
        spec["output"][field] = value;
        fixture.write_spec(&spec);
        let before = snapshot(&fixture.root);
        assert!(resolve(fixture.request()).is_err(), "must reject {pointer}");
        assert_eq!(snapshot(&fixture.root), before);
    }
    fixture.write_spec(&original);
    assert_eq!(snapshot(&fixture.root), before);
    for cache in ["bundle", "./bundle/./shared-cache"] {
        let mut spec = original.clone();
        spec["execution"] = json!({"composition": {"cache_directory": cache}});
        fixture.write_spec(&spec);
        let before = snapshot(&fixture.root);
        let error = resolve(fixture.request()).err().expect("shared cache must be outside bundle");
        assert!(format!("{error:#}").contains("composition.build.cache"));
        assert_eq!(snapshot(&fixture.root), before);
    }
}

#[test]
fn resume_assessment_and_invalidation_refuse_before_touching_checkpoint_stores() {
    let fixture = Fixture::new();
    let engine = EngineBuilder::new().build().unwrap();
    // A pre-existing sentinel also catches accidental clearing of generic state.
    let state = fixture.root.join(".renderflow");
    fs::create_dir(&state).unwrap();
    fs::write(state.join("checkpoints.json"), b"unrelated checkpoint sentinel").unwrap();
    let before = snapshot(&fixture.root);
    let resume = engine.resume(ExecutionRequest::from_path(&fixture.config)).unwrap_err();
    assert!(resume.to_string().contains("composition.build.resume_unsupported"));
    let assessment = engine.assess_resume(ExecutionRequest::from_path(&fixture.config)).unwrap_err();
    assert!(assessment.to_string().contains("composition.build.graph_only"));
    let invalidation = engine.invalidate_checkpoints(ExecutionRequest::from_path(&fixture.config), None)
        .unwrap_err();
    assert!(invalidation.to_string().contains("composition.build.graph_only"));
    assert_eq!(snapshot(&fixture.root), before);
}

#[test]
fn cancellation_before_or_after_resolution_has_no_persistent_side_effects() {
    let fixture = Fixture::new();
    let cancellation = CancellationToken::new();
    let engine = EngineBuilder::new().with_cancellation_token(cancellation.clone()).build().unwrap();
    let before = snapshot(&fixture.root);
    let resolved = engine.resolve_execution(ExecutionRequest::from_path(&fixture.config)).unwrap();
    cancellation.cancel();
    let result = engine.execute_resolved(resolved, false).unwrap();

    assert_eq!(result.run_manifest.state, RunState::Cancelled);
    assert!(result.run_manifest.steps.is_empty());
    assert!(result.manifest.artifacts.is_empty());
    assert!(result.manifest.outputs.is_empty());
    assert!(result.manifest_path.is_none());
    assert!(result.run_manifest.artifact_forest.as_ref().unwrap().produced_artifacts.is_empty());
    assert!(matches!(engine.execute(ExecutionRequest::from_path(&fixture.config)),
        Err(RenderflowError::Cancelled)));
    assert_eq!(snapshot(&fixture.root), before);
}

#[test]
fn existing_output_bundle_is_untouched_including_when_execution_is_refused() {
    let fixture = Fixture::new();
    fs::create_dir(&fixture.output).unwrap();
    fs::write(fixture.output.join("renderflow-run.json"), b"previous bundle sentinel").unwrap();
    let before = snapshot(&fixture.root);
    let error = execute(resolve(fixture.request()).unwrap(), false).unwrap_err();

    assert!(format!("{error:#}").contains("composition.output.exists"));
    assert_eq!(snapshot(&fixture.root), before);
}

#[test]
fn blocked_pdf_and_unavailable_native_export_do_not_prevent_transcript_sibling() {
    let fixture = Fixture::new();
    let mut spec = fixture.spec();
    let mut pdf = PdfSettings::default();
    pdf.dpi = 0;
    spec["targets"]["exact"] = json!([
        {"format": "pdf"}, {"format": "psd"}, {"format": "transcript_json"},
    ]);
    spec["execution"] = json!({"composition": {
        "locales": {"mode": "exact", "locale": "en"}, "pdf": pdf,
    }});
    fixture.write_spec(&spec);
    let resolved = resolve(fixture.request()).unwrap();
    let forest = resolved.plan().artifact_forest.as_ref().unwrap();
    assert_eq!(forest.branches.iter().find(|branch| branch.format == "pdf").unwrap().state,
        ForestBranchState::Unsupported);
    assert_eq!(forest.branches.iter().find(|branch| branch.format == "psd").unwrap().state,
        ForestBranchState::Unavailable);
    let result = execute(resolved, false).unwrap();
    let native: CompositionRun = fixture.read_json("composition-run.json");

    assert_eq!(result.run_manifest.state, RunState::Partial);
    assert!(!result.is_success());
    assert_eq!(result.run_manifest.artifact_manifest.outputs,
        vec!["outputs/en/page.transcript.json"]);
    assert!(native.branches.iter().any(|branch| branch.outcome == CompositionBranchOutcome::Blocked));
    assert!(native.branches.iter().any(|branch| branch.outcome == CompositionBranchOutcome::Unavailable));
    assert_eq!(result.run_manifest.artifact_forest.as_ref().unwrap().execution.as_ref().unwrap()
        .targets.iter().filter(|row| row.outcome == ForestTargetOutcome::Published).count(), 1);
    assert!(!fixture.output.join("outputs/en/page.pdf").exists());
    assert!(!fixture.output.join("outputs/en/page.psd").exists());
    assert!(fixture.output.join("renderflow-run.json").is_file());
}

#[test]
fn ordinary_v1_and_v2_configs_keep_the_original_graph_plan() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("input.md");
    fs::write(&source, "# Existing graph path\n").unwrap();
    for (name, spec) in [
        ("v1", json!({"input": source, "output_dir": directory.path().join("v1-output"),
            "outputs": [{"type": "html"}]})),
        ("v2", json!({"schema": "renderflow/v2",
            "sources": [{"id": "source", "path": source, "format": "markdown"}],
            "targets": {"exact": [{"format": "html"}]},
            "output": {"bundle_root": directory.path().join("v2-output")}})),
    ] {
        let config = directory.path().join(format!("{name}.yaml"));
        fs::write(&config, serde_yaml_ng::to_string(&spec).unwrap()).unwrap();
        let ordinary = renderflow::planning::resolve(PlanningRequest::from_path(&config)).unwrap();
        let build = resolve(PlanningRequest::from_path(&config)).unwrap();
        assert!(!build.is_composition());
        assert_eq!(serde_json::to_value(build.plan()).unwrap(),
            serde_json::to_value(ordinary.plan()).unwrap());
        assert_eq!(build.graph().unwrap().source_format(), ordinary.source_format());
        assert_eq!(build.source_version(), ordinary.source_version());
        let result = execute(build, true).unwrap();
        assert_eq!(result.run_manifest.state, RunState::Planned);
        assert!(!directory.path().join(format!("{name}-output")).exists());
    }
}
