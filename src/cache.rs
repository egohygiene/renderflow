use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

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
/// * optional template file content (empty string when absent or unreadable)
///
/// A change to any of these fields produces a different hash, causing a cache
/// miss and triggering a fresh pandoc run.  Including the template file content
/// (not just its name) means that editing a template file invalidates cached
/// render outputs even when the template path has not changed.
pub fn compute_output_hash(
    transformed_content: &str,
    output_type: &str,
    template: Option<&str>,
    template_content: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(transformed_content.as_bytes());
    hasher.update(b"\x00output-type\x00");
    hasher.update(output_type.as_bytes());
    hasher.update(b"\x00template\x00");
    hasher.update(template.unwrap_or("").as_bytes());
    hasher.update(b"\x00template-content\x00");
    hasher.update(template_content.unwrap_or("").as_bytes());
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

/// Persist the output cache to disk as compact JSON.
///
/// Errors are propagated to the caller.
pub fn save_output_cache(cache: &OutputCache, cache_path: &Path) -> Result<()> {
    let json = serde_json::to_string(cache)?;
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
/// * config file content (raw YAML bytes)
/// * variables (sorted for determinism, regardless of map insertion order)
///
/// This hash uniquely identifies a combination of inputs so that a cache hit
/// guarantees the transform pipeline would produce the same output.  Including
/// the raw config file content means that any change to the config (not only
/// to `variables`) invalidates the transform cache.
pub fn compute_input_hash(
    content: &str,
    config_content: &str,
    variables: &HashMap<String, String>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    hasher.update(b"\x00config\x00");
    hasher.update(config_content.as_bytes());

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

/// Compute a stable SHA-256 hash for a single DAG node execution.
///
/// The hash covers:
/// * the input content being transformed
/// * the source format identifier (e.g. `"markdown"`)
/// * the target format identifier (e.g. `"html"`)
///
/// Including both format strings ensures that the same input content
/// transformed to two different target formats receives distinct cache keys.
pub fn compute_dag_node_hash(input: &str, from: &str, to: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.update(b"\x00from\x00");
    hasher.update(from.as_bytes());
    hasher.update(b"\x00to\x00");
    hasher.update(to.as_bytes());
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
/// The cache is written as compact JSON to minimize file size and serialization
/// overhead.  Errors are propagated to the caller.
pub fn save_cache(cache: &TransformCache, cache_path: &Path) -> Result<()> {
    let json = serde_json::to_string(cache)?;
    fs::write(cache_path, json)?;
    Ok(())
}

// ── AiCache ──────────────────────────────────────────────────────────────────

/// Metadata and output for a single cached AI-transform result.
///
/// Each entry records the AI model used, a Unix-epoch timestamp (seconds) of
/// when the result was generated, the hex-encoded SHA-256 hash of the inputs
/// that produced it, and the generated output text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCacheEntry {
    /// Hex-encoded SHA-256 hash of the AI transform inputs (rendered prompt + model).
    pub input_hash: String,
    /// AI model identifier used to generate this output (e.g. `"mistral"`, `"gpt-4o"`).
    pub model: String,
    /// Unix epoch timestamp (seconds) of when this entry was generated.
    pub timestamp: u64,
    /// The AI-generated output text.
    pub output: String,
}

/// On-disk cache for AI-transform results.
///
/// Keys are hex-encoded SHA-256 hashes produced by [`compute_ai_input_hash`].
/// Each value is an [`AiCacheEntry`] that carries the cached output along with
/// generation metadata (model, timestamp, input hash).  A cache hit means the
/// AI backend can be skipped entirely for that input.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AiCache(HashMap<String, AiCacheEntry>);

impl AiCache {
    /// Look up a previously-cached AI result by its input hash.
    pub fn get(&self, hash: &str) -> Option<&AiCacheEntry> {
        self.0.get(hash)
    }

    /// Store an AI result keyed by the input hash embedded in `entry`.
    pub fn insert(&mut self, hash: String, entry: AiCacheEntry) {
        self.0.insert(hash, entry);
    }
}

/// Compute a stable SHA-256 hash of the AI transform inputs.
///
/// The hash covers:
/// * the fully-rendered prompt (prompt template with `{input}` substituted)
/// * the model identifier
///
/// A change to either field produces a different hash, causing a cache miss
/// and triggering a fresh AI call.
pub fn compute_ai_input_hash(prompt: &str, model: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(prompt.as_bytes());
    hasher.update(b"\x00model\x00");
    hasher.update(model.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Return the current time as a Unix epoch timestamp in seconds.
///
/// Falls back to `0` if the system clock is before the Unix epoch.
pub fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Load the AI cache from disk.
///
/// Returns an empty cache if the file does not exist or cannot be parsed.
/// Non-fatal errors are logged at `WARN` level so a corrupt or missing cache
/// never aborts the build.
pub fn load_ai_cache(cache_path: &Path) -> AiCache {
    if !cache_path.exists() {
        return AiCache::default();
    }

    match fs::read_to_string(cache_path) {
        Err(e) => {
            warn!(
                path = %cache_path.display(),
                error = %e,
                "Failed to read AI cache file; starting with empty cache"
            );
            AiCache::default()
        }
        Ok(content) => match serde_json::from_str(&content) {
            Ok(cache) => cache,
            Err(e) => {
                warn!(
                    path = %cache_path.display(),
                    error = %e,
                    "Failed to parse AI cache file; starting with empty cache"
                );
                AiCache::default()
            }
        },
    }
}

/// Persist the AI cache to disk as compact JSON.
///
/// Errors are propagated to the caller.
pub fn save_ai_cache(cache: &AiCache, cache_path: &Path) -> Result<()> {
    let json = serde_json::to_string(cache)?;
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
        let h1 = compute_input_hash("hello world", "", &vars(&[("key", "val")]));
        let h2 = compute_input_hash("hello world", "", &vars(&[("key", "val")]));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_different_content_produces_different_hash() {
        let h1 = compute_input_hash("content A", "", &vars(&[]));
        let h2 = compute_input_hash("content B", "", &vars(&[]));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_different_variables_produce_different_hash() {
        let h1 = compute_input_hash("same content", "", &vars(&[("k", "v1")]));
        let h2 = compute_input_hash("same content", "", &vars(&[("k", "v2")]));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_variable_order_does_not_affect_hash() {
        let h1 = compute_input_hash("content", "", &vars(&[("a", "1"), ("b", "2")]));
        let h2 = compute_input_hash("content", "", &vars(&[("b", "2"), ("a", "1")]));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_empty_variables_produces_stable_hash() {
        let h1 = compute_input_hash("content", "", &vars(&[]));
        let h2 = compute_input_hash("content", "", &vars(&[]));
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_is_hex_string() {
        let h = compute_input_hash("test", "", &vars(&[]));
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash must be hex: {h}");
        assert_eq!(h.len(), 64, "SHA-256 hex must be 64 chars");
    }

    #[test]
    fn test_different_config_content_produces_different_hash() {
        let h1 = compute_input_hash("same content", "config: a", &vars(&[]));
        let h2 = compute_input_hash("same content", "config: b", &vars(&[]));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_empty_config_content_differs_from_nonempty() {
        let h1 = compute_input_hash("content", "", &vars(&[]));
        let h2 = compute_input_hash("content", "outputs:\n  - type: html\n", &vars(&[]));
        assert_ne!(h1, h2);
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
        let h1 = compute_output_hash("content", "html", None, None);
        let h2 = compute_output_hash("content", "html", None, None);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_output_hash_different_output_type_differs() {
        let h1 = compute_output_hash("content", "html", None, None);
        let h2 = compute_output_hash("content", "pdf", None, None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_output_hash_different_template_differs() {
        let h1 = compute_output_hash("content", "html", Some("a.html"), None);
        let h2 = compute_output_hash("content", "html", Some("b.html"), None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_output_hash_no_template_differs_from_with_template() {
        let h1 = compute_output_hash("content", "html", None, None);
        let h2 = compute_output_hash("content", "html", Some("tmpl.html"), None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_output_hash_different_content_differs() {
        let h1 = compute_output_hash("content A", "html", None, None);
        let h2 = compute_output_hash("content B", "html", None, None);
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_output_hash_is_hex_string() {
        let h = compute_output_hash("content", "html", None, None);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash must be hex: {h}");
        assert_eq!(h.len(), 64, "SHA-256 hex must be 64 chars");
    }

    #[test]
    fn test_output_hash_different_template_content_differs() {
        let h1 = compute_output_hash("content", "html", Some("t.html"), Some("<html>v1</html>"));
        let h2 = compute_output_hash("content", "html", Some("t.html"), Some("<html>v2</html>"));
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_output_hash_no_template_content_differs_from_some() {
        let h1 = compute_output_hash("content", "html", Some("t.html"), None);
        let h2 = compute_output_hash("content", "html", Some("t.html"), Some("<html></html>"));
        assert_ne!(h1, h2);
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

    // ── compute_ai_input_hash ────────────────────────────────────────────────

    #[test]
    fn test_ai_hash_same_inputs_produce_same_hash() {
        let h1 = compute_ai_input_hash("Summarise: hello", "mistral");
        let h2 = compute_ai_input_hash("Summarise: hello", "mistral");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_ai_hash_different_prompt_produces_different_hash() {
        let h1 = compute_ai_input_hash("prompt A", "mistral");
        let h2 = compute_ai_input_hash("prompt B", "mistral");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_ai_hash_different_model_produces_different_hash() {
        let h1 = compute_ai_input_hash("same prompt", "mistral");
        let h2 = compute_ai_input_hash("same prompt", "gpt-4o");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_ai_hash_is_hex_string() {
        let h = compute_ai_input_hash("prompt", "model");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash must be hex: {h}");
        assert_eq!(h.len(), 64, "SHA-256 hex must be 64 chars");
    }

    // ── AiCache ──────────────────────────────────────────────────────────────

    fn make_entry(hash: &str, model: &str, output: &str) -> AiCacheEntry {
        AiCacheEntry {
            input_hash: hash.to_string(),
            model: model.to_string(),
            timestamp: 1_700_000_000,
            output: output.to_string(),
        }
    }

    #[test]
    fn test_ai_cache_miss_returns_none() {
        let cache = AiCache::default();
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_ai_cache_hit_returns_stored_entry() {
        let mut cache = AiCache::default();
        let entry = make_entry("abc123", "mistral", "AI output");
        cache.insert("abc123".to_string(), entry);
        let retrieved = cache.get("abc123").expect("entry should be present");
        assert_eq!(retrieved.output, "AI output");
        assert_eq!(retrieved.model, "mistral");
        assert_eq!(retrieved.input_hash, "abc123");
    }

    #[test]
    fn test_ai_cache_insert_overwrites_existing() {
        let mut cache = AiCache::default();
        cache.insert("key".to_string(), make_entry("key", "mistral", "first"));
        cache.insert("key".to_string(), make_entry("key", "mistral", "second"));
        assert_eq!(cache.get("key").unwrap().output, "second");
    }

    // ── load_ai_cache / save_ai_cache ────────────────────────────────────────

    #[test]
    fn test_load_ai_cache_missing_file_returns_empty() {
        let path = Path::new("/nonexistent/.renderflow-ai-cache.json");
        let cache = load_ai_cache(path);
        assert!(cache.get("anything").is_none());
    }

    #[test]
    fn test_save_and_reload_ai_cache_round_trips() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let cache_path = dir.path().join(".renderflow-ai-cache.json");

        let mut cache = AiCache::default();
        cache.insert("hash1".to_string(), make_entry("hash1", "mistral", "output1"));
        cache.insert("hash2".to_string(), make_entry("hash2", "gpt-4o", "output2"));

        save_ai_cache(&cache, &cache_path).expect("save should succeed");

        let reloaded = load_ai_cache(&cache_path);
        let e1 = reloaded.get("hash1").expect("entry 1 should be present");
        assert_eq!(e1.output, "output1");
        assert_eq!(e1.model, "mistral");
        let e2 = reloaded.get("hash2").expect("entry 2 should be present");
        assert_eq!(e2.output, "output2");
        assert_eq!(e2.model, "gpt-4o");
    }

    #[test]
    fn test_load_ai_cache_invalid_json_returns_empty() {
        let mut f = NamedTempFile::new().expect("failed to create temp file");
        f.write_all(b"not valid json {{").expect("write failed");
        let cache = load_ai_cache(f.path());
        assert!(cache.get("anything").is_none());
    }

    #[test]
    fn test_save_ai_cache_writes_valid_json() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let cache_path = dir.path().join(".renderflow-ai-cache.json");

        let mut cache = AiCache::default();
        cache.insert("testhash".to_string(), make_entry("testhash", "mistral", "testoutput"));
        save_ai_cache(&cache, &cache_path).expect("save should succeed");

        let raw = fs::read_to_string(&cache_path).expect("read failed");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("must be valid JSON");
        assert_eq!(parsed["testhash"]["model"], "mistral");
        assert_eq!(parsed["testhash"]["output"], "testoutput");
        assert_eq!(parsed["testhash"]["input_hash"], "testhash");
    }

    #[test]
    fn test_ai_cache_entry_stores_metadata() {
        let entry = AiCacheEntry {
            input_hash: "deadbeef".to_string(),
            model: "llava".to_string(),
            timestamp: 1_234_567_890,
            output: "generated text".to_string(),
        };
        assert_eq!(entry.input_hash, "deadbeef");
        assert_eq!(entry.model, "llava");
        assert_eq!(entry.timestamp, 1_234_567_890);
        assert_eq!(entry.output, "generated text");
    }

    #[test]
    fn test_current_unix_timestamp_is_reasonable() {
        // The timestamp should be after 2020-01-01 (Unix epoch 1577836800).
        let ts = current_unix_timestamp();
        assert!(ts > 1_577_836_800, "timestamp {ts} is before 2020");
    }

    // ── compute_dag_node_hash ────────────────────────────────────────────────

    #[test]
    fn test_dag_node_hash_same_inputs_stable() {
        let h1 = compute_dag_node_hash("content", "markdown", "html");
        let h2 = compute_dag_node_hash("content", "markdown", "html");
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_dag_node_hash_different_content_differs() {
        let h1 = compute_dag_node_hash("content A", "markdown", "html");
        let h2 = compute_dag_node_hash("content B", "markdown", "html");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_dag_node_hash_different_from_format_differs() {
        let h1 = compute_dag_node_hash("content", "markdown", "html");
        let h2 = compute_dag_node_hash("content", "rst", "html");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_dag_node_hash_different_to_format_differs() {
        let h1 = compute_dag_node_hash("content", "markdown", "html");
        let h2 = compute_dag_node_hash("content", "markdown", "pdf");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_dag_node_hash_is_hex_string() {
        let h = compute_dag_node_hash("test", "markdown", "html");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash must be hex: {h}");
        assert_eq!(h.len(), 64, "SHA-256 hex must be 64 chars");
    }
}
