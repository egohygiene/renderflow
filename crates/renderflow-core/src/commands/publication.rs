use std::fs;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::ai::{
    AiExecutionPreferenceV1, AiModelCatalog, AiSkillRegistry, AiSkillRuntime, OllamaProvider,
    OpenAiProvider,
};
use crate::artifact::ArtifactStore;
use crate::dna::ArtifactDna;
use crate::publication::lulu::{
    evaluate_request, LuluConformanceReport, LuluEligibility, LuluRulePack,
};
use crate::publication::magazine::{
    build_magazine_candidates, create_magazine_ai_request, MagazineCandidatePolicy,
};
use crate::spec::load_spec;

#[allow(clippy::too_many_arguments)]
pub fn run_magazine_candidates(
    config: &str,
    asset_role: &str,
    output: Option<&str>,
    format: &str,
    use_ai: bool,
    ai_catalog: Option<&str>,
    ai_preference: &str,
    allow_remote: bool,
    allow_unverified: bool,
    source_approved_for_ai: bool,
    privacy_approved_for_remote: bool,
    openai_endpoint: Option<&str>,
    openai_api_key_env: &str,
) -> Result<()> {
    if allow_remote && !use_ai {
        anyhow::bail!("--allow-remote requires --ai");
    }
    if privacy_approved_for_remote && !allow_remote {
        anyhow::bail!("--privacy-approved-for-remote requires --allow-remote");
    }
    let loaded = load_spec(config)?;
    let publication = loaded
        .spec
        .publication
        .as_ref()
        .context("magazine candidates require a publication contract")?;
    let asset = publication
        .artwork
        .iter()
        .find(|asset| asset.role == asset_role)
        .with_context(|| format!("publication artwork role '{asset_role}' was not found"))?;
    let dna_reference = asset.artifact_dna.as_deref().with_context(|| {
        format!("publication artwork role '{asset_role}' does not reference artifact_dna")
    })?;
    let config_directory = Path::new(config).parent().unwrap_or_else(|| Path::new("."));
    let dna_path = config_directory.join(dna_reference);
    let dna = ArtifactDna::load(&dna_path)?;
    let hygiene = loaded
        .spec
        .execution
        .hygiene_policy
        .as_deref()
        .and_then(|id| loaded.spec.hygiene.get(id));
    let policy = MagazineCandidatePolicy {
        protected_references: hygiene
            .map(|policy| policy.protected_references.terms.clone())
            .unwrap_or_default(),
        reject_pii: true,
        reject_secrets: hygiene
            .map(|policy| policy.secrets.enabled && policy.secrets.block)
            .unwrap_or(true),
        secret_markers: hygiene
            .map(|policy| policy.secrets.markers.clone())
            .unwrap_or_default(),
    };
    let mut candidate = build_magazine_candidates(publication, asset, &dna, &policy)?;

    if use_ai {
        let preference = AiExecutionPreferenceV1::from_str(ai_preference)?;
        if preference == AiExecutionPreferenceV1::RemoteOnly && !allow_remote {
            anyhow::bail!("remote-only AI preference requires --allow-remote");
        }
        let catalog = match ai_catalog {
            Some(path) => AiModelCatalog::load(path)?,
            None => AiModelCatalog::bundled()?,
        };
        let skills = AiSkillRegistry::bundled()?;
        let ollama = OllamaProvider::default_local();
        let mut openai = OpenAiProvider::new().with_api_key_env(openai_api_key_env);
        if let Some(endpoint) = openai_endpoint {
            openai = openai.with_endpoint(endpoint);
        }
        let runtime = AiSkillRuntime::new(&catalog, &skills, vec![&ollama, &openai]);
        let rights_approved = publication.rights.reviewed
            && asset
                .approval_reference
                .as_deref()
                .is_some_and(|reference| !reference.trim().is_empty());
        let request = create_magazine_ai_request(
            &candidate,
            &dna,
            &policy,
            preference,
            allow_remote,
            allow_unverified,
            source_approved_for_ai && rights_approved,
            privacy_approved_for_remote,
        )?;
        let store_root = std::env::temp_dir()
            .join("renderflow")
            .join("magazine-ai-candidates");
        let store = ArtifactStore::new(store_root)?;
        match runtime.execute(&request, &store) {
            Ok(outcome) => candidate.attach_ai_outcome(outcome, &store)?,
            Err(error) => {
                candidate.mark_ai_unavailable();
                eprintln!("Optional AI candidate unavailable: {error:#}");
            }
        }
    }

    candidate.validate()?;
    emit(&candidate, format, output)
}

pub fn run_lulu_rules(format: &str, output: Option<&str>) -> Result<()> {
    emit(&LuluRulePack::builtin()?, format, output)
}

pub fn run_lulu_preflight(
    request: &str,
    format: &str,
    output: Option<&str>,
    epubcheck: bool,
) -> Result<()> {
    let report = evaluate_request(Path::new(request), epubcheck)?;
    emit_report(&report, format, output)?;
    if report.channels.iter().any(|channel| {
        matches!(
            channel.eligibility,
            LuluEligibility::Ineligible | LuluEligibility::Unknown
        )
    }) {
        anyhow::bail!("one or more requested Lulu channels are not upload-ready; see report")
    }
    Ok(())
}

fn emit_report(report: &LuluConformanceReport, format: &str, output: Option<&str>) -> Result<()> {
    if format.eq_ignore_ascii_case("text") {
        let mut text = format!(
            "Lulu candidate preflight\nRule pack: {} (observed {})\nUpload performed: no\n\nChannels:\n",
            report.rule_pack, report.observed_on
        );
        for channel in &report.channels {
            text.push_str(&format!(
                "  {:?}: {:?}\n",
                channel.channel, channel.eligibility
            ));
            for code in &channel.blocking_findings {
                text.push_str(&format!("    BLOCK: {code}\n"));
            }
            for code in &channel.warning_findings {
                text.push_str(&format!("    WARN: {code}\n"));
            }
        }
        text.push_str("\nFindings:\n");
        for finding in &report.findings {
            text.push_str(&format!(
                "  [{:?}] {}: {}\n",
                finding.severity, finding.code, finding.message
            ));
        }
        write(&text, output)
    } else {
        emit(report, format, output)
    }
}

fn emit<T: Serialize>(value: &T, format: &str, output: Option<&str>) -> Result<()> {
    let content = match format.to_ascii_lowercase().as_str() {
        "json" => format!("{}\n", serde_json::to_string_pretty(value)?),
        "yaml" | "yml" | "text" => serde_yaml_ng::to_string(value)?,
        other => anyhow::bail!(
            "unknown publication output format '{other}'; supported: text, json, yaml"
        ),
    };
    write(&content, output)
}

fn write(content: &str, output: Option<&str>) -> Result<()> {
    if let Some(path) = output {
        fs::write(path, content)
            .with_context(|| format!("failed to write publication report '{path}'"))?;
    } else {
        print!("{content}");
    }
    Ok(())
}
