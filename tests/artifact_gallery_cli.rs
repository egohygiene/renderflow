//! Opt-in, real-provider gallery built through the public CLI. Normal workspace
//! tests validate the corpus contract without pretending an absent provider ran.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[cfg(unix)]
use renderflow::evidence::StepState;
use renderflow::evidence::{ArtifactRole, RunState, ValidationState};
use renderflow::graph::Format;
use renderflow::print_pdf_image::{inspect_print_image, PrintImageColorSpace};
use renderflow::print_pdf_inspect::{inspect_print_pdf, ExpectedPrintPage};
use renderflow::RunManifest;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("fixtures/golden-artifacts/v1/corpus.json");
const BIN: &str = env!("CARGO_BIN_EXE_renderflow");

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn fixture<'a>(corpus: &'a Value, id: &str) -> &'a Value {
    corpus["fixtures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap_or_else(|| panic!("gallery fixture {id} missing from golden corpus"))
}

fn expected<'a>(corpus: &'a Value, case_id: &str) -> &'a Value {
    corpus["scenarios"]
        .as_array()
        .unwrap()
        .iter()
        .find(|scenario| scenario["id"] == "real_artifact_gallery")
        .unwrap()["expected_artifacts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["case_id"] == case_id)
        .unwrap_or_else(|| panic!("gallery expected artifact {case_id} missing"))
}

fn source_bytes(entry: &Value) -> Vec<u8> {
    match entry["encoding"].as_str().unwrap() {
        "utf8" => entry["payload"].as_str().unwrap().as_bytes().to_vec(),
        "hex" => {
            let payload = entry["payload"].as_str().unwrap();
            assert_eq!(
                payload.len() % 2,
                0,
                "fixture has an odd-length hex payload"
            );
            payload
                .as_bytes()
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect()
        }
        other => panic!("unsupported corpus fixture encoding {other}"),
    }
}

fn materialize(entry: &Value, directory: &Path) -> (PathBuf, Vec<u8>) {
    let bytes = source_bytes(entry);
    assert_eq!(digest(&bytes), entry["sha256"].as_str().unwrap());
    let path = directory.join(entry["filename"].as_str().unwrap());
    fs::write(&path, &bytes).unwrap();
    (path, bytes)
}

fn invoke(directory: &Path, args: &[&str]) -> Output {
    Command::new(BIN)
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("could not launch Renderflow CLI: {error}"))
}

fn successful(directory: &Path, args: &[&str]) -> Output {
    let result = invoke(directory, args);
    assert!(
        result.status.success(),
        "gallery command {args:?} failed in {}: {}\n{}",
        directory.display(),
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
    result
}

fn published_files(root: &Path) -> Vec<PathBuf> {
    fn visit(directory: &Path, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            if kind.is_dir() {
                visit(&entry.path(), files);
            } else {
                assert!(kind.is_file(), "published output must be a regular file");
                files.push(entry.path());
            }
        }
    }
    let mut files = Vec::new();
    visit(&root.join("dist"), &mut files);
    files.sort();
    files
}

fn assert_published_files(directory: &Path, artifact: &str) {
    let mut expected = vec![
        directory.join("dist/renderflow-run.json"),
        directory.join("dist").join(artifact),
    ];
    expected.sort();
    assert_eq!(
        published_files(directory),
        expected,
        "unexpected published artifact or missing run manifest"
    );
}

fn execute_project(directory: &Path) -> RunManifest {
    successful(
        directory,
        &["spec", "validate", "--config", "renderflow.yaml"],
    );
    let preview = successful(
        directory,
        &["build", "--config", "renderflow.yaml", "--dry-run"],
    );
    let plan: Value = serde_json::from_slice(&preview.stdout).expect("dry-run plan is JSON");
    assert!(plan.get("edges").is_some(), "dry-run omitted planned edges");
    successful(directory, &["build", "--config", "renderflow.yaml"]);
    let manifest: RunManifest =
        serde_json::from_slice(&fs::read(directory.join("dist/renderflow-run.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest.state,
        RunState::Complete,
        "{:#?}",
        manifest.diagnostics
    );
    assert_eq!(manifest.artifact_manifest.outputs.len(), 1);
    manifest
}

fn checked_oracle_path(entry: &Value) -> PathBuf {
    let relative = entry["oracle_path"].as_str().unwrap();
    let path = Path::new(relative);
    assert!(
        !path.is_absolute()
            && path
                .components()
                .all(|component| matches!(component, std::path::Component::Normal(_))),
        "oracle path must remain within the corpus"
    );
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/golden-artifacts/v1")
        .join(path)
}

fn compare_text(case: &str, baseline: &Path, actual: &Path) {
    let expected = fs::read_to_string(baseline).unwrap();
    let produced = fs::read_to_string(actual).unwrap();
    if expected != produced {
        let first = expected
            .bytes()
            .zip(produced.bytes())
            .position(|(left, right)| left != right)
            .unwrap_or(expected.len().min(produced.len()));
        panic!(
            "{case}: reviewed artifact drift at byte {first}\nexpected ({}):\n{expected}\nactual ({}):\n{produced}\nEdit the reviewed oracle in a separate PR after examining the generated artifact.",
            baseline.display(),
            actual.display()
        );
    }
}

fn require_providers() -> (String, String) {
    let pandoc = Command::new("pandoc")
        .arg("--version")
        .output()
        .unwrap_or_else(|error| {
            panic!("unavailable: local Pandoc is required for real artifact gallery: {error}")
        });
    assert!(
        pandoc.status.success(),
        "unavailable: Pandoc version probe failed"
    );
    let pandoc_version = String::from_utf8_lossy(&pandoc.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_string();
    assert!(
        pandoc_version
            .strip_prefix("pandoc ")
            .and_then(|version| version.split('.').next())
            .and_then(|major| major.parse::<u64>().ok())
            .is_some_and(|major| major >= 2),
        "unavailable: Pandoc 2.0.0 or newer is required; observed: {pandoc_version}"
    );
    let img2pdf = std::env::var("RENDERFLOW_TEST_IMG2PDF")
        .expect("unavailable: set RENDERFLOW_TEST_IMG2PDF to an img2pdf 0.6.3 executable");
    let probe = Command::new(&img2pdf)
        .arg("--version")
        .output()
        .unwrap_or_else(|error| {
            panic!("unavailable: cannot run img2pdf provider {img2pdf}: {error}")
        });
    assert!(
        probe.status.success(),
        "unavailable: img2pdf version probe failed"
    );
    assert_eq!(
        String::from_utf8_lossy(&probe.stdout).trim(),
        "img2pdf 0.6.3",
        "unavailable: gallery requires the proven img2pdf 0.6.3 provider"
    );
    (pandoc_version, img2pdf)
}

fn write_markdown_config(directory: &Path, digest: &str) {
    fs::write(
        directory.join("renderflow.yaml"),
        format!(
            "schema: renderflow/v2\nsources:\n  - id: source.markdown\n    path: synthetic.md\n    format: markdown\n    media_type: text/markdown\n    sha256: \"{digest}\"\ntargets:\n  exact:\n    - id: target.web\n      role: web\n      format: html\n      requirement: required\noutput:\n  bundle_root: dist\n  naming_template: \"{{source.id}}/{{target.role}}.{{ext}}\"\n"
        ),
    )
    .unwrap();
}

fn write_pdf_config(directory: &Path, filenames: &[String], digests: &[String], executable: &str) {
    let executable = serde_json::to_string(executable).unwrap();
    fs::write(
        directory.join("renderflow.yaml"),
        format!(
            "schema: renderflow/v2\nsources:\n  - id: source.page001\n    path: {}\n    format: png\n    media_type: image/png\n    sha256: \"{}\"\n    geometry: {{ width: 90, height: 90, unit: mm, bleed: 5 }}\n  - id: source.page002\n    path: {}\n    format: png\n    media_type: image/png\n    sha256: \"{}\"\n    geometry: {{ width: 90, height: 90, unit: mm, bleed: 5 }}\n  - id: source.pages\n    kind: collection\n    members: [source.page001, source.page002]\ntargets:\n  exact:\n    - id: target.interior\n      role: interior\n      format: pdf\n      requirement: required\nexecution:\n  print_pdf_interior:\n    executable: {executable}\n    provider_version: \"0.6.3\"\n    box_policy: media_bleed_trim_inset\n    rotation: none\n    scaling: fit\n    color_policy: preserve_rgb_gray\n    max_pages: 2\n    max_input_bytes: 1000000\n    max_output_bytes: 5000000\n    timeout_seconds: 30\noutput:\n  bundle_root: dist\n  naming_template: \"{{source.id}}/{{target.role}}.{{ext}}\"\n",
            serde_json::to_string(&filenames[0]).unwrap(), digests[0],
            serde_json::to_string(&filenames[1]).unwrap(), digests[1],
        ),
    )
    .unwrap();
}

#[test]
#[ignore = "explicit real-provider gallery; use scripts/run-artifact-gallery.sh"]
fn real_artifact_gallery_v1() {
    let (pandoc_version, img2pdf) = require_providers();
    let corpus: Value = serde_json::from_str(CORPUS).unwrap();
    let markdown_decl = expected(&corpus, "markdown_html");
    let pdf_decl = expected(&corpus, "ordered_print_pdf");
    assert_eq!(markdown_decl["provider_id"], "tool.pandoc");
    assert_eq!(markdown_decl["role"], "web");
    assert_eq!(markdown_decl["format"], "html");
    assert_eq!(markdown_decl["media_type"], "text/html");
    assert_eq!(pdf_decl["provider_id"], "tool.img2pdf");
    assert_eq!(pdf_decl["provider_version"], "0.6.3");
    assert_eq!(pdf_decl["role"], "interior");
    assert_eq!(pdf_decl["format"], "pdf");
    assert_eq!(pdf_decl["media_type"], "application/pdf");
    let oracle: Value =
        serde_json::from_slice(&fs::read(checked_oracle_path(pdf_decl)).unwrap()).unwrap();
    assert_eq!(oracle["schema"], "renderflow.gallery-print-oracle/v1");
    let root_guard = tempfile::tempdir().unwrap();
    let root = if let Some(path) = std::env::var_os("RENDERFLOW_GALLERY_OUTPUT_DIR") {
        let path = PathBuf::from(path);
        assert!(
            path.is_absolute(),
            "gallery output directory must be absolute"
        );
        if path.exists() {
            assert!(
                fs::read_dir(&path).unwrap().next().is_none(),
                "gallery output directory must be empty to avoid stale evidence"
            );
        }
        fs::create_dir_all(&path).unwrap();
        path
    } else {
        root_guard.path().join("gallery")
    };
    fs::create_dir_all(&root).unwrap();
    eprintln!("Real artifact gallery output: {}", root.display());

    let markdown = fixture(&corpus, "fixture.document.markdown");
    let image_ids = ["fixture.image.print-page001", "fixture.image.print-page002"];
    let expected_source_ids = pdf_decl["source_ids"].as_array().unwrap();
    assert_eq!(expected_source_ids, &image_ids.map(|id| json!(id)));
    let mut html_builds = Vec::new();
    let mut pdf_builds = Vec::new();
    let mut pdf_inspections = Vec::new();
    let mut pdf_expectations = Vec::new();
    for iteration in 1..=2 {
        let html_dir = root.join(format!("markdown/run-{iteration}"));
        fs::create_dir_all(&html_dir).unwrap();
        let (source, source_bytes) = materialize(markdown, &html_dir);
        write_markdown_config(&html_dir, &digest(&source_bytes));
        let manifest = execute_project(&html_dir);
        let output = html_dir.join("dist/source.markdown/web.html");
        assert!(output.is_file());
        assert_published_files(&html_dir, "source.markdown/web.html");
        compare_text(
            "markdown_html",
            &checked_oracle_path(markdown_decl),
            &output,
        );
        assert_eq!(
            fs::read(&source).unwrap(),
            source_bytes,
            "Markdown source changed"
        );
        let terminal = manifest
            .artifact_manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.role == "web")
            .unwrap();
        assert_eq!(terminal.lifecycle, ArtifactRole::Terminal);
        assert_eq!(terminal.format, "html");
        assert_eq!(terminal.media_type, "text/html");
        assert_eq!(terminal.producer.provider.as_deref(), Some("tool.pandoc"));
        assert_eq!(terminal.digest.value, digest(&fs::read(&output).unwrap()));
        assert_eq!(terminal.sources.len(), 1);
        assert_eq!(
            manifest.artifact_manifest.outputs[0],
            "dist/source.markdown/web.html"
        );
        html_builds.push(fs::read(output).unwrap());

        let pdf_dir = root.join(format!("print/run-{iteration}"));
        fs::create_dir_all(&pdf_dir).unwrap();
        let mut image_paths = Vec::new();
        let mut original_images = Vec::new();
        let mut source_digests = Vec::new();
        let mut filenames = Vec::new();
        for id in image_ids {
            let declaration = fixture(&corpus, id);
            let (path, bytes) = materialize(declaration, &pdf_dir);
            filenames.push(declaration["filename"].as_str().unwrap().to_string());
            source_digests.push(digest(&bytes));
            image_paths.push(path);
            original_images.push(bytes);
        }
        write_pdf_config(&pdf_dir, &filenames, &source_digests, &img2pdf);
        let manifest = execute_project(&pdf_dir);
        let output = pdf_dir.join("dist/source.pages/interior.pdf");
        assert!(output.is_file());
        assert_published_files(&pdf_dir, "source.pages/interior.pdf");
        let declared_pages = oracle["pages"].as_array().unwrap();
        assert_eq!(
            declared_pages.len(),
            oracle["page_count"].as_u64().unwrap() as usize
        );
        let mut expected_pages = Vec::new();
        for (index, path) in image_paths.iter().enumerate() {
            let image = inspect_print_image(path, Format::Png).unwrap();
            let declared = &declared_pages[index];
            assert_eq!(declared["source_id"], image_ids[index]);
            assert_eq!(
                image.width_px,
                declared["pixel_width"].as_u64().unwrap() as u32
            );
            assert_eq!(
                image.height_px,
                declared["pixel_height"].as_u64().unwrap() as u32
            );
            assert_eq!(image.color_space, PrintImageColorSpace::Rgb);
            assert_eq!(
                image.image_stream_sha256,
                declared["image_stream_sha256"].as_str().unwrap()
            );
            expected_pages.push(ExpectedPrintPage {
                media_width_pt: oracle["media_width_pt"].as_f64().unwrap(),
                media_height_pt: oracle["media_height_pt"].as_f64().unwrap(),
                trim_inset_pt: oracle["trim_inset_pt"].as_f64().unwrap(),
                pixel_width: image.width_px,
                pixel_height: image.height_px,
                rotation: oracle["rotation"].as_u64().unwrap() as u16,
                color_space: oracle["color_space"].as_str().unwrap().to_string(),
                image_filter: oracle["image_filter"].as_str().unwrap().to_string(),
                image_stream_sha256: image.image_stream_sha256,
            });
        }
        let inspection = inspect_print_pdf(&output, &expected_pages).unwrap();
        assert_eq!(inspection.page_count, 2);
        let terminal = manifest
            .artifact_manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.role == "interior")
            .unwrap();
        assert_eq!(terminal.lifecycle, ArtifactRole::Terminal);
        assert_eq!(terminal.validation, ValidationState::Valid);
        assert_eq!(terminal.producer.provider.as_deref(), Some("tool.img2pdf"));
        assert_eq!(terminal.digest.value, inspection.sha256);
        assert_eq!(terminal.sources.len(), 2);
        let sources = manifest
            .artifact_manifest
            .artifacts
            .iter()
            .filter(|artifact| artifact.lifecycle == ArtifactRole::Source)
            .collect::<Vec<_>>();
        assert_eq!(sources.len(), 2);
        assert_eq!(
            terminal.sources,
            sources
                .iter()
                .map(|source| source.artifact_id.clone())
                .collect::<Vec<_>>()
        );
        for (index, source) in sources.iter().enumerate() {
            assert_eq!(source.metadata["renderflow.collection.index"], index);
            assert_eq!(
                fs::read(&image_paths[index]).unwrap(),
                original_images[index],
                "PNG source changed"
            );
        }
        assert_eq!(
            manifest.artifact_manifest.outputs[0],
            "dist/source.pages/interior.pdf"
        );
        assert_eq!(
            terminal.metadata["renderflow.print_pdf.inspection"]["sha256"],
            inspection.sha256
        );
        pdf_builds.push(fs::read(output).unwrap());
        pdf_inspections.push(inspection.sha256);
        pdf_expectations = expected_pages;
    }
    assert_eq!(
        html_builds[0], html_builds[1],
        "markdown_html changed across clean builds"
    );
    assert_eq!(
        pdf_builds[0], pdf_builds[1],
        "ordered_print_pdf changed across clean builds"
    );

    let stale_dir = root.join("negative/stale-source");
    fs::create_dir_all(&stale_dir).unwrap();
    let mut stale_filenames = Vec::new();
    let mut original_digests = Vec::new();
    for id in image_ids {
        let declaration = fixture(&corpus, id);
        let (_, bytes) = materialize(declaration, &stale_dir);
        stale_filenames.push(declaration["filename"].as_str().unwrap().to_string());
        original_digests.push(digest(&bytes));
    }
    write_pdf_config(&stale_dir, &stale_filenames, &original_digests, &img2pdf);
    fs::write(
        stale_dir.join(&stale_filenames[0]),
        b"deliberately changed source",
    )
    .unwrap();
    let stale = invoke(&stale_dir, &["build", "--config", "renderflow.yaml"]);
    fs::write(stale_dir.join("refusal.stderr.txt"), &stale.stderr).unwrap();
    assert!(!stale.status.success(), "changed source was accepted");
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("collection.member.digest_mismatch"),
        "changed source produced no typed refusal: {}",
        String::from_utf8_lossy(&stale.stderr)
    );
    assert!(
        !stale_dir.join("dist/source.pages/interior.pdf").exists(),
        "changed source published an output"
    );
    let mut negative_checks = vec![json!({
        "id":"stale_source", "expected":"collection.member.digest_mismatch",
        "evidence":"negative/stale-source/refusal.stderr.txt", "status":"passed"
    })];

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let refused_dir = root.join("negative/invalid-provider");
        fs::create_dir_all(&refused_dir).unwrap();
        let shim = refused_dir.join("fake-img2pdf");
        fs::write(
            &shim,
            "#!/bin/sh\nif [ \"${1:-}\" = \"--version\" ]; then printf 'img2pdf 0.6.3\\n'; exit 0; fi\nwhile [ \"$#\" -gt 0 ]; do\n  if [ \"$1\" = \"--output\" ]; then shift; printf 'invalid pdf' > \"$1\"; exit 0; fi\n  shift\ndone\nexit 24\n",
        )
        .unwrap();
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
        let mut filenames = Vec::new();
        let mut digests = Vec::new();
        for id in image_ids {
            let declaration = fixture(&corpus, id);
            let (_, bytes) = materialize(declaration, &refused_dir);
            filenames.push(declaration["filename"].as_str().unwrap().to_string());
            digests.push(digest(&bytes));
        }
        write_pdf_config(&refused_dir, &filenames, &digests, shim.to_str().unwrap());
        let refused = invoke(&refused_dir, &["build", "--config", "renderflow.yaml"]);
        fs::write(refused_dir.join("refusal.stderr.txt"), &refused.stderr).unwrap();
        assert!(
            !refused.status.success(),
            "invalid provider output was accepted"
        );
        assert!(
            !refused_dir.join("dist/source.pages/interior.pdf").exists(),
            "invalid provider output was published"
        );
        let failed: RunManifest = serde_json::from_slice(
            &fs::read(refused_dir.join("dist/renderflow-run.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(failed.state, RunState::Failed);
        assert!(failed.artifact_manifest.outputs.is_empty());
        assert!(
            failed.steps.iter().any(|step| {
                step.state == StepState::Failed
                    && step
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.code == "print_pdf.invalid_structure")
            }),
            "missing typed invalid structure refusal: {:?}",
            failed.steps
        );
        negative_checks.push(json!({
            "id":"invalid_provider_output", "expected":"print_pdf.invalid_structure",
            "evidence":"negative/invalid-provider/dist/renderflow-run.json", "status":"passed"
        }));
    }

    let invalid_dir = root.join("negative/invalid-pdf");
    fs::create_dir_all(&invalid_dir).unwrap();
    let invalid_pdf = invalid_dir.join("truncated.pdf");
    fs::write(&invalid_pdf, &pdf_builds[0][..pdf_builds[0].len() / 2]).unwrap();
    let invalid = inspect_print_pdf(&invalid_pdf, &pdf_expectations);
    fs::write(
        invalid_dir.join("refusal.txt"),
        format!("{}\n", invalid.as_ref().unwrap_err()),
    )
    .unwrap();
    assert!(
        invalid.is_err(),
        "truncated generated PDF passed the independent print inspector"
    );
    negative_checks.push(json!({
        "id":"invalid_pdf", "expected":"independent PDF inspection refusal",
        "evidence":"negative/invalid-pdf/refusal.txt", "status":"passed"
    }));
    let report = json!({
        "schema": "renderflow.artifact-gallery-report/v1",
        "corpus_version": corpus["corpus_version"],
        "status": "passed",
        "providers": {"pandoc": pandoc_version, "img2pdf": "0.6.3"},
        "cases": [
            {"case_id":"markdown_html","status":"passed","output":"markdown/run-1/dist/source.markdown/web.html","sha256":digest(&html_builds[0]),"clean_builds_equal":true},
            {"case_id":"ordered_print_pdf","status":"passed","output":"print/run-1/dist/source.pages/interior.pdf","sha256":pdf_inspections[0],"page_count":2,"clean_builds_equal":true}
        ],
        "negative_checks": negative_checks
    });
    fs::write(
        root.join("comparison.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    eprintln!("PASS: two real CLI artifact cases, three immutable source inputs, two clean builds each. Open {}", root.display());
}
