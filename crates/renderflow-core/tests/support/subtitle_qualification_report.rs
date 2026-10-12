//! Retained evidence for the explicitly requested native subtitle harness.
//! This helper never runs tools and never converts unavailable work to a pass.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;

pub(crate) const REQUIRED_CASES: &[(&str, &str)] = &[
    ("fixture_generation", "Generate original synthetic media fixtures"),
    ("srt_stream", "Extract the explicit nonzero SubRip stream"),
    ("webvtt_stream", "Extract the explicit nonzero WebVTT stream"),
    ("sdk_lineage", "Retain canonical transcript and export lineage through the SDK"),
    ("no_subtitles", "Report an empty inventory and refuse extraction"),
    ("unsupported_ass", "Discover and refuse styled ASS subtitles"),
    ("unsupported_mov_text", "Discover and refuse MP4 MOV timed text"),
    ("pre_cancelled", "Refuse a subtitle operation cancelled before native work"),
    ("malformed_container", "Refuse an invalid media container"),
    ("provider_versions", "Recheck exact provider version pins after all native cases"),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CaseStatus {
    Passed,
    Failed,
    Unavailable,
    NotRun,
}

#[derive(Debug, Serialize)]
struct ProviderEvidence {
    expected_version_line: String,
    observed_version_line: Option<String>,
    diagnostic: Option<String>,
}

#[derive(Debug, Serialize)]
struct CaseEvidence {
    id: String,
    description: String,
    required: bool,
    status: CaseStatus,
    detail: Value,
}

#[derive(Debug, Serialize)]
struct ImplementationEvidence {
    crate_version: String,
    /// SHA-256 of the compiled integration-test executable, including linked core.
    harness_sha256: String,
    /// Optional operator declaration; the harness does not verify it through Git.
    source_revision: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct QualificationReport {
    schema_version: &'static str,
    status: CaseStatus,
    os: &'static str,
    arch: &'static str,
    implementation: Option<ImplementationEvidence>,
    providers: BTreeMap<String, ProviderEvidence>,
    cases: Vec<CaseEvidence>,
    #[serde(skip)]
    owned_path: RefCell<Option<PathBuf>>,
}

impl QualificationReport {
    pub(crate) fn new(expected_ffmpeg: &str, expected_ffprobe: &str) -> Self {
        Self {
            schema_version: "renderflow.subtitle-qualification/v1",
            status: CaseStatus::NotRun,
            os: std::env::consts::OS,
            arch: std::env::consts::ARCH,
            implementation: None,
            providers: [("ffmpeg", expected_ffmpeg), ("ffprobe", expected_ffprobe)]
                .into_iter()
                .map(|(tool, expected)| (tool.to_string(), ProviderEvidence {
                    expected_version_line: expected.to_string(),
                    observed_version_line: None,
                    diagnostic: None,
                }))
                .collect(),
            cases: REQUIRED_CASES.iter().map(|(id, description)| CaseEvidence {
                id: (*id).to_string(),
                description: (*description).to_string(),
                required: true,
                status: CaseStatus::NotRun,
                detail: serde_json::json!({}),
            }).collect(),
            owned_path: RefCell::new(None),
        }
    }

    pub(crate) fn set_implementation(
        &mut self,
        crate_version: &str,
        harness_sha256: &str,
        source_revision: Option<&str>,
    ) -> Result<()> {
        if crate_version.is_empty() || harness_sha256.len() != 64
            || !harness_sha256.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            bail!("qualification implementation requires a crate version and lowercase SHA-256");
        }
        self.implementation = Some(ImplementationEvidence {
            crate_version: crate_version.to_string(),
            harness_sha256: harness_sha256.to_string(),
            source_revision: source_revision.map(str::to_string),
        });
        self.refresh_status();
        Ok(())
    }

    pub(crate) fn set_provider(
        &mut self,
        tool: &str,
        observed: Option<&str>,
        diagnostic: Option<&str>,
    ) -> Result<()> {
        let evidence = self.providers.get_mut(tool)
            .with_context(|| format!("unknown qualification provider '{tool}'"))?;
        evidence.observed_version_line = observed.map(str::to_string);
        evidence.diagnostic = diagnostic.map(str::to_string);
        self.refresh_status();
        Ok(())
    }

    pub(crate) fn record_case(&mut self, id: &str, status: CaseStatus, detail: Value) -> Result<()> {
        if !detail.is_object() {
            bail!("qualification case detail must be a JSON object");
        }
        let case = self.cases.iter_mut().find(|case| case.id == id)
            .with_context(|| format!("unknown qualification case '{id}'"))?;
        case.status = status;
        case.detail = detail;
        self.refresh_status();
        Ok(())
    }

    fn refresh_status(&mut self) {
        self.status = if self.cases.iter().any(|case| case.status == CaseStatus::Failed) {
            CaseStatus::Failed
        } else if self.cases.iter().any(|case| case.status == CaseStatus::Unavailable) {
            CaseStatus::Unavailable
        } else if self.cases.iter().filter(|case| case.required)
            .all(|case| case.status == CaseStatus::Passed)
        {
            if self.implementation.is_some() && self.providers.values().all(|provider| {
                !provider.expected_version_line.is_empty()
                    && provider.observed_version_line.as_deref() == Some(provider.expected_version_line.as_str())
                    && provider.diagnostic.is_none()
            }) { CaseStatus::Passed } else { CaseStatus::Unavailable }
        } else {
            CaseStatus::NotRun
        };
    }

    pub(crate) fn status(&self) -> CaseStatus { self.status }

    pub(crate) fn is_passed(&self) -> bool { self.status() == CaseStatus::Passed }

    /// The harness creates a new retained directory and persists once before
    /// probing tools. Initial publication refuses an existing report. Later
    /// calls replace only this object's own report through an adjacent temp file.
    pub(crate) fn persist(&self, path: &Path) -> Result<()> {
        let parent = path.parent().filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let parent = fs::canonicalize(parent).context("qualification output directory is unavailable")?;
        let path = parent.join(path.file_name().context("qualification report needs a filename")?);
        let initial = self.owned_path.borrow().is_none();
        if let Some(owned) = self.owned_path.borrow().as_ref() {
            if owned != &path { bail!("qualification report cannot switch its owned destination"); }
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&parent)?;
        serde_json::to_writer_pretty(temporary.as_file_mut(), self)?;
        temporary.write_all(b"\n")?;
        temporary.as_file().sync_all()?;
        if initial {
            temporary.persist_noclobber(&path).map_err(|error| error.error)
                .context("qualification report destination already exists or is unavailable")?;
            *self.owned_path.borrow_mut() = Some(path);
        } else {
            temporary.persist(&path).map_err(|error| error.error)
                .context("cannot atomically replace the owned qualification report")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualification_status_requires_every_case_and_preserves_failure_priority() {
        let mut report = QualificationReport::new("ffmpeg expected", "ffprobe expected");
        assert_eq!(report.status(), CaseStatus::NotRun);
        report.record_case("fixture_generation", CaseStatus::Passed, serde_json::json!({})).unwrap();
        assert!(!report.is_passed());
        report.record_case("srt_stream", CaseStatus::Unavailable, serde_json::json!({"reason":"missing provider"})).unwrap();
        assert_eq!(report.status(), CaseStatus::Unavailable);
        report.record_case("webvtt_stream", CaseStatus::Failed, serde_json::json!({"reason":"unexpected output"})).unwrap();
        assert_eq!(report.status(), CaseStatus::Failed);

        let mut complete = QualificationReport::new("ffmpeg expected", "ffprobe expected");
        complete.set_provider("ffmpeg", Some("ffmpeg expected"), None).unwrap();
        complete.set_provider("ffprobe", Some("ffprobe expected"), None).unwrap();
        for (id, _) in REQUIRED_CASES {
            complete.record_case(id, CaseStatus::Passed, serde_json::json!({})).unwrap();
        }
        assert!(!complete.is_passed(), "a pass requires the compiled harness identity");
        complete.set_implementation("0.1.0", &"a".repeat(64), None).unwrap();
        assert!(complete.is_passed());
        complete.set_provider("ffmpeg", Some("ffmpeg changed"), None).unwrap();
        assert_eq!(complete.status(), CaseStatus::Unavailable);
        assert!(complete.record_case("unknown", CaseStatus::Passed, serde_json::json!({})).is_err());
        assert!(complete.record_case("srt_stream", CaseStatus::Passed, Value::Null).is_err());
    }

    #[test]
    fn qualification_report_starts_unrun_and_refuses_preexisting_destinations() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("qualification.json");
        let mut report = QualificationReport::new("ffmpeg expected", "ffprobe expected");
        report.persist(&path).unwrap();
        let initial: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(initial["status"], "not_run");
        assert_eq!(initial["cases"].as_array().unwrap().len(), REQUIRED_CASES.len());
        assert!(initial["cases"].as_array().unwrap().iter().all(|case| case["status"] == "not_run"));
        assert!(initial["providers"]["ffmpeg"]["observed_version_line"].is_null());
        assert!(initial["implementation"].is_null());
        let before = fs::read(&path).unwrap();
        assert!(QualificationReport::new("other", "other").persist(&path).is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        report.set_provider("ffmpeg", None, Some("provider missing")).unwrap();
        report.set_implementation("0.1.0", &"b".repeat(64), Some("operator-declared-revision")).unwrap();
        report.record_case("fixture_generation", CaseStatus::Unavailable, serde_json::json!({"reason":"provider missing"})).unwrap();
        report.persist(&path).unwrap();
        let updated: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(updated["status"], "unavailable");
        assert_eq!(updated["providers"]["ffmpeg"]["diagnostic"], "provider missing");
        assert_eq!(updated["implementation"]["harness_sha256"], "b".repeat(64));
        assert_eq!(updated["implementation"]["source_revision"], "operator-declared-revision");
        assert!(report.persist(&directory.path().join("unowned.json")).is_err());
    }
}
