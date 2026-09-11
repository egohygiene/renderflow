//! Handlers for optional Artifact DNA extraction, validation, and comparison.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::artifact::ArtifactStore;
use crate::dna::{
    ArtifactDna, ArtifactDnaComparison, ArtifactDnaEngine, DnaExtractionPolicy,
    DnaExtractionStatus, DnaProtectedReferenceRule,
};
use crate::intake::{IntakeEngine, IntakeRequest};

#[allow(clippy::too_many_arguments)]
pub fn run_extract(
    input: &str,
    output: Option<&str>,
    store_path: &str,
    media_type: Option<&str>,
    format: &str,
    max_source_bytes: u64,
    max_observations: usize,
    allow_ai: bool,
    allow_network: bool,
    allow_remote: bool,
    protected_references: &[String],
) -> Result<()> {
    let store = ArtifactStore::new(store_path)?;
    let mut request = IntakeRequest::from_path(input);
    if let Some(media_type) = media_type {
        request = request.with_media_type(media_type);
    }
    let intake = IntakeEngine::new().intake(&request, &store)?;
    let mut policy = DnaExtractionPolicy::explicit_local();
    policy.max_source_bytes = max_source_bytes;
    policy.max_observations = max_observations;
    policy.allow_ai = allow_ai;
    policy.allow_network = allow_network;
    policy.allow_remote = allow_remote;
    policy.protected_references = protected_references
        .iter()
        .map(|term| DnaProtectedReferenceRule {
            term: term.clone(),
            descriptive_replacement: None,
        })
        .collect();
    let outcome = ArtifactDnaEngine::with_builtins().extract(&intake.source, &store, &policy)?;
    let Some(dna) = outcome.dna else {
        anyhow::bail!(
            "Artifact DNA extraction {:?}: {}",
            outcome.status,
            outcome.diagnostics.join("; ")
        );
    };
    let serialized = serialize(&dna, format)?;
    if let Some(output) = output {
        std::fs::write(output, &serialized)
            .with_context(|| format!("failed to write Artifact DNA '{}'", output))?;
        let artifact_id = outcome
            .artifact
            .as_ref()
            .map(|artifact| artifact.id().to_string())
            .unwrap_or_else(|| "unavailable".to_string());
        println!(
            "Artifact DNA {:?}: {} (stored as {})",
            outcome.status,
            Path::new(output).display(),
            artifact_id
        );
    } else {
        print!("{serialized}");
    }
    if outcome.status == DnaExtractionStatus::Partial {
        for diagnostic in outcome.diagnostics {
            eprintln!("warning: {diagnostic}");
        }
    }
    Ok(())
}

pub fn run_validate(input: &str, format: &str) -> Result<()> {
    let dna = ArtifactDna::load(input)?;
    dna.validate()?;
    match format {
        "text" => println!(
            "valid {}: {} observations across {} modalities",
            dna.schema_version,
            dna.observations.len(),
            dna.modalities.len()
        ),
        "json" | "yaml" => {
            let report = serde_json::json!({
                "schema_version": dna.schema_version,
                "valid": true,
                "observations": dna.observations.len(),
                "modalities": dna.modalities,
            });
            print!("{}", serialize(&report, format)?);
        }
        _ => anyhow::bail!("unknown output format '{format}'; expected text, json, or yaml"),
    }
    Ok(())
}

pub fn run_compare(left: &str, right: &str, output: Option<&str>, format: &str) -> Result<()> {
    let left = ArtifactDna::load(left)?;
    let right = ArtifactDna::load(right)?;
    let comparison = ArtifactDnaComparison::compare(&left, &right)?;
    let serialized = serialize(&comparison, format)?;
    if let Some(output) = output {
        std::fs::write(output, &serialized)
            .with_context(|| format!("failed to write DNA comparison '{}'", output))?;
        println!("Artifact DNA comparison written to {output}");
    } else {
        print!("{serialized}");
    }
    Ok(())
}

fn serialize(value: &impl Serialize, format: &str) -> Result<String> {
    match format {
        "json" => Ok(format!("{}\n", serde_json::to_string_pretty(value)?)),
        "yaml" => Ok(serde_yaml_ng::to_string(value)?),
        _ => anyhow::bail!("unknown output format '{format}'; expected json or yaml"),
    }
}
