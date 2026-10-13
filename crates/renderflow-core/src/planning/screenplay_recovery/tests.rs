use super::*;

fn policy() -> RecoveryExtractionConfig {
    RecoveryExtractionConfig { allow_unqualified: true, ..RecoveryExtractionConfig::default() }
}

fn pdf_policy() -> RecoveryExtractionConfig {
    let mut config = policy();
    config.pdftotext = Some(crate::screenplay::recovery::provider::PinnedRecoveryTool {
        executable: std::path::PathBuf::from("/declared/unobserved/pdftotext"),
        executable_sha256: "a".repeat(64), expected_version: "25.1.0".to_owned(),
    });
    config
}

fn spec() -> SpecV2 {
    serde_json::from_value(serde_json::json!({
        "schema": "renderflow/v2",
        "sources": [{"id": "script", "path": "script.txt", "format": "text", "detect": false}],
        "targets": {"exact": [{"format": "screenplay_json"}]},
        "execution": {"screenplay_recovery": {"allow_unqualified": true}}
    })).unwrap()
}

#[test]
fn recovery_routes_require_policy_and_never_regenerate_source_pdf() {
    for source in [Format::Pdf, Format::Text] {
        let mut graph = TransformGraph::new();
        let mut tools = ToolRegistry::builtins();
        let config = if source == Format::Pdf { pdf_policy() } else { policy() };
        register_recovery_edges(&mut graph, &mut tools, source, &config).unwrap();
        assert_eq!(graph.all_edges().len(), 5);
        assert!(graph.all_edges().iter().all(|edge| edge.to != Format::Pdf));
        let mut request = spec();
        assert_eq!(apply_execution_policy(&graph, &tools, &request).all_edges().len(), 5);
        request.execution.screenplay_recovery = None;
        assert!(apply_execution_policy(&graph, &tools, &request).all_edges().is_empty());
    }
}

#[test]
fn configured_recovery_does_not_bypass_native_tool_denials() {
    let mut graph = TransformGraph::new();
    let mut tools = ToolRegistry::builtins();
    register_recovery_edges(&mut graph, &mut tools, Format::Pdf, &pdf_policy()).unwrap();
    let mut request = spec();
    request.execution.tools.deny.push("tool.pdftotext".to_owned());
    let allowed = apply_execution_policy(&graph, &tools, &request);
    assert!(allowed.all_edges().iter().all(|edge| edge.from != Format::Pdf));
    assert!(recovery_provider_ids(Format::Pdf, Some(&policy())).contains("tool.pdftotext"));
    assert!(!recovery_provider_ids(Format::Text, Some(&policy())).contains("tool.pdftotext"));
}

#[test]
fn recovery_refuses_source_republication_and_unimplemented_target_settings() {
    assert!(validate_recovery_targets(&[ResolvedTarget::generated(Format::Pdf)]).is_err());
    assert!(validate_recovery_targets(&[ResolvedTarget::generated(Format::Text)]).is_err());
    for format in [Format::ScreenplayJson, Format::Fountain, Format::Fdx, Format::FadeIn, Format::Osf] {
        let mut target = ResolvedTarget::generated(format);
        assert!(validate_recovery_targets(&[target.clone()]).is_ok());
        target.template = Some("unimplemented".to_owned());
        assert!(validate_recovery_targets(&[target]).is_err());
    }
}

#[test]
fn frozen_policy_is_part_of_every_recovery_edge_identity() {
    let mut graph = TransformGraph::new();
    let mut tools = ToolRegistry::builtins();
    let config = policy();
    register_recovery_edges(&mut graph, &mut tools, Format::Text, &config).unwrap();
    let digest = sha256_serialized(&config).unwrap().value;
    for edge in graph.all_edges() {
        assert_eq!(edge.evidence.get("screenplay_recovery_policy_sha256"), Some(&digest));
        assert_eq!(edge.evidence.get("review_required").map(String::as_str), Some("true"));
        assert_eq!(edge.evidence.get("qualification").map(String::as_str), Some("authored_unqualified"));
    }
}

#[test]
fn pdf_without_a_declared_text_provider_is_unavailable_before_execution() {
    let mut graph = TransformGraph::new();
    let mut tools = ToolRegistry::builtins();
    assert!(register_recovery_edges(&mut graph, &mut tools, Format::Pdf, &policy())
        .unwrap_err().to_string().contains("provider_unavailable"));
    assert!(graph.all_edges().is_empty());
    assert!(validate_recovery_source(Format::Pdf, false, &policy(), &spec()).is_err());
}
