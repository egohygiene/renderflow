//! Recovery policy bindings. Planning records declarations and never probes tools.

use super::*;
use crate::screenplay::recovery::graph::{RecoveryArtifactTransform,
    RecoveryExportArtifactTransform, SCREENPLAY_RECOVERY_CAPABILITY,
    SCREENPLAY_RECOVERY_EXPORT_CAPABILITY};
use crate::screenplay::recovery::provider::RecoveryExtractionConfig;

const PROVIDER: &str = "tool.renderflow-screenplay-recovery";

fn require_pdf_text_provider(source: Format, policy: &RecoveryExtractionConfig) -> Result<()> {
    if source == Format::Pdf && policy.pdftotext.is_none() {
        anyhow::bail!("screenplay_recovery.provider_unavailable: PDF recovery requires an explicit pinned pdftotext configuration");
    }
    Ok(())
}

pub(super) fn validate_recovery_source(
    source: Format, collection: bool, policy: &RecoveryExtractionConfig, spec: &SpecV2,
) -> Result<()> {
    policy.validate()?;
    if collection || !matches!(source, Format::Pdf | Format::Text) {
        anyhow::bail!("screenplay_recovery.source.unsupported: select one original PDF or UTF-8 text artifact");
    }
    require_pdf_text_provider(source, policy)?;
    if effective_hygiene_policy(spec)?.is_some() {
        anyhow::bail!("screenplay_recovery.policy.hygiene: post-recovery hygiene would detach source and review evidence");
    }
    Ok(())
}

pub(super) fn validate_recovery_targets(targets: &[ResolvedTarget]) -> Result<()> {
    for target in targets {
        if !matches!(target.format, Format::ScreenplayJson | Format::Fountain
            | Format::Fdx | Format::FadeIn | Format::Osf) {
            anyhow::bail!("screenplay_recovery.target.unsupported: choose screenplay_json, fountain, fdx, fadein, or osf; recovery never regenerates the source PDF");
        }
        if target.preset.is_some() || target.template.is_some() || target.variant.is_some()
            || !target.options.is_empty() {
            anyhow::bail!("screenplay_recovery.target.options: recovery body exports do not accept presets, templates, variants, or custom options");
        }
    }
    Ok(())
}

/// These identifiers are planned requirements, not availability observations.
pub(super) fn recovery_provider_ids(source: Format, policy: Option<&RecoveryExtractionConfig>) -> HashSet<String> {
    let Some(policy) = policy else { return HashSet::new(); };
    let mut ids = HashSet::from([PROVIDER.to_owned()]);
    if source == Format::Pdf {
        ids.insert("tool.pdftotext".to_owned());
        if policy.ocr.is_some() {
            ids.insert("tool.pdftoppm".to_owned());
            ids.insert("tool.tesseract".to_owned());
        }
    }
    ids
}

pub(super) fn register_recovery_edges(
    graph: &mut TransformGraph, tools: &mut ToolRegistry, source: Format,
    policy: &RecoveryExtractionConfig,
) -> Result<()> {
    policy.validate()?;
    if !matches!(source, Format::Pdf | Format::Text) {
        anyhow::bail!("screenplay_recovery.source.unsupported: recovery consumes original PDF or text only");
    }
    require_pdf_text_provider(source, policy)?;
    for id in recovery_provider_ids(source, Some(policy)) {
        let mut descriptor = tools.get(&id)
            .with_context(|| format!("screenplay_recovery.provider.registry: {id} descriptor missing"))?.clone();
        // Native paths are never discovered through PATH for this policy, and
        // no configured runtime is reported as observed during planning.
        if id != PROVIDER {
            descriptor.discovery = ToolDiscovery::RuntimeService {
                service: format!("renderflow.configured.{}", id.trim_start_matches("tool.")),
            };
        }
        descriptor.version = ToolVersionRequirement::default();
        descriptor.determinism = ToolDeterminism::ConfigurationDependent;
        descriptor.locality = ToolLocality::Local;
        descriptor.fidelity = ToolFidelity::Lossy;
        tools.register(descriptor)?;
        tools.add_capability(&ToolId::new(&id)?, CapabilityId::new(SCREENPLAY_RECOVERY_CAPABILITY)?)?;
    }
    tools.add_capability(&ToolId::new(PROVIDER)?, CapabilityId::new(SCREENPLAY_RECOVERY_EXPORT_CAPABILITY)?)?;
    let policy_sha256 = sha256_serialized(policy)?.value;
    let annotate = |edge: TransformEdge, transform: &str| edge
        .with_evidence("adapter", SCREENPLAY_RECOVERY_ADAPTER_EVIDENCE)
        .with_evidence("transform_id", transform)
        .with_evidence("family", "screenplay")
        .with_evidence("screenplay_recovery_policy_sha256", &policy_sha256)
        .with_evidence("provider_identity", "declared_unobserved")
        .with_evidence("qualification", "authored_unqualified")
        .with_evidence("quality_interpretation", "unqualified_not_confidence")
        .with_evidence("lossy", "true")
        .with_evidence("review_required", "true")
        .with_evidence("cache", "disabled");
    let mut recovery = TransformEdge::new(source, Format::ScreenplayJson, 1.0, 0.0)
        .with_provider(PROVIDER, SCREENPLAY_RECOVERY_CAPABILITY);
    let mut required = recovery_provider_ids(source, Some(policy)).into_iter()
        .filter(|id| id != PROVIDER).collect::<Vec<_>>();
    required.sort();
    for id in required { recovery = recovery.with_required_provider(id); }
    graph.add_transform(annotate(recovery, SCREENPLAY_RECOVERY_CAPABILITY));
    for to in [Format::Fountain, Format::Fdx, Format::FadeIn, Format::Osf] {
        let id = format!("{SCREENPLAY_RECOVERY_EXPORT_CAPABILITY}.{to}");
        graph.add_transform(annotate(TransformEdge::new(Format::ScreenplayJson, to, 0.1, 0.0)
            .with_provider(PROVIDER, SCREENPLAY_RECOVERY_EXPORT_CAPABILITY), &id));
    }
    Ok(())
}

pub(super) fn register_recovery_executors(
    executor: &mut DagExecutor, dag: &MultiTargetDag,
    policy: Option<&RecoveryExtractionConfig>, cancellation: Option<Arc<AtomicBool>>,
) -> Result<()> {
    for edge in dag.all_edges() {
        if edge.evidence.get("adapter").map(String::as_str) != Some(SCREENPLAY_RECOVERY_ADAPTER_EVIDENCE) { continue; }
        let policy = policy.context("screenplay_recovery.policy.missing: explicit recovery configuration is required")?;
        let import = matches!(edge.from, Format::Pdf | Format::Text) && edge.to == Format::ScreenplayJson;
        let export = edge.from == Format::ScreenplayJson
            && matches!(edge.to, Format::Fountain | Format::Fdx | Format::FadeIn | Format::Osf);
        let expected_capability = if import { SCREENPLAY_RECOVERY_CAPABILITY } else { SCREENPLAY_RECOVERY_EXPORT_CAPABILITY };
        let expected_transform = if import { SCREENPLAY_RECOVERY_CAPABILITY.to_owned() }
            else { format!("{SCREENPLAY_RECOVERY_EXPORT_CAPABILITY}.{}", edge.to) };
        let mut required = if import { recovery_provider_ids(edge.from, Some(policy)).into_iter()
            .filter(|id| id != PROVIDER).collect::<Vec<_>>() } else { Vec::new() };
        required.sort();
        if (!import && !export) || edge.provider_id.as_deref() != Some(PROVIDER)
            || edge.capability_id.as_deref() != Some(expected_capability)
            || edge.required_provider_ids != required
            || edge.evidence.get("transform_id") != Some(&expected_transform)
            || edge.evidence.get("screenplay_recovery_policy_sha256") != Some(&sha256_serialized(policy)?.value) {
            anyhow::bail!("screenplay_recovery.plan.inconsistent: selected edge differs from the frozen recovery policy");
        }
        if import {
            executor.register_artifact_for_edge(edge,
                Arc::new(RecoveryArtifactTransform::new(policy.clone(), cancellation.clone())?));
        } else {
            executor.register_artifact_for_edge(edge,
                Arc::new(RecoveryExportArtifactTransform::new(edge.to, policy.clone())?));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
