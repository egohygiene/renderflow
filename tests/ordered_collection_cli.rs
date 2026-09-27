use std::fs;
use std::process::Command;

use renderflow::evidence::sha256_text;

#[test]
fn public_cli_validates_plans_and_executes_ordered_collection() {
    let dir = tempfile::tempdir().unwrap();
    let first =
        include_str!("../crates/renderflow-core/tests/fixtures/ordered-collection/page-001.md");
    let second =
        include_str!("../crates/renderflow-core/tests/fixtures/ordered-collection/page-002.md");
    let script = dir.path().join("aggregate.py");
    fs::write(dir.path().join("page-001.md"), first).unwrap();
    fs::write(dir.path().join("page-002.md"), second).unwrap();
    fs::write(
        &script,
        include_str!("../crates/renderflow-core/tests/fixtures/ordered-collection/aggregate.py"),
    )
    .unwrap();
    fs::write(dir.path().join("transforms.yaml"), format!(
        "transforms:\n  - name: synthetic.ordered-pages\n    program: python3\n    args: [\"{}\", \"{{output}}\", \"{{inputs}}\"]\n    input_kind: collection\n    from: markdown\n    to: html\n    cost: 1.0\n    quality: 1.0\n",
        script.display()
    )).unwrap();
    let config = dir.path().join("renderflow.yaml");
    fs::write(&config, format!(
        "schema: renderflow/v2\nsources:\n  - id: source.first\n    path: page-001.md\n    format: markdown\n    media_type: text/markdown\n    sha256: \"{}\"\n  - id: source.second\n    path: page-002.md\n    format: markdown\n    media_type: text/markdown\n    sha256: \"{}\"\n  - id: source.pages\n    kind: collection\n    members: [source.first, source.second]\ntargets:\n  exact:\n    - format: html\n      role: web\ntransforms: transforms.yaml\noutput:\n  bundle_root: dist\n",
        sha256_text(first).value,
        sha256_text(second).value,
    )).unwrap();
    let bin = env!("CARGO_BIN_EXE_renderflow");
    for args in [
        vec!["spec", "validate", "--config", "renderflow.yaml"],
        vec!["build", "--config", "renderflow.yaml", "--dry-run"],
        vec!["build", "--config", "renderflow.yaml"],
    ] {
        let output = Command::new(bin)
            .current_dir(dir.path())
            .args(&args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let output = fs::read_to_string(dir.path().join("dist/source.pages/web.html")).unwrap();
    assert!(output.find("First page").unwrap() < output.find("Second page").unwrap());
    assert!(dir.path().join("dist/renderflow-run.json").is_file());
}
