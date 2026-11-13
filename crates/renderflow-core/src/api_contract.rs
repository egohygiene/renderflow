//! Core-owned schema projection for existing SDK results. This only assembles
//! embedded declarations; it performs no provider, source or network operation.
use serde_json::{json, Map, Value};

pub const SDK_API_SCHEMA: &str = "renderflow.sdk-api-schema/v1";
pub const SOURCE: &str = include_str!("../../../schemas/renderflow-sdk-api-v1.schema.json");
const RUN: &str = include_str!("../../../schemas/renderflow-run-v1.schema.json");
const INTAKE: &str = include_str!("../../../schemas/renderflow-intake-v1.schema.json");

/// Self-contained HTTP/offline projection. Existing core schema files remain
/// the native source; no copied API-side model or runtime validator is added.
pub fn json_schema() -> serde_json::Result<Value> {
    let mut schema: Value = serde_json::from_str(SOURCE)?;
    let definitions = schema["$defs"]
        .as_object_mut()
        .expect("embedded SDK definitions");
    let mut run = import(serde_json::from_str(RUN)?, "run_", definitions);
    run["properties"]["toolchain"] = json!({"$ref":"#/$defs/ToolchainSnapshot"});
    definitions.insert("RunManifest".into(), run);
    let intake = import(serde_json::from_str(INTAKE)?, "intake_", definitions);
    definitions.insert("IntakeReport".into(), intake);
    rewrite(&mut schema, &|reference| {
        for (file, prefix, root) in [
            ("renderflow-run-v1.schema.json", "run_", "RunManifest"),
            (
                "renderflow-intake-v1.schema.json",
                "intake_",
                "IntakeReport",
            ),
        ] {
            if reference == file {
                return format!("#/$defs/{root}");
            }
            if let Some(name) = reference.strip_prefix(&format!("{file}#/$defs/")) {
                return format!("#/$defs/{prefix}{name}");
            }
        }
        reference.into()
    });
    Ok(schema)
}

fn import(mut source: Value, prefix: &str, definitions: &mut Map<String, Value>) -> Value {
    rewrite(&mut source, &|reference| {
        reference
            .strip_prefix("#/$defs/")
            .map(|name| format!("#/$defs/{prefix}{name}"))
            .unwrap_or_else(|| reference.into())
    });
    let map = source.as_object_mut().expect("embedded native schema");
    if let Some(Value::Object(children)) = map.remove("$defs") {
        for (name, value) in children {
            definitions.insert(format!("{prefix}{name}"), value);
        }
    }
    map.remove("$id");
    map.remove("$schema");
    source
}

fn rewrite(value: &mut Value, replace: &impl Fn(&str) -> String) {
    match value {
        Value::Object(map) => {
            if let Some(Value::String(reference)) = map.get_mut("$ref") {
                *reference = replace(reference);
            }
            for child in map.values_mut() {
                rewrite(child, replace);
            }
        }
        Value::Array(items) => {
            for child in items {
                rewrite(child, replace);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bundled_sdk_references_resolve_locally_and_typed_native_roots_are_present() {
        let schema = json_schema().unwrap();
        fn visit(value: &Value, root: &Value) {
            match value {
                Value::Object(map) => {
                    if let Some(Value::String(reference)) = map.get("$ref") {
                        assert!(
                            reference.starts_with("#/"),
                            "external schema dependency: {reference}"
                        );
                        assert!(
                            root.pointer(&reference[1..]).is_some(),
                            "dangling schema reference: {reference}"
                        );
                    }
                    for child in map.values() {
                        visit(child, root);
                    }
                }
                Value::Array(items) => {
                    for child in items {
                        visit(child, root);
                    }
                }
                _ => {}
            }
        }
        visit(&schema, &schema);
        assert!(schema["$defs"]["ExecutionPlan"]["properties"]["edges"]
            .get("items")
            .is_some());
        assert_eq!(
            schema["$defs"]["RunManifest"]["properties"]["toolchain"]["$ref"],
            "#/$defs/ToolchainSnapshot"
        );
        assert_eq!(
            schema["$defs"]["run_artifact"]["properties"]["metadata"]["type"],
            "object"
        );
    }

    #[test]
    fn actual_inspection_and_progress_keys_have_owned_schema_fields() {
        let profile = crate::sdk::ArtifactProfile {
            input_path: "input".into(),
            input_format: "srt".into(),
            output_dir: "private".into(),
            targets: vec!["vtt".into()],
            transforms_path: None,
        };
        let schema = json_schema().unwrap();
        let value = serde_json::to_value(profile).unwrap();
        for key in value.as_object().unwrap().keys() {
            assert!(schema["$defs"]["ArtifactProfile"]["properties"]
                .get(key)
                .is_some());
        }
        let required = schema["$defs"]["ProgressEvent"]["required"]
            .as_array()
            .unwrap();
        assert_eq!(required.len(), 3);
        assert!(required.contains(&json!("schema_version")));
    }
}
