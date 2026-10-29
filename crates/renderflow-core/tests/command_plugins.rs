//! Authored, UNRUN #466 regressions. Runtime cases require local Python 3.9+.
//! No mock or source assertion qualifies a native provider or a release.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc};
use std::time::{Duration, Instant};

use clap::Parser;
use renderflow::artifact::{ArtifactDescriptor, ArtifactStorageClass, ArtifactStore, ArtifactTransform};
use renderflow::cli::Cli;
use renderflow::evidence::{CacheDisposition, RunState};
use renderflow::graph::{Format, MultiTargetDag, TransformPath};
use renderflow::transforms::command_plugin::{CommandPlugin, CommandPluginDefinition};
use renderflow::transforms::yaml_loader::build_graph_executor_and_tools_from_yaml;
use renderflow::{EngineBuilder, ExecutionRequest};
use serde_json::{json, Value};

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/plugins/names")
}

fn registry() -> Value {
    serde_yaml_ng::from_str(&fs::read_to_string(example().join("transforms.yaml")).unwrap()).unwrap()
}

fn write_registry(root: &Path, value: &Value) -> PathBuf {
    let path = root.join("transforms.yaml");
    fs::write(&path, serde_yaml_ng::to_string(value).unwrap()).unwrap();
    path
}

#[test]
fn metadata_commands_never_need_a_runnable_plugin() {
    let root = tempfile::tempdir().unwrap();
    let mut value = registry();
    for plugin in value["plugins"].as_array_mut().unwrap() {
        plugin["program"] = json!("__renderflow_missing_plugin_for_metadata_test__");
    }
    let path = write_registry(root.path(), &value);
    for subcommand in ["list", "validate", "doctor"] {
        let cli = Cli::try_parse_from(["renderflow", "plugin", "--transforms", path.to_str().unwrap(),
            subcommand, "--format", "json"]).unwrap();
        renderflow::app::run_cli(cli).unwrap();
    }
    let (graph, _, _) = build_graph_executor_and_tools_from_yaml(path.to_str().unwrap()).unwrap();
    assert_eq!(graph.all_edges().len(), 2);
    assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn malformed_registration_and_unsupported_configuration_refuse_before_execution() {
    let original = registry();
    let mut cases = Vec::new();
    let mut value = original.clone(); value["plugins"][0]["descriptor"]["cache_policy"] = json!("content_addressed"); cases.push(value);
    let mut value = original.clone(); value["transforms"][0]["plugin"] = json!("missing"); cases.push(value);
    let mut value = original.clone(); value["transforms"][0]["program"] = json!("python3"); cases.push(value);
    let mut value = original.clone(); value["transforms"][0]["config"] = json!([]); cases.push(value);
    let mut value = original.clone(); value["transforms"][0]["to"] = json!("pdf"); cases.push(value);
    let mut value = original.clone(); value["transforms"][0]["input_kind"] = json!("collection"); cases.push(value);
    let mut value = original.clone(); value["plugins"][1] = value["plugins"][0].clone(); cases.push(value);
    let mut value = original.clone(); value["plugins"][0]["timeout_seconds"] = json!(0); cases.push(value);
    let mut value = original.clone(); value["plugins"][0]["max_output_bytes"] = json!(0); cases.push(value);
    let mut value = original.clone(); value["plugins"][0]["args"] = json!(["{input}", "{config}"]); cases.push(value);
    let mut value = original; value["transforms"][0]["unknown_option"] = json!(true); cases.push(value);
    for value in cases {
        let root = tempfile::tempdir().unwrap();
        let path = write_registry(root.path(), &value);
        assert!(build_graph_executor_and_tools_from_yaml(path.to_str().unwrap()).is_err());
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}

#[test]
fn parallel_plugin_edges_keep_exact_configuration_and_never_hit_cache() {
    let root = tempfile::tempdir().unwrap();
    fs::copy(example().join("names.py"), root.path().join("names.py")).unwrap();
    let mut value = registry();
    value["transforms"] = json!([
        {"name": "ada", "plugin": "example.names", "config": {"names": ["Ada"]}, "from": "text", "to": "json", "cost": 1.0, "quality": 1.0},
        {"name": "lin", "plugin": "example.names", "config": {"names": ["Lin"]}, "from": "text", "to": "json", "cost": 1.0, "quality": 1.0}
    ]);
    let path = write_registry(root.path(), &value);
    let (graph, executor, _) = build_graph_executor_and_tools_from_yaml(path.to_str().unwrap()).unwrap();
    let store = ArtifactStore::new(root.path().join("store")).unwrap();
    let input = store.put_bytes(b"Ada Ada Lin", ArtifactDescriptor::for_format(Format::Text, ArtifactStorageClass::Source)).unwrap();
    let executor = executor.with_cache(root.path().join("cache.json"));
    for edge in graph.all_edges() {
        let dag = MultiTargetDag::from_selected_path(Format::Text, &TransformPath {
            steps: vec![edge.clone()], total_cost: edge.cost, total_quality: edge.quality,
        }).unwrap();
        for _ in 0..2 {
            let result = executor.execute_artifact_with_evidence(&dag, Format::Text, input.clone(), &store).unwrap();
            let output = &result.artifacts[&Format::Json];
            let report: Value = serde_json::from_slice(&store.read_bytes(output).unwrap()).unwrap();
            let (name, count) = if edge.evidence["transform_id"] == "ada" { ("Ada", 2) } else { ("Lin", 1) };
            assert_eq!(report["names"][0]["name"], name);
            assert_eq!(report["names"][0]["count"], count);
            assert_eq!(output.sources(), &[input.id().clone()]);
            assert_eq!(output.metadata()["renderflow.plugin.transform_id"], "example.names");
            assert_eq!(result.steps[0].cache, CacheDisposition::NotApplicable);
        }
    }
    assert_eq!(store.read_bytes(&input).unwrap(), b"Ada Ada Lin");
}

#[test]
fn example_build_publishes_json_csv_and_preserves_original_source() {
    let root = tempfile::tempdir().unwrap();
    for name in ["renderflow.yaml", "transforms.yaml", "notes.txt", "names.py"] {
        fs::copy(example().join(name), root.path().join(name)).unwrap();
    }
    let source = fs::read(root.path().join("notes.txt")).unwrap();
    let result = EngineBuilder::new().build().unwrap()
        .execute(ExecutionRequest::from_path(root.path().join("renderflow.yaml"))).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete);
    assert_eq!(result.run_manifest.published_targets().unwrap().len(), 2);
    let report: Value = serde_json::from_slice(&fs::read(root.path().join("output/source.notes/names.json")).unwrap()).unwrap();
    assert_eq!(report["names"][0]["count"], 3);
    assert_eq!(report["names"][1]["count"], 3);
    let csv = fs::read_to_string(root.path().join("output/source.notes/counts.csv")).unwrap();
    assert!(csv.contains("Ada,3,1,") && csv.contains("Lin,3,1,"));
    assert_eq!(fs::read(root.path().join("notes.txt")).unwrap(), source);
}

fn command(root: &Path, source: &str) -> CommandPluginDefinition {
    fs::write(root.join("helper.py"), source).unwrap();
    let mut value = registry()["plugins"][0].clone();
    value["args"] = json!(["{registry_dir}/helper.py", "{input}", "{output}", "{config}"]);
    serde_json::from_value(value).unwrap()
}

#[test]
fn invalid_missing_and_oversized_outputs_cannot_become_artifacts() {
    for source in ["pass", "import pathlib, sys\npathlib.Path(sys.argv[2]).write_text('not json')",
        "import pathlib, sys\npathlib.Path(sys.argv[2]).write_bytes(b'x' * 100)"] {
        let root = tempfile::tempdir().unwrap();
        let mut definition = command(root.path(), source);
        definition.max_output_bytes = 32;
        let plugin = Arc::new(CommandPlugin::new(definition, root.path()).unwrap()).configured(json!({})).unwrap();
        let store = ArtifactStore::new(root.path().join("store")).unwrap();
        let input = store.put_bytes(b"original", ArtifactDescriptor::for_format(Format::Text, ArtifactStorageClass::Source)).unwrap();
        assert!(plugin.apply(&input, Format::Json, &store).is_err());
        assert_eq!(store.read_bytes(&input).unwrap(), b"original");
    }
}

#[test]
fn running_plugin_receives_dag_cancellation() {
    let root = tempfile::tempdir().unwrap();
    let definition = command(root.path(), "import pathlib, time\npathlib.Path(__file__).with_name('started').touch()\ntime.sleep(30)");
    let mut value = registry();
    value["plugins"] = json!([definition]);
    value["transforms"].as_array_mut().unwrap().truncate(1);
    let path = write_registry(root.path(), &value);
    let (graph, executor, _) = build_graph_executor_and_tools_from_yaml(path.to_str().unwrap()).unwrap();
    let dag = graph.build_multi_target_dag(Format::Text, &[Format::Json]).unwrap();
    let store = ArtifactStore::new(root.path().join("store")).unwrap();
    let input = store.put_bytes(b"Ada", ArtifactDescriptor::for_format(Format::Text, ArtifactStorageClass::Source)).unwrap();
    let flag = Arc::new(AtomicBool::new(false));
    let signal = flag.clone();
    let marker = root.path().join("started");
    let thread = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !marker.exists() && Instant::now() < deadline { std::thread::sleep(Duration::from_millis(10)); }
        signal.store(true, Ordering::SeqCst);
        marker.exists()
    });
    let start = Instant::now();
    let result = executor.with_cancellation_flag(flag).execute_artifact(&dag, Format::Text, input, &store);
    assert!(thread.join().unwrap(), "helper must start before cancellation");
    assert!(result.is_err());
    assert!(start.elapsed() < Duration::from_secs(10));
}

#[test]
fn input_limit_is_checked_before_the_program_is_launched() {
    let root = tempfile::tempdir().unwrap();
    let mut definition = command(root.path(), "raise RuntimeError('must not run')");
    definition.max_input_bytes = 2;
    definition.program = "__renderflow_missing_program__".to_string();
    let plugin = Arc::new(CommandPlugin::new(definition, root.path()).unwrap()).configured(json!({})).unwrap();
    let store = ArtifactStore::new(root.path().join("store")).unwrap();
    let input = store.put_bytes(b"Ada", ArtifactDescriptor::for_format(Format::Text, ArtifactStorageClass::Source)).unwrap();
    let error = plugin.apply(&input, Format::Json, &store).unwrap_err();
    assert!(error.to_string().contains("max_input_bytes"));
}
