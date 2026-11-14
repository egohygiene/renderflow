//! Authored and unrun while the maintainer defers executable qualification.

use renderflow::evidence::RunState;
use renderflow::spec::{json_schema, load_spec_str};
use renderflow::{EngineBuilder, ExecutionRequest};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::fs;

#[test]
fn one_manual_source_uses_normal_sdk_build_and_publishes_each_profile_identity_once() {
    let root = tempfile::tempdir().unwrap();
    let source = include_bytes!("../../../examples/lyrics/manual.srt");
    let input = root.path().join("manual lyrics.srt");
    fs::write(&input, source).unwrap();
    let config = json!({"schema":"renderflow/v2","sources":[{"id":"lyrics","path":input,"format":"srt"}],
        "targets":{"profiles":["lyrics"]},"execution":{"lyrics":{"source_sha256":format!("{:x}",Sha256::digest(source)),"language":"fr"}},
        "output":{"bundle_root":root.path().join("published")}});
    let config_path = root.path().join("renderflow.yaml");
    fs::write(&config_path, serde_json::to_vec_pretty(&config).unwrap()).unwrap();
    let result = EngineBuilder::new()
        .build()
        .unwrap()
        .execute(ExecutionRequest::from_path(config_path))
        .unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete);
    assert_eq!(result.run_manifest.published_targets().unwrap().len(), 8);
    assert_eq!(fs::read(&input).unwrap(), source);
}

#[test]
fn source_changes_and_automatic_speech_policies_cannot_reuse_manual_approval() {
    let root = tempfile::tempdir().unwrap();
    let input = root.path().join("manual.srt");
    fs::write(
        &input,
        b"1\n00:00:01,000 --> 00:00:02,000\nChanged words\n\n",
    )
    .unwrap();
    let config = json!({"schema":"renderflow/v2","sources":[{"id":"lyrics","path":input,"format":"srt"}],
        "targets":{"profiles":["lyrics"]},"execution":{"lyrics":{"source_sha256":"a".repeat(64),"review_state":"reviewed"}},
        "output":{"bundle_root":root.path().join("published")}});
    let path = root.path().join("renderflow.yaml");
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    assert!(EngineBuilder::new()
        .build()
        .unwrap()
        .execute(ExecutionRequest::from_path(path))
        .is_err());
    assert!(!root.path().join("published").exists());
    let mut mixed = config;
    mixed["execution"]["subtitles"] = json!({});
    assert!(load_spec_str(&serde_json::to_string(&mixed).unwrap()).is_err());
}

#[test]
fn bundled_profiles_and_published_policy_schema_match_the_runtime_contract() {
    let config = "schema: renderflow/v2\nsources:\n  - id: lyrics\n    path: manual.srt\n    format: srt\ntargets:\n  profiles: [lyrics]\nexecution:\n  lyrics:\n    source_sha256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\n";
    let spec = load_spec_str(config).unwrap();
    assert_eq!(spec.spec.profiles["lyrics"].targets.len(), 8);
    assert_eq!(
        spec.spec.execution.lyrics.unwrap().review_state,
        renderflow::lyrics::ReviewState::Unreviewed
    );
    let published: serde_json::Value =
        serde_json::from_str(include_str!("../../../schemas/renderflow-v2.schema.json")).unwrap();
    assert_eq!(
        json_schema()["$defs"]["lyricsPolicy"],
        published["$defs"]["lyricsPolicy"]
    );
}
