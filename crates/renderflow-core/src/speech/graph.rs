//! Opt-in speech recognition over immutable, content-addressed graph inputs.
//!
//! The canonical timed-transcript wire stays unchanged. Speech evidence and raw
//! provider/preparation payloads are retained as artifact objects, with complete
//! typed records under the native `renderflow.speech` metadata namespace.
//! Generic result reuse is disabled because linked native libraries and optional
//! accelerator sidecars are not fully qualified. Historical output selected by
//! an explicit receipt or an opt-in local index is fully verified before replay.

use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::artifact::{
    Artifact, ArtifactDescriptor, ArtifactDigest, ArtifactId, ArtifactStorageClass,
    ArtifactStore, ArtifactTransform, MediaType,
};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::process::ProcessCancellationToken;
use crate::timed_text::graph::CanonicalTranscriptReference;
use crate::timed_text::{self, MAX_INPUT_BYTES};

use super::audio::{self, AudioInspectRequest, AudioPrepareRequest, AudioTool};
use super::policy::SpeechPolicy;
use super::provider::{self, ensure_not_cancelled, expected_digest, sha256, FileSnapshot, SpeechRequest};
use super::{attach_audio_preparation, SpeechError, MAX_AUDIO_SOURCE_BYTES, MAX_PREPARED_WAV_BYTES};

pub const SPEECH_ARTIFACT_METADATA_KEY: &str = "renderflow.speech";
pub const SPEECH_ARTIFACT_SCHEMA: &str = "renderflow.speech-artifact/v1";
const ROLE_METADATA_KEY: &str = "renderflow.speech.role";
const MAX_METADATA_BYTES: usize = 64 * 1024;
const STRICT_WAV_TIMESTAMP_ORIGIN: &str = "input_wav_samples_from_zero";

/// Honest coarse phases, not estimated percentages or provider token progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechProgressPhase {
    VerifyInput,
    LookupSavedResult,
    SavedResultHit,
    SavedResultMiss,
    ReuseSavedResult,
    PrepareAudio,
    Transcribe,
    RetainArtifacts,
    Complete,
}

impl SpeechProgressPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VerifyInput => "verify_input",
            Self::LookupSavedResult => "lookup_saved_result",
            Self::SavedResultHit => "saved_result_hit",
            Self::SavedResultMiss => "saved_result_miss",
            Self::ReuseSavedResult => "reuse_saved_result",
            Self::PrepareAudio => "prepare_audio",
            Self::Transcribe => "transcribe",
            Self::RetainArtifacts => "retain_artifacts",
            Self::Complete => "complete",
        }
    }
}

/// SDK adapters may translate these local phase boundaries into progress events.
pub trait SpeechProgressObserver: Send + Sync {
    fn phase(&self, phase: SpeechProgressPhase);
    /// Selected historical receipt, never an observation of current native tools.
    fn saved_result(&self, _receipt_sha256: &str) {}
}

/// Complete artifact records keep sidecars recoverable without inventing a
/// record database in the payload-only artifact store. The canonical ID commits
/// to every retained sidecar through its ordered source list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpeechArtifactEvidence {
    pub schema_version: String,
    pub input_artifact_id: ArtifactId,
    pub input_sha256: String,
    pub canonical_transcript: CanonicalTranscriptReference,
    pub timestamp_origin: String,
    pub human_reviewed: bool,
    pub provider_output: Artifact,
    pub transcription: Artifact,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_probe: Option<Artifact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepared_audio: Option<Artifact>,
}

pub struct SpeechArtifactTransform {
    input_format: Format,
    policy: SpeechPolicy,
    cancellation: ProcessCancellationToken,
    observer: Option<Arc<dyn SpeechProgressObserver>>,
}

impl SpeechArtifactTransform {
    pub fn new(
        input_format: Format,
        policy: SpeechPolicy,
        cancellation: Option<Arc<AtomicBool>>,
    ) -> Result<Self> {
        policy.validate_source(input_format, false)?;
        Ok(Self {
            input_format,
            policy,
            cancellation: cancellation.map(ProcessCancellationToken::from_shared).unwrap_or_default(),
            observer: None,
        })
    }

    pub fn with_observer(mut self, observer: Arc<dyn SpeechProgressObserver>) -> Self {
        self.observer = Some(observer);
        self
    }

    fn phase(&self, phase: SpeechProgressPhase) -> Result<()> {
        ensure_not_cancelled(&self.cancellation)?;
        if let Some(observer) = &self.observer { observer.phase(phase); }
        ensure_not_cancelled(&self.cancellation)?;
        Ok(())
    }
}

impl ArtifactTransform for SpeechArtifactTransform {
    fn name(&self) -> &str { "renderflow.speech.transcribe" }

    fn version(&self) -> &str { "1" }

    fn cache_identity(&self) -> String {
        // Construction already structurally validated this serializable policy.
        // Generic cache reuse remains disabled; this identifies configured steps.
        let bytes = serde_json::to_vec(&self.policy).expect("SpeechPolicy serialization is infallible");
        format!("{}:v1:{}:{}", self.name(), self.input_format, sha256(&bytes))
    }

    fn cacheable(&self) -> bool { false }

    fn fidelity(&self) -> Option<FidelityDeclaration> { Some(FidelityDeclaration::Partial) }

    fn apply(&self, input: &Artifact, output_format: Format, store: &ArtifactStore) -> Result<Artifact> {
        if input.format().as_str() != self.input_format.to_string()
            || output_format != Format::TimedTranscriptJson
        {
            bail!("speech transform expected '{}' -> 'timed_transcript_json'", self.input_format);
        }
        self.phase(SpeechProgressPhase::VerifyInput)?;
        self.policy.validate_source(self.input_format, false)?;
        expected_digest(input.digest().value(), "artifact source digest")?;
        if input.id() != &ArtifactId::from_record(input.digest(), input.format(), input.sources()) {
            bail!("speech source artifact identity does not match its record");
        }
        if input.digest().value() != self.policy.source_sha256 {
            return Err(SpeechError::InvalidRequest(
                "speech policy source SHA-256 does not match the selected immutable artifact".to_owned(),
            ).into());
        }
        if !input.sources().is_empty() {
            return Err(SpeechError::InvalidRequest(
                "speech execution must consume the original source artifact, not a converted intermediate".to_owned(),
            ).into());
        }
        let maximum = if self.policy.audio.is_some() { MAX_AUDIO_SOURCE_BYTES } else { provider::MAX_SOURCE_BYTES };
        if input.size_bytes() == 0 || input.size_bytes() > maximum {
            return Err(SpeechError::InvalidRequest("speech source exceeds its nonempty byte profile".to_owned()).into());
        }
        let source_path = store.payload_path(input)?;
        let snapshot = FileSnapshot::read(&source_path, "artifact source", maximum, &self.cancellation)?;
        if snapshot.sha256 != input.digest().value() || snapshot.metadata.len() != input.size_bytes() {
            return Err(SpeechError::SourceChanged("artifact payload does not match its immutable record".to_owned()).into());
        }

        let saved = if let Some(recovery) = &self.policy.recovery {
            self.phase(SpeechProgressPhase::ReuseSavedResult)?;
            Some(super::recovery::inspect_saved_result(
                std::path::Path::new(&recovery.receipt_path), &recovery.receipt_sha256,
                &self.policy, self.input_format, Some(input), &self.cancellation,
            )?)
        } else if self.policy.saved_results.is_enabled() {
            self.phase(SpeechProgressPhase::LookupSavedResult)?;
            let saved = super::cache::lookup_saved_result(store.root(), &self.policy,
                self.input_format, Some(input), &self.cancellation)?;
            self.phase(if saved.is_some() { SpeechProgressPhase::SavedResultHit }
                else { SpeechProgressPhase::SavedResultMiss })?;
            saved
        } else { None };
        if let Some(saved) = saved {
            if let Some(observer) = &self.observer { observer.saved_result(saved.receipt_sha256()); }
            ensure_not_cancelled(&self.cancellation)?;
            if self.policy.audio.is_none() {
                let bytes = provider::read_bounded(&source_path, provider::MAX_SOURCE_BYTES, &self.cancellation)?;
                if provider::sha256(&bytes) != input.digest().value()
                    || Some(provider::inspect_pcm_wav(&bytes)?) != saved.original_source_wav_duration_ms()
                { bail!("speech saved-result WAV duration or digest differs from the current original source"); }
            }
            snapshot.recheck(&self.cancellation)?;
            let output = super::recovery::restore_saved_result(&saved, store, &self.cancellation)?;
            snapshot.recheck(&self.cancellation)?;
            self.phase(SpeechProgressPhase::Complete)?;
            return Ok(output);
        }

        let prepared = if let Some(audio) = &self.policy.audio {
            self.phase(SpeechProgressPhase::PrepareAudio)?;
            Some(audio::prepare_stored(&AudioPrepareRequest {
                inspection: AudioInspectRequest {
                    input: source_path.clone(),
                    ffprobe: AudioTool { path: PathBuf::from(&audio.ffprobe.path), sha256: audio.ffprobe.sha256.clone() },
                },
                expected_source_sha256: self.policy.source_sha256.clone(),
                ffmpeg: AudioTool { path: PathBuf::from(&audio.ffmpeg.path), sha256: audio.ffmpeg.sha256.clone() },
                stream_index: audio.stream_index,
                timeout_seconds: audio.timeout_seconds,
            }, self.input_format, self.cancellation.clone())?)
        } else { None };
        self.phase(SpeechProgressPhase::Transcribe)?;
        let recognition_source = prepared.as_ref().map(|audio| audio.path.clone()).unwrap_or(source_path);
        let recognition_sha256 = prepared.as_ref().map(|audio| audio.evidence.prepared_wav_sha256.as_str())
            .unwrap_or(self.policy.source_sha256.as_str());
        let run = provider::transcribe_stored(&SpeechRequest {
            input: recognition_source,
            provider_path: PathBuf::from(&self.policy.provider.path),
            provider_sha256: self.policy.provider.sha256.clone(),
            model_path: PathBuf::from(&self.policy.model.path),
            model_id: self.policy.model.id.clone(),
            model_sha256: self.policy.model.sha256.clone(),
            options: self.policy.options.clone(),
        }, recognition_sha256, self.cancellation.clone())?;
        let mut transcription = run.transcription;
        if let Some(audio) = &prepared {
            audio.recheck(&self.cancellation)?;
            attach_audio_preparation(&mut transcription, &audio.evidence)?;
        }
        snapshot.recheck(&self.cancellation)?;
        if sha256(&run.provider_output) != transcription.evidence.provider_output_sha256 {
            bail!("speech provider output does not match retained transcription evidence");
        }
        self.phase(SpeechProgressPhase::RetainArtifacts)?;
        // Serialize all bounded JSON before adding sidecar records. Failed or
        // cancelled runs may leave unreferenced objects, never a success receipt.
        let envelope_bytes = bounded_json(&transcription, MAX_INPUT_BYTES)?;
        let canonical_bytes = bounded_json(&transcription.transcript, MAX_INPUT_BYTES)?;
        timed_text::validate(&transcription.transcript)?;
        let (audio_probe, prepared_audio) = if let Some(audio) = &prepared {
            if sha256(&audio.probe_output) != audio.evidence.probe_output_sha256 {
                bail!("audio probe output does not match retained preparation evidence");
            }
            let waveform = audio.read_wav(&self.cancellation)?;
            let probe = store_sidecar(store, &audio.probe_output, Format::Json, "audio_probe", &[input.id().clone()])?;
            let wav = store_sidecar(store, &waveform, Format::Wav, "prepared_audio", &[input.id().clone(), probe.id().clone()])?;
            (Some(probe), Some(wav))
        } else { (None, None) };
        ensure_not_cancelled(&self.cancellation)?;
        let recognition_input = prepared_audio.as_ref().unwrap_or(input);
        let provider_output = store_sidecar(store, &run.provider_output, Format::Json,
            "provider_output", &[recognition_input.id().clone()])?;
        let mut envelope_sources = vec![input.id().clone(), provider_output.id().clone()];
        if let Some(probe) = &audio_probe { envelope_sources.push(probe.id().clone()); }
        if let Some(wav) = &prepared_audio { envelope_sources.push(wav.id().clone()); }
        let envelope = store_sidecar(store, &envelope_bytes, Format::Json, "transcription", &envelope_sources)?;
        let mut sources = vec![input.id().clone(), provider_output.id().clone(), envelope.id().clone()];
        if let Some(probe) = &audio_probe { sources.push(probe.id().clone()); }
        if let Some(wav) = &prepared_audio { sources.push(wav.id().clone()); }
        let canonical_sha256 = sha256(&canonical_bytes);
        let canonical_id = ArtifactId::from_record(&ArtifactDigest::sha256(canonical_sha256.clone()),
            &Format::TimedTranscriptJson.into(), &sources);
        let evidence = SpeechArtifactEvidence {
            schema_version: SPEECH_ARTIFACT_SCHEMA.to_owned(),
            input_artifact_id: input.id().clone(),
            input_sha256: input.digest().value().to_owned(),
            canonical_transcript: CanonicalTranscriptReference { artifact_id: canonical_id, sha256: canonical_sha256 },
            timestamp_origin: if prepared_audio.is_some() { super::AUDIO_TIMESTAMP_ORIGIN } else { STRICT_WAV_TIMESTAMP_ORIGIN }.to_owned(),
            human_reviewed: false,
            provider_output,
            transcription: envelope,
            audio_probe,
            prepared_audio,
        };
        ensure_not_cancelled(&self.cancellation)?;
        let output = store.put_bytes(&canonical_bytes,
            ArtifactDescriptor::for_format(Format::TimedTranscriptJson, ArtifactStorageClass::Intermediate)
                .with_sources(sources)
                .with_metadata(SPEECH_ARTIFACT_METADATA_KEY, serde_json::to_value(evidence)?))?;
        retained_artifacts(&output)?;
        self.phase(SpeechProgressPhase::Complete)?;
        Ok(output)
    }
}

fn store_sidecar(
    store: &ArtifactStore, bytes: &[u8], format: Format, role: &str, sources: &[ArtifactId],
) -> Result<Artifact> {
    store.put_bytes(bytes, ArtifactDescriptor::for_format(format, ArtifactStorageClass::Intermediate)
        .with_sources(sources.iter().cloned()).with_metadata(ROLE_METADATA_KEY, role))
}

/// Return retained speech records in source-before-descendant order. Metadata
/// without this namespace is ordinary non-speech input. A present but malformed
/// namespace is refused instead of silently dropping evidence. This validates
/// record structure and relationships; callers verify payloads through the store.
pub fn retained_artifacts(artifact: &Artifact) -> Result<Vec<Artifact>> {
    let Some(metadata) = artifact.metadata().get(SPEECH_ARTIFACT_METADATA_KEY) else { return Ok(Vec::new()); };
    let bytes = bounded_json(metadata, MAX_METADATA_BYTES).context("speech artifact metadata exceeds its bound")?;
    let evidence: SpeechArtifactEvidence = serde_json::from_slice(&bytes).context("invalid speech artifact metadata")?;
    if serde_json::to_value(&evidence)? != *metadata {
        bail!("speech artifact metadata must preserve every field in its typed representation");
    }
    if evidence.schema_version != SPEECH_ARTIFACT_SCHEMA || evidence.human_reviewed {
        bail!("unsupported or falsely reviewed speech artifact evidence");
    }
    expected_digest(&evidence.input_sha256, "speech input digest")?;
    expected_digest(&evidence.canonical_transcript.sha256, "canonical transcript digest")?;
    validate_artifact_id(&evidence.input_artifact_id)?;
    validate_artifact_id(&evidence.canonical_transcript.artifact_id)?;
    let mut retained = Vec::new();
    let mut envelope_sources = vec![evidence.input_artifact_id.clone(), evidence.provider_output.id().clone()];
    let mut canonical_sources = vec![evidence.input_artifact_id.clone(), evidence.provider_output.id().clone(), evidence.transcription.id().clone()];
    let recognition_source = match (&evidence.audio_probe, &evidence.prepared_audio) {
        (Some(probe), Some(wav)) => {
            if evidence.timestamp_origin != super::AUDIO_TIMESTAMP_ORIGIN { bail!("invalid prepared speech timestamp origin"); }
            validate_sidecar(probe, Format::Json, "audio_probe", audio::MAX_PROBE_BYTES as u64, &[evidence.input_artifact_id.clone()])?;
            validate_sidecar(wav, Format::Wav, "prepared_audio", MAX_PREPARED_WAV_BYTES, &[evidence.input_artifact_id.clone(), probe.id().clone()])?;
            retained.extend([probe.clone(), wav.clone()]);
            envelope_sources.extend([probe.id().clone(), wav.id().clone()]);
            canonical_sources.extend([probe.id().clone(), wav.id().clone()]);
            wav.id().clone()
        }
        (None, None) => {
            if evidence.timestamp_origin != STRICT_WAV_TIMESTAMP_ORIGIN { bail!("invalid strict WAV speech timestamp origin"); }
            evidence.input_artifact_id.clone()
        }
        _ => bail!("speech preparation requires both raw probe and prepared waveform references"),
    };
    validate_sidecar(&evidence.provider_output, Format::Json, "provider_output", provider::MAX_PROVIDER_OUTPUT_BYTES, &[recognition_source])?;
    validate_sidecar(&evidence.transcription, Format::Json, "transcription", MAX_INPUT_BYTES as u64, &envelope_sources)?;
    let expected_canonical_id = ArtifactId::from_record(
        &ArtifactDigest::sha256(evidence.canonical_transcript.sha256.clone()),
        &Format::TimedTranscriptJson.into(), &canonical_sources,
    );
    if evidence.canonical_transcript.artifact_id != expected_canonical_id {
        bail!("speech canonical transcript identity does not commit to retained evidence");
    }
    if artifact.format().as_str() == Format::TimedTranscriptJson.to_string() {
        if artifact.id() != &expected_canonical_id || artifact.digest().value() != evidence.canonical_transcript.sha256
            || artifact.sources() != canonical_sources
        { bail!("speech canonical transcript record does not match its evidence"); }
    } else if artifact.format().as_str() == Format::Srt.to_string() || artifact.format().as_str() == Format::WebVtt.to_string() {
        if artifact.sources() != [expected_canonical_id] { bail!("speech export must retain its canonical transcript parent"); }
    } else { bail!("speech evidence is attached to an unsupported artifact format"); }
    if artifact.id() != &ArtifactId::from_record(artifact.digest(), artifact.format(), artifact.sources()) {
        bail!("speech-bearing artifact identity does not match its record");
    }
    retained.extend([evidence.provider_output, evidence.transcription]);
    Ok(retained)
}

/// Closed output names for already validated retained records. Names never
/// derive from provider text, source paths, model labels, or arbitrary metadata.
pub fn retained_artifact_name(artifact: &Artifact) -> Result<&'static str> {
    match artifact.metadata().get(ROLE_METADATA_KEY).and_then(serde_json::Value::as_str) {
        Some("audio_probe") => Ok("audio-probe.json"),
        Some("prepared_audio") => Ok("prepared.wav"),
        Some("provider_output") => Ok("provider-output.json"),
        Some("transcription") => Ok("transcription.json"),
        _ => bail!("unsupported retained speech artifact role"),
    }
}

fn validate_sidecar(
    artifact: &Artifact, format: Format, role: &str, maximum: u64, sources: &[ArtifactId],
) -> Result<()> {
    expected_digest(artifact.digest().value(), "speech sidecar digest")?;
    validate_artifact_id(artifact.id())?;
    for source in artifact.sources() { validate_artifact_id(source)?; }
    let digest = artifact.digest().value();
    let expected_path = PathBuf::from("objects").join("sha256").join(&digest[..2]).join(digest);
    if artifact.format().as_str() != format.to_string()
        || artifact.media_type() != &MediaType::for_format(format)
        || artifact.size_bytes() == 0 || artifact.size_bytes() > maximum
        || artifact.storage_class() != ArtifactStorageClass::Intermediate
        || artifact.sources() != sources
        || artifact.payload().relative_path() != expected_path
        || artifact.metadata().len() != 1
        || artifact.metadata().get(ROLE_METADATA_KEY).and_then(serde_json::Value::as_str) != Some(role)
        || artifact.id() != &ArtifactId::from_record(artifact.digest(), artifact.format(), artifact.sources())
    { bail!("invalid retained speech artifact role '{role}'"); }
    Ok(())
}

fn validate_artifact_id(id: &ArtifactId) -> Result<()> {
    let Some(digest) = id.as_str().strip_prefix("artifact:sha256:") else { bail!("invalid speech artifact identifier"); };
    expected_digest(digest, "speech artifact identifier")?;
    Ok(())
}

pub(super) fn bounded_json(value: &impl Serialize, maximum: usize) -> Result<Vec<u8>> {
    let mut writer = BoundedJsonWriter { bytes: Vec::new(), maximum };
    serde_json::to_writer(&mut writer, value)?;
    writer.write_all(b"\n")?;
    Ok(writer.bytes)
}

struct BoundedJsonWriter { bytes: Vec<u8>, maximum: usize }

impl Write for BoundedJsonWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.maximum.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("speech JSON exceeds its byte bound"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::policy::{SpeechModelPolicy, SpeechToolPolicy};
    use crate::speech::{normalize_whisper_json, SpeechIdentity, SpeechOptions, WHISPER_PROVIDER_ID};
    use crate::timed_text::graph::TimedTextArtifactTransform;

    const PROVIDER_JSON: &[u8] = include_bytes!("../../../../tests/fixtures/speech/whisper-segments.json");

    fn policy(source_sha256: &str) -> SpeechPolicy {
        SpeechPolicy {
            source_sha256: source_sha256.to_owned(),
            provider: SpeechToolPolicy { path: std::env::temp_dir().join("synthetic-absent-whisper-cli").display().to_string(), sha256: "a".repeat(64) },
            model: SpeechModelPolicy {
                path: std::env::temp_dir().join("synthetic-absent-model.bin").display().to_string(), sha256: "b".repeat(64), id: "synthetic".to_owned(),
            },
            options: SpeechOptions::default(),
            audio: None,
            recovery: None,
            saved_results: Default::default(),
        }
    }

    fn source(store: &ArtifactStore) -> Artifact {
        store.put_bytes(b"Synthetic source; no audio or provider execution", ArtifactDescriptor::for_format(
            Format::Wav, ArtifactStorageClass::Source)).unwrap()
    }

    // Structural fixture only: provider text is authored JSON and the source is
    // deliberately not playable media. This never qualifies native recognition.
    fn canonical(store: &ArtifactStore, input: &Artifact, version: &str) -> Artifact {
        let transcription = normalize_whisper_json(PROVIDER_JSON, &SpeechIdentity {
            provider_id: WHISPER_PROVIDER_ID.to_owned(), provider_version: version.to_owned(),
            provider_sha256: "a".repeat(64), model_id: "synthetic".to_owned(),
            model_sha256: "b".repeat(64), source_sha256: input.digest().value().to_owned(),
        }, &SpeechOptions::default()).unwrap();
        let provider = store_sidecar(store, PROVIDER_JSON, Format::Json, "provider_output", &[input.id().clone()]).unwrap();
        let envelope = store_sidecar(store, &bounded_json(&transcription, MAX_INPUT_BYTES).unwrap(),
            Format::Json, "transcription", &[input.id().clone(), provider.id().clone()]).unwrap();
        let bytes = bounded_json(&transcription.transcript, MAX_INPUT_BYTES).unwrap();
        let sources = vec![input.id().clone(), provider.id().clone(), envelope.id().clone()];
        let digest = sha256(&bytes);
        let id = ArtifactId::from_record(&ArtifactDigest::sha256(digest.clone()),
            &Format::TimedTranscriptJson.into(), &sources);
        store.put_bytes(&bytes, ArtifactDescriptor::for_format(Format::TimedTranscriptJson, ArtifactStorageClass::Intermediate)
            .with_sources(sources)
            .with_metadata(SPEECH_ARTIFACT_METADATA_KEY, serde_json::to_value(SpeechArtifactEvidence {
                schema_version: SPEECH_ARTIFACT_SCHEMA.to_owned(),
                input_artifact_id: input.id().clone(), input_sha256: input.digest().value().to_owned(),
                canonical_transcript: CanonicalTranscriptReference { artifact_id: id, sha256: digest },
                timestamp_origin: STRICT_WAV_TIMESTAMP_ORIGIN.to_owned(), human_reviewed: false,
                provider_output: provider, transcription: envelope, audio_probe: None, prepared_audio: None,
            }).unwrap())).unwrap()
    }

    #[test]
    fn cancellation_and_source_pin_refuse_before_native_tool_access() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let input = source(&store);
        let transform = SpeechArtifactTransform::new(Format::Wav, policy(input.digest().value()),
            Some(Arc::new(AtomicBool::new(true)))).unwrap();
        assert!(!transform.cacheable());
        let error = transform.apply(&input, Format::TimedTranscriptJson, &store).unwrap_err();
        assert!(matches!(error.downcast_ref::<SpeechError>(), Some(SpeechError::Cancelled)));
        let mismatch = SpeechArtifactTransform::new(Format::Wav, policy(&"0".repeat(64)), None).unwrap();
        let error = mismatch.apply(&input, Format::TimedTranscriptJson, &store).unwrap_err();
        assert!(matches!(error.downcast_ref::<SpeechError>(), Some(SpeechError::InvalidRequest(message))
            if message.contains("selected immutable artifact")));
    }

    #[test]
    fn changed_stored_source_and_converted_intermediate_refuse_before_native_work() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let input = source(&store);
        let intermediate = store.put_bytes(b"derived", ArtifactDescriptor::for_format(Format::Wav,
            ArtifactStorageClass::Intermediate).with_source(input.id().clone())).unwrap();
        let transform = SpeechArtifactTransform::new(Format::Wav, policy(intermediate.digest().value()), None).unwrap();
        let error = transform.apply(&intermediate, Format::TimedTranscriptJson, &store).unwrap_err();
        assert!(matches!(error.downcast_ref::<SpeechError>(), Some(SpeechError::InvalidRequest(message))
            if message.contains("original source artifact")));
        std::fs::write(store.payload_path(&input).unwrap(), b"persistent mutation").unwrap();
        let transform = SpeechArtifactTransform::new(Format::Wav, policy(input.digest().value()), None).unwrap();
        let error = transform.apply(&input, Format::TimedTranscriptJson, &store).unwrap_err();
        assert!(matches!(error.downcast_ref::<SpeechError>(), Some(SpeechError::SourceChanged(_))));
    }

    #[test]
    fn retained_sidecars_survive_both_subtitle_exports_with_uncached_lineage() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let input = source(&store);
        let transcript = canonical(&store, &input, "synthetic-1");
        let retained = retained_artifacts(&transcript).unwrap();
        assert_eq!(retained.len(), 2);
        assert_eq!(retained_artifact_name(&retained[0]).unwrap(), "provider-output.json");
        assert_eq!(retained_artifact_name(&retained[1]).unwrap(), "transcription.json");
        assert_eq!(transcript.sources(), &[input.id().clone(), retained[0].id().clone(), retained[1].id().clone()]);
        for format in [Format::Srt, Format::WebVtt] {
            let transform = TimedTextArtifactTransform::new(Format::TimedTranscriptJson, format).unwrap().without_cache();
            assert!(!transform.cacheable());
            let exported = transform.apply(&transcript, format, &store).unwrap();
            assert_eq!(exported.sources(), &[transcript.id().clone()]);
            assert_eq!(retained_artifacts(&exported).unwrap(), retained);
            assert_eq!(exported.metadata()[SPEECH_ARTIFACT_METADATA_KEY], transcript.metadata()[SPEECH_ARTIFACT_METADATA_KEY]);
        }
        for artifact in &retained { store.verify(artifact).unwrap(); }
    }

    #[test]
    fn different_recognition_evidence_cannot_share_a_canonical_record_identity() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let input = source(&store);
        let first = canonical(&store, &input, "synthetic-1");
        let second = canonical(&store, &input, "synthetic-2");
        assert_eq!(first.digest(), second.digest());
        assert_ne!(first.id(), second.id());
        assert_ne!(retained_artifacts(&first).unwrap()[1].digest(), retained_artifacts(&second).unwrap()[1].digest());
    }

    #[test]
    fn malformed_retained_metadata_is_refused_instead_of_lost() {
        let directory = tempfile::tempdir().unwrap();
        let store = ArtifactStore::new(directory.path()).unwrap();
        let input = source(&store);
        let transcript = canonical(&store, &input, "synthetic-1");
        let original = serde_json::to_value(&transcript).unwrap();
        for mutation in 0..5 {
            let mut value = original.clone();
            let metadata = &mut value["metadata"][SPEECH_ARTIFACT_METADATA_KEY];
            match mutation {
                0 => metadata["provider_output"]["media_type"] = serde_json::json!("text/plain"),
                1 => metadata["provider_output"]["unknown"] = serde_json::json!(true),
                2 => metadata["canonical_transcript"]["sha256"] = serde_json::json!("0".repeat(64)),
                3 => metadata["prepared_audio"] = metadata["provider_output"].clone(),
                _ => metadata["provider_output"]["metadata"][ROLE_METADATA_KEY] = serde_json::json!("../arbitrary"),
            }
            let changed: Artifact = serde_json::from_value(value).unwrap();
            assert!(retained_artifacts(&changed).is_err());
        }
    }
}
