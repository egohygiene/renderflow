//! Public CLI smoke proof for the exact native ordered-page EPUB route.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::process::Command;

use renderflow::evidence::{RunState, ValidationState};
use renderflow::RunManifest;
use serde_json::Value;
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

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
    let with_manifest = Command::new(bin)
        .current_dir(dir.path())
        .args([
            "ebook",
            "inspect",
            "--input",
            "dist/source.pages/ebook.epub",
            "--run-manifest",
            "dist/renderflow-run.json",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        with_manifest.status.success(),
        "explicit run evidence failed: {}\n{}",
        String::from_utf8_lossy(&with_manifest.stdout),
        String::from_utf8_lossy(&with_manifest.stderr)
    );
    let bound: Value = serde_json::from_slice(&with_manifest.stdout).unwrap();
    assert_eq!(bound["provenance"]["status"], "verified");
    assert_eq!(bound["fixed_layout"]["status"], "validated");
    let absolute_manifest = dir.path().join("dist/renderflow-run.json");
    let external_cwd = Command::new(bin)
        .current_dir(dir.path().parent().unwrap())
        .args([
            "ebook",
            "inspect",
            "--input",
            output.to_str().unwrap(),
            "--run-manifest",
            absolute_manifest.to_str().unwrap(),
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(
        external_cwd.status.success(),
        "absolute paths from another CWD failed: {}\n{}",
        String::from_utf8_lossy(&external_cwd.stdout),
        String::from_utf8_lossy(&external_cwd.stderr)
    );
    let external: Value = serde_json::from_slice(&external_cwd.stdout).unwrap();
    assert_eq!(external["provenance"]["status"], "verified");

    let capabilities = Command::new(bin)
        .args(["ebook", "capabilities", "--format", "json"])
        .output()
        .unwrap();
    assert!(capabilities.status.success());
    let advertised: Value = serde_json::from_slice(&capabilities.stdout).unwrap();
    assert_eq!(advertised["schema"], "renderflow.ebook-capabilities/v1");
    assert_eq!(advertised["epub_fixed_layout_generation"], true);
    assert_eq!(advertised["kepub_fixed_layout_generation"], false);
    assert_eq!(
        advertised["fixed_layout_epub_route"]["capability"],
        "ebook.generate.epub.fixed-layout"
    );

    let camouflaged = dir.path().join("camouflaged-reflow.epub");
    write_camouflaged_epub(&output, &camouflaged);
    let unsupported = Command::new(bin)
        .current_dir(dir.path())
        .args([
            "ebook",
            "inspect",
            "--input",
            camouflaged.to_str().unwrap(),
            "--fixed-layout",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    assert!(!unsupported.status.success());
    let refused: Value = serde_json::from_slice(&unsupported.stdout).unwrap();
    assert_eq!(refused["valid"], false);
    assert_eq!(refused["layout"], "reflowable");
    assert!(refused["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["code"] == "ebook.fixed_layout.unsupported"));
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
        format!("{:x}", Sha256::digest(fs::read(&output).unwrap()))
    );
    for (index, bytes) in PAGES.iter().enumerate() {
        assert_eq!(
            fs::read(dir.path().join(format!("page-{:03}.png", index + 1))).unwrap(),
            *bytes
        );
    }
    #[cfg(unix)]
    verify_optional_epubcheck_outcomes(&dir, bin);
}

fn write_camouflaged_epub(input: &std::path::Path, output: &std::path::Path) {
    let mut source = ZipArchive::new(File::open(input).unwrap()).unwrap();
    let mut dest = ZipWriter::new(File::create(output).unwrap());
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for index in 0..source.len() {
        let mut entry = source.by_index(index).unwrap();
        let name = entry
            .name()
            .replace("pages/page-", "other/page-")
            .replace("images/page-", "photos/page-");
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        if name.ends_with(".opf") || name.ends_with(".xhtml") {
            let text = String::from_utf8(bytes).unwrap();
            bytes = text
                .replace("pre-paginated", "reflowable")
                .replace("pages/page-", "other/page-")
                .replace("images/page-", "photos/page-")
                .into_bytes();
        }
        dest.start_file(name, options).unwrap();
        dest.write_all(&bytes).unwrap();
    }
    dest.finish().unwrap();
}

#[cfg(unix)]
fn verify_optional_epubcheck_outcomes(dir: &tempfile::TempDir, bin: &str) {
    use std::os::unix::fs::PermissionsExt;

    let executable_dir = dir.path().join("fake-provider-bin");
    fs::create_dir(&executable_dir).unwrap();
    let inspect_with_path = |path: &std::path::Path| {
        let result = Command::new(bin)
            .current_dir(dir.path())
            .env("PATH", path)
            .args([
                "ebook",
                "inspect",
                "--input",
                "dist/source.pages/ebook.epub",
                "--format",
                "json",
                "--epubcheck",
            ])
            .output()
            .unwrap();
        let evidence: Value = serde_json::from_slice(&result.stdout).unwrap_or_else(|error| {
            panic!(
                "expected structured evidence, got {error}: stdout={} stderr={}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            )
        });
        (result, evidence)
    };

    let (unavailable, absent) = inspect_with_path(&executable_dir);
    assert!(!unavailable.status.success(), "missing EPUBCheck passed");
    assert_eq!(
        absent["valid"], true,
        "native result remains independently valid"
    );
    assert_eq!(absent["epubcheck"]["available"], false);
    assert_eq!(absent["epubcheck"]["status"], "unavailable");
    assert!(absent["epubcheck"]["passed"].is_null());
    assert!(absent["epubcheck"]["diagnostic"].is_string());

    let provider = executable_dir.join("epubcheck");
    let arguments = dir.path().join("provider-arguments.txt");
    fs::write(
        &provider,
        format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  printf 'EPUBCheck 5.2.1\\n'\n  exit 0\nfi\nprintf '%s\\n' \"$@\" > \"{}\"\nif [ \"$2\" != \"--json\" ]; then exit 83; fi\nprintf '{{\"checker\":{{\"checkerVersion\":\"EPUBCheck 5.2.1\",\"filename\":\"ebook.epub\",\"nFatal\":0,\"nError\":1}},\"publication\":{{}},\"items\":[],\"messages\":[{{\"ID\":\"RSC-005\",\"severity\":\"ERROR\"}}]}}\\n' > \"$3\"\nprintf 'synthetic conformance error\\n' >&2\nexit 2\n",
            arguments.display()
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&provider).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&provider, permissions).unwrap();
    let (failure, rejected) = inspect_with_path(&executable_dir);
    assert!(!failure.status.success(), "failed EPUBCheck passed");
    assert_eq!(rejected["epubcheck"]["provider_id"], "tool.epubcheck");
    assert_eq!(rejected["epubcheck"]["available"], true);
    assert_eq!(rejected["epubcheck"]["status"], "invalid");
    assert_eq!(rejected["epubcheck"]["version"], "EPUBCheck 5.2.1");
    assert_eq!(rejected["epubcheck"]["passed"], false);
    assert!(rejected["epubcheck"]["executable"]
        .as_str()
        .unwrap()
        .ends_with("/epubcheck"));
    assert_eq!(rejected["epubcheck"]["exit_code"], 2);
    assert_eq!(
        rejected["epubcheck"]["arguments"][0],
        "dist/source.pages/ebook.epub"
    );
    assert_eq!(rejected["epubcheck"]["arguments"][1], "--json");
    assert!(rejected["epubcheck"]["arguments"][2]
        .as_str()
        .unwrap()
        .ends_with("/epubcheck.json"));
    assert!(rejected["epubcheck"]["report_sha256"].is_string());
    assert_eq!(
        rejected["epubcheck"]["input_sha256"],
        format!(
            "{:x}",
            Sha256::digest(fs::read(dir.path().join("dist/source.pages/ebook.epub")).unwrap())
        )
    );
    assert_eq!(
        rejected["epubcheck"]["report"]["messages"][0]["ID"],
        "RSC-005"
    );
    assert!(rejected["epubcheck"]["diagnostic"]
        .as_str()
        .unwrap()
        .contains("EPUBCheck reported conformance errors"));
    let argv = fs::read_to_string(arguments).unwrap();
    let mut argv = argv.lines();
    assert_eq!(argv.next(), Some("dist/source.pages/ebook.epub"));
    assert_eq!(argv.next(), Some("--json"));
    assert!(argv.next().unwrap().ends_with("/epubcheck.json"));
    assert_eq!(argv.next(), None);

    fs::write(
        &provider,
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf 'EPUBCheck 5.2.1\\n'; exit 0; fi\nprintf '{\"checker\":{\"checkerVersion\":\"EPUBCheck 5.2.1\",\"filename\":\"ebook.epub\",\"nFatal\":0,\"nError\":0},\"publication\":{},\"items\":[],\"messages\":[]}\\n' > \"$3\"\nexit 0\n",
    )
    .unwrap();
    let (success, accepted) = inspect_with_path(&executable_dir);
    assert!(
        success.status.success(),
        "mock provider success was rejected"
    );
    assert_eq!(accepted["epubcheck"]["status"], "passed");
    assert_eq!(accepted["epubcheck"]["passed"], true);

    fs::write(
        &provider,
        "#!/bin/sh\nprintf 'synthetic probe failure\\n' >&2\nexit 2\n",
    )
    .unwrap();
    let (failed_probe, probe) = inspect_with_path(&executable_dir);
    assert!(!failed_probe.status.success(), "failed probe passed");
    assert_eq!(probe["epubcheck"]["available"], false);
    assert_eq!(probe["epubcheck"]["status"], "provider_error");
    assert!(probe["epubcheck"]["passed"].is_null());
    assert!(probe["epubcheck"]["diagnostic"].is_string());
}
