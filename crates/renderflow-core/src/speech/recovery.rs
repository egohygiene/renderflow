//! Explicit replay of one pinned, portable speech result.
//!
//! This verifies retained output bytes and their recorded provenance. It never
//! opens a model or native tool, predicts another inference, or attests the
//! historical process environment. Original media is not copied into a bundle;
//! offline inspection checks its recorded identity only. Execution separately
//! verifies the current original input before restoring any saved output.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactId, ArtifactStorageClass, ArtifactStore, MediaType};
use crate::graph::Format;
use crate::process::ProcessCancellationToken;
use crate::timed_text::MAX_INPUT_BYTES;

use super::graph::{retained_artifact_name, retained_artifacts, SpeechArtifactEvidence, SPEECH_ARTIFACT_METADATA_KEY};
use super::policy::SpeechPolicy;
use super::provider::{ensure_not_cancelled, expected_digest, inspect_pcm_wav, read_bounded, sha256};
use super::{attach_audio_preparation, normalize_whisper_json, SpeechTranscription, StrictValue, MAX_AUDIO_SOURCE_BYTES, MAX_PREPARED_AUDIO_DURATION_MS, MAX_PREPARED_WAV_BYTES, WHISPER_PROVIDER_ID};

pub const SAVED_RESULT_FILENAME: &str = "saved-result.json";
pub const SAVED_RESULT_SCHEMA: &str = "renderflow.speech-saved-result/v1";
pub const SAVED_RESULT_PROFILE: &str = "portable-speech-output-replay/v1";
pub const MAX_RECEIPT_BYTES: u64 = 256 * 1024;
pub const CANONICAL_FILENAME: &str = "transcript.timed-transcript.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSpeechReceipt {
    schema_version: String,
    profile: String,
    configuration_sha256: String,
    policy: SpeechPolicy,
    source: Artifact,
    /// Historical observation only during offline bundle inspection. The
    /// current WAV is inspected again during graph replay.
    #[serde(skip_serializing_if = "Option::is_none")]
    source_wav_duration_ms: Option<u64>,
    canonical: Artifact,
}

/// Available only after raw provider output, the exact canonical payload, and
/// every retained preparation payload have passed the shared verifier.
#[derive(Debug, Clone)]
pub struct VerifiedSpeechTranscription {
    transcription: SpeechTranscription,
    original_source: Artifact,
}

impl VerifiedSpeechTranscription {
    pub fn transcription(&self) -> &SpeechTranscription { &self.transcription }
    pub fn original_source(&self) -> &Artifact { &self.original_source }
}

/// A verified historical bundle, not a fresh native execution attestation.
/// Fields stay private so callers cannot manufacture a successful assessment.
#[derive(Debug, Clone)]
pub struct SavedSpeechResult {
    receipt: SavedSpeechReceipt,
    receipt_path: PathBuf,
    receipt_sha256: String,
    verified: VerifiedSpeechTranscription,
    payloads: Vec<(Artifact, Vec<u8>)>,
}

impl SavedSpeechResult {
    pub fn canonical(&self) -> &Artifact { &self.receipt.canonical }
    pub fn source(&self) -> &Artifact { &self.receipt.source }
    pub fn receipt_sha256(&self) -> &str { &self.receipt_sha256 }
    pub fn transcription(&self) -> &SpeechTranscription { self.verified.transcription() }
    pub fn verified_transcription(&self) -> &VerifiedSpeechTranscription { &self.verified }
    pub fn original_source_wav_duration_ms(&self) -> Option<u64> { self.receipt.source_wav_duration_ms }
}

/// Serialize a receipt only after verifying the complete source/evidence set.
/// The caller publishes this last, beside the closed canonical/sidecar names.
pub fn receipt_bytes(
    input: &Artifact, policy: &SpeechPolicy, canonical: &Artifact,
    store: &ArtifactStore, cancellation: &ProcessCancellationToken,
) -> Result<Vec<u8>> {
    let receipt = receipt_for_store(input, policy, canonical, store, cancellation)?;
    let payloads = read_store_payloads(&receipt, store, cancellation)?;
    verify_payloads(&receipt, &payloads, cancellation)?;
    let bytes = serde_json::to_vec(&receipt)?;
    if bytes.len() as u64 > MAX_RECEIPT_BYTES { bail!("speech saved-result receipt exceeds its byte limit"); }
    ensure_not_cancelled(cancellation)?;
    Ok(bytes)
}

/// Verify native evidence without publishing a receipt or running native tools.
pub fn verify_retained_transcription(
    input: &Artifact, policy: &SpeechPolicy, canonical: &Artifact,
    store: &ArtifactStore, cancellation: &ProcessCancellationToken,
) -> Result<VerifiedSpeechTranscription> {
    let receipt = receipt_for_store(input, policy, canonical, store, cancellation)?;
    let payloads = read_store_payloads(&receipt, store, cancellation)?;
    verify_payloads(&receipt, &payloads, cancellation)
}

/// Inspect an explicitly pinned portable bundle using its recorded intent.
/// Only fixed sibling filenames are read. No original source, executable,
/// model, shared cache, or private artifact store is accessed.
pub fn inspect_saved_bundle(
    receipt_path: &Path, receipt_sha256: &str, cancellation: &ProcessCancellationToken,
) -> Result<SavedSpeechResult> {
    ensure_not_cancelled(cancellation)?;
    expected_digest(receipt_sha256, "saved-result receipt digest")?;
    if receipt_path.file_name().and_then(|name| name.to_str()) != Some(SAVED_RESULT_FILENAME) {
        bail!("speech recovery requires the fixed saved-result.json receipt name");
    }
    let bytes = read_bounded(receipt_path, MAX_RECEIPT_BYTES, cancellation)?;
    if sha256(&bytes) != receipt_sha256 { bail!("speech saved-result receipt digest mismatch"); }
    let receipt: SavedSpeechReceipt = parse_exact(&bytes, "saved-result receipt")?;
    validate_receipt(&receipt)?;
    let parent = receipt_path.parent().unwrap_or_else(|| Path::new("."));
    let directory = std::fs::canonicalize(parent).context("speech saved-result directory is unavailable")?;
    let stable_receipt_path = directory.join(SAVED_RESULT_FILENAME);
    // Bind the resolved directory to the same exact receipt before sibling reads.
    if sha256(&read_bounded(&stable_receipt_path, MAX_RECEIPT_BYTES, cancellation)?) != receipt_sha256 {
        bail!("speech saved-result receipt changed while resolving its directory");
    }
    let mut payloads = Vec::new();
    for record in retained_artifacts(&receipt.canonical)? {
        let path = directory.join(retained_artifact_name(&record)?);
        let bytes = read_record(&path, &record, cancellation)?;
        payloads.push((record, bytes));
    }
    payloads.push((receipt.canonical.clone(), read_record(&directory.join(CANONICAL_FILENAME), &receipt.canonical, cancellation)?));
    let verified = verify_payloads(&receipt, &payloads, cancellation)?;
    if sha256(&read_bounded(&stable_receipt_path, MAX_RECEIPT_BYTES, cancellation)?) != receipt_sha256 {
        bail!("speech saved-result receipt changed during inspection");
    }
    Ok(SavedSpeechResult { receipt, receipt_path: stable_receipt_path, receipt_sha256: receipt_sha256.to_owned(), verified, payloads })
}

/// Additionally bind a saved result to the current declared source and intent.
/// `source` may be omitted by SDK assessment, which already hashes intake.
pub fn inspect_saved_result(
    receipt_path: &Path, receipt_sha256: &str, policy: &SpeechPolicy,
    source_format: Format, source: Option<&Artifact>, cancellation: &ProcessCancellationToken,
) -> Result<SavedSpeechResult> {
    policy.validate_source(source_format, false)?;
    let saved = inspect_saved_bundle(receipt_path, receipt_sha256, cancellation)?;
    if saved.receipt.policy != recognition_policy(policy)
        || saved.source().format().as_str() != source_format.to_string()
        || saved.source().digest().value() != policy.source_sha256
    { bail!("speech saved-result does not match the requested source and recognition policy"); }
    if source.is_some_and(|source| normalize_source(source) != *saved.source()) {
        bail!("speech saved-result original source record differs from current input");
    }
    ensure_not_cancelled(cancellation)?;
    Ok(saved)
}

/// Reverify the portable receipt and payloads, then restore exact records into
/// the destination store. Invalid historical bytes never fall back to inference.
/// The caller verifies current source bytes before invoking this operation.
pub fn restore_saved_result(
    saved: &SavedSpeechResult, store: &ArtifactStore, cancellation: &ProcessCancellationToken,
) -> Result<Artifact> {
    let checked = inspect_saved_bundle(&saved.receipt_path, &saved.receipt_sha256, cancellation)?;
    if checked.receipt != saved.receipt { bail!("speech saved-result changed after assessment"); }
    for (record, bytes) in &checked.payloads {
        ensure_not_cancelled(cancellation)?;
        let mut descriptor = ArtifactDescriptor::new(record.format().clone(), record.media_type().clone(), record.storage_class())
            .with_sources(record.sources().iter().cloned());
        for (key, value) in record.metadata() { descriptor = descriptor.with_metadata(key, value.clone()); }
        let restored = store.put_bytes(bytes, descriptor)?;
        if restored != *record { bail!("speech restored artifact differs from the selected historical record"); }
        read_record(&store.payload_path(&restored)?, &restored, cancellation)?;
    }
    ensure_not_cancelled(cancellation)?;
    Ok(checked.receipt.canonical)
}

fn recognition_policy(policy: &SpeechPolicy) -> SpeechPolicy {
    let mut policy = policy.clone();
    policy.recovery = None;
    policy
}

fn configuration_digest(policy: &SpeechPolicy) -> Result<String> {
    Ok(sha256(&serde_json::to_vec(&(SAVED_RESULT_PROFILE, recognition_policy(policy)))?))
}

fn receipt_for_store(
    input: &Artifact, policy: &SpeechPolicy, canonical: &Artifact,
    store: &ArtifactStore, cancellation: &ProcessCancellationToken,
) -> Result<SavedSpeechReceipt> {
    ensure_not_cancelled(cancellation)?;
    let format: Format = input.format().as_str().parse()?;
    policy.validate_source(format, false)?;
    validate_source(input, policy)?;
    // Source hashing is bounded and cancellable, including original containers.
    let source = super::provider::FileSnapshot::read(&store.payload_path(input)?, "speech receipt source", MAX_AUDIO_SOURCE_BYTES, cancellation)?;
    if source.sha256 != input.digest().value() || source.metadata.len() != input.size_bytes() {
        bail!("speech receipt source payload differs from its original record");
    }
    let source_wav_duration_ms = if policy.audio.is_none() {
        let bytes = read_record(&source.path, input, cancellation)?;
        Some(inspect_pcm_wav(&bytes)?)
    } else { None };
    source.recheck(cancellation)?;
    let receipt = SavedSpeechReceipt {
        schema_version: SAVED_RESULT_SCHEMA.to_owned(), profile: SAVED_RESULT_PROFILE.to_owned(),
        configuration_sha256: configuration_digest(policy)?, policy: recognition_policy(policy),
        source: normalize_source(input), source_wav_duration_ms, canonical: canonical.clone(),
    };
    validate_receipt(&receipt)?;
    Ok(receipt)
}

fn validate_receipt(receipt: &SavedSpeechReceipt) -> Result<()> {
    if receipt.schema_version != SAVED_RESULT_SCHEMA || receipt.profile != SAVED_RESULT_PROFILE
        || receipt.policy.recovery.is_some()
        || receipt.configuration_sha256 != configuration_digest(&receipt.policy)?
    { bail!("speech saved-result schema, profile, or recognition identity is invalid"); }
    let format: Format = receipt.source.format().as_str().parse()?;
    receipt.policy.validate_source(format, false)?;
    validate_source(&receipt.source, &receipt.policy)?;
    if !receipt.source.metadata().is_empty() { bail!("speech receipt source record must omit intake metadata"); }
    validate_record(&receipt.canonical, Format::TimedTranscriptJson, MAX_INPUT_BYTES as u64, ArtifactStorageClass::Intermediate)?;
    if receipt.canonical.metadata().len() != 1 { bail!("speech canonical metadata must contain exactly its speech evidence"); }
    let retained = retained_artifacts(&receipt.canonical)?;
    let expected_count = if receipt.policy.audio.is_some() { 4 } else { 2 };
    if retained.len() != expected_count { bail!("speech receipt preparation mode differs from retained records"); }
    let metadata: SpeechArtifactEvidence = serde_json::from_value(receipt.canonical.metadata().get(SPEECH_ARTIFACT_METADATA_KEY)
        .context("speech receipt canonical evidence is missing")?.clone())?;
    if metadata.input_artifact_id != *receipt.source.id() || metadata.input_sha256 != receipt.source.digest().value() {
        bail!("speech receipt canonical does not reference its original source");
    }
    match (receipt.policy.audio.is_some(), receipt.source_wav_duration_ms) {
        (false, Some(duration)) if (1..=MAX_PREPARED_AUDIO_DURATION_MS).contains(&duration) => {},
        (true, None) => {},
        _ => bail!("speech receipt original WAV duration is missing or inappropriate"),
    }
    Ok(())
}

fn validate_source(source: &Artifact, policy: &SpeechPolicy) -> Result<()> {
    let format: Format = source.format().as_str().parse()?;
    let maximum = if policy.audio.is_some() { MAX_AUDIO_SOURCE_BYTES } else { MAX_PREPARED_WAV_BYTES };
    validate_record(source, format, maximum, ArtifactStorageClass::Source)?;
    if !source.sources().is_empty() || source.digest().value() != policy.source_sha256 {
        bail!("speech saved-result requires an unmodified original source record matching its policy");
    }
    Ok(())
}

// Artifact record identity excludes descriptive intake metadata. Preserve every
// identity-bearing field while avoiding source-id/profile coupling in portable
// receipts and any duplication of arbitrary intake metadata.
fn normalize_source(source: &Artifact) -> Artifact {
    Artifact::new(source.id().clone(), source.format().clone(), source.media_type().clone(),
        source.digest().clone(), source.size_bytes(), source.payload().clone(),
        Default::default(), source.sources().to_vec(), source.storage_class())
}

fn validate_record(record: &Artifact, format: Format, maximum: u64, storage: ArtifactStorageClass) -> Result<()> {
    expected_digest(record.digest().value(), "speech saved artifact digest")?;
    let digest = record.digest().value();
    if record.format().as_str() != format.to_string() || record.media_type() != &MediaType::for_format(format)
        || record.size_bytes() == 0 || record.size_bytes() > maximum || record.storage_class() != storage
        || record.payload().relative_path() != PathBuf::from("objects").join("sha256").join(&digest[..2]).join(digest)
        || record.id() != &ArtifactId::from_record(record.digest(), record.format(), record.sources())
    { bail!("speech saved-result artifact record is inconsistent"); }
    Ok(())
}

fn read_store_payloads(receipt: &SavedSpeechReceipt, store: &ArtifactStore, cancellation: &ProcessCancellationToken) -> Result<Vec<(Artifact, Vec<u8>)>> {
    let mut records = retained_artifacts(&receipt.canonical)?;
    records.push(receipt.canonical.clone());
    records.into_iter().map(|record| {
        let bytes = read_record(&store.payload_path(&record)?, &record, cancellation)?;
        Ok((record, bytes))
    }).collect()
}

fn read_record(path: &Path, record: &Artifact, cancellation: &ProcessCancellationToken) -> Result<Vec<u8>> {
    ensure_not_cancelled(cancellation)?;
    let bytes = read_bounded(path, record.size_bytes(), cancellation)?;
    if bytes.len() as u64 != record.size_bytes() || sha256(&bytes) != record.digest().value() {
        bail!("speech saved-result payload differs from its pinned artifact record");
    }
    Ok(bytes)
}

fn payload<'a>(payloads: &'a [(Artifact, Vec<u8>)], artifact: &Artifact) -> Result<&'a [u8]> {
    payloads.iter().find(|(record, _)| record == artifact).map(|(_, bytes)| bytes.as_slice())
        .context("speech saved-result payload is missing")
}

fn verify_payloads(receipt: &SavedSpeechReceipt, payloads: &[(Artifact, Vec<u8>)], cancellation: &ProcessCancellationToken) -> Result<VerifiedSpeechTranscription> {
    ensure_not_cancelled(cancellation)?;
    let evidence: SpeechArtifactEvidence = serde_json::from_value(receipt.canonical.metadata()[SPEECH_ARTIFACT_METADATA_KEY].clone())?;
    let transcription: SpeechTranscription = parse_exact(payload(payloads, &evidence.transcription)?, "speech transcription envelope")?;
    let identity = &transcription.evidence.identity;
    let policy = &receipt.policy;
    if identity.provider_id != WHISPER_PROVIDER_ID || identity.provider_sha256 != policy.provider.sha256
        || identity.model_id != policy.model.id || identity.model_sha256 != policy.model.sha256
        || transcription.evidence.options != policy.options
    { bail!("speech saved-result provider, model, or options differ from its recognition policy"); }
    // The provider recorded the resolved model path. Its configured ancestor
    // may have been a symlink and need not exist now. The pinned receipt binds
    // configured paths; the envelope/raw JSON bind the historical resolved path.
    if !Path::new(&transcription.evidence.provider_reported_model).is_absolute() {
        bail!("speech saved-result historical model path must be absolute");
    }
    let raw = payload(payloads, &evidence.provider_output)?;
    let mut rebuilt = normalize_whisper_json(raw, identity, &policy.options)?;
    let recognition_duration = match (&policy.audio, &transcription.evidence.audio_preparation, &evidence.audio_probe, &evidence.prepared_audio) {
        (None, None, None, None) => {
            if identity.source_sha256 != receipt.source.digest().value() { bail!("speech saved strict-WAV identity differs from original source"); }
            receipt.source_wav_duration_ms.context("speech saved original WAV duration is missing")?
        }
        (Some(audio), Some(preparation), Some(probe), Some(wav)) => {
            preparation.validate()?;
            if preparation.source_sha256 != receipt.source.digest().value() || preparation.source_size_bytes != receipt.source.size_bytes()
                || preparation.selected_stream.index != audio.stream_index || preparation.timeout_seconds != audio.timeout_seconds
                || preparation.ffprobe.sha256 != audio.ffprobe.sha256 || preparation.ffmpeg.sha256 != audio.ffmpeg.sha256
                || preparation.probe_output_sha256 != probe.digest().value() || preparation.prepared_wav_sha256 != wav.digest().value()
                || preparation.prepared_wav_size_bytes != wav.size_bytes()
            { bail!("speech saved preparation differs from its source, selected stream, or pinned tools"); }
            let source_format: Format = receipt.source.format().as_str().parse()?;
            let (container_format, streams) = super::audio::parse_stored_probe(payload(payloads, probe)?, source_format, &preparation.demuxer)?;
            if container_format != preparation.container_format
                || !streams.iter().any(|stream| stream == &preparation.selected_stream)
            { bail!("speech saved preparation does not match its retained raw probe"); }
            let duration = inspect_pcm_wav(payload(payloads, wav)?)?;
            if duration != preparation.prepared_wav_duration_ms { bail!("speech saved prepared WAV duration differs from its envelope"); }
            attach_audio_preparation(&mut rebuilt, preparation)?;
            duration
        }
        _ => bail!("speech saved preparation policy and evidence are inconsistent"),
    };
    if transcription.transcript.cues.iter().any(|cue| cue.end_ms > recognition_duration)
        || rebuilt != transcription
        || super::graph::bounded_json(&transcription.transcript, MAX_INPUT_BYTES)? != payload(payloads, &receipt.canonical)?
    { bail!("speech saved-result envelope/canonical does not exactly reproduce its retained provider output"); }
    ensure_not_cancelled(cancellation)?;
    Ok(VerifiedSpeechTranscription { transcription, original_source: receipt.source.clone() })
}

fn parse_exact<T: for<'de> Deserialize<'de> + Serialize>(bytes: &[u8], name: &str) -> Result<T> {
    let StrictValue(value) = serde_json::from_slice::<StrictValue>(bytes).with_context(|| format!("invalid or duplicate-key {name}"))?;
    let parsed: T = serde_json::from_value(value.clone()).with_context(|| format!("invalid {name}"))?;
    if serde_json::to_value(&parsed)? != value { bail!("{name} contains unsupported or omitted fields"); }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::sync::atomic::AtomicBool;
    use crate::artifact::{ArtifactDigest, ArtifactTransform};
    use crate::speech::graph::{SpeechArtifactTransform, SpeechProgressObserver, SpeechProgressPhase, SPEECH_ARTIFACT_SCHEMA};
    use crate::speech::policy::{SpeechAudioPolicy, SpeechModelPolicy, SpeechRecoveryPolicy, SpeechToolPolicy};
    use crate::speech::{audio_preparation_configuration_digest, AudioPreparationEvidence, AudioPreparationLimits, AudioToolIdentity, SpeechError, SpeechIdentity, SpeechOptions, AUDIO_PREPARATION_PROFILE, AUDIO_TIMESTAMP_ORIGIN};
    use crate::timed_text::graph::CanonicalTranscriptReference;

    const RAW: &[u8] = include_bytes!("../../../../tests/fixtures/speech/whisper-segments.json");
    const PROBE: &[u8] = br#"{"format":{"format_name":"matroska,webm"},"streams":[{"index":2,"codec_name":"opus","codec_type":"audio","channels":2,"sample_rate":"48000","duration":"3.0","start_time":"0.5"}]}"#;

    struct Fixture {
        root: tempfile::TempDir,
        store: ArtifactStore,
        input: Artifact,
        policy: SpeechPolicy,
        canonical: Artifact,
        receipt_path: PathBuf,
        pin: String,
    }

    fn wav() -> Vec<u8> {
        let size = 96_000_u32;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36 + size).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16_u32.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&16_000_u32.to_le_bytes());
        bytes.extend_from_slice(&32_000_u32.to_le_bytes());
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&16_u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&size.to_le_bytes());
        bytes.resize(44 + size as usize, 0);
        bytes
    }

    fn sidecar(store: &ArtifactStore, bytes: &[u8], format: Format, role: &str, sources: Vec<ArtifactId>) -> Artifact {
        store.put_bytes(bytes, ArtifactDescriptor::for_format(format, ArtifactStorageClass::Intermediate)
            .with_sources(sources).with_metadata("renderflow.speech.role", role)).unwrap()
    }

    // Authored synthetic provider output and PCM only. No native process runs,
    // model files, real recordings, decoding qualification, or accuracy claims.
    fn fixture(prepared: bool) -> Fixture {
        let root = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(root.path().join("source-store")).unwrap();
        let waveform = wav();
        let format = if prepared { Format::Mkv } else { Format::Wav };
        let source_bytes = if prepared { b"synthetic original container record".as_slice() } else { &waveform };
        let input = store.put_bytes(source_bytes, ArtifactDescriptor::for_format(format, ArtifactStorageClass::Source)
            .with_metadata("renderflow.source_id", "original-voice")
            .with_metadata("renderflow.intake.schema", "synthetic-intake")
            .with_metadata("renderflow.intake.profile", "synthetic-profile")).unwrap();
        let tool = |name: &str, pin: char| SpeechToolPolicy {
            path: root.path().join(name).display().to_string(), sha256: pin.to_string().repeat(64),
        };
        let policy = SpeechPolicy {
            source_sha256: input.digest().value().to_owned(), provider: tool("absent-whisper-cli", 'a'),
            model: SpeechModelPolicy { path: root.path().join("absent-model.bin").display().to_string(), sha256: "b".repeat(64), id: "synthetic".to_owned() },
            options: SpeechOptions::default(),
            audio: prepared.then(|| SpeechAudioPolicy { stream_index: 2, ffprobe: tool("absent-ffprobe", 'c'), ffmpeg: tool("absent-ffmpeg", 'd'), timeout_seconds: 300 }),
            recovery: None,
        };
        let (probe, prepared_audio, preparation) = if prepared {
            let probe = sidecar(&store, PROBE, Format::Json, "audio_probe", vec![input.id().clone()]);
            let audio = sidecar(&store, &waveform, Format::Wav, "prepared_audio", vec![input.id().clone(), probe.id().clone()]);
            let (container_format, streams) = super::super::audio::parse_stored_probe(PROBE, Format::Mkv, "matroska").unwrap();
            let mut preparation = AudioPreparationEvidence {
                source_sha256: policy.source_sha256.clone(), source_size_bytes: input.size_bytes(), container_format,
                demuxer: "matroska".to_owned(), selected_stream: streams[0].clone(),
                ffprobe: AudioToolIdentity { tool_id: "tool.ffprobe".to_owned(), version: "ffprobe synthetic".to_owned(), sha256: "c".repeat(64) },
                ffmpeg: AudioToolIdentity { tool_id: "tool.ffmpeg".to_owned(), version: "ffmpeg synthetic".to_owned(), sha256: "d".repeat(64) },
                probe_output_sha256: probe.digest().value().to_owned(), prepared_wav_sha256: audio.digest().value().to_owned(),
                prepared_wav_size_bytes: audio.size_bytes(), prepared_wav_duration_ms: 3000,
                profile: AUDIO_PREPARATION_PROFILE.to_owned(), configuration_sha256: String::new(),
                timestamp_origin: AUDIO_TIMESTAMP_ORIGIN.to_owned(), timeout_seconds: 300,
                limits: AudioPreparationLimits::default(), limitations: vec!["Synthetic retained-evidence fixture; no native decoding occurred.".to_owned()],
            };
            preparation.configuration_sha256 = audio_preparation_configuration_digest(&preparation).unwrap();
            (Some(probe), Some(audio), Some(preparation))
        } else { (None, None, None) };
        let recognition_source = prepared_audio.as_ref().unwrap_or(&input);
        let mut transcription = normalize_whisper_json(RAW, &SpeechIdentity {
            provider_id: WHISPER_PROVIDER_ID.to_owned(), provider_version: "synthetic-whisper-1".to_owned(),
            provider_sha256: policy.provider.sha256.clone(), model_id: policy.model.id.clone(), model_sha256: policy.model.sha256.clone(),
            source_sha256: recognition_source.digest().value().to_owned(),
        }, &policy.options).unwrap();
        if let Some(preparation) = &preparation { attach_audio_preparation(&mut transcription, preparation).unwrap(); }
        let raw = sidecar(&store, RAW, Format::Json, "provider_output", vec![recognition_source.id().clone()]);
        let mut envelope_sources = vec![input.id().clone(), raw.id().clone()];
        if let Some(probe) = &probe { envelope_sources.push(probe.id().clone()); }
        if let Some(audio) = &prepared_audio { envelope_sources.push(audio.id().clone()); }
        let envelope = sidecar(&store, &super::super::graph::bounded_json(&transcription, MAX_INPUT_BYTES).unwrap(), Format::Json, "transcription", envelope_sources);
        let mut sources = vec![input.id().clone(), raw.id().clone(), envelope.id().clone()];
        if let Some(probe) = &probe { sources.push(probe.id().clone()); }
        if let Some(audio) = &prepared_audio { sources.push(audio.id().clone()); }
        // Exactly the graph serializer, including its trailing newline.
        let bytes = super::super::graph::bounded_json(&transcription.transcript, MAX_INPUT_BYTES).unwrap();
        let digest = sha256(&bytes);
        let canonical_id = ArtifactId::from_record(&ArtifactDigest::sha256(digest.clone()), &Format::TimedTranscriptJson.into(), &sources);
        let canonical = store.put_bytes(&bytes, ArtifactDescriptor::for_format(Format::TimedTranscriptJson, ArtifactStorageClass::Intermediate)
            .with_sources(sources).with_metadata(SPEECH_ARTIFACT_METADATA_KEY, serde_json::to_value(SpeechArtifactEvidence {
                schema_version: SPEECH_ARTIFACT_SCHEMA.to_owned(), input_artifact_id: input.id().clone(), input_sha256: policy.source_sha256.clone(),
                canonical_transcript: CanonicalTranscriptReference { artifact_id: canonical_id, sha256: digest },
                timestamp_origin: if prepared { AUDIO_TIMESTAMP_ORIGIN } else { "input_wav_samples_from_zero" }.to_owned(), human_reviewed: false,
                provider_output: raw, transcription: envelope, audio_probe: probe, prepared_audio,
            }).unwrap())).unwrap();
        let directory = root.path().join("portable-bundle");
        std::fs::create_dir(&directory).unwrap();
        for record in retained_artifacts(&canonical).unwrap() {
            std::fs::copy(store.payload_path(&record).unwrap(), directory.join(retained_artifact_name(&record).unwrap())).unwrap();
        }
        std::fs::write(directory.join(CANONICAL_FILENAME), &bytes).unwrap();
        let receipt = receipt_bytes(&input, &policy, &canonical, &store, &ProcessCancellationToken::default()).unwrap();
        let receipt_path = directory.join(SAVED_RESULT_FILENAME);
        std::fs::write(&receipt_path, &receipt).unwrap();
        Fixture { root, store, input, policy, canonical, receipt_path, pin: sha256(&receipt) }
    }

    #[derive(Default)]
    struct Phases(Mutex<Vec<SpeechProgressPhase>>);
    impl SpeechProgressObserver for Phases {
        fn phase(&self, phase: SpeechProgressPhase) { self.0.lock().unwrap().push(phase); }
    }

    #[test]
    fn portable_replay_preserves_exact_graph_bytes_without_native_tools_or_original_store() {
        let fixture = fixture(false);
        let new_store = ArtifactStore::new(fixture.root.path().join("new-store")).unwrap();
        let input = new_store.put_bytes(&wav(), ArtifactDescriptor::for_format(Format::Wav, ArtifactStorageClass::Source)
            .with_metadata("renderflow.source_id", "renamed-source")
            .with_metadata("renderflow.intake.profile", "new-intake-profile")).unwrap();
        let original_receipt = std::fs::read(&fixture.receipt_path).unwrap();
        // The portable bundle suffices; its historical CAS is not a dependency.
        std::fs::remove_dir_all(fixture.store.root()).unwrap();
        let mut policy = fixture.policy;
        policy.recovery = Some(SpeechRecoveryPolicy { receipt_path: fixture.receipt_path.display().to_string(), receipt_sha256: fixture.pin });
        let phases = Arc::new(Phases::default());
        let transform = SpeechArtifactTransform::new(Format::Wav, policy.clone(), None).unwrap().with_observer(phases.clone());
        let restored = transform.apply(&input, Format::TimedTranscriptJson, &new_store).unwrap();
        assert_eq!(restored, fixture.canonical);
        assert!(!transform.cacheable());
        assert_eq!(*phases.0.lock().unwrap(), vec![SpeechProgressPhase::VerifyInput, SpeechProgressPhase::ReuseSavedResult, SpeechProgressPhase::Complete]);
        assert_eq!(receipt_bytes(&input, &policy, &restored, &new_store, &ProcessCancellationToken::default()).unwrap(), original_receipt);
        for record in retained_artifacts(&restored).unwrap() { new_store.verify(&record).unwrap(); }
    }

    #[test]
    fn saved_policy_source_and_pin_mismatches_refuse_without_native_fallback() {
        let fixture = fixture(false);
        let token = ProcessCancellationToken::default();
        let mut changed = fixture.policy.clone();
        changed.options.threads += 1;
        assert!(inspect_saved_result(&fixture.receipt_path, &fixture.pin, &changed, Format::Wav, Some(&fixture.input), &token).is_err());
        changed = fixture.policy.clone();
        changed.source_sha256 = "f".repeat(64);
        assert!(inspect_saved_result(&fixture.receipt_path, &fixture.pin, &changed, Format::Wav, None, &token).is_err());
        assert!(inspect_saved_bundle(&fixture.receipt_path, &"0".repeat(64), &token).is_err());
        changed = fixture.policy.clone();
        changed.recovery = Some(SpeechRecoveryPolicy { receipt_path: fixture.receipt_path.display().to_string(), receipt_sha256: "0".repeat(64) });
        let error = SpeechArtifactTransform::new(Format::Wav, changed, None).unwrap()
            .apply(&fixture.input, Format::TimedTranscriptJson, &fixture.store).unwrap_err();
        assert!(error.to_string().contains("receipt digest mismatch"));
    }

    #[test]
    fn replay_rechecks_payloads_and_refuses_missing_or_changed_evidence() {
        let fixture = fixture(false);
        let token = ProcessCancellationToken::default();
        let saved = inspect_saved_bundle(&fixture.receipt_path, &fixture.pin, &token).unwrap();
        let raw = fixture.receipt_path.parent().unwrap().join("provider-output.json");
        std::fs::write(&raw, b"changed").unwrap();
        let new_store = ArtifactStore::new(fixture.root.path().join("new-store")).unwrap();
        assert!(restore_saved_result(&saved, &new_store, &token).is_err());
        assert!(!new_store.contains(&fixture.canonical));
        std::fs::remove_file(&raw).unwrap();
        assert!(inspect_saved_bundle(&fixture.receipt_path, &fixture.pin, &token).is_err());
    }

    #[test]
    fn a_new_receipt_pin_cannot_hide_policy_or_canonical_semantic_drift() {
        let fixture = fixture(false);
        let token = ProcessCancellationToken::default();
        let mut receipt: SavedSpeechReceipt = serde_json::from_slice(&std::fs::read(&fixture.receipt_path).unwrap()).unwrap();
        receipt.policy.options.threads += 1;
        receipt.configuration_sha256 = configuration_digest(&receipt.policy).unwrap();
        let bytes = serde_json::to_vec(&receipt).unwrap();
        std::fs::write(&fixture.receipt_path, &bytes).unwrap();
        let error = inspect_saved_bundle(&fixture.receipt_path, &sha256(&bytes), &token).unwrap_err();
        assert!(error.to_string().contains("provider, model, or options"));
    }

    #[test]
    fn prepared_bundle_revalidates_probe_waveform_and_preserves_clock_refusal() {
        let fixture = fixture(true);
        let token = ProcessCancellationToken::default();
        let saved = inspect_saved_bundle(&fixture.receipt_path, &fixture.pin, &token).unwrap();
        assert_eq!(saved.original_source_wav_duration_ms(), None);
        assert_eq!(saved.transcription().evidence.audio_preparation.as_ref().unwrap().selected_stream.start_time_ms, Some(500));
        assert!(crate::speech::timeline::require_original_media_timeline(&saved).is_err());
        std::fs::write(fixture.receipt_path.parent().unwrap().join("prepared.wav"), b"changed waveform").unwrap();
        assert!(inspect_saved_bundle(&fixture.receipt_path, &fixture.pin, &token).is_err());
    }

    #[test]
    fn cancellation_refuses_before_receipt_or_native_access() {
        let token = ProcessCancellationToken::from_shared(Arc::new(AtomicBool::new(true)));
        let error = inspect_saved_bundle(Path::new("/absent/saved-result.json"), &"0".repeat(64), &token).unwrap_err();
        assert!(matches!(error.downcast_ref::<SpeechError>(), Some(SpeechError::Cancelled)));
    }
}
