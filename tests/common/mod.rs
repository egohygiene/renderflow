use std::io::Write;
use tempfile::NamedTempFile;

pub fn valid_config_file() -> NamedTempFile {
    let mut f = NamedTempFile::new().expect("failed to create temp file");
    f.write_all(b"outputs: []\ninput: \"input.md\"\noutput_dir: \"dist\"\n")
        .expect("failed to write temp file");
    f
}
