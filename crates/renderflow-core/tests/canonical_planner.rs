use std::fs;
use std::path::PathBuf;

use renderflow::planning::{execute, resolve, PlanningRequest};
use renderflow::spec::SourceSpecVersion;

struct EquivalentConfigs {
    _temp_dir: tempfile::TempDir,
    v1: PathBuf,
    v2: PathBuf,
    v1_output: PathBuf,
    v2_output: PathBuf,
}

fn equivalent_configs() -> EquivalentConfigs {
    let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
    let input = temp_dir.path().join("input.md");
    fs::write(&input, "# Canonical planner\n").expect("failed to write source");

    let v1_output = temp_dir.path().join("dist-v1");
    let v1 = temp_dir.path().join("renderflow-v1.yaml");
    fs::write(
        &v1,
        format!(
            "input: \"{}\"\noutput_dir: \"{}\"\noutputs:\n  - type: html\n",
            input.display(),
            v1_output.display()
        ),
    )
    .expect("failed to write v1 spec");

    let v2_output = temp_dir.path().join("dist-v2");
    let v2 = temp_dir.path().join("renderflow-v2.yaml");
    fs::write(
        &v2,
        format!(
            "schema: renderflow/v2\nsources:\n  - id: source.main\n    role: manuscript\n    path: \"{}\"\n    format: markdown\ntargets:\n  exact:\n    - id: target.html\n      role: web\n      format: html\noutput:\n  bundle_root: \"{}\"\n  naming_template: \"{{target.role}}.{{ext}}\"\n  collision: error\n",
            input.display(),
            v2_output.display()
        ),
    )
    .expect("failed to write v2 spec");

    EquivalentConfigs {
        _temp_dir: temp_dir,
        v1,
        v2,
        v1_output,
        v2_output,
    }
}

#[test]
fn v1_and_v2_resolve_through_the_same_canonical_planner() {
    let configs = equivalent_configs();
    let v1 = resolve(PlanningRequest::from_path(&configs.v1)).expect("v1 should resolve");
    let v2 = resolve(PlanningRequest::from_path(&configs.v2)).expect("v2 should resolve");

    assert_eq!(v1.source_version(), SourceSpecVersion::V1);
    assert_eq!(v2.source_version(), SourceSpecVersion::V2);
    assert_eq!(v1.source_format(), v2.source_format());
    assert_eq!(v1.target_formats(), v2.target_formats());
    assert_eq!(v1.plan().source, v2.plan().source);
    assert_eq!(v1.plan().targets, v2.plan().targets);
    assert_eq!(v1.plan().metadata.total_edges, v2.plan().metadata.total_edges);
    assert_eq!(v1.plan().metadata.execution_depth, v2.plan().metadata.execution_depth);
}

#[test]
fn dry_run_returns_the_exact_frozen_plan_without_writing_outputs() {
    let configs = equivalent_configs();
    let resolved = resolve(PlanningRequest::from_path(&configs.v2)).expect("v2 should resolve");
    let frozen_plan = serde_json::to_value(resolved.plan()).expect("plan should serialize");

    assert!(!configs.v2_output.exists());
    let result = execute(resolved, true).expect("dry-run should succeed without provider execution");
    assert_eq!(
        serde_json::to_value(&result.plan).expect("result plan should serialize"),
        frozen_plan
    );
    assert!(!configs.v2_output.exists(), "dry-run must not create output root");
}

#[test]
fn v1_dry_run_is_also_side_effect_free() {
    let configs = equivalent_configs();
    let resolved = resolve(PlanningRequest::from_path(&configs.v1)).expect("v1 should resolve");
    assert!(!configs.v1_output.exists());
    execute(resolved, true).expect("v1 dry-run should succeed");
    assert!(!configs.v1_output.exists(), "v1 dry-run must not create output root");
}
