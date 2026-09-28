//! Public CLI smoke proof for the exact native ordered-page EPUB route.

use std::fs;
use std::process::Command;

use renderflow::evidence::{RunState, ValidationState};
use renderflow::RunManifest;
use serde_json::Value;
use sha2::{Digest, Sha256};

const PAGES: [&[u8]; 2] = [
    include_bytes!("../crates/renderflow-core/tests/fixtures/print-pdf/page-001.png"),
    include_bytes!("../crates/renderflow-core/tests/fixtures/print-pdf/page-002.png"),
];

#[test]
fn public_cli_builds_bounded_fixed_layout_epub() {
    let dir = tempfile::tempdir().unwrap();
    let mut sources = String::new();
    let mut artwork = String::new();
    for (index, bytes) in PAGES.iter().enumerate() {
        let filename = format!("page-{:03}.png", index + 1);
        fs::write(dir.path().join(&filename), bytes).unwrap();
        sources.push_str(&format!(
            "  - id: source.page{:03}\n    path: {filename}\n    format: png\n    media_type: image/png\n    sha256: \"{:x}\"\n    geometry: {{ width: 90, height: 90, unit: mm }}\n",
            index + 1,
            Sha256::digest(bytes),
        ));
        artwork.push_str(&format!(
            "    - role: page\n      path: {filename}\n      alt_text: Original synthetic page {}.\n",
            index + 1
        ));
    }
    fs::write(
        dir.path().join("renderflow.yaml"),
        format!(
            "schema: renderflow/v2\nsources:\n{sources}  - id: source.pages\n    kind: collection\n    members: [source.page001, source.page002]\npublication:\n  publication: Synthetic fixture\n  issue_id: synthetic-epub-cli\n  title: Original synthetic pages\n  contributors:\n    - name: Renderflow contributors\n      role: author\n  publication_date: \"2026-09-28\"\n  language: en-US\n  geometry: {{ width: 90, height: 90, unit: mm }}\n  artwork:\n{artwork}  rights:\n    license: CC0-1.0\n    rights_holder: Renderflow contributors\n  accessibility:\n    summary: Original synthetic geometric pages, each with a page description.\n    access_modes: [visual]\n    hazards: [none]\ntargets:\n  exact:\n    - id: target.ebook\n      role: ebook\n      format: epub\n      requirement: required\nexecution:\n  fixed_layout_epub:\n    page_progression_direction: ltr\n    spread: none\n    cover_member_id: source.page001\n    max_pages: 2\n    max_input_bytes: 1000000\n    max_output_bytes: 5000000\noutput:\n  bundle_root: dist\n"
        ),
    )
    .unwrap();
    let bin = env!("CARGO_BIN_EXE_renderflow");
    for args in [
        vec!["spec", "validate", "--config", "renderflow.yaml"],
        vec!["build", "--config", "renderflow.yaml", "--dry-run"],
        vec!["build", "--config", "renderflow.yaml"],
    ] {
        let result = Command::new(bin)
            .current_dir(dir.path())
            .args(&args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{args:?}: {}\n{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let output = dir.path().join("dist/source.pages/ebook.epub");
    assert!(output.is_file());
    let inspection = Command::new(bin)
        .current_dir(dir.path())
        .args([
            "ebook",
            "inspect",
            "--input",
            "dist/source.pages/ebook.epub",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        inspection.status.success(),
        "ebook inspect failed: {}\n{}",
        String::from_utf8_lossy(&inspection.stdout),
        String::from_utf8_lossy(&inspection.stderr)
    );
    let inspected: Value = serde_json::from_slice(&inspection.stdout).unwrap();
    assert_eq!(inspected["schema"], "renderflow.ebook-evidence/v1");
    assert_eq!(inspected["valid"], true);
    assert_eq!(inspected["variant"], "epub");
    assert_eq!(inspected["layout"], "pre_paginated");
    assert_eq!(inspected["epub_version"], "3.0");
    assert_eq!(inspected["spine_items"], 2);
    assert_eq!(inspected["page_list"], true);
    assert_eq!(inspected["navigation"], true);
    assert_eq!(
        inspected["source_sha256"],
        format!("{:x}", Sha256::digest(fs::read(&output).unwrap()))
    );
    let manifest: RunManifest =
        serde_json::from_slice(&fs::read(dir.path().join("dist/renderflow-run.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest.state,
        RunState::Complete,
        "{:?}",
        manifest.diagnostics
    );
    assert_eq!(manifest.artifact_manifest.outputs.len(), 6);
    assert_eq!(
        manifest
            .artifact_manifest
            .outputs
            .iter()
            .filter(|path| path.ends_with(".epub"))
            .count(),
        1
    );
    assert!(manifest
        .artifact_manifest
        .outputs
        .contains(&"dist/source.pages/ebook.epub".to_string()));
    for name in [
        "publication.json",
        "manifest.json",
        "provenance.json",
        "preflight.json",
        "checksums.sha256",
    ] {
        let locator = format!("dist/metadata/{name}");
        assert!(
            manifest.artifact_manifest.outputs.contains(&locator),
            "missing publication sidecar {locator}"
        );
        assert!(dir.path().join(&locator).is_file());
    }
    let terminal = manifest
        .artifact_manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.role == "ebook")
        .unwrap();
    assert_eq!(
        terminal.producer.provider.as_deref(),
        Some("tool.renderflow-epub")
    );
    assert!(matches!(
        terminal.validation,
        ValidationState::Valid | ValidationState::ValidWithWarnings
    ));
    assert_eq!(terminal.sources.len(), 2);
    assert_eq!(
        terminal.digest.value,
        format!("{:x}", Sha256::digest(fs::read(output).unwrap()))
    );
    for (index, bytes) in PAGES.iter().enumerate() {
        assert_eq!(
            fs::read(dir.path().join(format!("page-{:03}.png", index + 1))).unwrap(),
            *bytes
        );
    }
}
