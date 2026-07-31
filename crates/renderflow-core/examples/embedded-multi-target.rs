use std::fs;

use renderflow::{EngineBuilder, ExecutionRequest, PlanRequest};

fn main() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    let input = tmp.path().join("input.md");
    fs::write(&input, "hello")?;

    let transforms = tmp.path().join("transforms.yaml");
    fs::write(
        &transforms,
        r#"transforms:
  - name: md-to-html
    program: python3
    args:
      - -c
      - \"from pathlib import Path; import sys; Path(sys.argv[2]).write_text(Path(sys.argv[1]).read_text())\"
      - \"{input}\"
      - \"{output}\"
    from: markdown
    to: html
    cost: 1.0
    quality: 1.0
  - name: html-to-pdf
    program: python3
    args:
      - -c
      - \"from pathlib import Path; import sys; Path(sys.argv[2]).write_text(Path(sys.argv[1]).read_text())\"
      - \"{input}\"
      - \"{output}\"
    from: html
    to: pdf
    cost: 1.0
    quality: 1.0
"#,
    )?;

    let config = tmp.path().join("renderflow.yaml");
    fs::write(
        &config,
        format!(
            "outputs:\n  - type: html\n  - type: pdf\ninput: \"{}\"\noutput_dir: \"{}\"\ntransforms: \"{}\"\n",
            input.display(),
            tmp.path().join("dist").display(),
            transforms.display()
        ),
    )?;

    let engine = EngineBuilder::new().with_default_transforms().build()?;
    let plan = engine.plan(PlanRequest::from_path(&config).with_target("pdf"))?;
    let exec = engine.execute(
        ExecutionRequest::from_path(&config)
            .with_all_targets()
            .dry_run(true),
    )?;

    println!(
        "plan edges: {}, execution targets: {}",
        plan.metadata.total_edges,
        exec.manifest.outputs.join(", ")
    );
    Ok(())
}
