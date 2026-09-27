use std::fs;
use std::path::{Path, PathBuf};

use renderflow::evidence::{sha256_serialized, RunState};
use renderflow::planning::{execute, resolve, PlanningRequest};
use renderflow::spec::validate_spec_str;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const FIRST: &[u8] = include_bytes!("fixtures/ordered-collection/page-001.md");
const SECOND: &[u8] = include_bytes!("fixtures/ordered-collection/page-002.md");
const AGGREGATE: &str = include_str!("fixtures/ordered-collection/aggregate.py");

fn setup(count: usize) -> (TempDir, PathBuf, Vec<Vec<u8>>) {
    let dir = tempfile::tempdir().unwrap();
    let mut contents = Vec::new();
    let mut sources = String::new();
    let mut ids = Vec::new();
    for index in 0..count {
        let id = format!("source.page{index:03}");
        let path = format!("page-{index:03}.md");
        let bytes = if count == 2 {
            if index == 0 {
                FIRST.to_vec()
            } else {
                SECOND.to_vec()
            }
        } else {
            format!("<section id=\"page-{index:03}\">Page {index}</section>\n").into_bytes()
        };
        fs::write(dir.path().join(&path), &bytes).unwrap();
        let digest = format!("{:x}", Sha256::digest(&bytes));
        sources.push_str(&format!(
            "  - id: {id}\n    path: {path}\n    format: markdown\n    media_type: text/markdown\n    sha256: \"{digest}\"\n    geometry: {{ width: 210, height: 297, unit: mm }}\n"
        ));
        contents.push(bytes);
        ids.push(id);
    }
    sources.push_str(&format!(
        "  - id: source.pages\n    kind: collection\n    members: [{}]\n",
        ids.join(", ")
    ));
    let script = dir.path().join("aggregate.py");
    fs::write(&script, AGGREGATE).unwrap();
    fs::write(dir.path().join("transforms.yaml"), format!(
        "transforms:\n  - name: synthetic.ordered-pages\n    program: python3\n    args: [\"{}\", \"{{output}}\", \"{{inputs}}\"]\n    input_kind: collection\n    from: markdown\n    to: html\n    cost: 1.0\n    quality: 1.0\n",
        script.display()
    )).unwrap();
    let config = dir.path().join("renderflow.yaml");
    fs::write(&config, format!(
        "schema: renderflow/v2\nsources:\n{sources}targets:\n  exact:\n    - id: target.web\n      role: web\n      format: html\ntransforms: transforms.yaml\noutput:\n  bundle_root: \"{}\"\n  naming_template: \"{{source.id}}/{{target.role}}.{{ext}}\"\n",
        dir.path().join("dist").display()
    )).unwrap();
    (dir, config, contents)
}

fn plan_digest(path: &Path) -> String {
    let resolved = resolve(PlanningRequest::from_path(path)).unwrap();
    sha256_serialized(resolved.plan()).unwrap().value
}

#[test]
fn ordered_collection_executes_with_complete_lineage_and_immutable_sources() {
    let (dir, config, original) = setup(2);
    let resolved = resolve(PlanningRequest::from_path(&config)).unwrap();
    let collection = resolved.plan().source_collection.as_ref().unwrap();
    assert!(resolved.plan().source_artifact.is_none());
    assert_eq!(collection.source_id, "source.pages");
    assert_eq!(
        collection
            .members
            .iter()
            .map(|member| member.source_id.as_str())
            .collect::<Vec<_>>(),
        ["source.page000", "source.page001"]
    );
    assert_eq!(collection.members[0].locator, "page-000.md");
    assert_eq!(
        collection.members[0].geometry.as_ref().unwrap().width,
        210.0
    );
    let dry = execute(resolved, true).unwrap();
    assert_eq!(dry.run_manifest.state, RunState::Planned);
    assert!(dry.manifest_path.is_none());
    let result = execute(resolve(PlanningRequest::from_path(&config)).unwrap(), false).unwrap();
    assert_eq!(
        result.run_manifest.state,
        RunState::Complete,
        "{:?}",
        result.run_manifest.diagnostics
    );
    assert_eq!(result.outputs.len(), 1);
    let output = fs::read_to_string(&result.outputs[0]).unwrap();
    assert!(output.find("First page").unwrap() < output.find("Second page").unwrap());
    let evidence = &result.run_manifest.artifact_manifest.artifacts;
    assert_eq!(
        evidence
            .iter()
            .filter(|item| item.lifecycle == renderflow::evidence::ArtifactRole::Source)
            .count(),
        2
    );
    assert_eq!(evidence[0].metadata["renderflow.collection.index"], 0);
    assert_eq!(evidence[1].metadata["renderflow.collection.index"], 1);
    assert_eq!(
        evidence
            .iter()
            .find(|item| item.role == "web")
            .unwrap()
            .sources,
        vec![
            evidence[0].artifact_id.clone(),
            evidence[1].artifact_id.clone()
        ]
    );
    for (index, bytes) in original.iter().enumerate() {
        assert_eq!(
            &fs::read(dir.path().join(format!("page-{index:03}.md"))).unwrap(),
            bytes
        );
    }
    assert!(Path::new(result.manifest_path.as_ref().unwrap()).is_file());
    assert!(dir
        .path()
        .join(format!(
            ".renderflow/canonical-cache-{}.json",
            result.run_manifest.execution_plan_digest.value
        ))
        .is_file());
}

#[test]
fn collection_plan_binds_order_ids_geometry_configuration_and_larger_recipe() {
    let (dir, config, _) = setup(2);
    let original = fs::read_to_string(&config).unwrap();
    let first = plan_digest(&config);
    assert_eq!(first, plan_digest(&config));
    for changed in [
        original.replace(
            "source.page000, source.page001",
            "source.page001, source.page000",
        ),
        original.replace("source.page000", "source.front"),
        original.replace("width: 210", "width: 211"),
        original.replace("role: web", "role: alternate"),
    ] {
        fs::write(&config, changed).unwrap();
        assert_ne!(first, plan_digest(&config));
    }
    fs::write(&config, original).unwrap();
    let registry_path = dir.path().join("transforms.yaml");
    let registry = fs::read_to_string(&registry_path).unwrap();
    fs::write(
        &registry_path,
        registry.replace(
            "\"{output}\"",
            "\"--different-configuration\", \"{output}\"",
        ),
    )
    .unwrap();
    assert_ne!(
        first,
        plan_digest(&config),
        "transform argv must enter plan identity"
    );
    let (_large_dir, large_config, _) = setup(44);
    let resolved = resolve(PlanningRequest::from_path(large_config)).unwrap();
    assert_eq!(
        resolved
            .plan()
            .source_collection
            .as_ref()
            .unwrap()
            .members
            .len(),
        44
    );
}

#[test]
fn collection_refuses_duplicate_missing_escape_digest_media_and_stale_bytes() {
    let (dir, config, _) = setup(2);
    let original = fs::read_to_string(&config).unwrap();
    let duplicate = original.replace(
        "source.page000, source.page001",
        "source.page000, source.page000",
    );
    let report = validate_spec_str(&duplicate);
    assert!(report
        .diagnostics
        .iter()
        .any(|item| item.code == "collection.member.duplicate"));
    let invalid_geometry = original.replace("width: 210", "width: 0");
    assert!(validate_spec_str(&invalid_geometry)
        .diagnostics
        .iter()
        .any(|item| item.code == "collection.member.geometry"));
    let ambiguous = original.replace(
        "  - id: source.pages\n",
        "  - id: source.unreferenced\n    path: other.md\n  - id: source.pages\n",
    );
    fs::write(&config, ambiguous).unwrap();
    assert!(format!(
        "{:#}",
        resolve(PlanningRequest::from_path(&config)).err().unwrap()
    )
    .contains("collection.source.ambiguous"));
    for (spec, code) in [
        (
            original.replace("page-000.md", "missing.md"),
            "collection.member.missing",
        ),
        (
            original.replace("page-000.md", "../page-000.md"),
            "collection.member.locator",
        ),
        (
            original.replace("text/markdown", "application/pdf"),
            "collection.member.unsupported_media",
        ),
        (
            original.replace("sha256: \"", "sha256: \"f"),
            "collection.member.digest",
        ),
    ] {
        fs::write(&config, spec).unwrap();
        let error = resolve(PlanningRequest::from_path(&config)).err().unwrap();
        assert!(format!("{error:#}").contains(code), "{code}: {error:#}");
    }
    fs::write(&config, original).unwrap();
    let resolved = resolve(PlanningRequest::from_path(&config)).unwrap();
    fs::write(dir.path().join("page-001.md"), b"changed after planning").unwrap();
    let result = execute(resolved, false).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Failed);
    assert!(result.outputs.is_empty());
    assert!(result
        .run_manifest
        .diagnostics
        .iter()
        .any(|item| item.code == "execution.source_changed_after_intake"));
}

#[cfg(unix)]
#[test]
fn collection_refuses_symlink_substitution_before_planning_and_execution() {
    use std::os::unix::fs::symlink;
    let (dir, config, _) = setup(2);
    let original = fs::read_to_string(&config).unwrap();
    let alias = dir.path().join("alias.md");
    symlink(dir.path().join("page-000.md"), &alias).unwrap();
    fs::write(&config, original.replace("page-000.md", "alias.md")).unwrap();
    assert!(format!(
        "{:#}",
        resolve(PlanningRequest::from_path(&config)).err().unwrap()
    )
    .contains("collection.member.symlink"));
    fs::write(&config, original).unwrap();
    let resolved = resolve(PlanningRequest::from_path(&config)).unwrap();
    fs::rename(
        dir.path().join("page-000.md"),
        dir.path().join("retained.md"),
    )
    .unwrap();
    symlink(
        dir.path().join("retained.md"),
        dir.path().join("page-000.md"),
    )
    .unwrap();
    let result = execute(resolved, false).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Failed);
    assert!(result
        .run_manifest
        .diagnostics
        .iter()
        .any(|item| item.message.contains("collection.member.symlink")));
}
