//! Closed source/policy admission for the existing frozen local publisher.
use super::{
    CollisionPolicy, ExecutionPolicy, IntermediatePolicy, SourceKind, SpecDiagnostic, SpecV2,
    TargetRequirement, TargetSpec,
};
use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefingBuildPolicy {
    #[serde(default)]
    pub allow_draft: bool,
}
pub(crate) fn filename(target: &str) -> Option<&'static str> {
    match target {
        "briefing_script" => Some("script.json"),
        "briefing_text" => Some("transcript.txt"),
        "briefing_html" => Some("transcript.html"),
        "briefing_chapters" => Some("chapters.json"),
        "briefing_feed" => Some("feed.rss"),
        "briefing_audio" => Some("narration.wav"),
        _ => None,
    }
}
pub(crate) fn targets(values: &[TargetSpec]) -> Result<Vec<String>> {
    ensure!(
        !values.is_empty() && values.len() <= 6,
        "briefing.targets: select one to six exact primary representations"
    );
    let mut seen = BTreeSet::new();
    for target in values {
        ensure!(
            target.id.is_none()
                && target.role.is_none()
                && target.family.is_none()
                && target.capability.is_none()
                && target.transform.is_none()
                && target.provider.is_none()
                && target.variant.is_none()
                && target.preset.is_none()
                && target.template.is_none()
                && target.options.is_empty()
                && target.requirement == TargetRequirement::Required,
            "briefing.targets: only required exact formats are supported"
        );
        let name = target
            .format
            .as_deref()
            .filter(|v| filename(v).is_some())
            .ok_or_else(|| anyhow::anyhow!("briefing.targets: unsupported representation"))?;
        ensure!(
            seen.insert(name.to_owned()),
            "briefing.targets: duplicate representation"
        );
    }
    Ok(seen.into_iter().collect())
}
fn local(value: &str) -> bool {
    !value.trim().is_empty()
        && !value.contains('\0')
        && !value.contains("://")
        && !std::path::Path::new(value)
            .components()
            .any(|c| c == std::path::Component::ParentDir)
}
fn admitted(spec: &SpecV2) -> Result<()> {
    ensure!(
        spec.sources.len() == 1,
        "briefing.source: exactly one local source"
    );
    let s = &spec.sources[0];
    ensure!(
        s.kind == SourceKind::Briefing
            && s.path.as_deref().is_some_and(local)
            && s.uri.is_none()
            && s.members.is_empty()
            && s.sha256.is_none()
            && s.geometry.is_none()
            && s.format.is_none()
            && s.media_type.is_none()
            && s.role.is_none()
            && s.detect
            && s.immutable,
        "briefing.source: use only id, kind: briefing and a confined local model path"
    );
    targets(&spec.targets.exact)?;
    ensure!(
        spec.profiles.is_empty()
            && spec.targets.profiles.is_empty()
            && !spec.targets.all_reachable
            && spec.targets.include.is_empty()
            && spec.targets.exclude.is_empty()
            && spec.targets.intermediates == IntermediatePolicy::CacheOnly
            && spec.hygiene.is_empty()
            && spec.publication.is_none()
            && spec.transforms.is_none()
            && spec.variables.is_empty(),
        "briefing.build: selectors, profiles and other transformations are unsupported"
    );
    ensure!(
        spec.output.collision == CollisionPolicy::Error
            && local(&spec.output.bundle_root)
            && spec.output.naming_template == super::default_naming_template(),
        "briefing.output: require a new local directory and default naming"
    );
    let mut effective = spec.execution.clone();
    effective.briefing = None;
    let mut defaults = ExecutionPolicy::default();
    defaults.requirements.local_only = effective.requirements.local_only;
    defaults.requirements.offline = effective.requirements.offline;
    ensure!(
        effective == defaults,
        "briefing.execution: only briefing policy and local/offline intent are admitted"
    );
    Ok(())
}
pub(super) fn validate(spec: &SpecV2, diagnostics: &mut Vec<SpecDiagnostic>) {
    if spec.sources.iter().any(|s| s.kind == SourceKind::Briefing) {
        if let Err(e) = admitted(spec) {
            diagnostics.push(SpecDiagnostic::new(
                "$",
                "briefing.build.unsupported",
                format!("{e:#}"),
            ));
        }
    } else if spec.execution.briefing.is_some() {
        diagnostics.push(SpecDiagnostic::new(
            "$.execution.briefing",
            "briefing.source.required",
            "briefing policy requires a briefing source",
        ));
    }
}
pub(super) fn extend_schema(schema: &mut Value) {
    let fragment: Value = serde_json::from_str(include_str!(
        "../../../../schemas/fragments/briefing-build-v1.json"
    ))
    .expect("authored briefing fragment");
    schema["$defs"]
        .as_object_mut()
        .expect("v2 definitions")
        .extend(
            fragment["$defs"]
                .as_object()
                .expect("briefing definitions")
                .clone(),
        );
    let rules = schema["allOf"].as_array_mut().expect("v2 rules");
    for rule in fragment["allOf"]
        .as_array()
        .expect("briefing admission")
        .iter()
        .rev()
    {
        rules.insert(0, rule.clone());
    }
    schema["$defs"]["source"]["properties"]["kind"]["enum"] = json!([
        "artifact",
        "collection",
        "composition",
        "presentation",
        "briefing"
    ]);
    schema["$defs"]["executionPolicy"]["properties"]["briefing"] =
        json!({"anyOf":[{"$ref":"#/$defs/briefingBuildPolicy"},{"type":"null"}]});
}
