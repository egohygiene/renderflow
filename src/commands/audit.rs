use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

const AUDITS_DIR: &str = "audits";

/// Format a Unix timestamp (seconds) as `YYYYMMDD-HHMMSS`.
fn format_timestamp(secs: u64) -> String {
    // Days since Unix epoch
    let days = secs / 86400;
    let rem = secs % 86400;

    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;

    // Gregorian calendar calculation from days-since-epoch
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if mo <= 2 { y + 1 } else { y };

    format!("{:04}{:02}{:02}-{:02}{:02}{:02}", year, mo, d, h, m, s)
}

fn build_audit_content(timestamp: &str) -> String {
    format!(
        r#"================================================================================
  renderflow — Audit Log
  Generated: {timestamp}
================================================================================

## Summary of Changes

  - Added `audit` subcommand to the CLI for generating structured audit logs.
  - Introduced `audits/` directory to house timestamped log files.
  - The audit command writes a human-readable report covering architecture,
    improvements, and optimisation opportunities.

## Architecture Notes

  renderflow is a spec-driven document rendering engine written in Rust.
  The pipeline is composed of the following layers:

    CLI layer        cli.rs / commands/
      Parses user input via clap and dispatches to the appropriate command
      handler (build, audit).

    Config layer     config.rs
      Deserialises renderflow.yaml (or any user-supplied YAML) into a typed
      Config struct. Validation is performed before the pipeline starts.

    Asset layer      assets.rs
      Normalises relative asset paths referenced inside Markdown documents to
      absolute canonical paths so downstream tools operate correctly.

    Template layer   template.rs
      Initialises the Tera template engine and loads templates from the
      `templates/` directory.

    Pipeline layer   pipeline/
      Orchestrates transforms and strategy steps. Transforms mutate document
      content in memory; strategy steps invoke external renderers (Pandoc,
      Tectonic).

    Strategy layer   strategies/
      Selects the appropriate rendering backend (HTML via Pandoc, PDF via
      Tectonic) for each configured output format.

    Transform layer  transforms/
      Pure in-memory content mutations applied before rendering
      (e.g. EmojiTransform).

    Adapter layer    adapters/
      Thin wrappers around external command-line tools.

## Suggested Improvements

  1. Structured output formats
       Consider supporting JSON or TOML output in addition to YAML configs,
       widening compatibility with other toolchains.

  2. Parallel rendering
       Output formats are currently processed sequentially. Using Rayon or
       async tasks would speed up multi-format builds significantly.

  3. Incremental builds
       Hash input files and skip rendering when the output is already
       up-to-date, reducing rebuild times in watch mode.

  4. Plugin/transform registry
       Allow users to register custom transforms in renderflow.yaml instead of
       hard-coding EmojiTransform, making the pipeline extensible without
       recompilation.

  5. Richer error messages
       Surface line/column information from YAML parse errors and include
       actionable remediation hints.

  6. Template hot-reload
       In watch mode, re-initialise Tera only when template files change,
       rather than on every rebuild.

## Areas for Optimisation

  - Asset path normalisation rescans the document on every build; caching the
    result keyed on file mtime would eliminate redundant work.
  - The progress bar is constructed even when stdout is not a TTY; detect TTY
    and fall back to plain log lines to avoid broken output in CI.
  - Config deserialization allocates a fresh String for every field; using
    borrowed types (&str with a lifetime) where possible would reduce heap
    pressure for large configs.
  - tracing-subscriber is initialised once in main; consider exposing a
    structured JSON formatter (tracing-subscriber's json feature) for
    machine-readable log ingestion.

================================================================================
  End of audit log
================================================================================
"#,
        timestamp = timestamp,
    )
}

pub fn run(output_dir: Option<&str>) -> Result<PathBuf> {
    let dir = Path::new(output_dir.unwrap_or(AUDITS_DIR));

    fs::create_dir_all(dir)
        .with_context(|| format!("Failed to create audit directory: {}", dir.display()))?;

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("System clock is before the Unix epoch")?
        .as_secs();

    let timestamp = format_timestamp(secs);

    // If a log with this timestamp already exists, append an incrementing
    // counter so that rapid successive invocations never overwrite each other.
    let path = {
        let base = dir.join(format!("audit-{}.log", timestamp));
        if !base.exists() {
            base
        } else {
            let mut counter = 1u32;
            loop {
                let candidate = dir.join(format!("audit-{}-{}.log", timestamp, counter));
                if !candidate.exists() {
                    break candidate;
                }
                counter += 1;
            }
        }
    };

    let content = build_audit_content(&timestamp);

    fs::write(&path, &content)
        .with_context(|| format!("Failed to write audit log: {}", path.display()))?;

    info!("Audit log written to: {}", path.display());
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_timestamp_known_value() {
        // Unix epoch itself → 19700101-000000
        assert_eq!(format_timestamp(0), "19700101-000000");
    }

    #[test]
    fn test_format_timestamp_known_date() {
        // 2024-01-15 11:34:56 UTC = 1705318496 s
        assert_eq!(format_timestamp(1705318496), "20240115-113456");
    }

    #[test]
    fn test_format_timestamp_length() {
        let ts = format_timestamp(1_700_000_000);
        // Must be exactly 15 chars: YYYYMMDD-HHMMSS
        assert_eq!(ts.len(), 15, "unexpected length: {ts}");
    }

    #[test]
    fn test_run_creates_directory_and_file() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let audit_dir = dir.path().join("audits");

        let path = run(Some(audit_dir.to_str().unwrap()))
            .expect("audit run should succeed");

        assert!(audit_dir.exists(), "audit directory should be created");
        assert!(path.exists(), "audit log file should be created");

        let name = path.file_name().unwrap().to_string_lossy();
        assert!(
            name.starts_with("audit-"),
            "filename should start with 'audit-', got: {name}"
        );
        assert!(
            name.ends_with(".log"),
            "filename should end with '.log', got: {name}"
        );
    }

    #[test]
    fn test_run_file_contains_expected_sections() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let path = run(Some(dir.path().to_str().unwrap()))
            .expect("audit run should succeed");

        let content = fs::read_to_string(&path).expect("failed to read audit log");
        assert!(content.contains("Summary of Changes"), "missing Summary section");
        assert!(content.contains("Architecture Notes"), "missing Architecture section");
        assert!(content.contains("Suggested Improvements"), "missing Improvements section");
        assert!(content.contains("Areas for Optimisation"), "missing Optimisation section");
    }

    #[test]
    fn test_run_idempotent_creates_unique_files() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let audit_dir = dir.path().join("my-audits");

        // Run twice quickly; both calls must produce separate files even if they
        // share the same second-precision timestamp (counter suffix is appended).
        let p1 = run(Some(audit_dir.to_str().unwrap())).expect("first run should succeed");
        let p2 = run(Some(audit_dir.to_str().unwrap())).expect("second run should succeed");

        assert!(p1.exists(), "first audit log must exist");
        assert!(p2.exists(), "second audit log must exist");
        assert_ne!(p1, p2, "both runs must produce distinct file paths");
    }

    #[test]
    fn test_audit_content_contains_timestamp() {
        let ts = "20260323-172806";
        let content = build_audit_content(ts);
        assert!(
            content.contains(ts),
            "audit content should embed the timestamp"
        );
    }
}
