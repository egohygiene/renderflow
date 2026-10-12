//! Opt-in native subtitle qualification; all authored cases remain unrun until
//! this ignored test is explicitly selected with exact runtime version pins.
//! Fixture generation and every version probe use the bounded process service.
//! No downloads, global environment changes, private media, or checked-in binary
//! fixtures are involved. Results and generated media remain in a new directory.

#[path = "support/subtitle_qualification_report.rs"]
mod subtitle_qualification_report;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{bail, ensure, Context, Result};
use renderflow::evidence::{ArtifactRole, RunState, StepState};
use renderflow::process::{
    ProcessCancellationToken, ProcessEnvironment, ProcessExecutor, ProcessExpectedOutput,
    ProcessNetworkPolicy, ProcessRequest,
};
use renderflow::{EngineBuilder, ExecutionRequest};
use renderflow::timed_text::provider::{self, SubtitleProviderError};
use renderflow::timed_text::{self, TimedTextFormat, TimedTranscript};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use subtitle_qualification_report::{CaseStatus, QualificationReport, REQUIRED_CASES};

const SRT: &[u8] = include_bytes!("../../../tests/fixtures/timed-text/plain-unicode.srt");
const WEBVTT: &[u8] = include_bytes!("../../../tests/fixtures/timed-text/identified.vtt");
const ASS: &[u8] = include_bytes!("fixtures/timed-text/native/unsupported.ass");
const FIXTURE_MAX_BYTES: u64 = 1024 * 1024;
const CAPTURE_BYTES: usize = 64 * 1024;

#[test]
#[ignore = "requires explicit retained output directory and exact FFmpeg/FFprobe version-line pins"]
fn native_subtitle_qualification() -> Result<()> {
    let root = PathBuf::from(required_environment("RENDERFLOW_SUBTITLE_QUALIFICATION_DIR")?);
    ensure!(root.is_absolute(), "qualification output directory must be an absolute new path");
    let expected_ffmpeg = std::env::var("RENDERFLOW_SUBTITLE_FFMPEG_VERSION").unwrap_or_default();
    let expected_ffprobe = std::env::var("RENDERFLOW_SUBTITLE_FFPROBE_VERSION").unwrap_or_default();
    fs::create_dir(&root).context("qualification directory must not exist; its parent must exist")?;
    let root = fs::canonicalize(root)?;
    let report_path = root.join("qualification.json");
    let mut report = QualificationReport::new(&expected_ffmpeg, &expected_ffprobe);
    // Publish the full required not_run matrix before the first native process.
    report.persist(&report_path)?;

    for (tool, pin) in [("ffmpeg", &expected_ffmpeg), ("ffprobe", &expected_ffprobe)] {
        if !pin.starts_with(&format!("{tool} version ")) || pin.chars().any(char::is_control) {
            let reason = format!("{tool} requires its exact nonempty first version-output line pin");
            report.set_provider(tool, None, Some(&reason))?;
            for (case, _) in REQUIRED_CASES {
                report.record_case(case, CaseStatus::Unavailable, json!({"reason":reason}))?;
            }
            report.persist(&report_path)?;
            bail!("{reason}; see {}", report_path.display());
        }
    }
    let implementation_sha256 = match hash_harness_executable() {
        Ok(digest) => digest,
        Err(error) => {
            for (case, _) in REQUIRED_CASES {
                report.record_case(case, CaseStatus::Unavailable, json!({"reason":format!("harness identity unavailable: {error:#}")}))?;
            }
            report.persist(&report_path)?;
            return Err(error);
        }
    };
    let declared_revision = std::env::var("RENDERFLOW_SUBTITLE_QUALIFICATION_REVISION").ok();
    report.set_implementation(env!("CARGO_PKG_VERSION"), &implementation_sha256, declared_revision.as_deref())?;
    report.persist(&report_path)?;

    let executor = ProcessExecutor::new();
    let mut unavailable = Vec::new();
    for (tool, expected) in [("ffmpeg", &expected_ffmpeg), ("ffprobe", &expected_ffprobe)] {
        match version_line(&executor, tool) {
            Ok(observed) if observed == *expected => report.set_provider(tool, Some(&observed), None)?,
            Ok(observed) => {
                let reason = format!("exact version pin mismatch: expected {expected}; observed {observed}");
                report.set_provider(tool, Some(&observed), Some(&reason))?;
                unavailable.push(reason);
            }
            Err(error) => {
                let reason = format!("{error:#}");
                report.set_provider(tool, None, Some(&reason))?;
                unavailable.push(reason);
            }
        }
        report.persist(&report_path)?;
    }
    if !unavailable.is_empty() {
        for (case, _) in REQUIRED_CASES {
            report.record_case(case, CaseStatus::Unavailable, json!({"reason": unavailable}))?;
        }
        report.persist(&report_path)?;
        bail!("native subtitle qualification unavailable; see {}", report_path.display());
    }

    let mut generator_commands = Vec::new();
    let fixtures = match generate_fixtures(&root, &executor, &mut generator_commands)
        .and_then(|fixtures| fixtures.evidence().map(|evidence| (fixtures, evidence)))
    {
        Ok((fixtures, fixture_evidence)) => {
            report.record_case("fixture_generation", CaseStatus::Passed,
                json!({"commands": generator_commands, "fixtures": fixture_evidence,
                    "implementation": {"crate_version":env!("CARGO_PKG_VERSION"),"source_revision":declared_revision,"source":"operator_declared"}}))?;
            report.persist(&report_path)?;
            fixtures
        }
        Err(error) => {
            report.record_case("fixture_generation", CaseStatus::Failed,
                json!({"commands": generator_commands, "diagnostic": format!("{error:#}")}))?;
            report.persist(&report_path)?;
            return Err(error.context(format!("fixture generation failed; see {}", report_path.display())));
        }
    };

    run_case(&mut report, &report_path, "srt_stream", || {
        qualify_stream(&fixtures.supported, 1, "subrip", "eng", SRT, TimedTextFormat::Srt,
            &expected_ffmpeg, &expected_ffprobe, &root.join("extracted-srt.timed-transcript.json"))
    })?;
    run_case(&mut report, &report_path, "webvtt_stream", || {
        qualify_stream(&fixtures.supported, 2, "webvtt", "rus", WEBVTT, TimedTextFormat::WebVtt,
            &expected_ffmpeg, &expected_ffprobe, &root.join("extracted-vtt.timed-transcript.json"))
    })?;
    run_case(&mut report, &report_path, "sdk_lineage", || {
        qualify_sdk(&fixtures.supported, &root, &expected_ffmpeg, &expected_ffprobe)
    })?;
    run_case(&mut report, &report_path, "no_subtitles", || {
        let before = digest_file(&fixtures.no_subtitles)?;
        let inspection = provider::inspect(&fixtures.no_subtitles)?;
        ensure!(inspection.streams.is_empty(), "audio-only fixture unexpectedly advertised subtitles");
        ensure!(inspection.ffprobe_version == expected_ffprobe, "inspection provider changed after pinning");
        let error = provider_failure(provider::extract(&fixtures.no_subtitles, 0), "no subtitle may be invented")?;
        ensure!(matches!(error, SubtitleProviderError::NoSubtitleStreams), "unexpected refusal: {error}");
        ensure!(before == digest_file(&fixtures.no_subtitles)?, "no-subtitle source changed");
        Ok(json!({"inspection": inspection, "refusal": error.code(), "source_sha256": before}))
    })?;
    run_case(&mut report, &report_path, "unsupported_ass", || {
        qualify_unsupported(&fixtures.ass, "ass", &expected_ffprobe)
    })?;
    run_case(&mut report, &report_path, "unsupported_mov_text", || {
        qualify_unsupported(&fixtures.mov_text, "mov_text", &expected_ffprobe)
    })?;
    run_case(&mut report, &report_path, "pre_cancelled", || {
        let token = ProcessCancellationToken::new();
        token.cancel();
        let error = provider_failure(provider::extract_with_cancellation(&root.join("deliberately-absent.mkv"), 1, token),
            "pre-cancelled extraction must stop before source/provider access")?;
        ensure!(matches!(error, SubtitleProviderError::Cancelled), "unexpected cancellation: {error}");
        Ok(json!({"refusal": error.code(), "source": "deliberately absent", "scope": "pre-cancelled only; not mid-process cancellation"}))
    })?;
    run_case(&mut report, &report_path, "malformed_container", || {
        let path = root.join("malformed.mkv");
        fs::write(&path, b"Original synthetic bytes that are not a Matroska container.\n")?;
        let before = digest_file(&path)?;
        let error = provider_failure(provider::inspect(&path), "malformed media cannot produce successful inspection")?;
        ensure!(matches!(error, SubtitleProviderError::ProviderFailed { .. }), "unexpected malformed refusal: {error}");
        ensure!(before == digest_file(&path)?, "malformed source changed");
        Ok(json!({"refusal": error.code(), "source_sha256": before, "diagnostic": error.to_string()}))
    })?;

    // Do not qualify a run if the versions now on PATH differ from the pins
    // recorded before generation; extracted provenance is checked independently.
    let mut post_versions = serde_json::Map::new();
    let mut version_status = CaseStatus::Passed;
    for (tool, expected) in [("ffmpeg", &expected_ffmpeg), ("ffprobe", &expected_ffprobe)] {
        match version_line(&executor, tool) {
            Ok(observed) if observed == *expected => {
                report.set_provider(tool, Some(&observed), None)?;
                post_versions.insert(tool.to_string(), json!({"before":expected,"after":observed}));
            }
            Ok(observed) => {
                report.set_provider(tool, Some(&observed), Some("version pin changed at final probe"))?;
                post_versions.insert(tool.to_string(), json!({"before":expected,"after":observed}));
                version_status = CaseStatus::Failed;
            }
            Err(error) => {
                report.set_provider(tool, None, Some(&format!("final probe failed: {error:#}")))?;
                post_versions.insert(tool.to_string(), json!({"before":expected,"after":null,"diagnostic":format!("{error:#}")}));
                if version_status != CaseStatus::Failed { version_status = CaseStatus::Unavailable; }
            }
        }
        report.persist(&report_path)?;
    }
    report.record_case("provider_versions", version_status, Value::Object(post_versions))?;
    report.persist(&report_path)?;
    ensure!(report.is_passed(), "native subtitle qualification did not pass; see {}", report_path.display());
    Ok(())
}

fn required_environment(name: &str) -> Result<String> {
    let value = std::env::var(name).with_context(|| format!("set {name} explicitly before selecting the ignored native test"))?;
    ensure!(!value.trim().is_empty(), "{name} must not be empty");
    Ok(value)
}

fn hash_harness_executable() -> Result<String> {
    const LIMIT: u64 = 1024 * 1024 * 1024;
    let mut file = fs::File::open(std::env::current_exe()?)?;
    let metadata = file.metadata()?;
    ensure!(metadata.is_file() && metadata.len() <= LIMIT, "harness executable is not a bounded regular file");
    let started = Instant::now();
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut total = 0_u64;
    loop {
        ensure!(started.elapsed() < Duration::from_secs(120), "harness hash exceeded 120 seconds between reads");
        let count = file.read(&mut buffer)?;
        if count == 0 { break; }
        total += count as u64;
        ensure!(total <= LIMIT, "harness grew beyond its byte limit");
        digest.update(&buffer[..count]);
    }
    ensure!(total == metadata.len(), "harness executable size changed during hashing");
    Ok(format!("{:x}", digest.finalize()))
}

fn provider_failure<T>(result: std::result::Result<T, SubtitleProviderError>, expectation: &str) -> Result<SubtitleProviderError> {
    match result { Err(error) => Ok(error), Ok(_) => bail!("{expectation}") }
}

fn request(tool: &str) -> ProcessRequest {
    ProcessRequest::direct(tool)
        .capture_limit(CAPTURE_BYTES)
        .timeout(Duration::from_secs(30))
        .network_policy(ProcessNetworkPolicy::Deny)
        .environment(ProcessEnvironment::filtered_inherit()
            .deny("FFREPORT").deny("AV_LOG_FORCE_COLOR")
            .set("AV_LOG_FORCE_NOCOLOR", "1").set("LC_ALL", "C"))
}

fn version_line(executor: &ProcessExecutor, tool: &str) -> Result<String> {
    let result = executor.execute(request(tool).arg("-version").timeout(Duration::from_secs(5)))?;
    ensure!(!result.stdout().truncated() && !result.stderr().truncated(), "{tool} version capture truncated");
    result.ensure_success()?;
    let line = result.stdout().redacted_text().lines().chain(result.stderr().redacted_text().lines())
        .find(|line| !line.trim().is_empty()).context("empty provider version response")?.trim();
    ensure!(line.len() <= 4096 && !line.chars().any(char::is_control), "invalid provider version line");
    Ok(line.to_string())
}

fn run_case<F>(report: &mut QualificationReport, path: &Path, case: &str, execute: F) -> Result<()>
where F: FnOnce() -> Result<Value> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(execute)) {
        Ok(Ok(detail)) => report.record_case(case, CaseStatus::Passed, detail)?,
        Ok(Err(error)) => report.record_case(case, CaseStatus::Failed, json!({"diagnostic": format!("{error:#}")}))?,
        Err(panic) => {
            let message = panic.downcast_ref::<String>().map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied()).unwrap_or("non-string panic");
            report.record_case(case, CaseStatus::Failed, json!({"panic":message}))?;
        }
    }
    report.persist(path)
}

struct Fixtures { supported: PathBuf, no_subtitles: PathBuf, ass: PathBuf, mov_text: PathBuf }

impl Fixtures {
    fn evidence(&self) -> Result<Value> {
        Ok(json!({
            "supported": {"path": self.supported, "sha256": digest_file(&self.supported)?},
            "no_subtitles": {"path": self.no_subtitles, "sha256": digest_file(&self.no_subtitles)?},
            "ass": {"path": self.ass, "sha256": digest_file(&self.ass)?},
            "mov_text": {"path": self.mov_text, "sha256": digest_file(&self.mov_text)?},
            "reproducibility": "fixed synthetic recipes and bitexact mux flags; observed container digests recorded, cross-build byte identity not asserted"
        }))
    }
}

fn generate_fixtures(root: &Path, executor: &ProcessExecutor, commands: &mut Vec<Value>) -> Result<Fixtures> {
    let srt = root.join("synthetic.srt");
    let vtt = root.join("synthetic.vtt");
    let ass = root.join("synthetic.ass");
    let wav = root.join("silence.wav");
    fs::write(&srt, SRT)?;
    fs::write(&vtt, WEBVTT)?;
    fs::write(&ass, ASS)?;
    fs::write(&wav, silent_wav())?;
    let fixtures = Fixtures {
        supported: root.join("supported.mkv"), no_subtitles: root.join("no-subtitles.mkv"),
        ass: root.join("unsupported-ass.mkv"), mov_text: root.join("unsupported-mov-text.mp4"),
    };
    let mut args = generator_prefix();
    add_input(&mut args, "wav", &wav)?;
    add_input(&mut args, "srt", &srt)?;
    add_input(&mut args, "webvtt", &vtt)?;
    args.extend(["-map", "0:a:0", "-map", "1:s:0", "-map", "2:s:0", "-codec", "copy",
        "-metadata:s:s:0", "language=eng", "-metadata:s:s:1", "language=rus"].map(str::to_string));
    mux(executor, args, "matroska", &fixtures.supported, commands)?;

    let mut args = generator_prefix();
    add_input(&mut args, "wav", &wav)?;
    args.extend(["-map", "0:a:0", "-codec", "copy"].map(str::to_string));
    mux(executor, args, "matroska", &fixtures.no_subtitles, commands)?;

    let mut args = generator_prefix();
    add_input(&mut args, "ass", &ass)?;
    args.extend(["-map", "0:s:0", "-codec", "copy"].map(str::to_string));
    mux(executor, args, "matroska", &fixtures.ass, commands)?;

    let mut args = generator_prefix();
    add_input(&mut args, "srt", &srt)?;
    args.extend(["-map", "0:s:0", "-codec:s", "mov_text"].map(str::to_string));
    mux(executor, args, "mp4", &fixtures.mov_text, commands)?;
    Ok(fixtures)
}

fn generator_prefix() -> Vec<String> {
    ["-hide_banner", "-loglevel", "warning", "-nostdin", "-nostats", "-xerror", "-n"]
        .into_iter().map(str::to_string).collect()
}

fn add_input(args: &mut Vec<String>, format: &str, path: &Path) -> Result<()> {
    if format == "wav" {
        // The PCM fixture declares its channel count; no layout inference is
        // needed for stream-copy. FFmpeg otherwise warns while guessing mono.
        args.extend(["-guess_layout_max", "0"].map(str::to_string));
    }
    args.extend(["-protocol_whitelist", "file", "-format_whitelist", format, "-f", format, "-i"]
        .map(str::to_string));
    args.push(path.to_str().context("native fixture path must be UTF-8")?.to_string());
    Ok(())
}

fn mux(executor: &ProcessExecutor, mut args: Vec<String>, format: &str, output: &Path, commands: &mut Vec<Value>) -> Result<()> {
    args.extend(["-map_metadata", "-1", "-map_chapters", "-1", "-fflags", "+bitexact", "-f", format]
        .map(str::to_string));
    args.push(output.to_str().context("fixture output path must be UTF-8")?.to_string());
    commands.push(json!({"executable": "ffmpeg", "argv": args, "status": "started"}));
    let entry = commands.last_mut().context("generator command entry missing")?;
    let result = match executor.execute(request("ffmpeg").args(args)
        .expect_output(ProcessExpectedOutput::file(output).require_non_empty().max_bytes(FIXTURE_MAX_BYTES)))
    {
        Ok(result) => result,
        Err(error) => { entry["status"] = "failed".into(); entry["diagnostic"] = error.to_string().into(); return Err(error.into()); }
    };
    entry["termination"] = format!("{:?}", result.termination()).into();
    entry["stdout"] = result.stdout().redacted_text().into();
    entry["stderr"] = result.stderr().redacted_text().into();
    entry["stdout_truncated"] = result.stdout().truncated().into();
    entry["stderr_truncated"] = result.stderr().truncated().into();
    entry["status"] = "failed".into();
    ensure!(!result.stdout().truncated() && !result.stderr().truncated(), "fixture generation capture truncated");
    result.ensure_success()?;
    ensure!(result.stderr().redacted_text().trim().is_empty(), "fixture generation warned; refusing potentially partial fixture");
    ensure!(fs::metadata(output)?.len() <= FIXTURE_MAX_BYTES, "generated fixture exceeded size limit");
    entry["output_sha256"] = digest_file(output)?.into();
    entry["status"] = "passed".into();
    Ok(())
}

fn silent_wav() -> Vec<u8> {
    let samples = 8_000_u32;
    let data_bytes = samples * 2;
    let mut bytes = Vec::with_capacity(44 + data_bytes as usize);
    bytes.extend_from_slice(b"RIFF"); bytes.extend_from_slice(&(36 + data_bytes).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt "); bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes()); bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&8_000_u32.to_le_bytes()); bytes.extend_from_slice(&16_000_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes()); bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data"); bytes.extend_from_slice(&data_bytes.to_le_bytes());
    bytes.resize(44 + data_bytes as usize, 0);
    bytes
}

fn digest_file(path: &Path) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(read_retained(path)?)))
}

fn read_retained(path: &Path) -> Result<Vec<u8>> {
    const LIMIT: u64 = 16 * 1024 * 1024;
    let metadata = fs::symlink_metadata(path)?;
    ensure!(metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= LIMIT,
        "qualification payload is not a bounded regular file");
    let file = fs::File::open(path)?;
    let opened = file.metadata()?;
    ensure!(opened.is_file() && opened.len() == metadata.len(), "qualification payload changed before read");
    let mut bytes = Vec::with_capacity(opened.len() as usize);
    file.take(LIMIT + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= LIMIT && bytes.len() as u64 == opened.len(),
        "qualification payload grew/truncated during bounded read");
    Ok(bytes)
}

#[allow(clippy::too_many_arguments)]
fn qualify_stream(path: &Path, index: u32, codec: &str, language: &str, source: &[u8], format: TimedTextFormat,
    ffmpeg: &str, ffprobe: &str, transcript_path: &Path) -> Result<Value> {
    let before = digest_file(path)?;
    let inspection = provider::inspect(path)?;
    ensure!(inspection.ffprobe_version == ffprobe, "inspection version differs from pin");
    ensure!(inspection.source_sha256 == before, "inspection source identity differs");
    let stream = inspection.streams.iter().find(|stream| stream.index == index).context("selected global stream absent")?;
    ensure!(stream.supported && stream.codec == codec && stream.language.as_deref() == Some(language), "stream identity/language mismatch");
    let transcript = provider::extract(path, index)?;
    check_extraction_provenance(&transcript, &before, index, codec, language, ffmpeg, ffprobe)?;
    let original = timed_text::parse(source, format)?;
    ensure!(transcript.cues.len() == original.cues.len(), "extraction changed cue count");
    for (actual, expected) in transcript.cues.iter().zip(&original.cues) {
        ensure!((actual.start_ms, actual.end_ms, &actual.text) == (expected.start_ms, expected.end_ms, &expected.text), "extraction changed timing or text");
        if format == TimedTextFormat::WebVtt { ensure!(actual.id == expected.id, "WebVTT cue identifier changed"); }
    }
    ensure!(before == digest_file(path)?, "source changed during native extraction");
    let absent = provider_failure(provider::extract(path, 0), "audio stream index 0 must never select a subtitle implicitly")?;
    ensure!(matches!(absent, SubtitleProviderError::StreamNotFound(0)), "unexpected explicit-index refusal: {absent}");
    let bytes = timed_text::render(&transcript, TimedTextFormat::Json)?;
    fs::write(transcript_path, &bytes)?;
    Ok(json!({"inspection": inspection, "transcript": transcript_path, "transcript_sha256": format!("{:x}", Sha256::digest(&bytes)),
        "source_sha256": before, "wrong_stream_refusal": absent.code(), "provenance": transcript.source}))
}

fn check_extraction_provenance(transcript: &TimedTranscript, media_sha: &str, index: u32, codec: &str,
    language: &str, ffmpeg: &str, ffprobe: &str) -> Result<()> {
    ensure!(transcript.source.sha256 == media_sha, "original media hash not preserved");
    let stream = transcript.source.stream.as_ref().context("missing source stream provenance")?;
    ensure!(stream.index == index && stream.codec == codec && stream.language.as_deref() == Some(language), "extracted stream provenance mismatch");
    ensure!(transcript.language.as_deref() == Some(language), "transcript language missing");
    let extraction = transcript.source.extraction.as_ref().context("missing extraction provenance")?;
    ensure!(extraction.provider == "adapter.subtitles.ffmpeg", "wrong extraction provider");
    ensure!(extraction.ffmpeg_version == ffmpeg && extraction.ffprobe_version == ffprobe, "extraction versions differ from pinned probes");
    ensure!(extraction.subtitle_sha256.len() == 64 && extraction.subtitle_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()), "missing intermediate subtitle digest");
    Ok(())
}

fn qualify_unsupported(path: &Path, codec: &str, ffprobe: &str) -> Result<Value> {
    let before = digest_file(path)?;
    let inspection = provider::inspect(path)?;
    ensure!(inspection.ffprobe_version == ffprobe, "inspection version changed after pinning");
    ensure!(inspection.streams.len() == 1, "unsupported fixture must contain exactly one subtitle stream");
    let stream = &inspection.streams[0];
    ensure!(stream.codec == codec && !stream.supported, "unsupported codec advertised as executable");
    let error = provider_failure(provider::extract(path, stream.index), "unsupported codec must refuse extraction")?;
    match &error {
        SubtitleProviderError::UnsupportedCodec { index, codec: actual, .. } => {
            ensure!(*index == stream.index && actual == codec, "typed refusal lost codec/stream identity");
        }
        _ => bail!("unexpected unsupported-codec error: {error}"),
    }
    ensure!(before == digest_file(path)?, "unsupported source changed");
    Ok(json!({"inspection": inspection, "refusal": error.code(), "diagnostic": error.to_string(), "source_sha256": before}))
}

fn qualify_sdk(path: &Path, root: &Path, ffmpeg: &str, ffprobe: &str) -> Result<Value> {
    let source_sha256 = digest_file(path)?;
    let config = root.join("renderflow-native.yaml");
    let output = root.join("sdk-output");
    fs::write(&config, serde_yaml_ng::to_string(&json!({
        "schema": "renderflow/v2", "sources": [{"id":"source.media","path":path,"format":"mkv","sha256":source_sha256}],
        "targets": {"exact": [
            {"id":"target.transcript","format":"timed_transcript_json","role":"transcript","requirement":"required"},
            {"id":"target.subtitles","format":"vtt","role":"subtitles","requirement":"required"}
        ]},
        "execution": {"subtitles":{"stream_index":1}},
        "output": {"bundle_root":output,"naming_template":"{target.role}.{ext}","collision":"error"}
    }))?)?;
    let engine = EngineBuilder::new().build()?;
    let result = engine.execute(ExecutionRequest::from_path(&config))?;
    let manifest_path = root.join("sdk-run-manifest.json");
    fs::write(&manifest_path, serde_json::to_vec_pretty(&result.run_manifest)?)?;
    ensure!(result.run_manifest.state == RunState::Complete, "native SDK run was not complete: {:?}", result.run_manifest.diagnostics);
    ensure!(result.run_manifest.steps.iter().all(|step| step.state == StepState::Complete), "native first run unexpectedly skipped/reused a step");
    let artifacts = &result.run_manifest.artifact_manifest.artifacts;
    let source = artifacts.iter().find(|artifact| artifact.lifecycle == ArtifactRole::Source).context("missing source artifact")?;
    let canonical = artifacts.iter().find(|artifact| artifact.format == "timed_transcript_json").context("missing canonical artifact")?;
    let exported = artifacts.iter().find(|artifact| artifact.format == "vtt").context("missing subtitle artifact")?;
    ensure!(canonical.sources == vec![source.artifact_id.clone()] && exported.sources == vec![canonical.artifact_id.clone()], "native artifact lineage differs from source→canonical→export");
    ensure!(source.digest.value == source_sha256, "native manifest source digest differs from media bytes");
    ensure!(canonical.metadata["renderflow.subtitle"]["input_sha256"] == source_sha256, "immediate media digest missing");
    ensure!(exported.metadata["renderflow.subtitle"]["canonical_transcript"]["artifact_id"] == canonical.artifact_id, "export lost canonical reference");
    let canonical_path = output.join("transcript.timed-transcript.json");
    let canonical_bytes = read_retained(&canonical_path)?;
    let canonical_sha256 = format!("{:x}", Sha256::digest(&canonical_bytes));
    ensure!(canonical_sha256 == canonical.digest.value, "retained canonical bytes differ from manifest digest");
    let transcript = timed_text::parse(&canonical_bytes, TimedTextFormat::Json)?;
    check_extraction_provenance(&transcript, &source_sha256, 1, "subrip", "eng", ffmpeg, ffprobe)?;
    let exported_path = output.join("subtitles.vtt");
    let exported_bytes = read_retained(&exported_path)?;
    let exported_sha256 = format!("{:x}", Sha256::digest(&exported_bytes));
    ensure!(exported_sha256 == exported.digest.value, "retained WebVTT bytes differ from manifest digest");
    let exported_transcript = timed_text::parse(&exported_bytes, TimedTextFormat::WebVtt)?;
    ensure!(exported_transcript.cues == transcript.cues, "retained WebVTT cue IDs/timing/text differ from canonical transcript");
    let flow = result.run_manifest.flow_artifacts_v1();
    let flow_source = flow.iter().find(|artifact| artifact.digest == source.digest).context("missing Flow source")?;
    let flow_canonical = flow.iter().find(|artifact| artifact.digest == canonical.digest).context("missing Flow canonical")?;
    let flow_export = flow.iter().find(|artifact| artifact.digest == exported.digest).context("missing Flow export")?;
    ensure!(flow_canonical.sources == vec![flow_source.artifact_id.clone()] && flow_export.sources == vec![flow_canonical.artifact_id.clone()], "Flow minimal lineage mismatch");
    let flow_path = root.join("sdk-flow-artifacts.json");
    fs::write(&flow_path, serde_json::to_vec_pretty(&flow)?)?;
    ensure!(source_sha256 == digest_file(path)?, "SDK changed original media");
    Ok(json!({"run_manifest":manifest_path,"flow_artifacts":flow_path,"canonical_transcript":canonical_path,
        "source_sha256":source_sha256,"canonical_sha256":canonical_sha256,
        "exported_subtitles":exported_path,"exported_sha256":exported_sha256,
        "native_decode": "selected subtitle stream-copy only; no audio/video encode/decode request"}))
}
