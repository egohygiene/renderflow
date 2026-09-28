use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::{run_epubcheck_with_executable, EpubCheckStatus};

fn fake_checker(
    root: &Path,
    input: &Path,
    version: &str,
    report: Option<Value>,
    exit_code: i32,
) -> PathBuf {
    let script = root.join("epubcheck");
    let report_copy = report.map(|value| {
        let path = root.join("fake-report.json");
        std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
        path
    });
    let command = report_copy.map_or_else(
        || "".to_string(),
        |report| format!("cp \"{}\" \"$3\"\n", report.display()),
    );
    let body = format!(
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf \"EPUBCheck v{version}\\n\"; exit 0; fi\n{command}exit {exit_code}\n"
    );
    std::fs::write(&script, body).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(input.exists());
    script
}

fn report(input: &Path, errors: u64) -> Value {
    json!({
        "checker": {
            "checkerVersion": "5.3.0",
            "filename": input.file_name().unwrap().to_str().unwrap(),
            "nFatal": 0,
            "nError": errors,
            "nWarning": 0
        },
        "publication": {},
        "items": [],
        "messages": if errors == 0 { vec![] } else { vec![json!({"severity": "error"})] }
    })
}

#[test]
fn epubcheck_v5_report_binds_executable_input_arguments_and_exit() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("synthetic.epub");
    std::fs::write(&input, b"synthetic provider fixture").unwrap();
    let script = fake_checker(root.path(), &input, "5.3.0", Some(report(&input, 0)), 0);
    let evidence = run_epubcheck_with_executable(&input, script.to_str().unwrap()).unwrap();
    assert_eq!(evidence.status, Some(EpubCheckStatus::Passed));
    assert_eq!(evidence.passed, Some(true));
    assert_eq!(evidence.exit_code, Some(0));
    assert_eq!(evidence.executable.as_deref(), script.to_str());
    assert_eq!(evidence.arguments.len(), 3);
    assert_eq!(evidence.arguments[0], input.to_str().unwrap());
    assert_eq!(evidence.arguments[1], "--json");
    assert_eq!(evidence.input_sha256.as_deref().map(str::len), Some(64));
    assert_eq!(evidence.report_sha256.as_deref().map(str::len), Some(64));
    assert!(evidence.report.is_some());
}

#[test]
fn epubcheck_conformance_failure_is_distinct_from_provider_failure() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("synthetic.epub");
    std::fs::write(&input, b"synthetic provider fixture").unwrap();
    let script = fake_checker(root.path(), &input, "5.3.0", Some(report(&input, 1)), 1);
    let evidence = run_epubcheck_with_executable(&input, script.to_str().unwrap()).unwrap();
    assert_eq!(evidence.status, Some(EpubCheckStatus::Invalid));
    assert_eq!(evidence.passed, Some(false));
    assert_eq!(evidence.exit_code, Some(1));
}

#[test]
fn epubcheck_missing_malformed_and_mismatched_reports_cannot_pass() {
    for case in ["missing", "malformed", "wrong_version", "wrong_filename"] {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("synthetic.epub");
        std::fs::write(&input, b"synthetic provider fixture").unwrap();
        let generated = match case {
            "missing" => None,
            "malformed" => Some(json!({"checker": {}})),
            "wrong_version" => {
                let mut value = report(&input, 0);
                value["checker"]["checkerVersion"] = json!("5.2.0");
                Some(value)
            }
            "wrong_filename" => {
                let mut value = report(&input, 0);
                value["checker"]["filename"] = json!("other.epub");
                Some(value)
            }
            _ => unreachable!(),
        };
        let script = fake_checker(root.path(), &input, "5.3.0", generated, 0);
        let evidence = run_epubcheck_with_executable(&input, script.to_str().unwrap()).unwrap();
        assert_eq!(
            evidence.status,
            Some(EpubCheckStatus::ProviderError),
            "{case}"
        );
        assert_eq!(evidence.passed, None, "{case}");
    }
}

#[test]
fn epubcheck_incompatible_version_is_unavailable() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("synthetic.epub");
    std::fs::write(&input, b"synthetic provider fixture").unwrap();
    let script = fake_checker(root.path(), &input, "4.2.6", Some(report(&input, 0)), 0);
    let evidence = run_epubcheck_with_executable(&input, script.to_str().unwrap()).unwrap();
    assert_eq!(evidence.status, Some(EpubCheckStatus::Unavailable));
    assert!(!evidence.available);
    assert_eq!(evidence.passed, None);
    assert!(evidence.arguments.is_empty());
}

#[test]
fn missing_executable_and_failed_version_probe_have_distinct_states() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("synthetic.epub");
    std::fs::write(&input, b"synthetic provider fixture").unwrap();
    let missing = root.path().join("not-installed");
    let absent = run_epubcheck_with_executable(&input, missing.to_str().unwrap()).unwrap();
    assert_eq!(absent.status, Some(EpubCheckStatus::Unavailable));
    assert_eq!(absent.passed, None);

    let script = root.path().join("broken-epubcheck");
    std::fs::write(&script, "#!/bin/sh\nexit 7\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let failed = run_epubcheck_with_executable(&input, script.to_str().unwrap()).unwrap();
    assert_eq!(failed.status, Some(EpubCheckStatus::ProviderError));
    assert_eq!(failed.passed, None);
}
