//! Explicit selected-stem relationship metadata. Retained hashes do not prove
//! original-mix sample alignment or authenticate a separator's native execution.
use super::*;
use crate::dna::sonic::recipe::StemContext;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {
    schema: String,
    lineage: Lineage,
    upstream_analysis_artifact: ArtifactRef,
    provider: ProviderRef,
    implementation_sha256: String,
    configuration_sha256: String,
    provider_lock_sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Lineage {
    original_mix: Source,
    selected_stem: Source,
    stem_id: String,
    source_stage_id: String,
    source_mix_artifact_id: String,
    source_stem_artifact_id: String,
    source_stem_port: String,
    scope: Scope,
    range: FrameRange,
    relationship_evidence: ArtifactRef,
    authority_artifacts: Vec<ArtifactRef>,
    source_plan_sha256: String,
    source_checkpoint_sha256: String,
    source_provider: ProviderRef,
    source_provider_lock_sha256: String,
    source_implementation_sha256: String,
    source_configuration_sha256: String,
    duration_tolerance_milliseconds: u16,
    timing_basis: String,
}
fn reference(value: &ArtifactRef, artifacts: &BTreeMap<&str, (&str, u64)>) -> Result<()> {
    ensure!(
        artifacts.get(value.id.as_str()) == Some(&(value.sha256.as_str(), value.byte_size))
            && value.byte_size > 0
            && value.byte_size <= 8 * 1024 * 1024,
        "sonic.stem.retained_reference"
    );
    Ok(())
}
pub(super) fn project(
    value: &Analysis,
    selection: &StemContext,
    payloads: &BTreeMap<&str, &[u8]>,
    artifacts: &BTreeMap<&str, (&str, u64)>,
    manifest: &str,
    pcm: WavInspection,
) -> Result<SonicFact> {
    let stem = value
        .source
        .stem
        .as_ref()
        .context("sonic.stem.source_identity")?;
    ensure!(
        selection.lineage_artifact_id == stem.relationship_evidence_id
            && selection.lineage_artifact_id == "stem_lineage",
        "sonic.stem.lineage_selection"
    );
    let bytes = *payloads
        .get(selection.lineage_artifact_id.as_str())
        .context("sonic.stem.report_missing")?;
    let report: Report = local::parse(bytes)?;
    let l = &report.lineage;
    let cap = value
        .capabilities
        .iter()
        .find(|c| c.capability.id == "aniflow/audio-stem-lineage")
        .context("sonic.stem.capability")?;
    let provider = value
        .providers
        .iter()
        .find(|p| {
            cap.provider_evidence_ids.contains(&p.id)
                && p.provider == report.provider
                && p.implementation_sha256 == report.implementation_sha256
                && p.configuration_sha256 == report.configuration_sha256
        })
        .context("sonic.stem.provider")?;
    ensure!(
        report.schema == "aniflow.audio-stem-lineage/v1"
            && report.provider.id == "org.egohygiene.aniflow.audio-stem"
            && report.provider.version == "1.0.0"
            && cap.capability.version == "1.0.0"
            && matches!(cap.status.as_str(), "complete" | "partial")
            && cap
                .evidence_artifact_ids
                .contains(&selection.lineage_artifact_id),
        "sonic.stem.native_identity"
    );
    ensure!(
        l.selected_stem == value.source
            && l.selected_stem.artifact.id == "source_audio"
            && l.original_mix.artifact == stem.original_mix
            && l.original_mix.artifact.id == "original_mix"
            && l.original_mix.stem.is_none()
            && l.stem_id == stem.id
            && l.source_stem_artifact_id == stem.id,
        "sonic.stem.source_binding"
    );
    for id in [
        &l.stem_id,
        &l.source_stage_id,
        &l.source_mix_artifact_id,
        &l.source_stem_artifact_id,
        &l.source_stem_port,
    ] {
        local::identifier(id)?;
    }
    ensure!(
        l.scope.channels == (0..u32::from(pcm.channels)).collect::<Vec<_>>()
            && l.scope.stem_id.as_deref() == Some(stem.id.as_str())
            && l.range.start == 0
            && l.range.end == pcm.frame_count
            && l.timing_basis == "zero_origin_duration_only"
            && l.duration_tolerance_milliseconds <= 20,
        "sonic.stem.scope_or_timing_basis"
    );
    let mix = &l.original_mix;
    ensure!(
        mix.stream_index == 0
            && mix.origin.numerator == 0
            && mix.origin.denominator == 1
            && (8_000..=192_000).contains(&mix.sample_rate_hz)
            && (1..=8).contains(&mix.channels)
            && mix.frame_count > 0
            && mix.frame_count <= u64::from(mix.sample_rate_hz) * 600
            && mix.artifact.byte_size >= 44 + mix.frame_count * u64::from(mix.channels) * 2
            && mix.artifact.byte_size <= 256 * 1024 * 1024,
        "sonic.stem.mix_declaration"
    );
    let difference = (u128::from(mix.frame_count) * u128::from(pcm.sample_rate_hz))
        .abs_diff(u128::from(pcm.frame_count) * u128::from(mix.sample_rate_hz));
    ensure!(
        difference * 1000
            <= u128::from(l.duration_tolerance_milliseconds)
                * u128::from(mix.sample_rate_hz)
                * u128::from(pcm.sample_rate_hz),
        "sonic.stem.declared_duration"
    );
    ensure!(
        l.relationship_evidence.id == "separation_evidence"
            && matches!(
                report.upstream_analysis_artifact.id.as_str(),
                "analysis" | "signal_analysis"
            )
            && (7..=14).contains(&l.authority_artifacts.len()),
        "sonic.stem.authority_shape"
    );
    reference(&l.relationship_evidence, artifacts)?;
    reference(&report.upstream_analysis_artifact, artifacts)?;
    ensure!(
        [
            &value.source.artifact.id,
            &stem.original_mix.id,
            &l.relationship_evidence.id,
            &selection.lineage_artifact_id
        ]
        .into_iter()
        .all(|id| cap.evidence_artifact_ids.contains(id)),
        "sonic.stem.required_lineage"
    );
    let mut ids = BTreeSet::new();
    for r in &l.authority_artifacts {
        reference(r, artifacts)?;
        ensure!(
            r.id.starts_with("stem_authority_") && ids.insert(r.id.as_str()),
            "sonic.stem.authority_duplicate"
        );
    }
    for id in [
        "stem_authority_plan",
        "stem_authority_manifest",
        "stem_authority_checkpoint",
        "stem_authority_lock",
        "stem_authority_report",
    ] {
        ensure!(ids.remove(id), "sonic.stem.authority_missing");
    }
    for i in 0..ids.len() {
        ensure!(
            ids.contains(format!("stem_authority_validation_{i}").as_str()),
            "sonic.stem.validation_sequence"
        );
    }
    for hash in [
        &l.source_plan_sha256,
        &l.source_checkpoint_sha256,
        &l.source_provider_lock_sha256,
        &l.source_implementation_sha256,
        &l.source_configuration_sha256,
        &report.provider_lock_sha256,
    ] {
        files::digest(hash)?;
    }
    local::text(&l.source_provider.id, 128)?;
    local::text(&l.source_provider.version, 128)?;
    Ok(SonicFact {
        id: format!("analysis.{manifest}.stem.relationship"),
        dimension: "sonic.arrangement.declared_stem_relationship".into(),
        value: json!({"stem_id":stem.id,"selected_source_sha256":value.source.artifact.sha256,"selected_source_sample_rate_hz":pcm.sample_rate_hz,"selected_source_channels":pcm.channels,"selected_source_frames":pcm.frame_count,"original_mix":mix,"original_mix_bytes_observed":false,"declared_duration_tolerance_milliseconds":l.duration_tolerance_milliseconds,"timing_basis":l.timing_basis,"sample_alignment_to_original_mix":"not_established","stem_instrument_or_voice_role":"not_inferred"}),
        origin: DnaEvidenceOrigin::SourceReported,
        class: DnaDeterminism::Deterministic,
        confidence: Confidence::NotApplicable,
        evidence: json!({"manifest_sha256":manifest,"report_sha256":local::sha256(bytes),"report_pointer":"/lineage","provider_declaration":{"id":provider.provider.id,"version":provider.provider.version,"implementation_sha256":provider.implementation_sha256,"configuration_sha256":provider.configuration_sha256},"retained_authority_artifacts":l.authority_artifacts,"provider_authentication":"not_established","native_validation":"bounded_relationship_projection_only; raw_native_acceptance_not_replayed","original_mix_alignment":"not_established"}),
        correction_of: None,
    })
}
