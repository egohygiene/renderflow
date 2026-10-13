//! Optional creative-provider discovery without changing graph eligibility.
//!
//! This is an observation report, not an executable/model attestation or an
//! execution cache key. Provider discovery and authored adapter support are
//! deliberately separate: an installed candidate does not create a graph edge.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::toolchain::{
    ProcessToolProbe, ToolAvailability, ToolAvailabilityStatus, ToolDescriptor, ToolProbe, ToolRegistry,
    ToolRuntimeContext,
};

pub const CREATIVE_DOCTOR_SCHEMA: &str = "renderflow.creative-doctor/v1";
pub const CREATIVE_DOCTOR_PROFILE: &str = "creative-publishing";
pub const CREATIVE_FINGERPRINT_SCOPE: &str = "provider-discovery-observations/v1";

/// Optional creative tools excluded from plain `doctor`. Explicit profile and
/// complete inventories may report them without selecting execution providers.
pub const CREATIVE_OPTIONAL_TOOL_IDS: [&str; 6] = [
    "tool.scripttool", "tool.node", "tool.afterwriting", "tool.pdftotext", "tool.pdftoppm",
    "tool.renderflow-screenplay-recovery",
];

pub const CREATIVE_TOOL_IDS: [&str; 10] = [
    "tool.afterwriting", "tool.ffmpeg", "tool.ffprobe", "tool.node",
    "tool.pdftoppm", "tool.pdftotext", "tool.renderflow-screenplay-recovery",
    "tool.scripttool", "tool.tesseract", "tool.whisper_cpp",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreativeToolReport {
    pub availability: ToolAvailability,
    /// The exact selected registry declaration, including probe arguments,
    /// version constraints, capability IDs, and executable license notes.
    pub descriptor: Option<ToolDescriptor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreativeRouteImplementation {
    NotImplemented,
    AuthoredUnqualified,
}

impl CreativeRouteImplementation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotImplemented => "not_implemented",
            Self::AuthoredUnqualified => "authored_unqualified",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CreativeProviderStatus {
    Available,
    Unavailable,
    Unobserved,
}

impl CreativeProviderStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::Unobserved => "unobserved",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreativeRouteReport {
    pub id: String,
    pub implementation: CreativeRouteImplementation,
    /// Only the conjunction of declared provider discovery results. This is
    /// never a statement that a conversion or inference has been qualified.
    pub provider_status: CreativeProviderStatus,
    pub providers: Vec<String>,
    pub blocked_providers: Vec<String>,
    pub required_configuration: Vec<String>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreativeRequirementReport {
    pub id: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CreativeDoctorReport {
    pub schema: String,
    pub profile: String,
    pub fingerprint: String,
    pub fingerprint_scope: String,
    pub operating_system: String,
    pub architecture: String,
    pub registry_schema: String,
    pub tools: Vec<CreativeToolReport>,
    pub routes: Vec<CreativeRouteReport>,
    pub requirements: Vec<CreativeRequirementReport>,
    pub limitations: Vec<String>,
}

impl CreativeDoctorReport {
    /// Count discovery failures only. Missing models/fonts and unimplemented
    /// adapters remain visible without masquerading as executable failures.
    pub fn unavailable_tool_count(&self) -> usize {
        self.tools.iter().filter(|tool| !matches!(tool.availability.status,
            ToolAvailabilityStatus::Available | ToolAvailabilityStatus::NotProbed)).count()
    }

    pub fn unprobed_tool_count(&self) -> usize {
        self.tools.iter().filter(|tool| tool.availability.status == ToolAvailabilityStatus::NotProbed).count()
    }

    pub fn strict_success(&self) -> bool {
        self.unavailable_tool_count() == 0
    }
}

/// Run the bounded version probes declared by this optional profile. No media,
/// model, font, conversion, installer, or download operation is requested.
pub fn assess_current() -> Result<CreativeDoctorReport> {
    assess_with(
        &ToolRegistry::builtins(),
        &ProcessToolProbe::new(),
        &ToolRuntimeContext::current(),
    )
}

/// Assess only the profile members through an injected probe. No graph or
/// registry mutation occurs, and one failed provider never stops its peers.
pub fn assess_with(
    registry: &ToolRegistry,
    probe: &dyn ToolProbe,
    context: &ToolRuntimeContext,
) -> Result<CreativeDoctorReport> {
    let declarations = route_definitions()?;
    let inventory = registry.assess_ids_with(CREATIVE_TOOL_IDS, probe, context);
    let tools = inventory.tools.into_iter().map(|availability| {
        let descriptor = registry.get(availability.id.as_str()).cloned();
        CreativeToolReport { availability, descriptor }
    }).collect::<Vec<_>>();
    let mut routes = declarations.into_iter().map(|route| {
        let mut providers = route.providers;
        providers.sort();
        let blocked_providers = providers.iter().filter(|id| {
            !tools.iter().any(|tool| tool.availability.id.as_str() == id.as_str()
                && tool.availability.is_available())
        }).cloned().collect::<Vec<_>>();
        let unobserved_only = !blocked_providers.is_empty() && blocked_providers.iter().all(|id| {
            tools.iter().any(|tool| tool.availability.id.as_str() == id.as_str()
                && tool.availability.status == ToolAvailabilityStatus::NotProbed)
        });
        CreativeRouteReport {
            id: route.id,
            implementation: route.implementation,
            provider_status: if blocked_providers.is_empty() {
                CreativeProviderStatus::Available
            } else if unobserved_only {
                CreativeProviderStatus::Unobserved
            } else {
                CreativeProviderStatus::Unavailable
            },
            providers,
            blocked_providers,
            required_configuration: route.required_configuration,
            detail: route.detail,
        }
    }).collect::<Vec<_>>();
    routes.sort_by(|left, right| left.id.cmp(&right.id));
    let requirements = vec![
        requirement("ocr.language_data", "unobserved", "Tesseract language data, its exact identity, and its separate license are not inspected by doctor. Screenplay OCR fallback requires explicitly pinned language data and remains unqualified."),
        requirement("screenplay.recovery.review", "operator_review_required", "PDF/plain-text recovery and its exports are lossy candidates. Extraction evidence, per-element confidence, ambiguities, and review-required status remain attached; discovery and global AI allowance cannot approve a candidate."),
        requirement("screenplay.fonts", "unobserved", "The fixed A4/US Letter screenplay PDF profiles require pinned Courier and Helvetica font metrics in the selected package. Base14 Courier remains unembedded; viewer substitution, visual fidelity, and exact asset licensing are unqualified. Doctor does not inspect package or font bytes."),
        requirement("speech.model.configuration", "configuration_required", "Local speech execution requires an explicit model path, model ID, expected model SHA-256, and a pinned provider path/SHA-256. Doctor does not inspect model files or infer those settings from PATH."),
        requirement("speech.model.license", "operator_review_required", "Review the exact speech model's license separately from the executable and linked-library licenses before use or redistribution."),
    ];
    let mut report = CreativeDoctorReport {
        schema: CREATIVE_DOCTOR_SCHEMA.to_owned(),
        profile: CREATIVE_DOCTOR_PROFILE.to_owned(),
        fingerprint: String::new(),
        fingerprint_scope: CREATIVE_FINGERPRINT_SCOPE.to_owned(),
        operating_system: context.operating_system.clone(),
        architecture: context.architecture.clone(),
        registry_schema: registry.schema().to_owned(),
        tools,
        routes,
        requirements,
        limitations: vec![
            "The fingerprint covers selected registry declarations, observed discovery states/versions, platform labels, and report policy. It is not an executable-byte, model, font, linked-library, or environment attestation and must not be used as an execution cache key.".to_owned(),
            "Available providers do not establish qualified conversion routes or select execution providers. Strict success concerns assessable provider discovery only. Node and Afterwriting remain not_probed here because their exact installation is observed only by configured screenplay PDF preflight; they do not fail strict mode. Explicit source/configuration requirements and runtime qualification still apply.".to_owned(),
            "Doctor requests declared local discovery probes only. It does not install providers, download models, inspect user media, run recognition, or mutate planner eligibility.".to_owned(),
        ],
    };
    // The empty fingerprint is a fixed domain member, not a self-referential
    // digest. No timestamp or probe duration enters the serialized material.
    let material = serde_json::to_vec(&report).context("failed to serialize creative doctor fingerprint material")?;
    report.fingerprint = format!("sha256:{:x}", Sha256::digest(material));
    Ok(report)
}

fn requirement(id: &str, status: &str, detail: &str) -> CreativeRequirementReport {
    CreativeRequirementReport { id: id.to_owned(), status: status.to_owned(), detail: detail.to_owned() }
}

/// Shared source for the doctor and generated creative capability matrix.
/// Documentation metadata remains outside `routes` so publishing links does not
/// change the doctor's route wire shape or its discovery fingerprint.
const CREATIVE_ROUTE_CATALOG: &str = include_str!("../data/creative-routes.json");
const CREATIVE_ROUTE_CATALOG_SCHEMA: &str = "renderflow.creative-route-catalog/v1";

#[derive(Debug, Deserialize)]
struct RouteCatalog {
    schema_version: String,
    profile: String,
    routes: Vec<RouteDefinition>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RouteDefinition {
    id: String,
    implementation: CreativeRouteImplementation,
    providers: Vec<String>,
    required_configuration: Vec<String>,
    detail: String,
}

fn route_definitions() -> Result<Vec<RouteDefinition>> {
    let catalog: RouteCatalog = serde_json::from_str(CREATIVE_ROUTE_CATALOG)
        .context("failed to decode embedded creative route declarations")?;
    anyhow::ensure!(catalog.schema_version == CREATIVE_ROUTE_CATALOG_SCHEMA,
        "unsupported embedded creative route catalog schema");
    anyhow::ensure!(catalog.profile == CREATIVE_DOCTOR_PROFILE,
        "embedded creative route catalog profile does not match creative doctor");
    Ok(catalog.routes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use crate::process::{ProcessPlatform, ProcessTreeTermination, ToolProbeEvidence, ToolProbeStatus};
    use crate::toolchain::{CapabilityId, ToolDiscovery, ToolId, ToolSupportTier};

    #[derive(Default)]
    struct FakeProbe {
        observations: BTreeMap<String, (ToolProbeStatus, Option<String>)>,
        calls: Mutex<Vec<String>>,
    }

    impl ToolProbe for FakeProbe {
        fn probe(&self, executable: &str, _version_args: &[String]) -> ToolProbeEvidence {
            self.calls.lock().unwrap().push(executable.to_owned());
            let (status, version_line) = self.observations.get(executable).cloned()
                .unwrap_or((ToolProbeStatus::Missing, None));
            ToolProbeEvidence {
                executable: executable.to_owned(), status, version_line, duration_ms: 17,
                platform: ProcessPlatform { os: "test", arch: "test", tree_termination: ProcessTreeTermination::DirectChild },
                diagnostic: if status == ToolProbeStatus::Available { None } else { Some("fake missing provider".to_owned()) },
            }
        }
    }

    fn fixture() -> (ToolRegistry, FakeProbe, ToolRuntimeContext) {
        let mut registry = ToolRegistry::new();
        let mut probe = FakeProbe::default();
        for id in CREATIVE_TOOL_IDS {
            let mut descriptor = ToolDescriptor::command(ToolId::new(id).unwrap(), id);
            descriptor.support_tier = ToolSupportTier::Optional;
            descriptor.capabilities = vec![CapabilityId::new("test.creative").unwrap()];
            descriptor.version.min_inclusive = Some("1.0.0".to_owned());
            descriptor.license_notes = Some("Test executable license; model and font rights are separate.".to_owned());
            registry.register(descriptor).unwrap();
            probe.observations.insert(id.to_owned(), (ToolProbeStatus::Available, Some("provider 2.0.0".to_owned())));
        }
        (registry, probe, ToolRuntimeContext::for_platform("linux", "x86_64"))
    }

    fn route<'a>(report: &'a CreativeDoctorReport, id: &str) -> &'a CreativeRouteReport {
        report.routes.iter().find(|route| route.id == id).unwrap()
    }

    #[test]
    fn shared_route_catalog_retains_the_declared_creative_surface() {
        let declarations = route_definitions().unwrap();
        let mut ids = declarations.iter().map(|route| route.id.as_str()).collect::<Vec<_>>();
        ids.sort();
        assert_eq!(ids, vec![
            "ocr.extract.text", "pdf.extract.text", "screenplay.pdf.afterwriting",
            "screenplay.recover.structure", "screenplay.recovery.export", "screenplay.scripttool",
            "speech.align", "speech.audio.inspect", "speech.audio.prepare", "speech.transcribe",
            "subtitle.convert.plain", "subtitle.extract.plain", "subtitle.streams.inspect",
        ]);
        let registry = ToolRegistry::builtins();
        for declaration in declarations {
            assert_eq!(declaration.implementation, CreativeRouteImplementation::AuthoredUnqualified);
            assert!(!declaration.required_configuration.is_empty());
            for provider in declaration.providers {
                assert!(CREATIVE_TOOL_IDS.contains(&provider.as_str()));
                assert!(registry.get(&provider).is_some());
            }
        }
    }

    #[test]
    fn doctor_rows_use_the_shared_catalog_without_reinterpreting_declarations() {
        let (registry, probe, context) = fixture();
        let report = assess_with(&registry, &probe, &context).unwrap();
        let declarations = route_definitions().unwrap();
        assert_eq!(report.routes.len(), declarations.len());
        for mut declaration in declarations {
            declaration.providers.sort();
            let observed = route(&report, &declaration.id);
            assert_eq!(observed.implementation, declaration.implementation);
            assert_eq!(observed.providers, declaration.providers);
            assert_eq!(observed.required_configuration, declaration.required_configuration);
            assert_eq!(observed.detail, declaration.detail);
        }
    }

    #[test]
    fn generated_capability_json_tracks_the_shared_declaration_snapshot() {
        let catalog: serde_json::Value = serde_json::from_str(CREATIVE_ROUTE_CATALOG).unwrap();
        let generated: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/user-guide/creative-capabilities.json"
        )).unwrap();
        assert_eq!(generated["schema_version"], "renderflow.creative-capabilities/v1");
        assert_eq!(generated["catalog"]["schema_version"], catalog["schema_version"]);
        assert_eq!(generated["catalog"]["profile"], catalog["profile"]);
        assert_eq!(generated["catalog"]["sha256"],
            format!("{:x}", Sha256::digest(CREATIVE_ROUTE_CATALOG.as_bytes())));
        assert_eq!(generated["runtime_qualified"], catalog["runtime_qualified"]);
        assert_eq!(generated["scope"], catalog["scope"]);
        assert_eq!(generated["unsupported_directions"], catalog["unsupported_directions"]);
        let declarations = catalog["routes"].as_array().unwrap();
        let rows = generated["routes"].as_array().unwrap();
        let documentation = catalog["documentation"].as_object().unwrap();
        assert_eq!(rows.len(), declarations.len());
        assert_eq!(documentation.len(), declarations.len());
        for declaration in declarations {
            let id = declaration["id"].as_str().unwrap();
            let row = rows.iter().find(|row| row["id"] == id).unwrap();
            for (key, value) in declaration.as_object().unwrap() {
                assert_eq!(&row[key.as_str()], value, "generated route {id} has stale {key}");
            }
            assert_eq!(row["documentation"], documentation[id]);
        }
    }

    #[test]
    fn missing_node_blocks_only_its_route_and_does_not_hide_afterwriting() {
        let (registry, mut probe, context) = fixture();
        probe.observations.remove("tool.node");
        let report = assess_with(&registry, &probe, &context).unwrap();
        assert_eq!(report.unavailable_tool_count(), 1);
        assert_eq!(route(&report, "screenplay.pdf.afterwriting").blocked_providers, vec!["tool.node"]);
        assert_eq!(route(&report, "subtitle.extract.plain").provider_status, CreativeProviderStatus::Available);
        assert!(report.tools.iter().find(|tool| tool.availability.id.as_str() == "tool.afterwriting").unwrap().availability.is_available());
    }

    #[test]
    fn incompatible_versions_and_missing_executables_remain_distinct() {
        let (registry, mut probe, context) = fixture();
        probe.observations.remove("tool.pdftotext");
        probe.observations.insert("tool.tesseract".to_owned(), (ToolProbeStatus::Available, Some("provider 0.9.0".to_owned())));
        let report = assess_with(&registry, &probe, &context).unwrap();
        assert_eq!(report.tools.iter().find(|tool| tool.availability.id.as_str() == "tool.pdftotext").unwrap().availability.status, ToolAvailabilityStatus::MissingExecutable);
        assert_eq!(report.tools.iter().find(|tool| tool.availability.id.as_str() == "tool.tesseract").unwrap().availability.status, ToolAvailabilityStatus::UnsupportedVersion);
        assert_eq!(report.unavailable_tool_count(), 2);
    }

    #[test]
    fn strict_discovery_success_does_not_claim_adapter_or_model_readiness() {
        let (registry, probe, context) = fixture();
        let report = assess_with(&registry, &probe, &context).unwrap();
        assert!(report.strict_success());
        assert_eq!(route(&report, "screenplay.scripttool").implementation, CreativeRouteImplementation::AuthoredUnqualified);
        assert_eq!(route(&report, "screenplay.pdf.afterwriting").implementation, CreativeRouteImplementation::AuthoredUnqualified);
        assert_eq!(route(&report, "pdf.extract.text").implementation, CreativeRouteImplementation::AuthoredUnqualified);
        assert_eq!(route(&report, "screenplay.recover.structure").implementation, CreativeRouteImplementation::AuthoredUnqualified);
        assert_eq!(route(&report, "screenplay.recovery.export").implementation, CreativeRouteImplementation::AuthoredUnqualified);
        assert_eq!(route(&report, "speech.transcribe").implementation, CreativeRouteImplementation::AuthoredUnqualified);
        assert_eq!(report.requirements.iter().find(|requirement| requirement.id == "speech.model.configuration").unwrap().status, "configuration_required");
    }

    #[test]
    fn report_fingerprint_is_stable_for_identical_observations_and_changes_with_missing_tool() {
        let (registry, mut probe, context) = fixture();
        let first = assess_with(&registry, &probe, &context).unwrap();
        let second = assess_with(&registry, &probe, &context).unwrap();
        assert_eq!(first.fingerprint, second.fingerprint);
        probe.observations.remove("tool.ffmpeg");
        assert_ne!(first.fingerprint, assess_with(&registry, &probe, &context).unwrap().fingerprint);
    }

    #[test]
    fn fingerprint_includes_exact_version_platform_and_selected_registry_policy() {
        let (mut registry, mut probe, context) = fixture();
        let first = assess_with(&registry, &probe, &context).unwrap();
        probe.observations.insert("tool.ffmpeg".to_owned(), (ToolProbeStatus::Available, Some("provider 2.0.0 vendor-build-b".to_owned())));
        assert_ne!(first.fingerprint, assess_with(&registry, &probe, &context).unwrap().fingerprint);
        let other_platform = ToolRuntimeContext::for_platform("macos", "aarch64");
        assert_ne!(assess_with(&registry, &probe, &context).unwrap().fingerprint, assess_with(&registry, &probe, &other_platform).unwrap().fingerprint);
        let before_policy_change = assess_with(&registry, &probe, &context).unwrap().fingerprint;
        let mut descriptor = registry.get("tool.ffmpeg").unwrap().clone();
        descriptor.license_notes = Some("Different linked-library distribution terms".to_owned());
        registry.register(descriptor).unwrap();
        assert_ne!(before_policy_change, assess_with(&registry, &probe, &context).unwrap().fingerprint);
    }

    #[test]
    fn unrelated_tools_are_not_probed_or_fingerprinted() {
        let (mut registry, probe, context) = fixture();
        let before = assess_with(&registry, &probe, &context).unwrap();
        registry.register(ToolDescriptor::command(ToolId::new("tool.unrelated").unwrap(), "unrelated")).unwrap();
        assert_eq!(before.fingerprint, assess_with(&registry, &probe, &context).unwrap().fingerprint);
        assert!(!probe.calls.lock().unwrap().iter().any(|call| call == "unrelated"));
        assert!(probe.calls.lock().unwrap().iter().all(|call| CREATIVE_TOOL_IDS.contains(&call.as_str())));
    }

    #[test]
    fn unknown_registry_members_are_reported_without_stopping_peers() {
        let (_, probe, context) = fixture();
        let report = assess_with(&ToolRegistry::new(), &probe, &context).unwrap();
        assert_eq!(report.unavailable_tool_count(), CREATIVE_TOOL_IDS.len());
        assert!(report.tools.iter().all(|tool| tool.descriptor.is_none() && tool.availability.status == ToolAvailabilityStatus::UnknownProvider));
        assert!(probe.calls.lock().unwrap().is_empty());
        assert!(report.fingerprint.starts_with("sha256:"));
    }

    #[test]
    fn serialized_report_retains_licenses_and_separates_model_and_font_requirements() {
        let (registry, probe, context) = fixture();
        let report = assess_with(&registry, &probe, &context).unwrap();
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["schema"], CREATIVE_DOCTOR_SCHEMA);
        assert_eq!(value["fingerprint_scope"], CREATIVE_FINGERPRINT_SCOPE);
        assert!(value["tools"][0]["descriptor"]["license_notes"].as_str().unwrap().contains("executable license"));
        assert!(report.requirements.iter().any(|requirement| requirement.id == "speech.model.license"));
        assert!(report.requirements.iter().any(|requirement| requirement.id == "screenplay.fonts"));
        assert!(value.get("timestamp").is_none());
    }

    #[test]
    fn unprobed_candidate_is_not_launched_or_misreported_as_missing() {
        let (mut registry, probe, context) = fixture();
        let mut afterwriting = registry.get("tool.afterwriting").unwrap().clone();
        afterwriting.discovery = ToolDiscovery::Unprobed { reason: "No version-only CLI is available".to_owned() };
        registry.register(afterwriting).unwrap();
        let report = assess_with(&registry, &probe, &context).unwrap();
        assert!(report.strict_success());
        assert_eq!(report.unprobed_tool_count(), 1);
        assert_eq!(route(&report, "screenplay.pdf.afterwriting").provider_status, CreativeProviderStatus::Unobserved);
        assert!(!probe.calls.lock().unwrap().iter().any(|call| call == "tool.afterwriting"));
        assert_eq!(route(&report, "subtitle.convert.plain").provider_status, CreativeProviderStatus::Available);
    }

    #[test]
    fn builtin_profile_keeps_pinned_pdf_providers_unobserved_without_launching_them() {
        let registry = ToolRegistry::builtins();
        let probe = FakeProbe::default();
        let report = assess_with(
            &registry, &probe, &ToolRuntimeContext::for_platform("linux", "x86_64"),
        ).unwrap();
        assert_eq!(report.unprobed_tool_count(), 2);
        let pdf = route(&report, "screenplay.pdf.afterwriting");
        assert_eq!(pdf.implementation, CreativeRouteImplementation::AuthoredUnqualified);
        assert_eq!(pdf.provider_status, CreativeProviderStatus::Unobserved);
        assert_eq!(pdf.blocked_providers, vec!["tool.afterwriting", "tool.node"]);
        assert!(pdf.required_configuration.iter().any(|key| key == "execution.screenplay_pdf.node_sha256"));
        assert!(pdf.required_configuration.iter().any(|key| key == "execution.screenplay_pdf.package_tree_sha256"));
        for id in ["tool.node", "tool.afterwriting"] {
            let tool = report.tools.iter().find(|tool| tool.availability.id.as_str() == id).unwrap();
            assert_eq!(tool.availability.status, ToolAvailabilityStatus::NotProbed);
            assert!(tool.availability.selected_executable.is_none());
            assert!(tool.availability.version_line.is_none());
        }
        let calls = probe.calls.lock().unwrap();
        assert!(calls.iter().any(|call| call == "scripttool"));
        assert!(!calls.iter().any(|call| call == "node" || call == "afterwriting"));
    }

    #[test]
    fn runtime_service_hints_cannot_qualify_global_pdf_provider_descriptors() {
        let registry = ToolRegistry::builtins();
        let probe = FakeProbe::default();
        let context = ToolRuntimeContext::for_platform("linux", "x86_64")
            .with_runtime_service("renderflow.configured-node")
            .with_runtime_service("renderflow.configured-afterwriting")
            .with_runtime_service("renderflow.configured.node")
            .with_runtime_service("renderflow.configured.afterwriting");
        let inventory = registry.assess_ids_with(["tool.node", "tool.afterwriting"], &probe, &context);
        assert!(inventory.available_ids().is_empty());
        assert!(inventory.tools.iter().all(|tool| tool.status == ToolAvailabilityStatus::NotProbed));
        assert!(probe.calls.lock().unwrap().is_empty());
        assert!(registry.get("tool.afterwriting").unwrap().capabilities.iter()
            .any(|capability| capability.as_str() == "screenplay.render.fountain.pdf"));
    }

    #[test]
    fn creative_inventory_count_and_report_rows_match_the_published_schema() {
        let schema: serde_json::Value = serde_json::from_str(include_str!(
            "../../../schemas/renderflow-creative-doctor-v1.schema.json"
        )).unwrap();
        let registry = ToolRegistry::builtins();
        let probe = FakeProbe::default();
        let report = assess_with(&registry, &probe,
            &ToolRuntimeContext::for_platform("linux", "x86_64")).unwrap();
        let value = serde_json::to_value(&report).unwrap();
        let count = report.tools.len() as u64;
        assert_eq!(count, CREATIVE_TOOL_IDS.len() as u64);
        assert_eq!(schema["properties"]["tools"]["minItems"].as_u64(), Some(count));
        assert_eq!(schema["properties"]["tools"]["maxItems"].as_u64(), Some(count));

        // These source-contract assertions deliberately compare the serialized
        // producer with the separately maintained schema. They do not invoke
        // a schema engine, native tool, or conversion during this test.
        fn assert_object_fields(value: &serde_json::Value, definition: &serde_json::Value) {
            let actual = value.as_object().unwrap();
            let properties = definition["properties"].as_object().unwrap();
            for required in definition["required"].as_array().unwrap() {
                assert!(actual.contains_key(required.as_str().unwrap()), "missing required field {required}");
            }
            if definition["additionalProperties"] == false {
                for key in actual.keys() {
                    assert!(properties.contains_key(key), "schema does not declare emitted field {key}");
                }
            }
        }
        fn assert_enum(value: &serde_json::Value, declaration: &serde_json::Value) {
            assert!(declaration["enum"].as_array().unwrap().contains(value),
                "schema does not permit emitted enum value {value}");
        }
        assert_object_fields(&value, &schema);
        for tool in value["tools"].as_array().unwrap() {
            assert_object_fields(tool, &schema["$defs"]["tool"]);
            assert_object_fields(&tool["availability"], &schema["$defs"]["availability"]);
            assert_enum(&tool["availability"]["status"], &schema["$defs"]["availability"]["properties"]["status"]);
            assert_object_fields(&tool["descriptor"], &schema["$defs"]["descriptor"]);
            assert_object_fields(&tool["descriptor"]["version"], &schema["$defs"]["descriptor"]["properties"]["version"]);
            let discovery = &tool["descriptor"]["discovery"];
            let declaration = schema["$defs"]["discovery"]["oneOf"].as_array().unwrap().iter()
                .find(|branch| branch["properties"]["kind"]["const"] == discovery["kind"]).unwrap();
            assert_object_fields(discovery, declaration);
        }
        for route in value["routes"].as_array().unwrap() {
            assert_object_fields(route, &schema["$defs"]["route"]);
            for name in ["implementation", "provider_status"] {
                assert_enum(&route[name], &schema["$defs"]["route"]["properties"][name]);
            }
            for provider in route["providers"].as_array().unwrap() {
                assert!(report.tools.iter().any(|tool| tool.availability.id.as_str() == provider.as_str().unwrap()));
            }
        }
        for requirement in value["requirements"].as_array().unwrap() {
            assert_object_fields(requirement, &schema["$defs"]["requirement"]);
            assert_enum(&requirement["status"], &schema["$defs"]["requirement"]["properties"]["status"]);
        }
    }

    #[test]
    fn creative_only_additions_stay_outside_the_plain_doctor_inventory() {
        let registry = ToolRegistry::builtins();
        let plain_ids = registry.all()
            .filter(|tool| !CREATIVE_OPTIONAL_TOOL_IDS.contains(&tool.id.as_str()))
            .map(|tool| tool.id.as_str()).collect::<Vec<_>>();
        for id in ["tool.scripttool", "tool.node", "tool.afterwriting", "tool.pdftotext",
            "tool.pdftoppm", "tool.renderflow-screenplay-recovery"] {
            assert!(!plain_ids.contains(&id));
            assert!(CREATIVE_TOOL_IDS.contains(&id));
            assert!(registry.get(id).is_some());
        }
        // These existing providers retain their original plain-doctor scope.
        for id in ["tool.ffmpeg", "tool.ffprobe", "tool.tesseract", "tool.whisper_cpp"] {
            assert!(plain_ids.contains(&id));
        }
    }
}
