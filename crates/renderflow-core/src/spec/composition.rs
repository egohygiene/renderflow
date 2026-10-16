//! Bounded admission of native composition packages through `renderflow/v2`.
//!
//! Keep locale/output semantics in the native model. This module only rejects
//! general build options that the composition backend cannot honor.

use std::collections::BTreeSet;
use std::path::{Component, Path};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::composition::{CompositionOutput, CompositionRequest, LocaleSelector, PdfSettings, RasterSettings};

use super::{CollisionPolicy, ExecutionPolicy, IntermediatePolicy, SourceKind, SpecDiagnostic, SpecV2, TargetRequirement, TargetSpec};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionBuildPolicy {
    #[serde(default = "default_locales")]
    pub locales: LocaleSelector,
    /// Used only by selected PNG branches; native planning reports bad settings.
    #[serde(default)]
    pub raster: RasterSettings,
    /// Used only by selected PDF branches; native planning reports bad settings.
    #[serde(default)]
    pub pdf: PdfSettings,
    /// Optional local terminal-output cache, relative to the configuration directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_directory: Option<String>,
}

fn default_locales() -> LocaleSelector { LocaleSelector::AllDeclared }

impl Default for CompositionBuildPolicy {
    fn default() -> Self {
        Self {
            locales: default_locales(),
            raster: RasterSettings::default(),
            pdf: PdfSettings::default(),
            cache_directory: None,
        }
    }
}

impl CompositionBuildPolicy {
    /// Translate exact native target names without inventing a second output
    /// enum, locale selector, or rendering policy for the ordinary build path.
    pub fn request(&self, targets: &[TargetSpec]) -> Result<CompositionRequest> {
        ensure!(!targets.is_empty(), "composition.targets.empty: at least one exact output is required");
        let targets = targets.iter().map(output_for_target).collect::<Result<Vec<_>>>()?;
        ensure!(targets.iter().copied().collect::<BTreeSet<_>>().len() == targets.len(),
            "composition.targets.duplicate: each output format must be requested once");
        Ok(CompositionRequest {
            locales: self.locales.clone(), targets,
            raster: self.raster.clone(), pdf: self.pdf.clone(),
        })
    }
}

fn output_for_target(target: &TargetSpec) -> Result<CompositionOutput> {
    ensure!(target.id.is_none() && target.role.is_none() && target.family.is_none()
        && target.capability.is_none() && target.transform.is_none() && target.provider.is_none()
        && target.variant.is_none() && target.preset.is_none() && target.template.is_none()
        && target.options.is_empty() && target.requirement == TargetRequirement::Required,
        "composition targets support only format and requirement: required; locale identities come from the composition package");
    let format = target.format.as_ref().context("composition targets require a native output format")?;
    format.parse()
        .context("unknown composition output; use svg, html, transcript_json, transcript_text, png, pdf, ora, psd, or idml")
}

pub(super) fn validate(spec: &SpecV2, diagnostics: &mut Vec<SpecDiagnostic>) {
    let composition_sources = spec.sources.iter().filter(|source| source.kind == SourceKind::Composition).count();
    if composition_sources == 0 {
        if spec.execution.composition.is_some() {
            diagnostics.push(SpecDiagnostic::new("$.execution.composition", "composition.source.required",
                "execution.composition requires exactly one source with kind: composition"));
        }
        return;
    }
    let mut reject = |path: &str, message: &str| {
        diagnostics.push(SpecDiagnostic::new(path, "composition.build.unsupported", message));
    };
    if composition_sources != 1 || spec.sources.len() != 1 {
        reject("$.sources", "composition builds require exactly one local composition source; artifact and collection sources belong in a separate build");
    }
    for (index, source) in spec.sources.iter().enumerate().filter(|(_, source)| source.kind == SourceKind::Composition) {
        let base = format!("$.sources[{index}]");
        if !source.path.as_deref().is_some_and(local_path_without_parent_traversal) {
            reject(&format!("{base}.path"), "composition sources require a nonempty local manifest path without parent traversal");
        }
        for (field, unsupported) in [
            ("uri", source.uri.is_some()), ("members", !source.members.is_empty()),
            ("sha256", source.sha256.is_some()), ("geometry", source.geometry.is_some()),
            ("format", source.format.is_some()), ("media_type", source.media_type.is_some()),
            ("role", source.role.is_some()), ("detect", !source.detect),
        ] {
            if unsupported {
                reject(&format!("{base}.{field}"), "this source field is not supported by composition builds; the package declares and pins its own inputs");
            }
        }
    }
    if !spec.profiles.is_empty() { reject("$.profiles", "composition builds do not yet support named derivative profiles"); }
    if !spec.hygiene.is_empty() { reject("$.hygiene", "composition builds do not yet support publication hygiene policies"); }
    if spec.publication.is_some() { reject("$.publication", "composition page geometry and provenance belong in the composition package"); }
    if !spec.variables.is_empty() { reject("$.variables", "composition builds do not substitute build variables"); }
    if spec.transforms.is_some() { reject("$.transforms", "composition builds use native output routes and do not accept custom transform registries"); }
    if !spec.targets.profiles.is_empty() { reject("$.targets.profiles", "composition builds require explicit native output formats"); }
    if spec.targets.all_reachable { reject("$.targets.all_reachable", "composition builds require explicit native output formats"); }
    if !spec.targets.include.is_empty() { reject("$.targets.include", "composition builds do not expand target selectors"); }
    if !spec.targets.exclude.is_empty() { reject("$.targets.exclude", "composition builds do not filter target selectors"); }
    if spec.targets.intermediates != IntermediatePolicy::CacheOnly {
        reject("$.targets.intermediates", "composition stages are ephemeral; retained intermediate artifacts are not supported");
    }
    if spec.output.naming_template != super::default_naming_template() {
        reject("$.output.naming_template", "composition builds use fixed outputs/{locale}/ names; custom naming templates are not supported");
    }
    if spec.output.collision != CollisionPolicy::Error {
        reject("$.output.collision", "composition builds require collision: error and a fresh output bundle");
    }
    if !local_path_without_parent_traversal(&spec.output.bundle_root) {
        reject("$.output.bundle_root", "composition output bundles require a nonempty local path without parent traversal");
    }

    let defaults = ExecutionPolicy::default();
    let policy = &spec.execution;
    // A native, in-process backend satisfies explicit local/offline requests.
    // Determinism and fidelity claims still require the deferred qualification.
    for (field, unsupported) in [
        ("optimization", policy.optimization != defaults.optimization),
        ("derivative_planning", policy.derivative_planning != defaults.derivative_planning),
        ("max_parallel", policy.max_parallel != defaults.max_parallel),
        ("budgets", policy.budgets != defaults.budgets),
        ("tools", policy.tools != defaults.tools),
        ("transforms", policy.transforms != defaults.transforms),
        ("requirements.deterministic", policy.requirements.deterministic),
        ("network", policy.network != defaults.network), ("ai", policy.ai != defaults.ai),
        ("retry_policy", policy.retry_policy.is_some()), ("timeout_policy", policy.timeout_policy.is_some()),
        ("validation", policy.validation != defaults.validation),
        ("minimum_fidelity", policy.minimum_fidelity.is_some()),
        ("reject_loss_classes", !policy.reject_loss_classes.is_empty()),
        ("publication_policy", policy.publication_policy.is_some()),
        ("redaction_policy", policy.redaction_policy.is_some()), ("hygiene_policy", policy.hygiene_policy.is_some()),
        ("print_pdf_interior", policy.print_pdf_interior.is_some()),
        ("fixed_layout_epub", policy.fixed_layout_epub.is_some()), ("subtitles", policy.subtitles.is_some()),
        ("speech", policy.speech.is_some()), ("screenplay", policy.screenplay.is_some()),
        ("screenplay_body", policy.screenplay_body.is_some()),
        ("screenplay_pdf", policy.screenplay_pdf.is_some()), ("screenplay_recovery", policy.screenplay_recovery.is_some()),
    ] {
        if unsupported {
            reject(&format!("$.execution.{field}"), "this execution policy is not supported by the bounded composition backend; use the default or a separate artifact build");
        }
    }
    if let Some(policy) = &policy.composition {
        if policy.cache_directory.as_deref().is_some_and(|path| !local_path_without_parent_traversal(path)) {
            reject("$.execution.composition.cache_directory", "composition caches require a nonempty local path without parent traversal");
        }
        match &policy.locales {
            LocaleSelector::Exact { locale } if locale.trim().is_empty() || locale.contains('\0') => {
                reject("$.execution.composition.locales.locale", "an exact locale must be nonempty; the package must declare the requested locale");
            }
            LocaleSelector::Subset { locales } => {
                if locales.is_empty() || locales.len() > 64
                    || locales.iter().any(|locale| locale.trim().is_empty() || locale.contains('\0'))
                    || locales.iter().collect::<BTreeSet<_>>().len() != locales.len()
                {
                    reject("$.execution.composition.locales.locales", "a locale subset must contain 1 to 64 distinct nonempty locale names");
                }
            }
            _ => {},
        }
    }
    // Target parsing uses the native enum's serde vocabulary. Unadopted native
    // outputs remain representable and produce unavailable branches in planning.
    let mut outputs = BTreeSet::new();
    for (index, target) in spec.targets.exact.iter().enumerate() {
        let path = format!("$.targets.exact[{index}]");
        match output_for_target(target) {
            Ok(output) if outputs.insert(output) => {},
            Ok(_) => reject(&path, "composition output formats must be distinct; locale expansion happens inside the composition planner"),
            Err(error) => reject(&path, &format!("{error:#}")),
        }
    }
}

fn local_path_without_parent_traversal(path: &str) -> bool {
    !path.trim().is_empty() && !path.contains('\0') && !path.contains("://")
        && !Path::new(path).components().any(|component| component == Component::ParentDir)
}

pub(super) fn extend_schema(schema: &mut Value) {
    // This additive schema fragment keeps the general v2 definitions stable and
    // shares output/renderer settings with the versioned native contract.
    let additions = json!({
        "$defs": {
            "compositionBuildPolicy": {
                "type": "object", "additionalProperties": false,
                "description": "Native composition settings. Omission selects all declared locales and fixed renderer defaults. Settings only affect matching selected output formats; invalid selected renderer settings become branch-local diagnostics.",
                "properties": {
                    "locales": {"$ref": "#/$defs/compositionLocales", "default": {"mode": "all_declared"}},
                    "raster": {"$ref": "renderflow-composition-evidence-v5.schema.json#/$defs/rasterSettings"},
                    "pdf": {"$ref": "renderflow-composition-evidence-v5.schema.json#/$defs/pdfSettings"},
                    "cache_directory": {"type": ["string", "null"], "minLength": 1, "description": "Optional local terminal-output cache, resolved relative to the configuration directory. Parent traversal and symlink components are refused."}
                }
            },
            "compositionLocales": {
                "oneOf": [
                    {"type": "object", "additionalProperties": false, "required": ["mode"], "properties": {"mode": {"const": "all_declared"}}},
                    {"type": "object", "additionalProperties": false, "required": ["mode", "locale"], "properties": {"mode": {"const": "exact"}, "locale": {"type": "string", "minLength": 1, "pattern": "\\S"}}},
                    {"type": "object", "additionalProperties": false, "required": ["mode", "locales"], "properties": {"mode": {"const": "subset"}, "locales": {"type": "array", "minItems": 1, "maxItems": 64, "uniqueItems": true, "items": {"type": "string", "minLength": 1, "pattern": "\\S"}}}}
                ]
            },
            "compositionSource": {
                "required": ["kind", "path"],
                "description": "One local composition manifest; assets, content and fonts are declared and pinned inside its package. Paths resolve relative to the configuration directory. Parent traversal and symlink components are refused.",
                "properties": {
                    "kind": {"const": "composition"}, "path": {"type": "string", "minLength": 1},
                    "uri": {"type": "null"}, "members": {"maxItems": 0}, "sha256": {"type": "null"},
                    "geometry": {"type": "null"}, "format": {"type": "null"}, "media_type": {"type": "null"},
                    "role": {"type": "null"}, "detect": {"const": true}
                }
            },
            "compositionTargets": {
                "required": ["exact"],
                "properties": {
                    "exact": {"type": "array", "minItems": 1, "uniqueItems": true, "items": {
                        "required": ["format"],
                        "properties": {
                            "format": {"$ref": "renderflow-composition-evidence-v5.schema.json#/$defs/output"},
                            "requirement": {"const": "required"}, "options": {"maxProperties": 0},
                            "id": {"type": "null"}, "role": {"type": "null"}, "family": {"type": "null"},
                            "capability": {"type": "null"}, "transform": {"type": "null"}, "provider": {"type": "null"},
                            "variant": {"type": "null"}, "preset": {"type": "null"}, "template": {"type": "null"}
                        }
                    }},
                    "profiles": {"maxItems": 0}, "all_reachable": {"const": false},
                    "include": {"$ref": "#/$defs/compositionEmptySelectors"}, "exclude": {"$ref": "#/$defs/compositionEmptySelectors"},
                    "intermediates": {"const": "cache_only"}
                }
            },
            "compositionEmptySelectors": {"additionalProperties": {"type": "array", "maxItems": 0}},
            "compositionExecution": {
                "description": "Only native composition settings and explicit local/offline requirements are configurable. Other execution fields must retain their ordinary defaults; runtime/visual qualification is not asserted.",
                "properties": {
                    "optimization": {"const": "balanced"}, "max_parallel": {"const": 1},
                    "derivative_planning": {"properties": {"schema": {"const": "renderflow/derivative-policy-v1"}, "max_hops": {"const": 8}, "max_candidates": {"const": 10000}, "max_loss": {"type": "null"}}},
                    "budgets": {"additionalProperties": {"type": "null"}},
                    "tools": {"additionalProperties": {"type": "array", "maxItems": 0}},
                    "transforms": {"additionalProperties": {"type": "array", "maxItems": 0}},
                    "requirements": {"properties": {"deterministic": {"const": false}}},
                    "network": {"const": "deny"}, "ai": {"const": "deny"},
                    "retry_policy": {"type": "null"}, "timeout_policy": {"type": "null"},
                    "validation": {"properties": {"required": {"const": true}, "validators": {"maxItems": 0}, "failure_mode": {"const": "branch_local"}, "allow_unavailable": {"const": false}}},
                    "minimum_fidelity": {"type": "null"}, "reject_loss_classes": {"maxItems": 0},
                    "publication_policy": {"type": "null"}, "redaction_policy": {"type": "null"}, "hygiene_policy": {"type": "null"},
                    "print_pdf_interior": {"type": "null"}, "fixed_layout_epub": {"type": "null"}, "subtitles": {"type": "null"},
                    "speech": {"type": "null"}, "screenplay": {"type": "null"}, "screenplay_body": {"type": "null"}, "screenplay_pdf": {"type": "null"}, "screenplay_recovery": {"type": "null"}
                }
            }
        },
        "allOf": [{
            "if": {"properties": {"sources": {"contains": {"required": ["kind"], "properties": {"kind": {"const": "composition"}}}}}},
            "then": {"properties": {
                "sources": {"minItems": 1, "maxItems": 1, "items": {"$ref": "#/$defs/compositionSource"}},
                "targets": {"$ref": "#/$defs/compositionTargets"}, "execution": {"$ref": "#/$defs/compositionExecution"},
                "profiles": {"maxProperties": 0}, "hygiene": {"maxProperties": 0}, "variables": {"maxProperties": 0},
                "publication": {"type": "null"}, "transforms": {"type": "null"},
                "output": {"properties": {"naming_template": {"const": "{source.id}/{target.role}.{ext}"}, "collision": {"const": "error"}}}
            }},
            "else": {"properties": {"execution": {"properties": {"composition": {"type": "null"}}}}}
        }]
    });
    schema["$defs"].as_object_mut().expect("v2 schema definitions").extend(
        additions["$defs"].as_object().expect("composition schema definitions").clone(),
    );
    schema["allOf"] = additions["allOf"].clone();
    schema["$defs"]["source"]["properties"]["kind"]["enum"] = json!(["artifact", "collection", "composition"]);
    schema["$defs"]["executionPolicy"]["properties"]["composition"] = json!({
        "anyOf": [{"$ref": "#/$defs/compositionBuildPolicy"}, {"type": "null"}]
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::load_spec_str;

    const MINIMAL: &str = r#"
schema: renderflow/v2
sources:
  - id: page
    kind: composition
    path: composition/page.json
targets:
  exact:
    - format: svg
    - format: transcript_json
"#;

    fn minimal() -> SpecV2 { load_spec_str(MINIMAL).unwrap().spec }

    #[test]
    fn minimal_composition_build_uses_native_defaults_without_an_empty_policy() {
        let spec = minimal();
        assert!(spec.execution.composition.is_none());
        let request = CompositionBuildPolicy::default().request(&spec.targets.exact).unwrap();
        assert_eq!(request.locales, LocaleSelector::AllDeclared);
        assert_eq!(request.targets, vec![CompositionOutput::Svg, CompositionOutput::TranscriptJson]);
        assert_eq!(request.raster, RasterSettings::default());
        assert_eq!(request.pdf, PdfSettings::default());
        let mut explicit = spec;
        explicit.execution.composition = Some(CompositionBuildPolicy::default());
        explicit.execution.requirements.local_only = true;
        explicit.execution.requirements.offline = true;
        assert!(explicit.validate().is_empty());
    }

    #[test]
    fn native_output_names_and_branch_local_renderer_settings_are_preserved() {
        let mut spec = minimal();
        spec.targets.exact = ["svg", "html", "transcript_json", "transcript_text", "png", "pdf", "ora", "psd", "idml"]
            .into_iter().map(|format| TargetSpec { format: Some(format.to_owned()), ..TargetSpec::default() }).collect();
        let mut policy = CompositionBuildPolicy::default();
        policy.raster.dpi = 0;
        policy.pdf.dpi = 0;
        policy.pdf.background = "transparent".to_owned();
        spec.execution.composition = Some(policy.clone());
        assert!(spec.validate().is_empty(), "invalid renderer settings are native branch-local decisions");
        let request = policy.request(&spec.targets.exact).unwrap();
        assert_eq!(request.targets.len(), 9);
        assert_eq!(request.raster.dpi, 0);
        assert_eq!(request.pdf.background, "transparent");
        spec.targets.exact.retain(|target| target.format.as_deref() == Some("svg"));
        assert!(spec.validate().is_empty(), "settings for deselected output kinds permit a --target override");
    }

    #[test]
    fn composition_sources_and_policy_cannot_leak_into_general_builds() {
        let mut spec = minimal();
        spec.sources.push(spec.sources[0].clone());
        spec.sources[1].id = "other".to_owned();
        spec.sources[1].kind = SourceKind::Artifact;
        assert!(spec.validate().iter().any(|item| item.path == "$.sources"));
        let mut spec = minimal();
        spec.sources[0].kind = SourceKind::Artifact;
        spec.execution.composition = Some(CompositionBuildPolicy::default());
        assert!(spec.validate().iter().any(|item| item.code == "composition.source.required"));
    }

    #[test]
    fn unsupported_source_target_and_execution_intent_is_never_silently_ignored() {
        let base = serde_json::to_value(minimal()).unwrap();
        let cases = [
            ("source", "role", json!("cover")), ("source", "detect", json!(false)),
            ("source", "format", json!("json")), ("source", "uri", json!("https://example.test/page.json")),
            ("target", "id", json!("chosen")), ("target", "role", json!("preview")),
            ("target", "provider", json!("native")), ("target", "requirement", json!("optional")),
            ("target", "options", json!({"dpi": 72})), ("target", "format", json!("json")),
            ("execution", "max_parallel", json!(2)), ("execution", "network", json!("allow")),
            ("execution", "ai", json!("allow")), ("execution", "optimization", json!("quality")),
            ("execution", "requirements", json!({"deterministic": true})),
            ("execution", "budgets", json!({"max_output_bytes": 1024})),
            ("execution", "validation", json!({"allow_unavailable": true})),
            ("execution", "subtitles", json!({})),
            ("execution", "screenplay_body", json!({"allow_unqualified": true})),
        ];
        for (section, field, value) in cases {
            let mut candidate = base.clone();
            match section {
                "source" => candidate["sources"][0][field] = value,
                "target" => candidate["targets"]["exact"][0][field] = value,
                _ => candidate["execution"][field] = value,
            }
            let candidate: SpecV2 = serde_json::from_value(candidate).unwrap();
            assert!(candidate.validate().iter().any(|item| item.code == "composition.build.unsupported"),
                "ignored {section}.{field}");
        }
        let mut duplicate = minimal();
        duplicate.targets.exact.push(duplicate.targets.exact[0].clone());
        assert!(duplicate.validate().iter().any(|item| item.path == "$.targets.exact[2]"));
    }

    #[test]
    fn unsupported_selection_layout_and_generic_maps_are_rejected() {
        let base = serde_json::to_value(minimal()).unwrap();
        let cases = [
            ("profiles", "unused", json!({})), ("variables", "title", json!("ignored")),
            ("targets", "all_reachable", json!(true)), ("targets", "include", json!({"formats": ["png"]})),
            ("targets", "intermediates", json!("retain")),
            ("output", "naming_template", json!("custom/{ext}")), ("output", "collision", json!("replace")),
        ];
        for (section, field, value) in cases {
            let mut candidate = base.clone();
            candidate[section][field] = value;
            let candidate: SpecV2 = serde_json::from_value(candidate).unwrap();
            assert!(candidate.validate().iter().any(|item| item.code == "composition.build.unsupported"),
                "ignored {section}.{field}");
        }
    }

    #[test]
    fn locale_and_local_path_declarations_fail_before_execution() {
        for locales in [
            LocaleSelector::Exact { locale: " ".to_owned() },
            LocaleSelector::Subset { locales: Vec::new() },
            LocaleSelector::Subset { locales: vec!["en".to_owned(), "en".to_owned()] },
        ] {
            let mut spec = minimal();
            spec.execution.composition = Some(CompositionBuildPolicy { locales, ..CompositionBuildPolicy::default() });
            assert!(spec.validate().iter().any(|item| item.path.starts_with("$.execution.composition.locales")));
        }
        for path in ["", "https://example.test/output", "../dist", "dist/../cache"] {
            let mut spec = minimal();
            spec.sources[0].path = Some(path.to_owned());
            assert!(spec.validate().iter().any(|item| item.path == "$.sources[0].path"));
            let mut spec = minimal();
            spec.output.bundle_root = path.to_owned();
            assert!(spec.validate().iter().any(|item| item.path == "$.output.bundle_root"));
            let mut spec = minimal();
            spec.execution.composition = Some(CompositionBuildPolicy {
                cache_directory: Some(path.to_owned()), ..CompositionBuildPolicy::default()
            });
            assert!(spec.validate().iter().any(|item| item.path == "$.execution.composition.cache_directory"));
        }
    }

    #[test]
    fn published_composition_schema_fragment_matches_runtime_contract() {
        let runtime = crate::spec::json_schema();
        let published: Value = serde_json::from_str(include_str!("../../../../schemas/renderflow-v2.schema.json")).unwrap();
        for key in ["compositionBuildPolicy", "compositionLocales", "compositionSource", "compositionTargets", "compositionEmptySelectors", "compositionExecution"] {
            assert_eq!(runtime["$defs"][key], published["$defs"][key]);
        }
        assert_eq!(runtime["allOf"], published["allOf"]);
        assert_eq!(runtime["$defs"]["source"]["properties"]["kind"], published["$defs"]["source"]["properties"]["kind"]);
        assert_eq!(runtime["$defs"]["executionPolicy"]["properties"]["composition"], published["$defs"]["executionPolicy"]["properties"]["composition"]);
        assert!(serde_json::from_value::<CompositionBuildPolicy>(json!({"unknown": true})).is_err());
    }
}
