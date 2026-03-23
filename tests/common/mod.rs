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
