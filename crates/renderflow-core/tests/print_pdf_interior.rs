//! Synthetic, canonical print-interior integration coverage. A real img2pdf
//! executable is used only when explicitly supplied by the test environment.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(unix)]
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant};

use renderflow::evidence::{ArtifactRole, RunState, StepState, ValidationState};
use renderflow::planning::{cancelled, execute, resolve, PlanningRequest};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const PNG_FIRST: &[u8] = include_bytes!("fixtures/print-pdf/page-001.png");
const PNG_SECOND: &[u8] = include_bytes!("fixtures/print-pdf/page-002.png");
const JPEG_FIRST: &[u8] = include_bytes!("fixtures/print-pdf/page-001.jpg");
const JPEG_SECOND: &[u8] = include_bytes!("fixtures/print-pdf/page-002.jpg");

struct Fixture {
    dir: TempDir,
    config: PathBuf,
    originals: Vec<Vec<u8>>,
}

impl Fixture {
    fn new(format: &str, count: usize, executable: &Path, version: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let (extension, media_type, pages) = match format {
            "png" => ("png", "image/png", [PNG_FIRST, PNG_SECOND]),
            "jpeg" => ("jpg", "image/jpeg", [JPEG_FIRST, JPEG_SECOND]),
            other => panic!("unsupported test format: {other}"),
        };
        let mut sources = String::new();
        let mut member_ids = Vec::new();
        let mut originals = Vec::new();
        for index in 0..count {
            let id = format!("source.page{index:03}");
            let locator = format!("page-{index:03}.{extension}");
            let bytes = pages[index % pages.len()];
            fs::write(dir.path().join(&locator), bytes).unwrap();
            let digest = format!("{:x}", Sha256::digest(bytes));
            sources.push_str(&format!(
                "  - id: {id}\n    path: {locator}\n    format: {format}\n    media_type: {media_type}\n    sha256: \"{digest}\"\n    geometry: {{ width: 90, height: 90, unit: mm, bleed: 5 }}\n"
            ));
            member_ids.push(id);
            originals.push(bytes.to_vec());
        }
        sources.push_str(&format!(
            "  - id: source.interior\n    kind: collection\n    members: [{}]\n",
            member_ids.join(", ")
        ));
        let config = dir.path().join("renderflow.yaml");
        fs::write(
            &config,
            format!(
                "schema: renderflow/v2\nsources:\n{sources}targets:\n  exact:\n    - id: target.interior\n      role: interior\n      format: pdf\n      requirement: required\nexecution:\n  print_pdf_interior:\n    executable: \"{}\"\n    provider_version: \"{version}\"\n    box_policy: media_bleed_trim_inset\n    rotation: none\n    scaling: fit\n    color_policy: preserve_rgb_gray\n    max_pages: 50\n    max_input_bytes: 1000000\n    max_output_bytes: 5000000\n    timeout_seconds: 2\noutput:\n  bundle_root: \"{}\"\n  naming_template: \"{{source.id}}/{{target.role}}.{{ext}}\"\n",
                executable.display(),
                dir.path().join("dist").display()
            ),
        )
        .unwrap();
        Self {
            dir,
            config,
            originals,
        }
    }

    fn source(&self, index: usize, extension: &str) -> PathBuf {
        self.dir.path().join(format!("page-{index:03}.{extension}"))
    }

    fn rewrite(&self, from: &str, to: &str) {
        let spec = fs::read_to_string(&self.config).unwrap();
        assert!(
            spec.contains(from),
            "expected test material was missing: {from}"
        );
        fs::write(&self.config, spec.replacen(from, to, 1)).unwrap();
    }
}

fn error_for(path: &Path) -> String {
    format!(
        "{:#}",
        resolve(PlanningRequest::from_path(path)).err().unwrap()
    )
}

fn unavailable_provider(dir: &TempDir) -> PathBuf {
    dir.path().join("missing-img2pdf-executable")
}

fn verify_unchanged(fixture: &Fixture, extension: &str) {
    for (index, bytes) in fixture.originals.iter().enumerate() {
        assert_eq!(fs::read(fixture.source(index, extension)).unwrap(), *bytes);
    }
}

#[test]
fn absent_provider_keeps_plan_inspectable_and_refuses_execution() {
    for (format, extension) in [("png", "png"), ("jpeg", "jpg")] {
        let temporary = tempfile::tempdir().unwrap();
        let fixture = Fixture::new(format, 2, &unavailable_provider(&temporary), "0.6.3");
        let resolved = resolve(PlanningRequest::from_path(&fixture.config)).unwrap();
        assert!(resolved.plan().toolchain.is_none());
        assert_eq!(
            resolved
                .plan()
                .source_collection
                .as_ref()
                .unwrap()
                .members
                .len(),
            2
        );
        assert!(resolved.plan().edges.iter().any(|edge| {
            edge.capability_id.as_deref() == Some("publication.generate.pdf.interior")
        }));
        let planned = execute(
            resolve(PlanningRequest::from_path(&fixture.config)).unwrap(),
            true,
        )
        .unwrap();
        assert_eq!(planned.run_manifest.state, RunState::Planned);
        assert!(planned.manifest_path.is_none());
        assert!(!fixture.dir.path().join("dist").exists());
        let result = execute(resolved, false).unwrap();
        assert_eq!(result.run_manifest.state, RunState::Failed);
        assert!(result.outputs.is_empty());
        assert!(result.run_manifest.diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("print_pdf.provider.unavailable")
        }));
        assert!(Path::new(result.manifest_path.as_ref().unwrap()).is_file());
        verify_unchanged(&fixture, extension);
    }
}

#[test]
fn print_policy_rejects_geometry_aspect_bounds_and_stale_declarations() {
    let temporary = tempfile::tempdir().unwrap();
    let provider = unavailable_provider(&temporary);
    for (before, after, expected) in [
        ("bleed: 5", "bleed: 0", "print_pdf.geometry.mixed"),
        (
            "width: 90, height: 90",
            "width: 91, height: 91",
            "print_pdf.geometry.mixed",
        ),
        ("max_pages: 50", "max_pages: 1", "print_pdf.bounds"),
        (
            "max_input_bytes: 1000000",
            "max_input_bytes: 1",
            "print_pdf.bounds",
        ),
        ("role: interior", "role: cover", "print_pdf.target"),
    ] {
        let fixture = Fixture::new("png", 2, &provider, "0.6.3");
        fixture.rewrite(before, after);
        let error = error_for(&fixture.config);
        assert!(error.contains(expected), "expected {expected}: {error}");
        assert!(!fixture.dir.path().join("dist").exists());
    }
    let fixture = Fixture::new("png", 2, &provider, "0.6.3");
    fixture.rewrite("width: 90", "width: 100");
    assert!(error_for(&fixture.config).contains("print_pdf.scaling.aspect"));

    let fixture = Fixture::new("png", 2, &provider, "0.6.3");
    fixture.rewrite("sha256: \"", "sha256: \"f");
    assert!(error_for(&fixture.config).contains("collection.member.digest"));

    let fixture = Fixture::new("png", 2, &provider, "0.6.3");
    fixture.rewrite("page-000.png", "../page-000.png");
    assert!(error_for(&fixture.config).contains("collection.member.locator"));

    let fixture = Fixture::new("png", 2, &provider, "0.6.3");
    fixture.rewrite("page-000.png", "missing.png");
    assert!(error_for(&fixture.config).contains("collection.member.missing"));
}

#[test]
fn cancellation_before_transform_has_no_pdf_and_keeps_source_immutable() {
    let temporary = tempfile::tempdir().unwrap();
    let fixture = Fixture::new("png", 2, &unavailable_provider(&temporary), "0.6.3");
    let result = cancelled(resolve(PlanningRequest::from_path(&fixture.config)).unwrap()).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Cancelled);
    assert!(result.outputs.is_empty());
    assert!(Path::new(result.manifest_path.as_ref().unwrap()).is_file());
    verify_unchanged(&fixture, "png");
}

#[cfg(unix)]
fn shim(root: &Path, action: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let path = root.join("fake-img2pdf");
    fs::write(
        &path,
        format!(
            "#!/bin/sh\nif [ \"${{1:-}}\" = \"--version\" ]; then\n  printf \"img2pdf 0.6.3\\n\"\n  exit 0\nfi\n{action}\n"
        ),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(unix)]
#[test]
fn stale_source_is_refused_even_with_available_provider() {
    let temporary = tempfile::tempdir().unwrap();
    let provider = shim(temporary.path(), "exit 99");
    let fixture = Fixture::new("png", 2, &provider, "0.6.3");
    let resolved = resolve(PlanningRequest::from_path(&fixture.config)).unwrap();
    assert!(
        resolved.plan().toolchain.is_some(),
        "{:?}",
        resolved.plan().diagnostics
    );
    fs::write(fixture.source(1, "png"), b"changed after planning").unwrap();
    let result = execute(resolved, false).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Failed);
    assert!(result.outputs.is_empty());
    assert!(result
        .run_manifest
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "execution.source_changed_after_intake" }));
}

#[cfg(unix)]
#[test]
fn nonzero_timeout_and_invalid_provider_outputs_never_publish_a_pdf() {
    for (action, code) in [
        ("exit 23", "print_pdf.provider.nonzero"),
        ("sleep 4", "print_pdf.provider.timeout"),
        ("exit 0", "print_pdf.provider.output"),
        ("while [ \"$#\" -gt 0 ]; do if [ \"$1\" = \"--output\" ]; then shift; printf \"invalid pdf\" > \"$1\"; exit 0; fi; shift; done; exit 24", "print_pdf.invalid_structure"),
    ] {
        let temporary = tempfile::tempdir().unwrap();
        let provider = shim(temporary.path(), action);
        let fixture = Fixture::new("png", 2, &provider, "0.6.3");
        let result = execute(resolve(PlanningRequest::from_path(&fixture.config)).unwrap(), false).unwrap();
        assert_ne!(result.run_manifest.state, RunState::Complete);
        assert!(result.outputs.is_empty(), "provider action published output: {action}");
        assert!(result.run_manifest.steps.iter().any(|step| {
            step.state == StepState::Failed
                && step.diagnostics.iter().any(|diagnostic| diagnostic.code == code)
        }), "missing typed failure {code} for {action}: {:?}", result.run_manifest.steps);
        assert!(Path::new(result.manifest_path.as_ref().unwrap()).is_file());
        verify_unchanged(&fixture, "png");
    }
}

#[cfg(unix)]
#[test]
fn cancelling_a_running_provider_records_cancelled_step_without_pdf() {
    let temporary = tempfile::tempdir().unwrap();
    let marker = temporary.path().join("provider-started");
    let provider = shim(
        temporary.path(),
        &format!(
            "printf \"started\\n\" > \"{}\"\nsleep 4\nexit 0",
            marker.display()
        ),
    );
    let fixture = Fixture::new("png", 2, &provider, "0.6.3");
    let cancellation = Arc::new(AtomicBool::new(false));
    let resolved = resolve(PlanningRequest::from_path(&fixture.config))
        .unwrap()
        .with_cancellation_flag(Arc::clone(&cancellation));
    let execution = thread::spawn(move || execute(resolved, false).unwrap());
    let deadline = Instant::now() + Duration::from_secs(3);
    while !marker.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(20));
    }
    assert!(marker.exists(), "provider did not start before its timeout");
    cancellation.store(true, Ordering::SeqCst);
    let result = execution.join().unwrap();
    assert_eq!(result.run_manifest.state, RunState::Cancelled);
    assert!(result.outputs.is_empty());
    assert!(result.run_manifest.steps.iter().any(|step| {
        step.state == StepState::Cancelled
            && step
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "print_pdf.provider.cancelled")
    }));
    assert!(Path::new(result.manifest_path.as_ref().unwrap()).is_file());
    verify_unchanged(&fixture, "png");
}

fn installed_provider() -> Option<(PathBuf, String)> {
    let path = std::env::var_os("RENDERFLOW_TEST_IMG2PDF").map(PathBuf::from)?;
    let output = Command::new(&path)
        .arg("--version")
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "RENDERFLOW_TEST_IMG2PDF={} cannot run: {error}",
                path.display()
            )
        });
    assert!(
        output.status.success(),
        "configured img2pdf --version failed"
    );
    let line = String::from_utf8_lossy(&output.stdout);
    let version = line
        .split(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
        .find(|part| {
            part.split('.').count() == 3
                && part.split('.').all(|number| {
                    !number.is_empty() && number.bytes().all(|byte| byte.is_ascii_digit())
                })
        })
        .unwrap_or_else(|| panic!("img2pdf emitted no numeric version: {line}"));
    let numbers = version.split('.').collect::<Vec<_>>();
    assert_eq!(
        numbers.len(),
        3,
        "img2pdf version must have three numbers: {line}"
    );
    assert_eq!(
        version, "0.6.3",
        "the tested print route is pinned to img2pdf 0.6.3"
    );
    Some((path, version.to_string()))
}

#[test]
fn installed_img2pdf_proves_ordered_pdf_and_clean_build_determinism() {
    let Some((provider, version)) = installed_provider() else {
        eprintln!("set RENDERFLOW_TEST_IMG2PDF to run the real-provider PDF integration fixture");
        return;
    };
    for (format, extension) in [("png", "png"), ("jpeg", "jpg")] {
        let mut pdfs = Vec::new();
        for _ in 0..2 {
            let fixture = Fixture::new(format, 2, &provider, &version);
            let resolved = resolve(PlanningRequest::from_path(&fixture.config)).unwrap();
            assert!(resolved.plan().toolchain.is_some());
            let result = execute(resolved, false).unwrap();
            assert_eq!(
                result.run_manifest.state,
                RunState::Complete,
                "{:?}",
                result.run_manifest.diagnostics
            );
            assert_eq!(result.outputs.len(), 1);
            let pdf = fs::read(&result.outputs[0]).unwrap();
            assert!(pdf.starts_with(b"%PDF-"));
            let artifacts = &result.run_manifest.artifact_manifest.artifacts;
            let sources = artifacts
                .iter()
                .filter(|item| item.lifecycle == ArtifactRole::Source)
                .collect::<Vec<_>>();
            let interior = artifacts
                .iter()
                .find(|item| item.role == "interior")
                .unwrap();
            assert_eq!(sources.len(), 2);
            assert_eq!(sources[0].metadata["renderflow.collection.index"], 0);
            assert_eq!(sources[1].metadata["renderflow.collection.index"], 1);
            assert_eq!(
                interior.sources,
                sources
                    .iter()
                    .map(|item| item.artifact_id.clone())
                    .collect::<Vec<_>>()
            );
            assert_eq!(interior.validation, ValidationState::Valid);
            let inspection = &interior.metadata["renderflow.print_pdf.inspection"];
            assert_eq!(inspection["page_count"], 2);
            assert_eq!(inspection["pages"].as_array().unwrap().len(), 2);
            assert_ne!(
                inspection["pages"][0]["image_stream_sha256"],
                inspection["pages"][1]["image_stream_sha256"]
            );
            verify_unchanged(&fixture, extension);
            pdfs.push(pdf);
        }
        assert_eq!(pdfs[0], pdfs[1], "{format} clean builds differ");
    }

    let fixture = Fixture::new("png", 44, &provider, &version);
    let result = execute(
        resolve(PlanningRequest::from_path(&fixture.config)).unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(
        result.run_manifest.state,
        RunState::Complete,
        "{:?}",
        result.run_manifest.diagnostics
    );
    let interior = result
        .run_manifest
        .artifact_manifest
        .artifacts
        .iter()
        .find(|item| item.role == "interior")
        .unwrap();
    assert_eq!(
        interior.metadata["renderflow.print_pdf.inspection"]["page_count"],
        44
    );
    assert_eq!(interior.sources.len(), 44);
    verify_unchanged(&fixture, "png");
}
