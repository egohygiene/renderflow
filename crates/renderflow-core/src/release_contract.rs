//! Compiled source interfaces; no tool probes, environment discovery or execution.
use crate::{graph::TransformGraph, toolchain::ToolRegistry};
use anyhow::Result;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const RELEASE_CONTRACT_V2: &str = "renderflow.release-contract/v2";

pub fn describe() -> Result<Value> {
    let mut graph = TransformGraph::new();
    let mut tools = ToolRegistry::builtins();
    crate::planning::register_builtin_strategy_edges(&mut graph, &mut tools)?;
    let mut routes = graph.all_edges().into_iter().map(|edge| json!({
        "input": edge.from.to_string(), "output": edge.to.to_string(),
        "input_kind": edge.input_kind.to_string(), "provider_id": edge.provider_id,
        "capability_id": edge.capability_id, "required_provider_ids": edge.required_provider_ids,
        "variant_id": edge.variant_id, "evidence": edge.evidence,
        "availability": "source_declared_requires_current_preparation_and_native_evidence"
    })).collect::<Vec<_>>();
    routes.sort_by_key(Value::to_string);
    let creative: Value = serde_json::from_str(include_str!("../data/creative-routes.json"))?;
    let digest = |name: &str, bytes: &[u8]| json!({"name": name, "sha256": format!("{:x}", Sha256::digest(bytes))});
    Ok(json!({
        "schema_version": RELEASE_CONTRACT_V2, "tool": "renderflow", "version": env!("CARGO_PKG_VERSION"),
        "library_crate": "renderflow", "library_version": env!("CARGO_PKG_VERSION"),
        "inventory_scope": "canonical built-in graph plus embedded creative declarations; scoped native APIs remain distinct",
        "profiles": [{"id":"canonical-build-v1", "input":"renderflow/v2 with explicit source and output scope",
            "plan_argv":["build","--config","{configuration}","--dry-run"],
            "run_argv":["build","--config","{configuration}"],
            "resume_argv":["build","--config","{configuration}","--resume"],
            "acceptance_argv":["publication-status","--manifest","{manifest}","--manifest-sha256","{manifest_sha256}","--output-root","{output_root}"],
            "source_mutation":false, "resume_scope":"graph checkpoints only; frozen native builds refuse resume"}],
        "routes":routes, "tool_requirements":tools.all().collect::<Vec<_>>(), "creative_routes":creative,
        "additional_native_surfaces":["ordered collection PDF/EPUB", "composition", "presentation", "briefing", "print", "speech", "Sonic evidence", "screenplay"],
        "schemas":[
            digest("renderflow-v2",include_bytes!("../../../schemas/renderflow-v2.schema.json")),
            digest("renderflow-run-v1",include_bytes!("../../../schemas/renderflow-run-v1.schema.json")),
            digest("renderflow-provider-v1",include_bytes!("../../../schemas/renderflow-provider-v1.schema.json"))
        ],
        "target_objectives":["aarch64-apple-darwin","x86_64-unknown-linux-gnu","x86_64-apple-darwin","x86_64-pc-windows-msvc"],
        "qualification":"not_established_by_contract_description", "runtime_availability":"requires_current_tool_model_and_native_evidence"
    }))
}

/// Native receipt and actual confined payload bytes; keeps partial and fidelity
/// states explicit. A verified file is not native-fidelity or creative approval.
pub fn publication_status(
    manifest: &std::path::Path,
    sha256: &str,
    output_root: &std::path::Path,
) -> Result<Value> {
    use crate::evidence::verification::{PinnedRunReceipt, VerificationLimits};
    let limits = VerificationLimits::default();
    let token = crate::process::ProcessCancellationToken::default();
    let receipt = PinnedRunReceipt::read(manifest, sha256, &limits, token.clone())?;
    receipt
        .verify(output_root, &limits, token)
        .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    #[test]
    fn inventory_retains_graph_directions_and_unqualified_creative_requirements() {
        let contract = super::describe().unwrap();
        assert!(contract["routes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["input"] == "markdown" && r["output"] == "html"));
        assert!(contract["routes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["input"] != "markdown" && r["output"] != "pdf"));
        assert_eq!(
            contract["qualification"],
            "not_established_by_contract_description"
        );
        assert_eq!(contract, super::describe().unwrap());
    }
}
