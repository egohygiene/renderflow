use std::fs;

use renderflow::{EngineBuilder, ExecutionRequest};

fn main() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let input = tmp.path().join("input.md");
    fs::write(&input, "# Embedded\n")?;

    let config = tmp.path().join("renderflow.yaml");
    fs::write(
        &config,
        format!(
            "outputs:\n  - type: html\ninput: \"{}\"\noutput_dir: \"{}\"\n",
            input.display(),
            tmp.path().join("dist").display()
        ),
    )?;

    let engine = EngineBuilder::new().with_default_transforms().build()?;
    let result = engine.execute(ExecutionRequest::from_path(&config).dry_run(true))?;

    println!("planned outputs: {}", result.manifest.outputs.join(", "));
    Ok(())
}
