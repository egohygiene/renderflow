use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Resolves an asset path relative to `base_dir`.
///
/// If `asset_path` is absolute it is validated as-is. If it is relative it is
/// joined to `base_dir`. Returns the canonical (absolute) path on success, or
/// an error if the file does not exist.
pub fn resolve_asset_path(base_dir: &Path, asset_path: &str) -> Result<PathBuf> {
    let path = Path::new(asset_path);
    let resolved = if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    };
    if !resolved.exists() {
        anyhow::bail!("Asset not found: {}", resolved.display());
    }
    resolved
        .canonicalize()
        .with_context(|| format!("Failed to resolve asset path: {}", asset_path))
}

/// Parses Markdown `content` for image references (`![alt](path)`), resolves
/// every local path relative to `base_dir`, validates that each asset exists,
/// and returns the content with all local paths replaced by their canonical
/// absolute equivalents.
///
/// URL-based paths (e.g. `http://`, `https://`, `ftp://`, `data:`) are left
/// unchanged. Paths that cannot be found on disk cause an immediate error.
pub fn normalize_asset_paths(content: &str, base_dir: &Path) -> Result<String> {
    let mut result = String::with_capacity(content.len());
    let mut remaining = content;

    while let Some(img_start) = remaining.find("![") {
        // Append everything before this image reference unchanged.
        result.push_str(&remaining[..img_start]);
        remaining = &remaining[img_start..];

        // Locate the closing bracket of the alt text.
        let close_bracket = match remaining.find(']') {
            Some(pos) => pos,
            None => {
                // No closing bracket — not a valid reference; keep remainder as-is.
                result.push_str(remaining);
                return Ok(result);
            }
        };

        // Ensure the bracket is followed by '(' to form an inline image link.
        let after_bracket = &remaining[close_bracket + 1..];
        if !after_bracket.starts_with('(') {
            // Not an inline link — advance past the bracket and continue.
            result.push_str(&remaining[..close_bracket + 1]);
            remaining = &remaining[close_bracket + 1..];
            continue;
        }

        // Find the closing ')' that ends the path/URL.
        let path_area = &remaining[close_bracket + 2..];
        let close_paren = match path_area.find(')') {
            Some(pos) => pos,
            None => {
                // No closing paren — keep remainder as-is.
                result.push_str(remaining);
                return Ok(result);
            }
        };

        let raw_path = &path_area[..close_paren];

        // Separate the path from an optional title (`"title"` or `'title'`).
        let (path_str, title_part) = split_path_and_title(raw_path);

        // Leave URL schemes untouched.
        if is_url(path_str) {
            let full_ref_len = close_bracket + 2 + close_paren + 1;
            result.push_str(&remaining[..full_ref_len]);
        } else {
            // Resolve and validate the local path.
            let canonical = resolve_asset_path(base_dir, path_str)?;
            let alt_and_bracket = &remaining[..close_bracket + 1];
            result.push_str(alt_and_bracket);
            result.push('(');
            result.push_str(&canonical.to_string_lossy());
            if !title_part.is_empty() {
                result.push(' ');
                result.push_str(title_part);
            }
            result.push(')');
        }

        remaining = &remaining[close_bracket + 2 + close_paren + 1..];
    }

    result.push_str(remaining);
    Ok(result)
}

/// Returns `true` when `path` begins with a URL scheme that should be left
/// untouched (not resolved against the filesystem).
fn is_url(path: &str) -> bool {
    path.starts_with("http://")
        || path.starts_with("https://")
        || path.starts_with("ftp://")
        || path.starts_with("data:")
}

/// Splits a raw path string into `(path, title)` where `title` may include a
/// trailing `"title"` or `'title'` attribute written after whitespace.
fn split_path_and_title(raw: &str) -> (&str, &str) {
    if let Some(pos) = raw.find('"') {
        (raw[..pos].trim_end(), &raw[pos..])
    } else if let Some(pos) = raw.find('\'') {
        (raw[..pos].trim_end(), &raw[pos..])
    } else {
        (raw.trim(), "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_asset(dir: &TempDir, name: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, b"").expect("failed to create asset");
        path
    }

    // ── resolve_asset_path ───────────────────────────────────────────────────

    #[test]
    fn test_resolve_relative_path_success() {
        let dir = TempDir::new().unwrap();
        make_asset(&dir, "image.png");
        let result = resolve_asset_path(dir.path(), "image.png");
        assert!(result.is_ok(), "expected Ok for existing relative asset");
        let canonical = result.unwrap();
        assert!(canonical.is_absolute(), "resolved path must be absolute");
        assert!(canonical.exists());
    }

    #[test]
    fn test_resolve_absolute_path_success() {
        let dir = TempDir::new().unwrap();
        let asset = make_asset(&dir, "photo.jpg");
        let result = resolve_asset_path(dir.path(), asset.to_str().unwrap());
        assert!(result.is_ok(), "expected Ok for existing absolute asset");
        assert!(result.unwrap().is_absolute());
    }

    #[test]
    fn test_resolve_missing_asset_returns_error() {
        let dir = TempDir::new().unwrap();
        let result = resolve_asset_path(dir.path(), "nonexistent.png");
        assert!(result.is_err(), "expected error for missing asset");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Asset not found"), "unexpected error: {}", msg);
    }

    #[test]
    fn test_resolve_nested_relative_path() {
        let dir = TempDir::new().unwrap();
        fs::create_dir(dir.path().join("assets")).unwrap();
        fs::write(dir.path().join("assets").join("logo.svg"), b"").unwrap();
        let result = resolve_asset_path(dir.path(), "assets/logo.svg");
        assert!(result.is_ok(), "expected Ok for nested relative path");
    }

    // ── normalize_asset_paths ────────────────────────────────────────────────

    #[test]
    fn test_normalize_resolves_relative_image() {
        let dir = TempDir::new().unwrap();
        make_asset(&dir, "photo.png");
        let content = "# Doc\n\n![A photo](photo.png)\n";
        let normalized = normalize_asset_paths(content, dir.path()).unwrap();
        // The resolved path must be absolute and point to the real file.
        assert!(
            normalized.contains(&dir.path().to_string_lossy().to_string()),
            "expected absolute path in output: {}",
            normalized
        );
        assert!(normalized.contains("photo.png"));
    }

    #[test]
    fn test_normalize_missing_asset_returns_error() {
        let dir = TempDir::new().unwrap();
        let content = "![Missing](missing.png)";
        let result = normalize_asset_paths(content, dir.path());
        assert!(result.is_err(), "expected error for missing asset");
        let msg = format!("{}", result.unwrap_err());
        assert!(msg.contains("Asset not found"), "unexpected error: {}", msg);
    }

    #[test]
    fn test_normalize_leaves_urls_unchanged() {
        let dir = TempDir::new().unwrap();
        let content = "![Remote](https://example.com/image.png)";
        let normalized = normalize_asset_paths(content, dir.path()).unwrap();
        assert_eq!(normalized, content, "URL-based image should not be modified");
    }

    #[test]
    fn test_normalize_leaves_http_url_unchanged() {
        let dir = TempDir::new().unwrap();
        let content = "![Logo](http://example.com/logo.png)";
        let normalized = normalize_asset_paths(content, dir.path()).unwrap();
        assert_eq!(normalized, content);
    }

    #[test]
    fn test_normalize_leaves_data_url_unchanged() {
        let dir = TempDir::new().unwrap();
        let content = "![Icon](data:image/png;base64,abc==)";
        let normalized = normalize_asset_paths(content, dir.path()).unwrap();
        assert_eq!(normalized, content);
    }

    #[test]
    fn test_normalize_preserves_plain_text() {
        let dir = TempDir::new().unwrap();
        let content = "No images here, just text.";
        let normalized = normalize_asset_paths(content, dir.path()).unwrap();
        assert_eq!(normalized, content);
    }

    #[test]
    fn test_normalize_multiple_images() {
        let dir = TempDir::new().unwrap();
        make_asset(&dir, "a.png");
        make_asset(&dir, "b.png");
        let content = "![A](a.png) and ![B](b.png)";
        let normalized = normalize_asset_paths(content, dir.path()).unwrap();
        let base = dir.path().to_string_lossy();
        assert!(normalized.contains(&*base), "expected absolute paths: {}", normalized);
    }

    #[test]
    fn test_normalize_preserves_title_attribute() {
        let dir = TempDir::new().unwrap();
        make_asset(&dir, "photo.png");
        let content = r#"![Alt](photo.png "My title")"#;
        let normalized = normalize_asset_paths(content, dir.path()).unwrap();
        assert!(
            normalized.contains("\"My title\""),
            "title should be preserved: {}",
            normalized
        );
    }

    #[test]
    fn test_normalize_empty_content() {
        let dir = TempDir::new().unwrap();
        let normalized = normalize_asset_paths("", dir.path()).unwrap();
        assert_eq!(normalized, "");
    }
}
