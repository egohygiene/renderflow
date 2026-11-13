//! Closed admission for the frozen presentation backend, not generic JSON.
use super::{
    CollisionPolicy, ExecutionPolicy, IntermediatePolicy, SourceKind, SpecDiagnostic, SpecV2,
    TargetRequirement, TargetSpec,
};
use crate::presentation::{slidev::SlidevProvider, PresentationTarget};
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Component, Path};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationBuildPolicy {
    #[serde(default)]
    pub provider: Option<SlidevProvider>,
    #[serde(default)]
    pub include_speaker_notes: bool,
    #[serde(default)]
    pub allow_unqualified: bool,
}

pub(crate) fn targets(values: &[TargetSpec]) -> Result<Vec<PresentationTarget>> {
    ensure!(
        !values.is_empty() && values.len() <= 3,
        "presentation.targets: select one to three exact targets"
    );
    let mut seen = BTreeSet::new();
    values
        .iter()
        .map(|value| {
            ensure!(
                value.id.is_none()
                    && value.role.is_none()
                    && value.family.is_none()
                    && value.capability.is_none()
                    && value.transform.is_none()
                    && value.provider.is_none()
                    && value.variant.is_none()
                    && value.preset.is_none()
                    && value.template.is_none()
                    && value.options.is_empty()
                    && value.requirement == TargetRequirement::Required,
                "presentation.targets: only exact format with required membership is supported"
            );
            let target = match value.format.as_deref() {
                Some("presentation_source") => PresentationTarget::Source,
                Some("presentation_web") => PresentationTarget::Web,
                Some("pdf") => PresentationTarget::Pdf,
                _ => anyhow::bail!(
                    "presentation.targets: use presentation_source, presentation_web, or pdf"
                ),
            };
            ensure!(
                seen.insert(target),
                "presentation.targets: duplicate target"
            );
            Ok(target)
        })
        .collect()
}

pub(super) fn validate(spec: &SpecV2, diagnostics: &mut Vec<SpecDiagnostic>) {
    let count = spec
        .sources
        .iter()
        .filter(|s| s.kind == SourceKind::Presentation)
        .count();
    if count == 0 {
        if spec.execution.presentation.is_some() {
            diagnostics.push(SpecDiagnostic::new(
                "$.execution.presentation",
                "presentation.source.required",
                "presentation settings require one kind: presentation source",
            ));
        }
        return;
    }
    if let Err(error) = admitted(spec) {
        diagnostics.push(SpecDiagnostic::new(
            "$",
            "presentation.build.unsupported",
            format!("{error:#}"),
        ));
    }
}

fn local(path: &str) -> bool {
    !path.trim().is_empty()
        && !path.contains('\0')
        && !path.contains("://")
        && !Path::new(path)
            .components()
            .any(|c| c == Component::ParentDir)
}

fn admitted(spec: &SpecV2) -> Result<()> {
    ensure!(
        spec.sources.len() == 1,
        "presentation.source: exactly one local model is required"
    );
    let source = &spec.sources[0];
    ensure!(source.kind == SourceKind::Presentation && source.path.as_deref().is_some_and(local)
        && source.uri.is_none() && source.members.is_empty() && source.sha256.is_none()
        && source.geometry.is_none() && source.format.is_none() && source.media_type.is_none()
        && source.role.is_none() && source.detect && source.immutable,
        "presentation.source: use only id, kind: presentation, and a local model path without parent traversal");
    targets(&spec.targets.exact)?;
    ensure!(spec.profiles.is_empty() && spec.targets.profiles.is_empty() && !spec.targets.all_reachable
        && spec.targets.include.is_empty() && spec.targets.exclude.is_empty()
        && spec.targets.intermediates == IntermediatePolicy::CacheOnly
        && spec.hygiene.is_empty() && spec.publication.is_none() && spec.transforms.is_none()
        && spec.variables.is_empty(), "presentation.build: profiles, selectors, custom transforms, hygiene, publication settings and variables are unsupported");
    ensure!(
        spec.output.collision == CollisionPolicy::Error
            && local(&spec.output.bundle_root)
            && spec.output.naming_template == super::default_naming_template(),
        "presentation.output: require a fresh local bundle_root and default naming"
    );
    // Closed comparison catches future execution fields too. Explicit local and
    // offline intent is allowed, but is not an OS/network sandbox assertion.
    let mut effective = spec.execution.clone();
    effective.presentation = None;
    let mut defaults = ExecutionPolicy::default();
    defaults.requirements.local_only = effective.requirements.local_only;
    defaults.requirements.offline = effective.requirements.offline;
    ensure!(effective == defaults, "presentation.execution: only presentation settings and local/offline intent are supported; no ignored execution overrides");
    if let Some(provider) = spec
        .execution
        .presentation
        .as_ref()
        .and_then(|p| p.provider.as_ref())
    {
        crate::presentation::slidev::parse_provider(&serde_json::to_vec(provider)?)
            .context("presentation.provider: invalid explicit provider")?;
    }
    Ok(())
}

pub(super) fn extend_schema(schema: &mut Value) {
    let fragment: Value = serde_json::from_str(include_str!(
        "../../../../schemas/fragments/presentation-build-v1.json"
    ))
    .expect("authored presentation schema fragment");
    schema["$defs"]
        .as_object_mut()
        .expect("v2 definitions")
        .extend(
            fragment["$defs"]
                .as_object()
                .expect("presentation definitions")
                .clone(),
        );
    schema["allOf"]
        .as_array_mut()
        .expect("v2 admission rules")
        .extend(
            fragment["allOf"]
                .as_array()
                .expect("presentation admission")
                .clone(),
        );
    schema["$defs"]["source"]["properties"]["kind"]["enum"] =
        json!(["artifact", "collection", "composition", "presentation"]);
    schema["$defs"]["executionPolicy"]["properties"]["presentation"] =
        json!({"anyOf":[{"$ref":"#/$defs/presentationBuildPolicy"},{"type":"null"}]});
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> SpecV2 {
        crate::spec::load_spec_str("schema: renderflow/v2\nsources:\n  - id: deck\n    kind: presentation\n    path: deck.json\ntargets:\n  exact:\n    - format: presentation_source\n").unwrap().spec
    }
    #[test]
    fn presentation_admission_rejects_ignored_policies_and_wrong_source() {
        assert!(spec().validate().is_empty());
        let mut s = spec();
        s.execution.max_parallel = 2;
        assert!(!s.validate().is_empty());
        let mut s = spec();
        s.sources[0].path = Some("../private.json".into());
        assert!(!s.validate().is_empty());
        let mut s = spec();
        s.execution.presentation = Some(PresentationBuildPolicy::default());
        s.sources[0].kind = SourceKind::Artifact;
        assert!(s
            .validate()
            .iter()
            .any(|d| d.code == "presentation.source.required"));
    }
    #[test]
    fn source_web_and_pdf_are_distinct_and_targets_are_closed() {
        let mut s = spec();
        s.targets.exact.push(TargetSpec {
            format: Some("presentation_web".into()),
            ..TargetSpec::default()
        });
        assert_eq!(targets(&s.targets.exact).unwrap().len(), 2);
        s.targets.exact.push(s.targets.exact[0].clone());
        assert!(targets(&s.targets.exact).is_err());
    }
}
