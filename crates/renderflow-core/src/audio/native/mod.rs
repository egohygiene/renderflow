//! Opt-in source-bound native audio validation over immutable CAS payloads.
//!
//! Existing strategies still encode; these pinned tools are verification tools.
//! PCM equivalence does not claim container metadata, mastering, playback, native
//! library attestation or an operating-system sandbox.
mod features;
mod policy;
pub(crate) use features::Admission as FeatureAdmission;
mod probe;
#[cfg(test)]
mod tests;

use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::artifact::{Artifact, ArtifactStore};
use crate::dna::sonic::wav::{self, WavInspection};
use crate::evidence::{
    FidelityDeclaration, StepEvidence, StepState, ValidationDiagnostic, ValidationState,
    ValidatorEvidence,
};
use crate::graph::{
    capability::{FormatCapabilityRegistry, FormatFamily},
    Format,
};
use crate::process::{
    ProcessCancellationToken, ProcessEnvironment, ProcessExecutor, ProcessExpectedOutput,
    ProcessNetworkPolicy, ProcessOutputMode, ProcessRequest, ProcessResult, ProcessTermination,
};
use crate::speech::provider::FileSnapshot;

pub use policy::schema as policy_schema;
pub use policy::{AudioValidationPolicy, Tool};

pub(crate) const VALIDATOR: &str = "validator.audio.pinned_pcm_delivery";
const MAX_MEDIA_BYTES: u64 = 32 * 1024 * 1024;
const MAX_TOOL_BYTES: u64 = 256 * 1024 * 1024;
const MAX_DECODED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_PROBE_BYTES: usize = 4 * 1024 * 1024;
const PROBE_ENTRIES: &str = "stream=index,codec_name,profile,codec_type,channels,sample_rate,bits_per_raw_sample,bits_per_sample,time_base,start_pts,duration_ts:stream_disposition=:stream_tags=:stream_side_data=:packet=codec_type,pts,duration:packet_side_data=side_data_type,skip_samples,discard_padding";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Failure {
    pub state: ValidationState,
    pub code: &'static str,
}
impl Failure {
    pub fn invalid(code: &'static str) -> Self {
        Self {
            state: ValidationState::Invalid,
            code,
        }
    }
    pub fn unavailable(code: &'static str) -> Self {
        Self {
            state: ValidationState::Unavailable,
            code,
        }
    }
    pub fn validator(self) -> ValidatorEvidence {
        validator(
            self.state,
            vec![ValidationDiagnostic {
                code: self.code.to_owned(),
                message: self.code.to_owned(),
            }],
        )
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.code)
    }
}
impl std::error::Error for Failure {}
fn invalid(code: &'static str) -> Failure {
    Failure::invalid(code)
}
fn unavailable(code: &'static str) -> Failure {
    Failure::unavailable(code)
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn validator(state: ValidationState, diagnostics: Vec<ValidationDiagnostic>) -> ValidatorEvidence {
    ValidatorEvidence {
        validator_id: VALIDATOR.to_owned(),
        validator_version: env!("CARGO_PKG_VERSION").to_owned(),
        provider: "renderflow.pinned-audio-verification".to_owned(),
        state,
        diagnostics,
    }
}
pub(crate) fn requested_format(format: Format) -> bool {
    FormatCapabilityRegistry::global()
        .get(format)
        .is_some_and(|d| d.families.contains(&FormatFamily::Audio))
}
fn demuxer(format: Format) -> Result<&'static str, Failure> {
    match format {
        Format::Flac => Ok("flac"),
        Format::M4aAlac | Format::M4aAac => Ok("mov"),
        Format::Mp3 => Ok("mp3"),
        _ => Err(unavailable("audio.native.format_unsupported")),
    }
}

#[derive(Debug, Clone, Serialize)]
struct ToolObservation {
    tool_id: String,
    version: String,
    sha256: String,
    size_bytes: u64,
}
#[derive(Debug, Clone, Serialize)]
struct SourcePcm {
    sample_rate_hz: u32,
    channels: u16,
    precision_bits: u16,
    frames: u64,
    sample_sha256: String,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Evidence {
    schema_version: &'static str,
    validation_profile: &'static str,
    target_identity: String,
    requested_audio_profile: Option<String>,
    producing_step_id: String,
    producing_step_configuration_sha256: String,
    source_artifact_id: String,
    source_sha256: String,
    artifact_id: String,
    artifact_sha256: String,
    verifier_ffmpeg: ToolObservation,
    verifier_ffprobe: ToolObservation,
    feature_admission: FeatureAdmission,
    probe_output_sha256: String,
    decoded_pcm_sha256: String,
    decoded_precision_bits: u16,
    decoded_frames: u64,
    source_pcm: SourcePcm,
    observed_clock: probe::Clock,
    profile_changes_pcm_layout: bool,
    decoded_samples_equal_original: bool,
    pub fidelity: FidelityDeclaration,
    scope: &'static str,
    encoded_bitrate_or_compression_attested: bool,
    encoder_executable_hash_attested: bool,
    native_library_or_os_sandbox_attested: bool,
    mastering_playback_or_content_approval: bool,
}
impl Evidence {
    pub fn validator(&self) -> ValidatorEvidence {
        validator(ValidationState::Valid, Vec::new())
    }
}

pub(crate) struct Context {
    source: FileSnapshot,
    source_id: String,
    source_layout: WavInspection,
    source_pcm: Vec<u8>,
    ffmpeg: FileSnapshot,
    ffprobe: FileSnapshot,
    ffmpeg_identity: ToolObservation,
    ffprobe_identity: ToolObservation,
    ffmpeg_features: features::Features,
    ffprobe_features: features::Features,
    temporary: tempfile::TempDir,
    token: ProcessCancellationToken,
}
impl Context {
    pub fn new(
        source: &Artifact,
        source_format: Format,
        store: &ArtifactStore,
        policy: &AudioValidationPolicy,
        token: ProcessCancellationToken,
    ) -> Result<Self, Failure> {
        if token.is_cancelled() {
            return Err(unavailable("audio.native.cancelled"));
        }
        policy
            .validate()
            .map_err(|_| invalid("audio.native.policy"))?;
        if source_format != Format::Wav
            || !source.sources().is_empty()
            || source.storage_class() != crate::artifact::ArtifactStorageClass::Source
        {
            return Err(unavailable("audio.native.source_format_unsupported"));
        }
        if source.size_bytes() > MAX_MEDIA_BYTES {
            return Err(unavailable("audio.native.source_byte_limit"));
        }
        store
            .verify(source)
            .map_err(|_| invalid("audio.native.source_cas"))?;
        let path = store
            .payload_path(source)
            .map_err(|_| invalid("audio.native.source_cas"))?;
        let captured = FileSnapshot::read(&path, "native audio source", MAX_MEDIA_BYTES, &token)
            .map_err(|_| invalid("audio.native.source_snapshot"))?;
        if captured.digest() != source.digest().value() {
            return Err(invalid("audio.native.source_digest"));
        }
        let bytes = captured
            .bytes(&token)
            .map_err(|_| invalid("audio.native.source_snapshot"))?;
        let layout =
            wav::inspect(&bytes, &token).map_err(|_| invalid("audio.native.source_wave"))?;
        if !matches!(layout.bits_per_sample, 16 | 24)
            || !matches!(layout.encoding, "pcm_s16le" | "pcm_s24le")
        {
            return Err(unavailable("audio.native.source_precision_unsupported"));
        }
        let pcm = bytes[layout.data_start..layout.data_end].to_vec();
        drop(bytes);
        let ffmpeg = tool(&policy.ffmpeg, "ffmpeg", &token)?;
        let ffprobe = tool(&policy.ffprobe, "ffprobe", &token)?;
        let temporary = tempfile::Builder::new()
            .prefix("renderflow-native-audio-")
            .tempdir()
            .map_err(|_| unavailable("audio.native.temporary_unavailable"))?;
        let directory = temporary.path();
        let ffmpeg_identity = version(&ffmpeg, "ffmpeg", directory, &token)?;
        let ffprobe_identity = version(&ffprobe, "ffprobe", directory, &token)?;
        let ffmpeg_features = features::Features::observe(&ffmpeg, "ffmpeg", directory, &token)?;
        let ffprobe_features = features::Features::observe(&ffprobe, "ffprobe", directory, &token)?;
        Ok(Self {
            source: captured,
            source_id: source.id().to_string(),
            source_layout: layout,
            source_pcm: pcm,
            ffmpeg,
            ffprobe,
            ffmpeg_identity,
            ffprobe_identity,
            ffmpeg_features,
            ffprobe_features,
            temporary,
            token,
        })
    }
    pub fn feature_admission(
        &self,
        format: Format,
        requested_profile: Option<&str>,
    ) -> Option<FeatureAdmission> {
        let audio = format
            .to_string()
            .parse::<crate::audio::AudioFormat>()
            .ok()?;
        let options = crate::audio::ffmpeg::configured_profile(requested_profile)
            .ok()?
            .resolve(audio);
        let bits = if matches!(format, Format::Flac | Format::M4aAlac) {
            options
                .bit_depth
                .map(u16::from)
                .unwrap_or(self.source_layout.bits_per_sample)
        } else {
            self.source_layout.bits_per_sample
        };
        features::admit(&self.ffmpeg_features, &self.ffprobe_features, format, bits)
    }
    pub fn validate(
        &self,
        artifact: &Artifact,
        format: Format,
        requested_profile: Option<&str>,
        target_identity: &str,
        step: &StepEvidence,
        store: &ArtifactStore,
    ) -> Result<Evidence, Failure> {
        self.recheck()?;
        let demuxer = demuxer(format)?;
        bind_direct(&self.source_id, artifact, format, step)?;
        if artifact.size_bytes() > MAX_MEDIA_BYTES {
            return Err(unavailable("audio.native.output_byte_limit"));
        }
        let audio_format = format
            .to_string()
            .parse::<crate::audio::AudioFormat>()
            .map_err(|_| unavailable("audio.native.format_unsupported"))?;
        let profile = crate::audio::ffmpeg::configured_profile(requested_profile)
            .map_err(|_| invalid("audio.native.profile"))?;
        let options = profile.resolve(audio_format);
        let codec = match format {
            Format::Flac => "flac",
            Format::M4aAlac => "alac",
            Format::M4aAac => "aac",
            Format::Mp3 => "mp3",
            _ => unreachable!(),
        };
        let selected_codec = options
            .codec
            .as_deref()
            .or_else(|| audio_format.ffmpeg_codec());
        if selected_codec.map(|c| if c == "libmp3lame" { "mp3" } else { c }) != Some(codec) {
            return Err(invalid("audio.native.profile.codec"));
        }
        let feature_admission = self
            .feature_admission(format, requested_profile)
            .ok_or(unavailable("audio.native.format_or_precision_unsupported"))?;
        feature_admission.ensure()?;
        store
            .verify(artifact)
            .map_err(|_| invalid("audio.native.output_cas"))?;
        let path = store
            .payload_path(artifact)
            .map_err(|_| invalid("audio.native.output_cas"))?;
        let output = FileSnapshot::read(&path, "native audio output", MAX_MEDIA_BYTES, &self.token)
            .map_err(|_| invalid("audio.native.output_snapshot"))?;
        if output.digest() != artifact.digest().value() {
            return Err(invalid("audio.native.output_digest"));
        }
        self.recheck()?;
        let mut args = input_arguments(demuxer, false);
        args.extend(
            [
                "-show_packets",
                "-show_streams",
                "-show_entries",
                PROBE_ENTRIES,
                "-print_format",
                "json",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        args.push(path_string(output.path())?);
        let observed = execute(
            request(self.ffprobe.path(), self.temporary.path(), &self.token)?
                .stdout(ProcessOutputMode::capture(MAX_PROBE_BYTES))
                .args(args),
            false,
        )?;
        let parsed = probe::Probe::parse(observed.stdout().bytes())?;
        let clock = parsed.clock(format)?;
        let source = self.source_layout;
        admit_layout(source, &options, &clock)?;
        let bits = clock
            .output_precision_bits
            .unwrap_or(source.bits_per_sample);
        let frame_bytes = u64::from(bits / 8) * u64::from(clock.channels);
        let bound = clock
            .presentation_samples
            .checked_add(if matches!(format, Format::Mp3 | Format::M4aAac) {
                2304
            } else {
                0
            })
            .and_then(|n| n.checked_mul(frame_bytes))
            .ok_or(unavailable("audio.native.decoded_byte_limit"))?;
        if bound > MAX_DECODED_BYTES {
            return Err(unavailable("audio.native.decoded_byte_limit"));
        }
        let raw = self
            .temporary
            .path()
            .join(format!("decoded-{}.pcm", artifact.digest().value()));
        // An earlier identical target may share bytes. Each validation still decodes
        // afresh into a new task-owned directory; never reuse a proof file.
        let decoded_dir = tempfile::Builder::new()
            .prefix("decode-")
            .tempdir_in(self.temporary.path())
            .map_err(|_| unavailable("audio.native.temporary_unavailable"))?;
        let raw = decoded_dir
            .path()
            .join(raw.file_name().ok_or(invalid("audio.native.output_path"))?);
        let mut args = input_arguments(demuxer, true);
        args.push("-i".to_owned());
        args.push(path_string(output.path())?);
        args.extend(
            [
                "-map",
                "0:a:0",
                "-vn",
                "-sn",
                "-dn",
                "-map_metadata",
                "-1",
                "-map_chapters",
                "-1",
                "-threads",
                "1",
                "-c:a",
            ]
            .into_iter()
            .map(str::to_owned),
        );
        args.push(format!("pcm_s{bits}le"));
        args.push("-f".to_owned());
        args.push(format!("s{bits}le"));
        args.push("-fs".to_owned());
        args.push((MAX_DECODED_BYTES + 1).to_string());
        args.push(path_string(&raw)?);
        execute(
            request(self.ffmpeg.path(), self.temporary.path(), &self.token)?
                .args(args)
                .expect_output(
                    ProcessExpectedOutput::file(&raw)
                        .require_non_empty()
                        .max_bytes(MAX_DECODED_BYTES),
                ),
            false,
        )?;
        let decoded =
            FileSnapshot::read(&raw, "native decoded PCM", MAX_DECODED_BYTES, &self.token)
                .map_err(|_| invalid("audio.native.decoded_snapshot"))?;
        let decoded_bytes = decoded
            .bytes(&self.token)
            .map_err(|_| invalid("audio.native.decoded_snapshot"))?;
        let Comparison {
            frames,
            changes,
            equal,
            fidelity,
        } = compare(source, &self.source_pcm, &clock, &decoded_bytes, format)?;
        output
            .recheck(&self.token)
            .map_err(|_| invalid("audio.native.output_changed"))?;
        self.recheck()?;
        Ok(Evidence {schema_version:"renderflow.native-audio-validation/v1",validation_profile:policy::PROFILE,
            target_identity:target_identity.to_owned(),requested_audio_profile:requested_profile.map(str::to_owned),
            producing_step_id:step.step_id.clone(),producing_step_configuration_sha256:step.configuration_digest.value.clone(),
            source_artifact_id:self.source_id.clone(),source_sha256:self.source.digest().to_owned(),artifact_id:artifact.id().to_string(),artifact_sha256:output.digest().to_owned(),
            verifier_ffmpeg:self.ffmpeg_identity.clone(),verifier_ffprobe:self.ffprobe_identity.clone(),feature_admission,probe_output_sha256:hash(observed.stdout().bytes()),
            decoded_pcm_sha256:decoded.digest().to_owned(),decoded_precision_bits:bits,decoded_frames:frames,
            source_pcm:SourcePcm {sample_rate_hz:source.sample_rate_hz,channels:source.channels,precision_bits:source.bits_per_sample,frames:source.frame_count,sample_sha256:hash(&self.source_pcm)},
            observed_clock:clock,profile_changes_pcm_layout:changes,decoded_samples_equal_original:equal,
            fidelity,
            scope:"Decoded PCM essence and clocks only; container/ancillary metadata equality, perceptual quality and release qualification are not asserted.",
            encoded_bitrate_or_compression_attested:false,
            encoder_executable_hash_attested:false,native_library_or_os_sandbox_attested:false,mastering_playback_or_content_approval:false})
    }
    fn recheck(&self) -> Result<(), Failure> {
        if self.token.is_cancelled() {
            return Err(unavailable("audio.native.cancelled"));
        }
        self.source
            .recheck(&self.token)
            .map_err(|_| invalid("audio.native.source_changed"))?;
        for tool in [&self.ffmpeg, &self.ffprobe] {
            tool.recheck(&self.token)
                .map_err(|_| invalid("audio.native.verifier_changed"))?;
        }
        Ok(())
    }
}
#[derive(Debug)]
struct Comparison {
    frames: u64,
    changes: bool,
    equal: bool,
    fidelity: FidelityDeclaration,
}
fn compare(
    source: WavInspection,
    source_pcm: &[u8],
    clock: &probe::Clock,
    decoded_bytes: &[u8],
    format: Format,
) -> Result<Comparison, Failure> {
    let bits = clock
        .output_precision_bits
        .unwrap_or(source.bits_per_sample);
    if !matches!(bits, 16 | 24) || !(1..=8).contains(&clock.channels) {
        return Err(invalid("audio.native.decoded_layout"));
    }
    let frame_bytes = u64::from(bits / 8) * u64::from(clock.channels);
    if decoded_bytes.len() as u64 % frame_bytes != 0 {
        return Err(invalid("audio.native.decoded_frame_alignment"));
    }
    let frames = decoded_bytes.len() as u64 / frame_bytes;
    let correct_count = if format == Format::M4aAac {
        frames == clock.presentation_samples.div_ceil(1024) * 1024
    } else {
        frames == clock.presentation_samples
    };
    if !correct_count {
        return Err(invalid("audio.native.decoded_frame_count"));
    }
    let changes = clock.rate_hz != source.sample_rate_hz
        || clock.channels != source.channels
        || bits != source.bits_per_sample;
    let lossless_codec = matches!(format, Format::Flac | Format::M4aAlac);
    let equal = !changes && frames == source.frame_count && decoded_bytes == source_pcm;
    if lossless_codec && !changes && !equal {
        return Err(invalid("audio.native.pcm_mismatch"));
    }
    Ok(Comparison {
        frames,
        changes,
        equal,
        fidelity: if lossless_codec && equal {
            FidelityDeclaration::Lossless
        } else {
            FidelityDeclaration::Lossy
        },
    })
}

fn admit_layout(
    source: WavInspection,
    options: &crate::audio::profile::ProfileOptions,
    clock: &probe::Clock,
) -> Result<(), Failure> {
    if clock.rate_hz != options.sample_rate.unwrap_or(source.sample_rate_hz)
        || clock.channels != options.channels.map(u16::from).unwrap_or(source.channels)
        || clock.output_precision_bits.is_some_and(|actual| {
            actual
                != options
                    .bit_depth
                    .map(u16::from)
                    .unwrap_or(source.bits_per_sample)
        })
    {
        return Err(invalid("audio.native.profile.layout"));
    }
    let scaled = u128::from(source.frame_count) * u128::from(clock.rate_hz);
    let denominator = u128::from(source.sample_rate_hz);
    let floor = scaled / denominator;
    let ceil = scaled.div_ceil(denominator);
    if ![floor, ceil].contains(&u128::from(clock.presentation_samples)) {
        return Err(invalid("audio.native.presentation_mismatch"));
    }
    Ok(())
}

fn bind_direct(
    source_id: &str,
    artifact: &Artifact,
    format: Format,
    step: &StepEvidence,
) -> Result<(), Failure> {
    if artifact.format().as_str() != format.to_string()
        || artifact.sources().len() != 1
        || artifact.sources()[0].as_str() != source_id
        || artifact
            .metadata()
            .get("renderflow.adapter")
            .and_then(serde_json::Value::as_str)
            != Some("builtin.strategy")
        || artifact
            .metadata()
            .get("renderflow.from")
            .and_then(serde_json::Value::as_str)
            != Some("wav")
        || artifact
            .metadata()
            .get("renderflow.to")
            .and_then(serde_json::Value::as_str)
            != Some(format.to_string().as_str())
        || !matches!(step.state, StepState::Complete | StepState::Reused)
        || step.transform != "renderflow.strategy-adapter"
        || step.provider.as_deref() != Some("tool.ffmpeg")
        || step.capability.as_deref() != Some(format!("transform.wav.{format}").as_str())
        || step.input_artifacts != [source_id.to_owned()]
        || step.output_artifacts != [artifact.id().to_string()]
    {
        return Err(unavailable("audio.native.direct_route_unsupported"));
    }
    Ok(())
}

fn path_string(path: &Path) -> Result<String, Failure> {
    path.to_str()
        .map(str::to_owned)
        .ok_or(invalid("audio.native.path"))
}
fn request(
    executable: &Path,
    directory: &Path,
    token: &ProcessCancellationToken,
) -> Result<ProcessRequest, Failure> {
    Ok(ProcessRequest::direct(path_string(executable)?)
        .working_directory(directory)
        .environment(ProcessEnvironment::clear().set("LC_ALL", "C"))
        .stdout(ProcessOutputMode::capture(64 * 1024))
        .stderr(ProcessOutputMode::capture(64 * 1024))
        .timeout(Duration::from_secs(30))
        .cancellation(token.clone())
        .network_policy(ProcessNetworkPolicy::Deny))
}
fn execute(request: ProcessRequest, prerequisite: bool) -> Result<ProcessResult, Failure> {
    let result = ProcessExecutor::new()
        .execute(request)
        .map_err(|_| unavailable("audio.native.process_unavailable"))?;
    match result.termination() {
        ProcessTermination::Cancelled => return Err(unavailable("audio.native.cancelled")),
        ProcessTermination::TimedOut | ProcessTermination::OutputLimitExceeded => {
            return Err(unavailable("audio.native.process_limit"))
        }
        _ => {}
    }
    if !result.is_success() || result.stdout().truncated() || result.stderr().truncated() {
        return Err(if prerequisite {
            unavailable("audio.native.verifier_unavailable")
        } else {
            invalid("audio.native.media_process_failed")
        });
    }
    Ok(result)
}
fn tool(
    intent: &Tool,
    name: &'static str,
    token: &ProcessCancellationToken,
) -> Result<FileSnapshot, Failure> {
    match std::fs::symlink_metadata(&intent.path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(unavailable("audio.native.verifier_missing"))
        }
        Err(_) => return Err(unavailable("audio.native.verifier_unavailable")),
        Ok(metadata) if metadata.len() > MAX_TOOL_BYTES => {
            return Err(unavailable("audio.native.verifier_byte_limit"))
        }
        _ => {}
    }
    let captured = FileSnapshot::read(Path::new(&intent.path), name, MAX_TOOL_BYTES, token)
        .map_err(|_| invalid("audio.native.verifier_snapshot"))?;
    if captured.digest() != intent.sha256 {
        return Err(invalid("audio.native.verifier_digest"));
    }
    Ok(captured)
}
fn version(
    tool: &FileSnapshot,
    name: &str,
    directory: &Path,
    token: &ProcessCancellationToken,
) -> Result<ToolObservation, Failure> {
    let observed = execute(
        request(tool.path(), directory, token)?
            .arg("-version")
            .timeout(Duration::from_secs(5)),
        true,
    )?;
    let text = std::str::from_utf8(observed.stdout().bytes())
        .map_err(|_| unavailable("audio.native.verifier_version"))?;
    let first = text.lines().next().unwrap_or("");
    let words = first.split_whitespace().take(3).collect::<Vec<_>>();
    if first.len() > 4096
        || first.chars().any(char::is_control)
        || words != [name, "version", "8.0.1"]
    {
        return Err(unavailable("audio.native.verifier_version"));
    }
    tool.recheck(token)
        .map_err(|_| invalid("audio.native.verifier_changed"))?;
    Ok(ToolObservation {
        tool_id: format!("tool.{name}"),
        version: "8.0.1".to_owned(),
        sha256: tool.digest().to_owned(),
        size_bytes: tool.size_bytes(),
    })
}
fn input_arguments(demuxer: &str, decoder: bool) -> Vec<String> {
    let mut args = vec![
        "-hide_banner".to_owned(),
        "-loglevel".to_owned(),
        "error".to_owned(),
    ];
    if decoder {
        args.extend(
            ["-nostdin", "-n", "-xerror", "-filter_threads", "1"]
                .into_iter()
                .map(str::to_owned),
        );
    }
    args.extend(
        [
            "-protocol_whitelist",
            "file",
            "-format_whitelist",
            demuxer,
            "-f",
            demuxer,
            "-max_streams",
            "1",
            "-probesize",
            "5000000",
            "-analyzeduration",
            "5000000",
            "-threads",
            "1",
        ]
        .into_iter()
        .map(str::to_owned),
    );
    if demuxer == "mov" {
        args.extend(
            ["-enable_drefs", "0", "-use_absolute_path", "0"]
                .into_iter()
                .map(str::to_owned),
        );
    }
    args
}
