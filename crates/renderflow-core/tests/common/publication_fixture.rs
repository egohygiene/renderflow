//! Original synthetic receipts for file-backed publication cases, not provider evidence.

use renderflow::RunManifest;
use serde_json::json;
use sha2::{Digest, Sha256};

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn manifest(bytes: &[u8]) -> RunManifest {
    serde_json::from_value(json!({
        "schema_version":"renderflow.run/v1","run_id":"synthetic-files",
        "execution_plan_digest":{"algorithm":"sha256","value":"0".repeat(64)},
        "source_spec_digest":{"algorithm":"sha256","value":"1".repeat(64)},
        "engine_version":"fixture","started_at_unix_ms":0,"completed_at_unix_ms":1,
        "state":"partial","steps":[],"diagnostics":[],
        "artifact_manifest":{"schema_version":"renderflow.artifact-manifest/v1","run_id":"synthetic-files",
            "output_dir":"do-not-use-this-receipt-path","outputs":["do-not-use-this-receipt-path/result.txt"],
            "artifacts":[{"artifact_id":"synthetic-subject","role":"body","lifecycle":"terminal",
                "locator":"bundle:result.txt","format":"text","media_type":"text/plain",
                "digest":{"algorithm":"sha256","value":digest(bytes)},"size_bytes":bytes.len(),
                "producer":{"system":"renderflow"},"sources":["synthetic-original"],
                "cache":"miss","validation":"skipped","fidelity":"partial",
                "metadata":{"synthetic.review_required":true}}]},
        "artifact_forest":{"schema":"renderflow/artifact-forest-v1","intermediates":"cache_only",
            "branches":[{"target_id":"target-one","format":"text","requirement":"required",
                "state":"selected","reason_code":"fixture","reason":"Synthetic file verification"}],
            "produced_artifacts":["synthetic-subject"],
            "execution":{"schema":"renderflow/forest-execution-v1","targets":[{
                "target_id":"target-one","outcome":"published","source_identity":false,
                "artifact_evidence_index":0,"artifact_id":"synthetic-subject","locator":"bundle:result.txt"}]}}
    })).unwrap()
}

pub fn relocate(run: &mut RunManifest, locator: &str) {
    run.artifact_manifest.artifacts[0].locator = locator.into();
    run.artifact_forest
        .as_mut()
        .unwrap()
        .execution
        .as_mut()
        .unwrap()
        .targets[0]
        .locator = Some(locator.into());
}

pub fn second_target(run: &mut RunManifest) {
    let forest = run.artifact_forest.as_mut().unwrap();
    let mut branch = forest.branches[0].clone();
    branch.target_id = "target-two".into();
    forest.branches.push(branch);
    let mut target = forest.execution.as_ref().unwrap().targets[0].clone();
    target.target_id = "target-two".into();
    target.artifact_evidence_index = Some(1);
    target.locator = Some("bundle:second.txt".into());
    forest.execution.as_mut().unwrap().targets.push(target);
    let mut record = run.artifact_manifest.artifacts[0].clone();
    record.locator = "bundle:second.txt".into();
    run.artifact_manifest.artifacts.push(record);
    run.artifact_manifest
        .outputs
        .push("unused/second.txt".into());
}
