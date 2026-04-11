use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::warn;

// ── FileDependency ────────────────────────────────────────────────────────────

/// A single file dependency: the path to the file and a hex-encoded SHA-256
/// hash of its contents at the time the dependent output was last built.
///
/// When the file's current content hash differs from the recorded hash the
/// dependency is considered stale, and every output that listed it as a
/// dependency must be rebuilt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileDependency {
    /// Canonical path to the dependency file.
    pub path: String,
    /// Hex-encoded SHA-256 hash of the file's contents when the output was built.
    pub hash: String,
}

// ── DependencyMap ─────────────────────────────────────────────────────────────

/// Persistent map from output file paths to the set of file dependencies that
/// produced them.
///
/// This is the core data structure of the incremental build system.  By
/// recording exactly which input files contributed to each output, the build
/// system can:
///
/// * answer "is this output up-to-date?" by comparing each dependency's
///   current content hash with the stored hash;
/// * answer "which outputs are affected if file F changes?" by scanning every
///   output's dependency list for an entry matching F.
///
/// The map is persisted to disk as compact JSON in the output directory
/// (`.renderflow-deps.json`) and loaded at the start of each build.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DependencyMap(HashMap<String, Vec<FileDependency>>);

impl DependencyMap {
    /// Record that `output_path` was produced from the given file dependencies.
    ///
    /// Any previous entry for `output_path` is replaced.
    pub fn record(&mut self, output_path: String, deps: Vec<FileDependency>) {
        self.0.insert(output_path, deps);
    }

    /// Return the recorded file dependencies for `output_path`, or `None` if
    /// the output has never been built (or was built without dependency tracking).
    pub fn dependencies_for(&self, output_path: &str) -> Option<&[FileDependency]> {
        self.0.get(output_path).map(Vec::as_slice)
    }

    /// Return `true` when every dependency recorded for `output_path` still
    /// has the same content hash as when the output was last built.
    ///
    /// Returns `false` in any of these situations:
    /// * `output_path` has no recorded dependencies (never tracked).
    /// * Any recorded dependency's current hash differs from the stored hash.
    /// * `current_deps` is empty (no dependencies provided by the caller).
    pub fn is_output_up_to_date(
        &self,
        output_path: &str,
        current_deps: &[FileDependency],
    ) -> bool {
        if current_deps.is_empty() {
            return false;
        }
        let Some(recorded) = self.dependencies_for(output_path) else {
            return false;
        };
        // Build a lookup from path → hash for the recorded state.
        let recorded_map: HashMap<&str, &str> = recorded
            .iter()
            .map(|d| (d.path.as_str(), d.hash.as_str()))
            .collect();

        // Every dependency provided by the caller must match the recorded hash.
        current_deps.iter().all(|dep| {
            recorded_map
                .get(dep.path.as_str())
                .is_some_and(|&stored_hash| stored_hash == dep.hash)
        })
    }

    /// Return the paths of all outputs that have `changed_path` listed as a
    /// dependency **and** whose recorded hash for that dependency differs from
    /// `changed_hash`.
    ///
    /// This can be used to determine which outputs must be rebuilt when a
    /// specific file is modified.
    pub fn outputs_affected_by(&self, changed_path: &str, changed_hash: &str) -> Vec<String> {
        self.0
            .iter()
            .filter_map(|(output, deps)| {
                let affected = deps.iter().any(|dep| {
                    dep.path == changed_path && dep.hash != changed_hash
                });
                if affected { Some(output.clone()) } else { None }
            })
            .collect()
    }
}

// ── File hashing ──────────────────────────────────────────────────────────────

/// Compute a hex-encoded SHA-256 hash of the contents of the file at `path`.
///
/// Returns an error if the file cannot be read.
pub fn hash_file(path: &Path) -> Result<String> {
    let contents = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&contents);
    Ok(format!("{:x}", hasher.finalize()))
}

// ── Dependency construction ───────────────────────────────────────────────────

/// Collect the file-level dependencies for a single output.
///
/// The dependency list always includes:
/// * the primary input file (`input_path`)
/// * the configuration file (`config_path`)
///
/// When a Pandoc template file is specified and can be read, its path and hash
/// are appended as well.  If the template file cannot be read, a warning is
/// logged and the dependency is omitted (a subsequent build will treat the
/// output as stale, which is the safe default).
pub fn build_output_dependencies(
    input_path: &Path,
    config_path: &Path,
    template_path: Option<&Path>,
) -> Vec<FileDependency> {
    let mut deps = Vec::new();

    for file in [input_path, config_path] {
        match hash_file(file) {
            Ok(h) => deps.push(FileDependency {
                path: file.to_string_lossy().into_owned(),
                hash: h,
            }),
            Err(e) => warn!(
                path = %file.display(),
                error = %e,
                "Could not hash dependency file; output will be treated as stale"
            ),
        }
    }

    if let Some(tmpl) = template_path {
        match hash_file(tmpl) {
            Ok(h) => deps.push(FileDependency {
                path: tmpl.to_string_lossy().into_owned(),
                hash: h,
            }),
            Err(e) => warn!(
                path = %tmpl.display(),
                error = %e,
                "Could not hash template dependency; output will be treated as stale"
            ),
        }
    }

    deps
}

// ── Persistence ───────────────────────────────────────────────────────────────

/// Load the dependency map from disk.
///
/// Returns an empty map if the file does not exist or cannot be parsed.
/// Non-fatal errors are logged at `WARN` level so a corrupt or missing map
/// never aborts the build.
pub fn load_dependency_map(cache_path: &Path) -> DependencyMap {
    if !cache_path.exists() {
        return DependencyMap::default();
    }

    match fs::read_to_string(cache_path) {
        Err(e) => {
            warn!(
                path = %cache_path.display(),
                error = %e,
                "Failed to read dependency map file; starting with empty map"
            );
            DependencyMap::default()
        }
        Ok(content) => match serde_json::from_str(&content) {
            Ok(map) => map,
            Err(e) => {
                warn!(
                    path = %cache_path.display(),
                    error = %e,
                    "Failed to parse dependency map file; starting with empty map"
                );
                DependencyMap::default()
            }
        },
    }
}

/// Persist the dependency map to disk as compact JSON.
///
/// Errors are propagated to the caller.
pub fn save_dependency_map(map: &DependencyMap, cache_path: &Path) -> Result<()> {
    let json = serde_json::to_string(map)?;
    fs::write(cache_path, json)?;
    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn dep(path: &str, hash: &str) -> FileDependency {
        FileDependency {
            path: path.to_string(),
            hash: hash.to_string(),
        }
    }

    // ── DependencyMap::record / dependencies_for ──────────────────────────────

    #[test]
    fn test_record_and_retrieve_dependencies() {
        let mut map = DependencyMap::default();
        let deps = vec![dep("/in/doc.md", "aaa"), dep("/cfg/build.yaml", "bbb")];
        map.record("/out/doc.html".to_string(), deps.clone());
        assert_eq!(map.dependencies_for("/out/doc.html"), Some(deps.as_slice()));
    }

    #[test]
    fn test_missing_output_returns_none() {
        let map = DependencyMap::default();
        assert!(map.dependencies_for("/out/missing.html").is_none());
    }

    #[test]
    fn test_record_overwrites_previous_entry() {
        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/in/doc.md", "old")]);
        map.record("/out/doc.html".to_string(), vec![dep("/in/doc.md", "new")]);
        let deps = map.dependencies_for("/out/doc.html").unwrap();
        assert_eq!(deps[0].hash, "new");
    }

    // ── DependencyMap::is_output_up_to_date ───────────────────────────────────

    #[test]
    fn test_up_to_date_when_all_deps_match() {
        let mut map = DependencyMap::default();
        let deps = vec![dep("/in/doc.md", "hash1"), dep("/cfg/build.yaml", "hash2")];
        map.record("/out/doc.html".to_string(), deps.clone());
        assert!(map.is_output_up_to_date("/out/doc.html", &deps));
    }

    #[test]
    fn test_stale_when_dep_hash_changed() {
        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/in/doc.md", "old_hash")]);
        let current = vec![dep("/in/doc.md", "new_hash")];
        assert!(!map.is_output_up_to_date("/out/doc.html", &current));
    }

    #[test]
    fn test_stale_when_output_not_recorded() {
        let map = DependencyMap::default();
        let current = vec![dep("/in/doc.md", "hash1")];
        assert!(!map.is_output_up_to_date("/out/doc.html", &current));
    }

    #[test]
    fn test_stale_when_current_deps_empty() {
        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/in/doc.md", "hash1")]);
        assert!(!map.is_output_up_to_date("/out/doc.html", &[]));
    }

    #[test]
    fn test_stale_when_new_dep_not_in_recorded() {
        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/in/doc.md", "hash1")]);
        // Current state has an extra dep the recorded map doesn't know about.
        let current = vec![
            dep("/in/doc.md", "hash1"),
            dep("/templates/tmpl.html", "tmpl_hash"),
        ];
        assert!(!map.is_output_up_to_date("/out/doc.html", &current));
    }

    // ── DependencyMap::outputs_affected_by ───────────────────────────────────

    #[test]
    fn test_outputs_affected_by_detects_changed_dep() {
        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/templates/a.html", "old")]);
        map.record("/out/doc.pdf".to_string(), vec![dep("/in/doc.md", "hash1")]);

        let affected = map.outputs_affected_by("/templates/a.html", "new");
        assert_eq!(affected, vec!["/out/doc.html".to_string()]);
    }

    #[test]
    fn test_outputs_affected_by_returns_empty_when_dep_unchanged() {
        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/templates/a.html", "same")]);

        let affected = map.outputs_affected_by("/templates/a.html", "same");
        assert!(affected.is_empty());
    }

    #[test]
    fn test_outputs_affected_by_returns_empty_when_file_not_tracked() {
        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/in/doc.md", "hash1")]);

        let affected = map.outputs_affected_by("/unrelated/file.txt", "some_hash");
        assert!(affected.is_empty());
    }

    #[test]
    fn test_multiple_outputs_affected_by_shared_template() {
        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/templates/shared.html", "old")]);
        map.record("/out/report.html".to_string(), vec![dep("/templates/shared.html", "old")]);
        map.record("/out/other.pdf".to_string(), vec![dep("/in/doc.md", "hash1")]);

        let mut affected = map.outputs_affected_by("/templates/shared.html", "new");
        affected.sort();
        assert_eq!(affected, vec!["/out/doc.html".to_string(), "/out/report.html".to_string()]);
    }

    // ── hash_file ─────────────────────────────────────────────────────────────

    #[test]
    fn test_hash_file_returns_hex_sha256() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"hello world").unwrap();
        let hash = hash_file(f.path()).expect("hash_file should succeed");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_hash_file_same_content_stable() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"stable content").unwrap();
        let h1 = hash_file(f.path()).unwrap();
        let h2 = hash_file(f.path()).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_file_different_content_differs() {
        let mut f1 = NamedTempFile::new().unwrap();
        f1.write_all(b"content A").unwrap();
        let mut f2 = NamedTempFile::new().unwrap();
        f2.write_all(b"content B").unwrap();
        assert_ne!(hash_file(f1.path()).unwrap(), hash_file(f2.path()).unwrap());
    }

    #[test]
    fn test_hash_file_missing_returns_error() {
        let result = hash_file(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
    }

    // ── build_output_dependencies ─────────────────────────────────────────────

    #[test]
    fn test_build_output_dependencies_includes_input_and_config() {
        let mut input = NamedTempFile::new().unwrap();
        input.write_all(b"# Hello").unwrap();
        let mut cfg = NamedTempFile::new().unwrap();
        cfg.write_all(b"outputs:\n  - type: html\n").unwrap();

        let deps = build_output_dependencies(input.path(), cfg.path(), None);
        assert_eq!(deps.len(), 2);
        assert_eq!(deps[0].path, input.path().to_string_lossy());
        assert_eq!(deps[1].path, cfg.path().to_string_lossy());
    }

    #[test]
    fn test_build_output_dependencies_includes_template_when_provided() {
        let mut input = NamedTempFile::new().unwrap();
        input.write_all(b"# Hello").unwrap();
        let mut cfg = NamedTempFile::new().unwrap();
        cfg.write_all(b"outputs:\n  - type: html\n").unwrap();
        let mut tmpl = NamedTempFile::new().unwrap();
        tmpl.write_all(b"<html>{{body}}</html>").unwrap();

        let deps = build_output_dependencies(input.path(), cfg.path(), Some(tmpl.path()));
        assert_eq!(deps.len(), 3);
        assert_eq!(deps[2].path, tmpl.path().to_string_lossy());
    }

    #[test]
    fn test_build_output_dependencies_skips_missing_template_gracefully() {
        let mut input = NamedTempFile::new().unwrap();
        input.write_all(b"# Hello").unwrap();
        let mut cfg = NamedTempFile::new().unwrap();
        cfg.write_all(b"outputs:\n  - type: html\n").unwrap();
        let missing = Path::new("/nonexistent/template.html");

        // Should not panic; the missing template is silently omitted.
        let deps = build_output_dependencies(input.path(), cfg.path(), Some(missing));
        assert_eq!(deps.len(), 2);
    }

    // ── load_dependency_map / save_dependency_map ────────────────────────────

    #[test]
    fn test_load_dependency_map_missing_file_returns_empty() {
        let path = Path::new("/nonexistent/.renderflow-deps.json");
        let map = load_dependency_map(path);
        assert!(map.dependencies_for("/any/output").is_none());
    }

    #[test]
    fn test_save_and_reload_dependency_map_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".renderflow-deps.json");

        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/in/doc.md", "h1"), dep("/cfg/build.yaml", "h2")]);
        map.record("/out/doc.pdf".to_string(), vec![dep("/in/doc.md", "h1")]);

        save_dependency_map(&map, &path).expect("save should succeed");

        let reloaded = load_dependency_map(&path);
        let html_deps = reloaded.dependencies_for("/out/doc.html").unwrap();
        assert_eq!(html_deps.len(), 2);
        assert_eq!(html_deps[0].hash, "h1");

        let pdf_deps = reloaded.dependencies_for("/out/doc.pdf").unwrap();
        assert_eq!(pdf_deps.len(), 1);
    }

    #[test]
    fn test_load_dependency_map_invalid_json_returns_empty() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"not valid json {{").unwrap();
        let map = load_dependency_map(f.path());
        assert!(map.dependencies_for("/any/output").is_none());
    }

    #[test]
    fn test_save_dependency_map_writes_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".renderflow-deps.json");

        let mut map = DependencyMap::default();
        map.record("/out/doc.html".to_string(), vec![dep("/in/doc.md", "abc")]);
        save_dependency_map(&map, &path).expect("save should succeed");

        let raw = fs::read_to_string(&path).expect("read failed");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("must be valid JSON");
        // The JSON should contain the output path as a key.
        assert!(parsed.get("/out/doc.html").is_some());
    }
}
