//! Explicit direct exports from one immutable original Fountain artifact.
//!
//! The built-in body codec launches no native tools. Its authored subset stays
//! unqualified, and every selected edge binds the explicit opt-in policy.

use super::*;
use crate::screenplay::body_graph::{
    BodyExportPolicy, FountainBodyArtifactTransform, SCREENPLAY_BODY_CAPABILITY,
};
use crate::toolchain::ToolDescriptor;

pub(super) const SCREENPLAY_BODY_ADAPTER_EVIDENCE: &str = "builtin.screenplay.body";
const PROVIDER: &str = "tool.renderflow-screenplay-body";

fn supports_body_output(format: Format) -> bool {
    matches!(format, Format::ScreenplayJson | Format::Fdx | Format::FadeIn
        | Format::Osf | Format::Trelby)
}

pub(super) fn validate_body_source(
    source: Format, collection: bool, policy: &BodyExportPolicy, spec: &SpecV2,
) -> Result<()> {
    policy.validate()?;
    if collection || source != Format::Fountain {
        anyhow::bail!("screenplay_body.source.unsupported: choose exactly one original Fountain artifact");
    }
    if spec.transforms.is_some() || spec.execution.screenplay.is_some()
        || spec.execution.screenplay_pdf.is_some() || spec.execution.screenplay_recovery.is_some()
        || spec.execution.speech.is_some() || spec.execution.subtitles.is_some()
        || spec.execution.print_pdf_interior.is_some() || spec.execution.fixed_layout_epub.is_some()
    {
        anyhow::bail!("screenplay_body.policy.route: original-Fountain body exports cannot combine custom transforms, recovery, scripttool, PDF, subtitle, speech, or collection rendering policies");
    }
    if effective_hygiene_policy(spec)?.is_some() {
        anyhow::bail!("screenplay_body.policy.hygiene: post-export hygiene would detach canonical source and body evidence");
    }
    Ok(())
}

pub(super) fn validate_body_targets(targets: &[ResolvedTarget]) -> Result<()> {
    if targets.is_empty() || targets.iter().any(|target| !supports_body_output(target.format)) {
        anyhow::bail!("screenplay_body.target.unsupported: choose screenplay_json, fdx, fadein, osf, or trelby; same-format, PDF, text, recovery, and transitive exports are unavailable");
    }
    if targets.iter().any(|target| target.preset.is_some() || target.template.is_some()
        || target.variant.is_some() || !target.options.is_empty()) {
        anyhow::bail!("screenplay_body.target.options: the bounded body profile does not accept presets, templates, variants, or custom formatting options");
    }
    Ok(())
}

fn body_edge(output: Format, policy: &BodyExportPolicy) -> Result<TransformEdge> {
    policy.validate()?;
    if !supports_body_output(output) {
        anyhow::bail!("screenplay_body.target.unsupported: unsupported direct body output '{output}'");
    }
    Ok(TransformEdge::new(Format::Fountain, output, 0.1, 0.0)
        .with_provider(PROVIDER, SCREENPLAY_BODY_CAPABILITY)
        .with_evidence("adapter", SCREENPLAY_BODY_ADAPTER_EVIDENCE)
        .with_evidence("family", "screenplay")
        .with_evidence("transform_id", format!("builtin.screenplay.body.fountain.{output}"))
        .with_evidence("screenplay_body_policy_sha256", sha256_serialized(policy)?.value)
        .with_evidence("original_source_required", "true")
        .with_evidence("canonical_retained", "true")
        .with_evidence("qualification", "authored_unqualified")
        .with_evidence("quality_interpretation", "unqualified_not_fidelity_score")
        .with_evidence("cache", "disabled"))
}

pub(super) fn register_body_edges(
    graph: &mut TransformGraph, tools: &mut ToolRegistry, source: Format,
    policy: &BodyExportPolicy,
) -> Result<()> {
    policy.validate()?;
    if source != Format::Fountain {
        anyhow::bail!("screenplay_body.source.unsupported: body exports consume only the original Fountain artifact");
    }
    let mut descriptor = ToolDescriptor::virtual_provider(
        ToolId::new(PROVIDER)?, "Renderflow bounded original-Fountain body codec",
    );
    descriptor.capabilities = vec![CapabilityId::new(SCREENPLAY_BODY_CAPABILITY)?];
    descriptor.determinism = ToolDeterminism::Deterministic;
    descriptor.locality = ToolLocality::Local;
    descriptor.fidelity = ToolFidelity::PartialLoss;
    descriptor.support_tier = ToolSupportTier::Experimental;
    descriptor.distribution_notes = Some(
        "Built into Renderflow; authored bounded body semantics only, without native application, layout, or pagination qualification".to_owned(),
    );
    tools.register(descriptor)?;
    for output in [Format::ScreenplayJson, Format::Fdx, Format::FadeIn, Format::Osf, Format::Trelby] {
        graph.add_transform(body_edge(output, policy)?);
    }
    Ok(())
}

pub(super) fn register_body_executors(
    executor: &mut DagExecutor, dag: &MultiTargetDag, policy: Option<&BodyExportPolicy>,
    cancellation: Option<Arc<AtomicBool>>,
) -> Result<()> {
    for edge in dag.all_edges() {
        if edge.evidence.get("adapter").map(String::as_str) != Some(SCREENPLAY_BODY_ADAPTER_EVIDENCE) {
            continue;
        }
        let policy = policy.context("screenplay_body.policy.missing: explicit original-Fountain body export policy is required")?;
        // Compare the complete closed edge, including source, provider, policy,
        // transform, input kind and required-tool list before binding execution.
        if edge != &body_edge(edge.to, policy)? {
            anyhow::bail!("screenplay_body.plan.inconsistent: selected edge differs from the frozen original-Fountain body policy");
        }
        executor.register_artifact_for_edge(edge, Arc::new(FountainBodyArtifactTransform::new(
            edge.to, policy.clone(), cancellation.clone(),
        )?));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> BodyExportPolicy {
        BodyExportPolicy { allow_unqualified: true }
    }

    fn spec() -> SpecV2 {
        serde_json::from_value(serde_json::json!({
            "schema": "renderflow/v2",
            "sources": [{"id": "script", "path": "script.fountain", "format": "fountain", "detect": false}],
            "targets": {"exact": [{"format": "fdx"}]},
            "execution": {"screenplay_body": {"allow_unqualified": true}}
        })).unwrap()
    }

    #[test]
    fn body_edges_are_direct_original_only_and_require_explicit_policy() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_body_edges(&mut graph, &mut tools, Format::Fountain, &policy()).unwrap();
        assert_eq!(graph.all_edges().len(), 5);
        assert!(graph.all_edges().iter().all(|edge| edge.from == Format::Fountain
            && edge.provider_id.as_deref() == Some(PROVIDER) && edge.required_provider_ids.is_empty()));
        for output in [Format::ScreenplayJson, Format::Fdx, Format::FadeIn, Format::Osf, Format::Trelby] {
            assert!(graph.find_path(Format::Fountain, output).is_some());
            assert!(graph.transforms_from(output).is_empty());
        }
        assert!(graph.find_path(Format::Fountain, Format::Pdf).is_none());
        let mut request = spec();
        assert_eq!(apply_execution_policy(&graph, &tools, &request).all_edges().len(), 5);
        request.execution.screenplay_body = None;
        assert!(apply_execution_policy(&graph, &tools, &request).all_edges().is_empty());
        assert!(register_body_edges(&mut TransformGraph::new(), &mut tools,
            Format::Fountain, &BodyExportPolicy { allow_unqualified: false }).is_err());
    }

    #[test]
    fn body_policy_refuses_derivative_sources_collections_and_custom_routes() {
        let request = spec();
        assert!(validate_body_source(Format::Fountain, false, &policy(), &request).is_ok());
        assert!(validate_body_source(Format::Fountain, true, &policy(), &request).is_err());
        for source in [Format::ScreenplayJson, Format::Fdx, Format::FadeIn, Format::Osf,
            Format::Trelby, Format::Pdf, Format::Text] {
            assert!(validate_body_source(source, false, &policy(), &request).is_err());
        }
        let mut custom = request.clone();
        custom.transforms = Some("custom.yaml".to_owned());
        assert!(validate_body_source(Format::Fountain, false, &policy(), &custom).is_err());
        let mut recovery = request.clone();
        recovery.execution.screenplay_recovery = Some(
            crate::screenplay::recovery::provider::RecoveryExtractionConfig::default(),
        );
        assert!(validate_body_source(Format::Fountain, false, &policy(), &recovery).is_err());
        let mut native = request;
        native.execution.screenplay = Some(ScripttoolConfig {
            executable: std::env::temp_dir().join("scripttool"),
            executable_sha256: "a".repeat(64), expected_version: "0.0.10".to_owned(),
            timeout_seconds: 30, allow_unqualified: true,
        });
        assert!(validate_body_source(Format::Fountain, false, &policy(), &native).is_err());
    }

    #[test]
    fn body_target_whitelist_rejects_source_publication_and_ignored_options() {
        assert!(validate_body_targets(&[]).is_err());
        for format in [Format::Fountain, Format::Pdf, Format::Text, Format::Json] {
            assert!(validate_body_targets(&[ResolvedTarget::generated(format)]).is_err());
        }
        for format in [Format::ScreenplayJson, Format::Fdx, Format::FadeIn, Format::Osf, Format::Trelby] {
            let mut target = ResolvedTarget::generated(format);
            assert!(validate_body_targets(&[target.clone()]).is_ok());
            target.options.insert("font".to_owned(), serde_json::json!("unimplemented"));
            assert!(validate_body_targets(&[target]).is_err());
        }
    }

    #[test]
    fn body_exports_honor_tool_denials_and_retained_policy_identity() {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        register_body_edges(&mut graph, &mut tools, Format::Fountain, &policy()).unwrap();
        let descriptor = tools.get(PROVIDER).unwrap();
        assert_eq!(descriptor.discovery, ToolDiscovery::Virtual);
        assert_eq!(descriptor.locality, ToolLocality::Local);
        let digest = sha256_serialized(&policy()).unwrap().value;
        for edge in graph.all_edges() {
            assert_eq!(edge.evidence.get("screenplay_body_policy_sha256"), Some(&digest));
            assert_eq!(edge.evidence.get("cache").map(String::as_str), Some("disabled"));
        }
        let mut request = spec();
        request.execution.tools.deny.push(PROVIDER.to_owned());
        assert!(apply_execution_policy(&graph, &tools, &request).all_edges().is_empty());
    }

    #[test]
    fn body_hygiene_inherited_from_a_selected_profile_is_refused() {
        let mut request = spec();
        request.hygiene.insert("hygiene.body".to_owned(), HygienePolicy::default());
        request.profiles.insert("body.parent".to_owned(), DerivativeProfile {
            hygiene_policy: Some("hygiene.body".to_owned()), ..Default::default()
        });
        request.profiles.insert("body.child".to_owned(), DerivativeProfile {
            extends: vec!["body.parent".to_owned()], ..Default::default()
        });
        request.targets.profiles.push("body.child".to_owned());
        compose_selected_profiles(&mut request).unwrap();
        assert!(validate_body_source(Format::Fountain, false, &policy(), &request)
            .unwrap_err().to_string().contains("screenplay_body.policy.hygiene"));
    }

    #[test]
    fn frozen_body_edge_cannot_change_source_provider_or_policy_before_executor_binding() {
        let expected = body_edge(Format::Fdx, &policy()).unwrap();
        let mut wrong_policy = expected.clone();
        wrong_policy.evidence.insert("screenplay_body_policy_sha256".to_owned(), "0".repeat(64));
        let mut wrong_provider = expected.clone();
        wrong_provider.provider_id = Some("tool.scripttool".to_owned());
        let mut wrong_source = expected;
        wrong_source.from = Format::ScreenplayJson;
        for edge in [wrong_policy, wrong_provider, wrong_source] {
            let source = edge.from;
            let mut graph = TransformGraph::new();
            graph.add_transform(edge);
            let dag = graph.build_multi_target_dag(source, &[Format::Fdx]).unwrap();
            assert!(register_body_executors(&mut DagExecutor::new(), &dag, Some(&policy()), None)
                .unwrap_err().to_string().contains("screenplay_body.plan.inconsistent"));
        }
    }

    #[test]
    fn native_only_body_plan_counts_the_retained_canonical_without_publishing_it() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("original.fountain");
        fs::write(&source, "INT. ROOM - DAY\n\nA lamp glows.\n").unwrap();
        let mut request = spec();
        request.sources[0].path = Some(source.to_string_lossy().into_owned());
        request.execution.budgets.max_artifacts = Some(2);
        let path = directory.path().join("body.json");
        fs::write(&path, serde_json::to_vec(&request).unwrap()).unwrap();
        let resolved = resolve(PlanningRequest::from_path(path)).unwrap();
        assert_eq!(resolved.targets.len(), 1);
        assert_eq!(resolved.targets[0].format, Format::Fdx);
        assert!(resolved.plan.toolchain.is_none());
        assert!(validate_pre_execution_budgets(&resolved).unwrap_err().to_string()
            .contains("artifact estimate 3"));
    }
}
