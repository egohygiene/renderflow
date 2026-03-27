use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::warn;

// ── OutputCache ──────────────────────────────────────────────────────────────

/// On-disk representation of the output cache.
///
/// Keys are output file paths; values are hex-encoded SHA-256 hashes of the
/// render inputs (transformed content + output type + template) that produced
/// them.  A cache hit means the output file is already up-to-date and pandoc
/// can be skipped.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct OutputCache(HashMap<String, String>);

impl OutputCache {
    /// Look up the stored render-input hash for a given output file path.
    pub fn get(&self, output_path: &str) -> Option<&str> {
        self.0.get(output_path).map(String::as_str)
    }

    /// Record that `output_path` was produced from the inputs identified by
    /// `hash`.
    pub fn insert(&mut self, output_path: String, hash: String) {
        self.0.insert(output_path, hash);
    }
}

/// Compute a stable SHA-256 hash of the render inputs for one output.
///
/// The hash covers:
/// * transformed document content (post-transform pipeline)
/// * output type string (e.g. `"html"`, `"pdf"`, `"docx"`)
/// * optional template name (empty string when absent)
///
/// A change to any of these fields produces a different hash, causing a cache
/// miss and triggering a fresh pandoc run.
pub fn compute_output_hash(transformed_content: &str, output_type: &str, template: Option<&str>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(transformed_content.as_bytes());
    hasher.update(b"\x00output-type\x00");
    hasher.update(output_type.as_bytes());
    hasher.update(b"\x00template\x00");
    hasher.update(template.unwrap_or("").as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Load the output cache from disk.
///
/// Returns an empty cache if the file does not exist or cannot be parsed.
/// Non-fatal errors are logged at `WARN` level.
pub fn load_output_cache(cache_path: &Path) -> OutputCache {
    if !cache_path.exists() {
        return OutputCache::default();
    }

    match fs::read_to_string(cache_path) {
        Err(e) => {
            warn!(
                path = %cache_path.display(),
                error = %e,
                "Failed to read output cache file; starting with empty cache"
            );
            OutputCache::default()
        }
        Ok(content) => match serde_json::from_str(&content) {
            Ok(cache) => cache,
            Err(e) => {
                warn!(
                    path = %cache_path.display(),
                    error = %e,
                    "Failed to parse output cache file; starting with empty cache"
                );
                OutputCache::default()
            }
        },
    }
}

/// Persist the output cache to disk as pretty-printed JSON.
///
/// Errors are propagated to the caller.
pub fn save_output_cache(cache: &OutputCache, cache_path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(cache)?;
    fs::write(cache_path, json)?;
    Ok(())
}

/// On-disk representation of the transform cache.
///
/// Keys are hex-encoded SHA-256 hashes of the transform inputs (file content
/// plus variables).  Values are the fully-transformed document content that
/// would be produced by the transform pipeline for that combination of inputs.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct TransformCache(HashMap<String, String>);

impl TransformCache {
    /// Look up a previously-cached transform result by its input hash.
    pub fn get(&self, hash: &str) -> Option<&str> {
        self.0.get(hash).map(String::as_str)
    }

    /// Store a transform result keyed by its input hash.
    pub fn insert(&mut self, hash: String, transformed: String) {
        self.0.insert(hash, transformed);
    }
}

/// Compute a stable SHA-256 hash of the transform inputs.
///
/// The hash covers:
/// * normalized input file content
/// * variables (sorted for determinism, regardless of map insertion order)
///
/// This hash uniquely identifies a combination of inputs so that a cache hit
/// guarantees the transform pipeline would produce the same output.
pub fn compute_input_hash(content: &str, variables: &HashMap<String, String>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());

    // Sort entries so the hash is stable regardless of HashMap iteration order.
    let mut sorted: Vec<(&String, &String)> = variables.iter().collect();
    sorted.sort_by_key(|(k, _)| k.as_str());
    for (k, v) in sorted {
        hasher.update(k.as_bytes());
        hasher.update(b"=");
        hasher.update(v.as_bytes());
        hasher.update(b"\n");
    }

    format!("{:x}", hasher.finalize())
}

/// Load the transform cache from disk.
///
/// Returns an empty cache if the file does not exist or cannot be parsed.
/// Non-fatal errors are logged at `WARN` level so a corrupt or missing cache
/// never aborts the build.
pub fn load_cache(cache_path: &Path) -> TransformCache {
    if !cache_path.exists() {
        return TransformCache::default();
    }

    match fs::read_to_string(cache_path) {
        Err(e) => {
            warn!(
                path = %cache_path.display(),
                error = %e,
                "Failed to read cache file; starting with empty cache"
            );
            TransformCache::default()
        }
        Ok(content) => match serde_json::from_str(&content) {
            Ok(cache) => cache,
            Err(e) => {
                warn!(
                    path = %cache_path.display(),
                    error = %e,
                    "Failed to parse cache file; starting with empty cache"
                );
                TransformCache::default()
            }
        },
    }
}

/// Persist the transform cache to disk.
///
/// The cache is written as pretty-printed JSON so it is human-readable and
/// diff-friendly.  Errors are propagated to the caller.
pub fn save_cache(cache: &TransformCache, cache_path: &Path) -> Result<()> {
    let json = serde_json::to_string_pretty(cache)?;
    fs::write(cache_path, json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // ── compute_input_hash ───────────────────────────────────────────────────

    #[test]
    fn test_same_inputs_produce_same_hash() {
        let h1 = compute_input_hash("hello world", &vars(&[("key", "val")]));
        let h2 = compute_input_hash("hello world", &vars(&[("key", "val")]));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_different_content_produces_different_hash() {
        let h1 = compute_input_hash("content A", &vars(&[]));
        let h2 = compute_input_hash("content B", &vars(&[]));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_different_variables_produce_different_hash() {
        let h1 = compute_input_hash("same content", &vars(&[("k", "v1")]));
        let h2 = compute_input_hash("same content", &vars(&[("k", "v2")]));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_variable_order_does_not_affect_hash() {
        let h1 = compute_input_hash("content", &vars(&[("a", "1"), ("b", "2")]));
        let h2 = compute_input_hash("content", &vars(&[("b", "2"), ("a", "1")]));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_empty_variables_produces_stable_hash() {
        let h1 = compute_input_hash("content", &vars(&[]));
        let h2 = compute_input_hash("content", &vars(&[]));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_is_hex_string() {
        let h = compute_input_hash("test", &vars(&[]));
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash must be hex: {h}");
        assert_eq!(h.len(), 64, "SHA-256 hex must be 64 chars");
    }

    // ── TransformCache ───────────────────────────────────────────────────────

    #[test]
    fn test_cache_miss_returns_none() {
        let cache = TransformCache::default();
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_cache_hit_returns_stored_value() {
        let mut cache = TransformCache::default();
        cache.insert("abc123".to_string(), "transformed content".to_string());
        assert_eq!(cache.get("abc123"), Some("transformed content"));
    }

    #[test]
    fn test_cache_insert_overwrites_existing() {
        let mut cache = TransformCache::default();
        cache.insert("key".to_string(), "first".to_string());
        cache.insert("key".to_string(), "second".to_string());
        assert_eq!(cache.get("key"), Some("second"));
    }

    // ── load_cache / save_cache ──────────────────────────────────────────────

    #[test]
    fn test_load_cache_missing_file_returns_empty() {
        let path = Path::new("/nonexistent/.renderflow-cache.json");
        let cache = load_cache(path);
        assert!(cache.get("anything").is_none());
    }

    #[test]
    fn test_save_and_reload_cache_round_trips() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let cache_path = dir.path().join(".renderflow-cache.json");

        let mut cache = TransformCache::default();
        cache.insert("hash1".to_string(), "result1".to_string());
        cache.insert("hash2".to_string(), "result2".to_string());

        save_cache(&cache, &cache_path).expect("save should succeed");

        let reloaded = load_cache(&cache_path);
        assert_eq!(reloaded.get("hash1"), Some("result1"));
        assert_eq!(reloaded.get("hash2"), Some("result2"));
    }

    #[test]
    fn test_load_cache_invalid_json_returns_empty() {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(b"not valid json {{").expect("write failed");
        let cache = load_cache(f.path());
        assert!(cache.get("anything").is_none());
    }

    #[test]
    fn test_save_cache_writes_valid_json() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let cache_path = dir.path().join(".renderflow-cache.json");

        let mut cache = TransformCache::default();
        cache.insert("testhash".to_string(), "testcontent".to_string());
        save_cache(&cache, &cache_path).expect("save should succeed");

        let raw = fs::read_to_string(&cache_path).expect("read failed");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("must be valid JSON");
        assert_eq!(parsed["testhash"], "testcontent");
    }

    // ── compute_output_hash ──────────────────────────────────────────────────

    #[test]
    fn test_output_hash_same_inputs_stable() {
        let h1 = compute_output_hash("content", "html", None);
        let h2 = compute_output_hash("content", "html", None);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_output_hash_different_output_type_differs() {
        let h1 = compute_output_hash("content", "html", None);
        let h2 = compute_output_hash("content", "pdf", None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_output_hash_different_template_differs() {
        let h1 = compute_output_hash("content", "html", Some("a.html"));
        let h2 = compute_output_hash("content", "html", Some("b.html"));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_output_hash_no_template_differs_from_with_template() {
        let h1 = compute_output_hash("content", "html", None);
        let h2 = compute_output_hash("content", "html", Some("tmpl.html"));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_output_hash_different_content_differs() {
        let h1 = compute_output_hash("content A", "html", None);
        let h2 = compute_output_hash("content B", "html", None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_output_hash_is_hex_string() {
        let h = compute_output_hash("content", "html", None);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash must be hex: {h}");
        assert_eq!(h.len(), 64, "SHA-256 hex must be 64 chars");
    }

    // ── OutputCache ──────────────────────────────────────────────────────────

    #[test]
    fn test_output_cache_miss_returns_none() {
        let cache = OutputCache::default();
        assert!(cache.get("/tmp/output.html").is_none());
    }

    #[test]
    fn test_output_cache_hit_returns_stored_value() {
        let mut cache = OutputCache::default();
        cache.insert("/tmp/output.html".to_string(), "abc123".to_string());
        assert_eq!(cache.get("/tmp/output.html"), Some("abc123"));
    }

    #[test]
    fn test_output_cache_insert_overwrites_existing() {
        let mut cache = OutputCache::default();
        cache.insert("/tmp/output.html".to_string(), "old_hash".to_string());
        cache.insert("/tmp/output.html".to_string(), "new_hash".to_string());
        assert_eq!(cache.get("/tmp/output.html"), Some("new_hash"));
    }

    // ── load_output_cache / save_output_cache ────────────────────────────────

    #[test]
    fn test_load_output_cache_missing_file_returns_empty() {
        let path = Path::new("/nonexistent/.renderflow-output-cache.json");
        let cache = load_output_cache(path);
        assert!(cache.get("/any/path").is_none());
    }

    #[test]
    fn test_save_and_reload_output_cache_round_trips() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let cache_path = dir.path().join(".renderflow-output-cache.json");

        let mut cache = OutputCache::default();
        cache.insert("/out/doc.html".to_string(), "hash1".to_string());
        cache.insert("/out/doc.pdf".to_string(), "hash2".to_string());

        save_output_cache(&cache, &cache_path).expect("save should succeed");

        let reloaded = load_output_cache(&cache_path);
        assert_eq!(reloaded.get("/out/doc.html"), Some("hash1"));
        assert_eq!(reloaded.get("/out/doc.pdf"), Some("hash2"));
    }

    #[test]
    fn test_load_output_cache_invalid_json_returns_empty() {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(b"not valid json {{").expect("write failed");
        let cache = load_output_cache(f.path());
        assert!(cache.get("/any/path").is_none());
    }

    #[test]
    fn test_save_output_cache_writes_valid_json() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let cache_path = dir.path().join(".renderflow-output-cache.json");

        let mut cache = OutputCache::default();
        cache.insert("/out/doc.html".to_string(), "testhash".to_string());
        save_output_cache(&cache, &cache_path).expect("save should succeed");

        let raw = fs::read_to_string(&cache_path).expect("read failed");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("must be valid JSON");
        assert_eq!(parsed["/out/doc.html"], "testhash");
    }
}
