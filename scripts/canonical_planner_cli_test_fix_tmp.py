from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one {label} anchor, found {count}")
    return text.replace(old, new, 1)


common_path = Path("tests/common/mod.rs")
common = common_path.read_text(encoding="utf-8")
common = replace_once(
    common,
    '''    let config_content = format!(\n        "input: \\\"{}\\\"\\noutput_dir: \\\"{}\\\"\\ntransforms: \\\"{}\\\"\\n",\n        input_path.display(),\n        output_dir.display(),\n        transforms_path.display(),\n    );''',
    '''    // Keep this as an explicit v1 compatibility fixture, but make it valid under the\n    // canonical loader. Graph/inspect commands now share the same spec validation path as build.\n    let config_content = format!(\n        "outputs:\\n  - type: html\\ninput: \\\"{}\\\"\\noutput_dir: \\\"{}\\\"\\ntransforms: \\\"{}\\\"\\n",\n        input_path.display(),\n        output_dir.display(),\n        transforms_path.display(),\n    );''',
    "graph_config_file v1 fixture",
)
common_path.write_text(common, encoding="utf-8")

cli_path = Path("tests/cli_tests.rs")
cli = cli_path.read_text(encoding="utf-8")
cli = replace_once(
    cli,
    '''#[test]\nfn test_inspect_without_transforms_exits_with_error() {\n    let (f, _dir) = common::valid_config_file();\n    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))\n        .args(["inspect", "--config"])\n        .arg(f.path())\n        .output()\n        .expect("failed to execute renderflow");\n\n    assert!(\n        !output.status.success(),\n        "inspect without a 'transforms' key in config should fail"\n    );\n    let stderr = String::from_utf8_lossy(&output.stderr);\n    assert!(\n        stderr.contains("transforms"),\n        "error should mention 'transforms', got: {stderr}"\n    );\n}''',
    '''#[test]\nfn test_inspect_without_transforms_uses_builtin_capability_registry() {\n    let (f, _dir) = common::valid_config_file();\n    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))\n        .args(["inspect", "--config"])\n        .arg(f.path())\n        .output()\n        .expect("failed to execute renderflow");\n\n    assert!(\n        output.status.success(),\n        "inspect should resolve built-in capabilities without a transforms file: {}",\n        String::from_utf8_lossy(&output.stderr)\n    );\n    let stdout = String::from_utf8_lossy(&output.stdout);\n    assert!(\n        stdout.contains("DAG Execution Plan"),\n        "inspect should render the canonical plan, got: {stdout}"\n    );\n}''',
    "inspect without transforms test",
)
cli = replace_once(
    cli,
    '''#[test]\nfn test_graph_plan_without_transforms_exits_with_error() {\n    let (config_file, _dir) = common::valid_config_file();\n    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))\n        .args([\n            "graph",\n            "plan",\n            "--config",\n            config_file.path().to_str().unwrap(),\n        ])\n        .output()\n        .expect("failed to execute renderflow");\n\n    assert!(\n        !output.status.success(),\n        "graph plan without transforms should exit with error"\n    );\n}''',
    '''#[test]\nfn test_graph_plan_without_transforms_uses_builtin_capability_registry() {\n    let (config_file, _dir) = common::valid_config_file();\n    let output = Command::new(env!("CARGO_BIN_EXE_renderflow"))\n        .args([\n            "graph",\n            "plan",\n            "--config",\n            config_file.path().to_str().unwrap(),\n        ])\n        .output()\n        .expect("failed to execute renderflow");\n\n    assert!(\n        output.status.success(),\n        "graph plan should resolve built-in capabilities without a transforms file: {}",\n        String::from_utf8_lossy(&output.stderr)\n    );\n    let stdout = String::from_utf8_lossy(&output.stdout);\n    assert!(\n        stdout.contains("Execution Plan"),\n        "graph plan should render the canonical execution plan, got: {stdout}"\n    );\n}''',
    "graph plan without transforms test",
)
cli_path.write_text(cli, encoding="utf-8")

integration_path = Path("tests/graph_integration_test.rs")
integration = integration_path.read_text(encoding="utf-8")
integration = replace_once(
    integration,
    '''    let config = format!(\n        "input: \\\"{}\\\"\\noutput_dir: \\\"{}\\\"\\ntransforms: \\\"{}\\\"\\n",\n        input_path.display(),\n        output_dir.display(),\n        transforms_path.display(),\n    );''',
    '''    // The integration suite intentionally exercises the explicit v1 compatibility path.\n    // Keep the fixture valid under the canonical loader; CLI target overrides still choose\n    // the exact/all-reachable execution set used by each test.\n    let config = format!(\n        "outputs:\\n  - type: html\\ninput: \\\"{}\\\"\\noutput_dir: \\\"{}\\\"\\ntransforms: \\\"{}\\\"\\n",\n        input_path.display(),\n        output_dir.display(),\n        transforms_path.display(),\n    );''',
    "graph integration v1 fixture",
)
integration_path.write_text(integration, encoding="utf-8")

bench_path = Path("crates/renderflow-core/benches/cache.rs")
bench = bench_path.read_text(encoding="utf-8")
bench = replace_once(
    bench,
    "            || TransformCache::default(),",
    "            TransformCache::default,",
    "TransformCache benchmark constructor",
)
bench_path.write_text(bench, encoding="utf-8")
