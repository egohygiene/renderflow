use std::fs;
use std::io::Write;
use tempfile::NamedTempFile;

pub fn valid_config_file() -> (NamedTempFile, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let input_path = dir.path().join("input.md");
    fs::write(&input_path, "# Test\n").expect("failed to write input file");
    let output_dir = dir.path().join("dist");
    let config_content = format!(
        "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
        input_path.display(),
        output_dir.display()
    );
    let mut f = NamedTempFile::new().expect("failed to create temp file");
    f.write_all(config_content.as_bytes())
        .expect("failed to write temp file");
    (f, dir)
}

/// Create a temporary directory and config file suitable for graph-based
/// execution tests (`--target`, `--all`).
///
/// Returns `(config_file, _dir)` where `_dir` must be kept alive for the
/// duration of the test.
pub fn graph_config_file() -> (NamedTempFile, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("failed to create temp dir");
    let input_path = dir.path().join("doc.md");
    fs::write(&input_path, "# Hello\n").expect("failed to write input");

    // A single markdown→html edge backed by `cat`.
    let transforms_path = dir.path().join("transforms.yaml");
    let transforms_yaml = "\
transforms:\n  \
  - name: md-to-html\n    \
    program: cat\n    \
    from: markdown\n    \
    to: html\n    \
    cost: 1.0\n    \
    quality: 0.9\n";
    fs::write(&transforms_path, transforms_yaml).expect("failed to write transforms");

    let output_dir = dir.path().join("dist");
    let config_content = format!(
        "input: \"{}\"\noutput_dir: \"{}\"\ntransforms: \"{}\"\n",
        input_path.display(),
        output_dir.display(),
        transforms_path.display(),
    );
    let mut config_file = NamedTempFile::new().expect("failed to create temp config");
    config_file
        .write_all(config_content.as_bytes())
        .expect("failed to write config");
    (config_file, dir)
}
