//! Explicit manual-lyrics admission to the existing graph and forest planner.

use super::*;
use crate::lyrics::{self, LyricsPolicy};
use crate::lyrics::graph::LyricsTransform;
use crate::toolchain::ToolDescriptor;

pub(super) const LYRICS_ADAPTER: &str = "builtin.lyrics";
const PROVIDER: &str = "tool.renderflow-lyrics";

pub(super) fn validate_source(source:Format,collection:bool,policy:&LyricsPolicy,spec:&SpecV2) -> Result<()> {
    policy.validate()?;
    lyrics::input_format(source)?;
    if collection || spec.sources.len() != 1 || spec.sources[0].kind != SourceKind::Artifact {
        anyhow::bail!("lyrics.source: choose exactly one manual timed-text artifact");
    }
    if spec.transforms.is_some() || spec.execution.subtitles.is_some() || spec.execution.speech.is_some()
        || spec.execution.screenplay.is_some() || spec.execution.screenplay_body.is_some()
        || spec.execution.screenplay_pdf.is_some() || spec.execution.screenplay_recovery.is_some()
        || spec.execution.print_pdf_interior.is_some() || spec.execution.fixed_layout_epub.is_some()
        || spec.execution.composition.is_some() || effective_hygiene_policy(spec)?.is_some()
    { anyhow::bail!("lyrics.policy.route: use a dedicated manual-lyrics spec without other native, custom-transform or post-export hygiene policies"); }
    Ok(())
}

pub(super) fn validate_targets(targets:&[ResolvedTarget]) -> Result<()> {
    if targets.is_empty() || targets.iter().any(|target| !lyrics::OUTPUTS.contains(&target.format)
        || target.preset.is_some() || target.template.is_some() || target.variant.is_some() || !target.options.is_empty()) {
        anyhow::bail!("lyrics.targets: choose the bounded lyrics profile or exact supported formats without custom presets/templates/options");
    }
    Ok(())
}

fn edge(from:Format,to:Format,policy:&LyricsPolicy) -> Result<TransformEdge> {
    LyricsTransform::new(from,to,policy.clone(),None)?;
    Ok(TransformEdge::new(from,to,0.1,0.0)
        .with_provider(PROVIDER,format!("lyrics.convert.{from}.{to}"))
        .with_evidence("adapter",LYRICS_ADAPTER).with_evidence("family","lyrics")
        .with_evidence("transform_id",format!("builtin.lyrics.{from}.{to}"))
        .with_evidence("lyrics_policy_sha256",sha256_serialized(policy)?.value)
        .with_evidence("qualification","authored_unqualified")
        .with_evidence("quality_interpretation","unqualified_not_fidelity_score")
        .with_evidence("cache","disabled"))
}

pub(super) fn register_edges(graph:&mut TransformGraph,tools:&mut ToolRegistry,source:Format,policy:&LyricsPolicy) -> Result<()> {
    lyrics::input_format(source)?;
    let mut edges = vec![edge(source,Format::LyricsJson,policy)?];
    for target in lyrics::OUTPUTS.iter().copied().filter(|target| *target != Format::LyricsJson) {
        edges.push(edge(Format::LyricsJson,target,policy)?);
    }
    let mut descriptor = ToolDescriptor::virtual_provider(ToolId::new(PROVIDER)?,"Renderflow manual timed lyrics codecs");
    descriptor.capabilities = edges.iter().map(|edge| CapabilityId::new(format!("lyrics.convert.{}.{}",edge.from,edge.to))).collect::<Result<Vec<_>>>()?;
    descriptor.determinism = ToolDeterminism::Deterministic;
    descriptor.locality = ToolLocality::Local;
    descriptor.fidelity = ToolFidelity::PartialLoss;
    descriptor.support_tier = ToolSupportTier::Experimental;
    descriptor.distribution_notes = Some("Built-in manual timed-text profile; no recognition, alignment, model or native provider. Runtime qualification remains deferred.".into());
    tools.register(descriptor)?;
    for edge in edges { graph.add_transform(edge); }
    Ok(())
}

pub(super) fn register_executors(executor:&mut DagExecutor,dag:&MultiTargetDag,policy:Option<&LyricsPolicy>,
    cancellation:Option<Arc<AtomicBool>>) -> Result<()> {
    for selected in dag.all_edges() {
        if selected.evidence.get("adapter").map(String::as_str) != Some(LYRICS_ADAPTER) { continue; }
        let policy = policy.context("lyrics.policy.missing")?;
        if selected != &edge(selected.from,selected.to,policy)? { anyhow::bail!("lyrics.plan.changed: edge differs from captured policy"); }
        executor.register_artifact_for_edge(selected,Arc::new(LyricsTransform::new(selected.from,selected.to,policy.clone(),cancellation.clone())?));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lyrics::ReviewState;

    #[test]
    fn manual_profile_has_one_normalization_hub_and_no_inferred_audio_or_chord_route() {
        let policy = LyricsPolicy {source_sha256:"a".repeat(64),review_state:ReviewState::Unreviewed,
            language:None,revision:None,sections:BTreeMap::new()};
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_edges(&mut graph,&mut tools,Format::Srt,&policy).unwrap();
        assert_eq!(graph.all_edges().len(),8);
        assert_eq!(graph.transforms_from(Format::Srt).len(),1);
        for format in lyrics::OUTPUTS {
            assert!(graph.find_path(Format::Srt,*format).is_some());
        }
        assert!(graph.find_path(Format::Srt,Format::Wav).is_none());
        assert!(graph.find_path(Format::Srt,Format::Xml).is_none());
        assert!(graph.transforms_from(Format::Text).is_empty());
    }
}
