//! Authored contract coverage; these fake-runner tests do not qualify scripttool.

use std::cell::RefCell;

use super::*;

const SOURCE: &[u8] = br#"<FinalDraft DocumentType="Script" Template="No" Version="1"><Content><Paragraph Type="Scene Heading"><Text>INT. LAB - DAY</Text></Paragraph><Paragraph Type="Action"><Text>A light blinks.</Text></Paragraph></Content></FinalDraft>"#;
const OUTPUT: &[u8] = b"INT. LAB - DAY\n\nA light blinks.\n";

#[derive(Clone, Copy)]
enum Behavior {
    Good, Nonzero, Truncated, Timeout, Cancelled, Empty, Missing, Malformed,
    ChangedMeaning, WrongVersion, PrecreatedOutput, MutateStagedInput, MutateProvider,
    Oversized, VersionTruncated,
    #[cfg(unix)]
    Symlink,
}

struct FakeRunner {
    behavior: Behavior,
    arguments: RefCell<Vec<Vec<String>>>,
}

impl FakeRunner {
    fn new(behavior: Behavior) -> Self { Self { behavior, arguments: RefCell::new(Vec::new()) } }
    fn calls(&self) -> usize { self.arguments.borrow().len() }
}

impl ScripttoolRunner for FakeRunner {
    fn run(&self, invocation: &ScripttoolInvocation, _: &ProcessCancellationToken) -> Result<ScripttoolProcessOutput, ScripttoolError> {
        self.arguments.borrow_mut().push(invocation.arguments.clone());
        let mut result = ScripttoolProcessOutput {
            termination: ProcessTermination::Exited { code: 0 }, stdout: Vec::new(),
            stderr: Vec::new(), capture_truncated: false, output_failures: Vec::new(),
        };
        assert!(invocation.executable.is_absolute());
        assert!(invocation.working_directory.is_absolute());
        if invocation.arguments == ["-version"] {
            assert_eq!(invocation.timeout, VERSION_TIMEOUT);
            let version = if matches!(self.behavior, Behavior::WrongVersion) { "0.0.9" } else { SCRIPTTOOL_VERSION };
            result.stdout = format!("{} {version}\n", invocation.executable.file_name().unwrap().to_str().unwrap()).into_bytes();
            if matches!(self.behavior, Behavior::VersionTruncated) { result.capture_truncated = true; }
            if matches!(self.behavior, Behavior::PrecreatedOutput) { fs::write(invocation.working_directory.join("output.fountain"), OUTPUT)?; }
            return Ok(result);
        }
        assert_eq!(invocation.arguments, ["fdx2fountain", "-i", "source.fdx", "-o", "output.fountain"]);
        assert_eq!(invocation.timeout, Duration::from_secs(30));
        assert_eq!(fs::read(invocation.working_directory.join("source.fdx"))?, SOURCE);
        let output_path = invocation.expected_output.as_ref().unwrap();
        assert!(!output_path.exists());
        match self.behavior {
            Behavior::Missing => {}
            Behavior::Empty => fs::write(output_path, [])?,
            Behavior::Malformed => fs::write(output_path, b"not a body-only screenplay")?,
            Behavior::ChangedMeaning => fs::write(output_path, b"INT. LAB - DAY\n\nA door opens.\n")?,
            Behavior::Oversized => fs::write(output_path, vec![b'x'; MAX_SCREENPLAY_BYTES as usize + 1])?,
            #[cfg(unix)]
            Behavior::Symlink => {
                let target = invocation.working_directory.join("elsewhere.fountain");
                fs::write(&target, OUTPUT)?;
                std::os::unix::fs::symlink(target, output_path)?;
            }
            _ => fs::write(output_path, OUTPUT)?,
        }
        match self.behavior {
            Behavior::Nonzero => result.termination = ProcessTermination::Exited { code: 7 },
            Behavior::Truncated => result.capture_truncated = true,
            Behavior::Timeout => result.termination = ProcessTermination::TimedOut,
            Behavior::Cancelled => result.termination = ProcessTermination::Cancelled,
            Behavior::MutateStagedInput => fs::write(invocation.working_directory.join("source.fdx"), b"changed")?,
            Behavior::MutateProvider => fs::write(&invocation.executable, b"changed")?,
            _ => {}
        }
        Ok(result)
    }
}

fn fixture() -> (tempfile::TempDir, ScripttoolRequest) {
    let directory = tempfile::tempdir().unwrap();
    let directory_path = fs::canonicalize(directory.path()).unwrap();
    let input = directory_path.join("original.fdx");
    let executable = directory_path.join("scripttool");
    fs::write(&input, SOURCE).unwrap();
    fs::write(&executable, b"fake executable bytes, never executed").unwrap();
    let config = ScripttoolConfig {
        executable, executable_sha256: sha256(b"fake executable bytes, never executed"),
        expected_version: SCRIPTTOOL_VERSION.to_owned(), timeout_seconds: 30, allow_unqualified: true,
    };
    (directory, ScripttoolRequest {
        input, source_format: NativeScreenplayFormat::Fdx,
        target_format: NativeScreenplayFormat::Fountain, config,
        expected_source_sha256: Some(sha256(SOURCE)),
    })
}

#[test]
fn successful_import_binds_exact_bytes_argv_and_semantic_evidence() {
    let (_directory, request) = fixture();
    let runner = FakeRunner::new(Behavior::Good);
    let run = convert_with_runner(&request, &runner, ProcessCancellationToken::new()).unwrap();
    assert_eq!(runner.calls(), 2);
    assert_eq!(run.output, OUTPUT);
    assert_eq!(run.evidence.source_sha256, sha256(SOURCE));
    assert_eq!(run.evidence.output_sha256, sha256(OUTPUT));
    assert_eq!(run.screenplay.source.sha256, sha256(OUTPUT));
    assert_eq!(run.evidence.argv_sha256, sha256(&serde_json::to_vec(&run.evidence.argv).unwrap()));
    assert_eq!(run.evidence.provider_version, "scripttool 0.0.10");
    assert_eq!(run.evidence.qualification, "authored_unqualified");
    assert!(!run.evidence.executable_source_attested);
    assert!(run.evidence.semantic_projection_matched);
    assert_eq!(run.evidence.source_element_count, 2);
    assert_eq!(run.evidence.output_element_count, 2);
    assert_eq!(run.evidence.losses.len(), 4);
    assert_eq!(fs::read(&request.input).unwrap(), SOURCE);
}

#[test]
fn all_reverse_exports_and_same_format_routes_are_refused() {
    use NativeScreenplayFormat::{FadeIn, Fdx, Fountain, Osf};
    for source in [FadeIn, Fdx, Fountain, Osf] {
        for target in [FadeIn, Fdx, Fountain, Osf] {
            let allowed = source != Fountain && target == Fountain;
            assert_eq!(supported_route(source, target), allowed);
            assert_eq!(conversion_command(source, target).is_ok(), allowed);
        }
    }
}

#[test]
fn unsupported_route_is_rejected_before_reading_or_spawning() {
    let (_directory, mut request) = fixture();
    request.source_format = NativeScreenplayFormat::Fountain;
    request.target_format = NativeScreenplayFormat::Fdx;
    fs::remove_file(&request.input).unwrap();
    let runner = FakeRunner::new(Behavior::Good);
    assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::UnsupportedRoute(_))));
    assert_eq!(runner.calls(), 0);
}

#[test]
fn unsupported_native_metadata_is_rejected_before_any_provider_process() {
    let (_directory, mut request) = fixture();
    let source = String::from_utf8(SOURCE.to_vec()).unwrap().replace("</FinalDraft>", "<TitlePage/></FinalDraft>");
    fs::write(&request.input, source).unwrap();
    request.expected_source_sha256 = None;
    let runner = FakeRunner::new(Behavior::Good);
    assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::InvalidInput(_))));
    assert_eq!(runner.calls(), 0);
}

#[test]
fn executable_digest_mismatch_blocks_even_the_version_probe() {
    let (_directory, mut request) = fixture();
    request.config.executable_sha256 = "0".repeat(64);
    let runner = FakeRunner::new(Behavior::Good);
    assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::ProviderUnavailable(_))));
    assert_eq!(runner.calls(), 0);
}

#[test]
fn source_artifact_digest_mismatch_blocks_all_provider_processes() {
    let (_directory, mut request) = fixture();
    request.expected_source_sha256 = Some("0".repeat(64));
    let runner = FakeRunner::new(Behavior::Good);
    assert!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()).is_err());
    assert_eq!(runner.calls(), 0);
}

#[test]
fn explicit_unqualified_opt_in_is_required_before_processes() {
    let (_directory, mut request) = fixture();
    request.config.allow_unqualified = false;
    let runner = FakeRunner::new(Behavior::Good);
    assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::InvalidRequest(_))));
    assert_eq!(runner.calls(), 0);
}

#[test]
fn version_contract_rejects_suffixes_extra_lines_and_other_versions() {
    let path = Path::new("/selected/scripttool");
    assert!(supported_version_line("scripttool 0.0.10", SCRIPTTOOL_VERSION, path));
    for line in ["scripttool 0.0.9", "scripttool 0.0.10-preview", "scripttool 0.0.10 extra", "scripttool 0.0.10\n", "other 0.0.10"] {
        assert!(!supported_version_line(line, SCRIPTTOOL_VERSION, path));
    }
    assert!(parse_version(b"scripttool 0.0.10\n", SCRIPTTOOL_VERSION, path).is_ok());
    assert!(parse_version(b"scripttool 0.0.10\nextra\n", SCRIPTTOOL_VERSION, path).is_err());
}

#[test]
fn wrong_observed_version_blocks_conversion() {
    let (_directory, request) = fixture();
    let runner = FakeRunner::new(Behavior::WrongVersion);
    assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::ProviderUnavailable(_))));
    assert_eq!(runner.calls(), 1);
}

#[test]
fn truncated_version_output_blocks_conversion() {
    let (_directory, request) = fixture();
    let runner = FakeRunner::new(Behavior::VersionTruncated);
    assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::ProviderFailed(_))));
    assert_eq!(runner.calls(), 1);
}

#[test]
fn output_precreated_during_version_probe_cannot_be_accepted_as_conversion() {
    let (_directory, request) = fixture();
    let runner = FakeRunner::new(Behavior::PrecreatedOutput);
    assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::InvalidOutput(_))));
    assert_eq!(runner.calls(), 1);
}

#[test]
fn nonzero_or_truncated_conversion_never_returns_output() {
    for behavior in [Behavior::Nonzero, Behavior::Truncated] {
        let (_directory, request) = fixture();
        let runner = FakeRunner::new(behavior);
        assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::ProviderFailed(_))));
        assert_eq!(runner.calls(), 2);
    }
}

#[test]
fn provider_timeout_and_cancellation_are_distinct_failures() {
    let (_directory, request) = fixture();
    assert!(matches!(convert_with_runner(&request, &FakeRunner::new(Behavior::Timeout), ProcessCancellationToken::new()), Err(ScripttoolError::TimedOut)));
    assert!(matches!(convert_with_runner(&request, &FakeRunner::new(Behavior::Cancelled), ProcessCancellationToken::new()), Err(ScripttoolError::Cancelled)));
}

#[test]
fn already_cancelled_request_does_not_spawn() {
    let (_directory, request) = fixture();
    let runner = FakeRunner::new(Behavior::Good);
    let cancellation = ProcessCancellationToken::new();
    cancellation.cancel();
    assert!(matches!(convert_with_runner(&request, &runner, cancellation), Err(ScripttoolError::Cancelled)));
    assert_eq!(runner.calls(), 0);
}

#[test]
fn missing_empty_malformed_or_oversized_output_never_succeeds() {
    for behavior in [Behavior::Missing, Behavior::Empty, Behavior::Malformed, Behavior::Oversized] {
        let (_directory, request) = fixture();
        let runner = FakeRunner::new(behavior);
        assert!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()).is_err());
        assert_eq!(runner.calls(), 2);
    }
}

#[test]
fn valid_syntax_with_changed_semantics_is_rejected() {
    let (_directory, request) = fixture();
    let runner = FakeRunner::new(Behavior::ChangedMeaning);
    assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::InvalidOutput(_))));
}

#[test]
fn staged_or_executable_mutation_is_detected_before_publication() {
    for behavior in [Behavior::MutateStagedInput, Behavior::MutateProvider] {
        let (_directory, request) = fixture();
        let runner = FakeRunner::new(behavior);
        assert!(matches!(convert_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ScripttoolError::SourceChanged(_))));
        assert_eq!(fs::read(&request.input).unwrap(), SOURCE);
    }
}

#[test]
fn declarative_policy_rejects_ambiguous_pins_relative_paths_and_unbounded_timeout() {
    let (_directory, request) = fixture();
    let mut config = request.config;
    config.timeout_seconds = 0;
    assert!(config.validate().is_err());
    config.timeout_seconds = 301;
    assert!(config.validate().is_err());
    config.timeout_seconds = 30;
    config.executable_sha256 = "A".repeat(64);
    assert!(config.validate().is_err());
    config.executable_sha256 = "a".repeat(64);
    config.executable = PathBuf::from("scripttool");
    assert!(config.validate().is_err());
}

#[cfg(unix)]
#[test]
fn symlink_provider_output_is_rejected_even_when_target_bytes_are_valid() {
    let (_directory, request) = fixture();
    assert!(convert_with_runner(&request, &FakeRunner::new(Behavior::Symlink), ProcessCancellationToken::new()).is_err());
}
