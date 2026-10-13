//! Authored fake-runner coverage. No native tool qualification is implied.

use std::cell::RefCell;
use lopdf::{dictionary, Stream};
use super::*;

const TEXT: &str = "INT. LAB - DAY\n\nA lamp flickers.\n";
const TOOL_BYTES: &[u8] = b"fake pinned executable; this file is never launched";
const DATA_BYTES: &[u8] = b"fake traineddata; this file is never loaded by native code";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Behavior { Good, EmptyFirst, EmptyAll, WrongVersion, Nonzero, Timeout, Cancelled,
    Truncated, InvalidUtf8, EmbeddedControl, InvalidRaster, ExcessiveRaster, MutateSource,
    MutateTool, MutateData, PrecreateRaster }

struct FakeRunner { behavior: Behavior, calls: RefCell<Vec<(String, Vec<String>)>> }
impl FakeRunner {
    fn new(behavior: Behavior) -> Self { Self { behavior, calls: RefCell::new(Vec::new()) } }
}
impl RecoveryExtractionRunner for FakeRunner {
    fn run(&self, invocation: &ExtractionInvocation, _: &ProcessCancellationToken)
        -> Result<ExtractionProcessOutput, ExtractionError> {
        let name = invocation.executable.file_name().unwrap().to_str().unwrap().to_owned();
        self.calls.borrow_mut().push((name.clone(), invocation.arguments.clone()));
        assert!(invocation.executable.is_absolute());
        assert!(invocation.working_directory.is_absolute());
        assert!(invocation.timeout <= Duration::from_secs(300));
        let mut result = ExtractionProcessOutput { termination: ProcessTermination::Exited { code: 0 },
            stdout: Vec::new(), stderr: Vec::new(), capture_truncated: false, output_failures: Vec::new() };
        if invocation.arguments == ["-v"] || invocation.arguments == ["--version"] {
            let version = if self.behavior == Behavior::WrongVersion { "0.0.0" } else if name == "tesseract" { "5.5.1" } else { "25.01.0" };
            if name == "tesseract" { result.stdout = format!("tesseract {version}\n library details\n").into_bytes(); }
            else { result.stderr = format!("{name} version {version}\nCopyright notices\n").into_bytes(); }
            if self.behavior == Behavior::PrecreateRaster && name == "pdftoppm" {
                fs::write(invocation.working_directory.join("page-1.pgm"), b"precreated")?;
            }
            return Ok(result);
        }
        match name.as_str() {
            "pdftotext" => {
                let page: u32 = invocation.arguments[1].parse().unwrap();
                assert_eq!(invocation.arguments, text_arguments(page));
                result.stdout = if self.behavior == Behavior::EmptyAll || (self.behavior == Behavior::EmptyFirst && page == 1)
                    || matches!(self.behavior, Behavior::InvalidRaster | Behavior::ExcessiveRaster | Behavior::MutateData | Behavior::PrecreateRaster) {
                    b"\n  1\n".to_vec()
                } else { TEXT.as_bytes().to_vec() };
                match self.behavior {
                    Behavior::Nonzero => result.termination = ProcessTermination::Exited { code: 1 },
                    Behavior::Timeout => result.termination = ProcessTermination::TimedOut,
                    Behavior::Cancelled => result.termination = ProcessTermination::Cancelled,
                    Behavior::Truncated => result.capture_truncated = true,
                    Behavior::InvalidUtf8 => result.stdout = vec![0xff],
                    Behavior::EmbeddedControl => result.stdout = b"bad\0text".to_vec(),
                    Behavior::MutateSource => fs::write(invocation.working_directory.join("source.pdf"), b"changed")?,
                    Behavior::MutateTool => fs::write(&invocation.executable, b"changed")?,
                    _ => {}
                }
            }
            "pdftoppm" => {
                let page = invocation.arguments[1].parse().unwrap();
                assert_eq!(invocation.arguments, raster_arguments(page, 3000));
                let (path, _) = invocation.expected_output.as_ref().unwrap();
                assert!(!path.exists());
                let bytes: &[u8] = match self.behavior {
                    Behavior::InvalidRaster => b"not an image",
                    Behavior::ExcessiveRaster => b"P5\n4000 4000\n255\n0",
                    _ => b"P5\n2 2\n255\n\x00\xff\x7f\x80",
                };
                fs::write(path, bytes)?;
            }
            "tesseract" => {
                let page: u32 = invocation.arguments[0].strip_prefix("page-").unwrap().strip_suffix(".pgm").unwrap().parse().unwrap();
                assert_eq!(invocation.arguments, ocr_arguments(page, "eng"));
                assert_eq!(fs::read(invocation.working_directory.join("tessdata/eng.traineddata"))?, DATA_BYTES);
                if self.behavior == Behavior::MutateData { fs::write(invocation.working_directory.join("tessdata/eng.traineddata"), b"changed")?; }
                result.stdout = TEXT.as_bytes().to_vec();
            }
            _ => panic!("unexpected native tool"),
        }
        Ok(result)
    }
}

fn pdf(page_count: usize, mutate: impl FnOnce(&mut Document, ObjectId)) -> Vec<u8> {
    let mut document = Document::with_version("1.4");
    document.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    let root = document.new_object_id();
    let mut children: Vec<Object> = Vec::new();
    for _ in 0..page_count {
        let stream = document.add_object(Stream::new(dictionary! {}, b"BT (content) Tj ET".to_vec()));
        let page = document.add_object(dictionary! { "Type" => "Page", "Parent" => root,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()], "Contents" => stream });
        children.push(page.into());
    }
    document.objects.insert(root, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => children, "Count" => page_count as i64 }));
    let catalog = document.add_object(dictionary! { "Type" => "Catalog", "Pages" => root });
    document.trailer.set("Root", catalog);
    mutate(&mut document, root);
    let mut output = Vec::new();
    document.save_to(&mut output).unwrap();
    output
}

fn tool(directory: &Path, name: &str) -> PinnedRecoveryTool {
    let executable = directory.join(name);
    fs::write(&executable, TOOL_BYTES).unwrap();
    PinnedRecoveryTool { executable, executable_sha256: digest(TOOL_BYTES),
        expected_version: if name == "tesseract" { "5.5.1" } else { "25.01.0" }.to_owned() }
}
fn fixture(page_count: usize, ocr: bool) -> (tempfile::TempDir, ExtractionRequest) {
    let directory = tempfile::tempdir().unwrap();
    let path = fs::canonicalize(directory.path()).unwrap();
    let input = path.join("original.pdf");
    let bytes = pdf(page_count, |_, _| {});
    fs::write(&input, &bytes).unwrap();
    let mut config = RecoveryExtractionConfig { pdftotext: Some(tool(&path, "pdftotext")),
        allow_unqualified: true, ..RecoveryExtractionConfig::default() };
    if ocr {
        let traineddata = path.join("source.traineddata");
        fs::write(&traineddata, DATA_BYTES).unwrap();
        config.ocr = Some(RecoveryOcrConfig { pdftoppm: tool(&path, "pdftoppm"), tesseract: tool(&path, "tesseract"),
            language: "eng".to_owned(), traineddata, traineddata_sha256: digest(DATA_BYTES) });
    }
    (directory, ExtractionRequest { input, source_format: ExtractionSourceFormat::Pdf,
        expected_source_sha256: Some(digest(&bytes)), config })
}

#[test]
fn text_path_uses_no_tools_and_retains_form_feed_page_identity() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("script.txt");
    fs::write(&input, format!("{TEXT}\u{000c}EXT. FIELD - NIGHT\n")).unwrap();
    let request = ExtractionRequest { input: input.clone(), source_format: ExtractionSourceFormat::Text,
        expected_source_sha256: None, config: RecoveryExtractionConfig { allow_unqualified: true, ..RecoveryExtractionConfig::default() } };
    let runner = FakeRunner::new(Behavior::Good);
    let result = extract_with_runner(&request, &runner, ProcessCancellationToken::new()).unwrap();
    assert!(runner.calls.borrow().is_empty());
    assert_eq!(result.pages.len(), 2);
    assert_eq!(result.pages[1].page_number, 2);
    assert_eq!(result.pages[0].method, ExtractionMethod::PlainText);
    assert_eq!(result.evidence.source_sha256, digest(&fs::read(input).unwrap()));
    assert!(result.evidence.review_required);
}

#[test]
fn opt_in_and_declarations_are_checked_without_native_calls_or_file_reads() {
    let mut config = RecoveryExtractionConfig::default();
    assert!(config.validate().is_err());
    config.allow_unqualified = true;
    config.pdftotext = Some(PinnedRecoveryTool { executable: PathBuf::from("/does/not/exist/pdftotext"),
        executable_sha256: "0".repeat(64), expected_version: "25.01.0".to_owned() });
    assert!(config.validate().is_ok());
    config.pdftotext.as_mut().unwrap().expected_version = ">=25".to_owned();
    assert!(config.validate().is_err());
}

#[test]
fn pdf_extracts_each_page_once_without_probing_unused_ocr() {
    let (_directory, request) = fixture(2, true);
    let runner = FakeRunner::new(Behavior::Good);
    let result = extract_with_runner(&request, &runner, ProcessCancellationToken::new()).unwrap();
    assert_eq!(result.pages.len(), 2);
    assert_eq!(result.evidence.providers.len(), 1);
    assert_eq!(runner.calls.borrow().len(), 3);
    assert!(result.evidence.traineddata_sha256.is_none());
    assert_eq!(result.pages[0].text, TEXT);
    assert_eq!(result.pages[0].method, ExtractionMethod::EmbeddedText);
    validate_extraction_evidence(&result.pages, &result.evidence, &request.config, &result.source_sha256).unwrap();
    assert_eq!(digest(&fs::read(&request.input).unwrap()), result.source_sha256);
}

#[test]
fn mixed_pdf_ocr_is_local_pinned_and_preserves_original_page_number_and_embedded_text() {
    let (_directory, request) = fixture(2, true);
    let runner = FakeRunner::new(Behavior::EmptyFirst);
    let result = extract_with_runner(&request, &runner, ProcessCancellationToken::new()).unwrap();
    assert_eq!(result.pages[0].method, ExtractionMethod::Ocr);
    assert_eq!(result.pages[1].method, ExtractionMethod::EmbeddedText);
    assert_eq!(result.evidence.pages[0].embedded_text.as_deref(), Some("\n  1\n"));
    assert_eq!(result.evidence.pages[0].raster_dimensions, Some([2, 2]));
    assert_eq!(result.evidence.providers.len(), 3);
    assert_eq!(result.evidence.traineddata_sha256.as_deref(), Some(digest(DATA_BYTES).as_str()));
    assert_eq!(runner.calls.borrow().len(), 7);
    validate_extraction_evidence(&result.pages, &result.evidence, &request.config, &result.source_sha256).unwrap();
}

#[test]
fn image_only_without_ocr_is_explicitly_unavailable() {
    let (_directory, request) = fixture(1, false);
    let error = extract_with_runner(&request, &FakeRunner::new(Behavior::EmptyAll), ProcessCancellationToken::new()).unwrap_err();
    assert!(matches!(error, ExtractionError::ProviderUnavailable(_)));
}

#[test]
fn mixed_pdf_without_ocr_preserves_missing_page_evidence() {
    let (_directory, request) = fixture(2, false);
    let result = extract_with_runner(&request, &FakeRunner::new(Behavior::EmptyFirst), ProcessCancellationToken::new()).unwrap();
    assert!(!result.evidence.pages[0].usable_text);
    assert!(!result.evidence.pages[0].diagnostics.is_empty());
    assert_eq!(result.pages[1].page_number, 2);
}

#[test]
fn malformed_pdf_and_excessive_page_count_do_not_launch_any_tool() {
    let (_directory, mut request) = fixture(2, false);
    request.config.limits.max_pages = 1;
    let runner = FakeRunner::new(Behavior::Good);
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::LimitExceeded(_))));
    fs::write(&request.input, b"%PDF-1.4\nmalformed\n%%EOF").unwrap();
    request.expected_source_sha256 = None;
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::MalformedPdf(_))));
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn active_pdf_and_cyclic_page_tree_are_rejected_before_native_execution() {
    let (_directory, mut request) = fixture(1, false);
    let bytes = pdf(1, |document, _| { document.add_object(dictionary! { "JS" => Object::string_literal("alert(1)") }); });
    fs::write(&request.input, bytes).unwrap();
    request.expected_source_sha256 = None;
    let runner = FakeRunner::new(Behavior::Good);
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::UnsupportedInput(_))));
    let bytes = pdf(1, |document, root| { document.get_dictionary_mut(root).unwrap().set("Kids", vec![Object::Reference(root)]); });
    fs::write(&request.input, bytes).unwrap();
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::MalformedPdf(_))));
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn malformed_catalog_parent_and_launch_action_cannot_bypass_pdf_profile() {
    let (_directory, mut request) = fixture(1, false);
    request.expected_source_sha256 = None;
    let runner = FakeRunner::new(Behavior::Good);
    let malformed_parent = pdf(1, |document, root| { document.get_dictionary_mut(root).unwrap().set("Parent", 42); });
    fs::write(&request.input, malformed_parent).unwrap();
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::MalformedPdf(_))));
    let malformed_catalog = pdf(1, |document, _| {
        let catalog = document.trailer.get(b"Root").unwrap().as_reference().unwrap();
        document.get_dictionary_mut(catalog).unwrap().set("Type", "NotACatalog");
    });
    fs::write(&request.input, malformed_catalog).unwrap();
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::MalformedPdf(_))));
    let launch = pdf(1, |document, _| { document.add_object(dictionary! { "S" => "Launch" }); });
    fs::write(&request.input, launch).unwrap();
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::UnsupportedInput(_))));
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn encrypted_pdf_is_typed_and_never_sent_to_a_native_provider() {
    let (_directory, mut request) = fixture(1, false);
    let bytes = pdf(1, |document, _| {
        // A synthetic Standard security dictionary is sufficient to exercise
        // the loader/encryption boundary; this does not qualify decryption.
        let encryption = document.add_object(dictionary! {
            "Filter" => "Standard", "V" => 1, "R" => 2, "Length" => 40,
            "O" => Object::String(vec![0; 32], lopdf::StringFormat::Hexadecimal),
            "U" => Object::String(vec![0; 32], lopdf::StringFormat::Hexadecimal), "P" => -4,
        });
        document.trailer.set("Encrypt", encryption);
        document.trailer.set("ID", vec![Object::String(vec![1; 16], lopdf::StringFormat::Hexadecimal),
            Object::String(vec![1; 16], lopdf::StringFormat::Hexadecimal)]);
    });
    fs::write(&request.input, bytes).unwrap();
    request.expected_source_sha256 = None;
    let runner = FakeRunner::new(Behavior::Good);
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::EncryptedPdf)));
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn input_and_tool_hash_mismatches_block_native_probes() {
    let (_directory, mut request) = fixture(1, false);
    let runner = FakeRunner::new(Behavior::Good);
    request.expected_source_sha256 = Some("0".repeat(64));
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::SourceChanged(_))));
    request.expected_source_sha256 = None;
    request.config.pdftotext.as_mut().unwrap().executable_sha256 = "0".repeat(64);
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::ProviderUnavailable(_))));
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn absent_provider_and_wrong_observed_version_are_typed_unavailable() {
    let (_directory, mut request) = fixture(1, false);
    let runner = FakeRunner::new(Behavior::WrongVersion);
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::ProviderUnavailable(_))));
    request.config.pdftotext = None;
    assert!(matches!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(ExtractionError::ProviderUnavailable(_))));
    assert_eq!(runner.calls.borrow().len(), 1);
}

#[test]
fn process_failures_capture_limits_and_bad_text_never_yield_a_partial_success() {
    for behavior in [Behavior::Nonzero, Behavior::Timeout, Behavior::Cancelled, Behavior::Truncated,
        Behavior::InvalidUtf8, Behavior::EmbeddedControl, Behavior::MutateSource, Behavior::MutateTool] {
        let (_directory, request) = fixture(2, false);
        assert!(extract_with_runner(&request, &FakeRunner::new(behavior), ProcessCancellationToken::new()).is_err());
    }
}

#[test]
fn malformed_oversized_precreated_and_changed_ocr_artifacts_are_rejected() {
    for behavior in [Behavior::InvalidRaster, Behavior::ExcessiveRaster, Behavior::MutateData, Behavior::PrecreateRaster] {
        let (_directory, request) = fixture(1, true);
        assert!(extract_with_runner(&request, &FakeRunner::new(behavior), ProcessCancellationToken::new()).is_err());
    }
}

#[test]
fn cancellation_prevents_even_input_reads_and_probes() {
    let (_directory, request) = fixture(1, false);
    fs::remove_file(&request.input).unwrap();
    let cancellation = ProcessCancellationToken::new();
    cancellation.cancel();
    let runner = FakeRunner::new(Behavior::Good);
    assert!(matches!(extract_with_runner(&request, &runner, cancellation), Err(ExtractionError::Cancelled)));
    assert!(runner.calls.borrow().is_empty());
}

#[test]
fn total_page_text_bound_is_not_reset_between_native_calls() {
    let (_directory, mut request) = fixture(2, false);
    request.config.limits.max_page_text_bytes = TEXT.len();
    request.config.limits.max_total_text_bytes = TEXT.len();
    assert!(matches!(extract_with_runner(&request, &FakeRunner::new(Behavior::Good), ProcessCancellationToken::new()), Err(ExtractionError::LimitExceeded(_))));
}

#[test]
fn evidence_replay_rejects_changed_pages_pins_commands_and_ocr_claims() {
    let (_directory, request) = fixture(2, true);
    let result = extract_with_runner(&request, &FakeRunner::new(Behavior::EmptyFirst), ProcessCancellationToken::new()).unwrap();
    let mut changed = result.clone();
    changed.pages[0].text.push_str("invented");
    assert!(validate_extraction_evidence(&changed.pages, &changed.evidence, &request.config, &result.source_sha256).is_err());
    let mut changed = result.evidence.clone();
    changed.providers[0].observed_version = "25.02.0".to_owned();
    assert!(validate_extraction_evidence(&result.pages, &changed, &request.config, &result.source_sha256).is_err());
    let mut changed = result.evidence.clone();
    changed.invocations[1].argv.push("--extra".to_owned());
    changed.invocations[1].argv_sha256 = digest(&serde_json::to_vec(&changed.invocations[1].argv).unwrap());
    assert!(validate_extraction_evidence(&result.pages, &changed, &request.config, &result.source_sha256).is_err());
    let mut changed = result.evidence.clone();
    changed.pages[0].embedded_text = Some("usable original text".to_owned());
    changed.pages[0].embedded_text_sha256 = Some(digest(b"usable original text"));
    assert!(validate_extraction_evidence(&result.pages, &changed, &request.config, &result.source_sha256).is_err());
}

#[test]
fn pgm_validation_checks_exact_pixels_and_bounded_dimensions() {
    assert_eq!(inspect_pgm(b"P5\n# header\n2 1\n255\n\x00\xff", 64).unwrap(), [2, 1]);
    for bytes in [b"P5\n2 1\n255\n\x00".as_slice(), b"P5\n2 1\n255\n\x00\xffextra", b"P6\n1 1\n255\n000", b"P5\n9999 1\n255\n0"] {
        assert!(inspect_pgm(bytes, 64).is_err());
    }
}

#[test]
fn language_paths_multiple_models_and_implicit_ocr_are_refused() {
    let (_directory, mut request) = fixture(1, true);
    for language in ["../eng", "eng+fra", "script/Latin", "eng\n"] {
        request.config.ocr.as_mut().unwrap().language = language.to_owned();
        assert!(request.config.validate().is_err());
    }
    request.config.ocr.as_mut().unwrap().language = "eng".to_owned();
    request.config.pdftotext = None;
    assert!(request.config.validate().is_err());
}

#[cfg(unix)]
#[test]
fn source_symlink_final_component_is_refused() {
    let (directory, mut request) = fixture(1, false);
    let link = directory.path().join("linked.pdf");
    std::os::unix::fs::symlink(&request.input, &link).unwrap();
    request.input = link;
    let runner = FakeRunner::new(Behavior::Good);
    assert!(extract_with_runner(&request, &runner, ProcessCancellationToken::new()).is_err());
    assert!(runner.calls.borrow().is_empty());
}
