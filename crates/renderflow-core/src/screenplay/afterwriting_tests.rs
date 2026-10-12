//! Authored contract coverage using fake processes and synthetic PDFs only.
//! No test, Node executable, afterwriting conversion, or visual check was run.

use std::cell::RefCell;
use lopdf::{dictionary, Document, Object, Stream};
use super::*;

const SOURCE: &[u8] = b"INT. LAB - DAY\n\nA light blinks.\n";
const NODE: &[u8] = b"fake Node bytes, never executed";
const DEPENDENCIES: &[&str] = &["aw-liner", "aw-parser", "d3", "handlebars", "jquery", "lodash", "pdfkit", "protoplast", "requirejs-text", "stdio"];

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
fn fake_package(root: &Path) {
    fs::create_dir(root).unwrap();
    let dependencies: serde_json::Map<String, Value> = DEPENDENCIES.iter().map(|name| (name.to_string(), json!("*"))).collect();
    write(root, "package.json", &serde_json::to_vec(&json!({ "name": "afterwriting", "version": "1.17.3",
        "bin": { "afterwriting": "./awc.js" }, "dependencies": dependencies })).unwrap());
    for path in ["awc.js", "js/bootstrap.js", "js/client/client-config.js", "js/client/options.js", "js/client/awrequire.js"] {
        write(root, path, b"// synthetic source, never executed\n");
    }
    for name in DEPENDENCIES {
        write(root, &format!("node_modules/{name}/package.json"), br#"{"main":"index.js"}"#);
        write(root, &format!("node_modules/{name}/index.js"), b"// synthetic dependency\n");
    }
    write(root, "node_modules/pdfkit/js/pdfkit.js", b"// synthetic PDFKit\n");
    for name in ["Courier", "Courier-Bold", "Courier-Oblique", "Courier-BoldOblique", "Helvetica"] {
        write(root, &format!("node_modules/pdfkit/js/data/{name}.afm"), b"synthetic metrics, never parsed by provider\n");
    }
}
fn fixture() -> (tempfile::TempDir, AfterwritingRequest) {
    let directory = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(directory.path()).unwrap();
    let package_root = root.join("afterwriting");
    fake_package(&package_root);
    let input = root.join("source with spaces;$(literal).fountain");
    fs::write(&input, SOURCE).unwrap();
    let node_executable = root.join("node");
    fs::write(&node_executable, NODE).unwrap();
    let package_tree_sha256 = package_fingerprint(&package_root, ProcessCancellationToken::new()).unwrap().tree_sha256;
    (directory, AfterwritingRequest { input, expected_source_sha256: Some(digest(SOURCE)), config: AfterwritingConfig {
        node_executable, node_sha256: digest(NODE), expected_node_version: "v22.1.0".to_owned(),
        package_root, package_tree_sha256, expected_provider_version: AFTERWRITING_VERSION.to_owned(),
        profile: ScreenplayPdfProfile::UsLetter, timeout_seconds: 30, allow_unqualified: true,
    } })
}
fn repin(request: &mut AfterwritingRequest) {
    request.config.package_tree_sha256 = package_fingerprint(&request.config.package_root, ProcessCancellationToken::new()).unwrap().tree_sha256;
}
fn pdf() -> Vec<u8> {
    let mut doc = Document::with_version("1.3");
    doc.reference_table.cross_reference_type = lopdf::xref::XrefType::CrossReferenceTable;
    let pages = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier", "Encoding" => "WinAnsiEncoding" });
    let content = doc.add_object(Stream::new(dictionary! {}, b"BT /F1 12 Tf 108 720 Td (INT. LAB - DAY) Tj ET".to_vec()));
    let page = doc.add_object(dictionary! { "Type" => "Page", "Parent" => pages, "Contents" => content,
        "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } } });
    doc.objects.insert(pages, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
    doc.trailer.set("Root", catalog);
    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).unwrap();
    bytes
}

#[derive(Clone, Copy)]
enum Behavior { Good, Missing, Empty, Malformed, Nonzero, Timeout, Cancelled, Truncated,
    WrongVersion, VersionStderr, VersionTruncated, PrecreatedOutput, MutateNode, MutatePackage,
    MutateSource, MutateConfig, MutateOriginal, WrongPaper,
    #[cfg(unix)]
    SymlinkOutput,
}
struct FakeRunner { behavior: Behavior, original: PathBuf, profile: ScreenplayPdfProfile, calls: RefCell<Vec<Vec<String>>> }
impl FakeRunner {
    fn new(behavior: Behavior, request: &AfterwritingRequest) -> Self {
        Self { behavior, original: request.input.clone(), profile: request.config.profile, calls: RefCell::new(Vec::new()) }
    }
    fn calls(&self) -> usize { self.calls.borrow().len() }
}
impl AfterwritingRunner for FakeRunner {
    fn run(&self, invocation: &AfterwritingInvocation, _: &ProcessCancellationToken)
        -> Result<AfterwritingProcessOutput, AfterwritingError> {
        self.calls.borrow_mut().push(invocation.arguments.clone());
        assert!(invocation.executable.is_absolute());
        assert!(invocation.working_directory.is_absolute());
        let mut result = AfterwritingProcessOutput { termination: ProcessTermination::Exited { code: 0 },
            stdout: Vec::new(), stderr: Vec::new(), capture_truncated: false, output_failures: Vec::new() };
        if invocation.expected_output.is_none() {
            assert_eq!(invocation.arguments, ["--no-addons", "--no-global-search-paths", "--version"]);
            assert_eq!(invocation.timeout, VERSION_TIMEOUT);
            result.stdout = if matches!(self.behavior, Behavior::WrongVersion) { b"v24.1.0\n".to_vec() } else { b"v22.1.0\n".to_vec() };
            if matches!(self.behavior, Behavior::VersionStderr) { result.stderr = b"warning\n".to_vec(); }
            if matches!(self.behavior, Behavior::VersionTruncated) { result.capture_truncated = true; }
            if matches!(self.behavior, Behavior::PrecreatedOutput) { fs::write(invocation.working_directory.join("output.pdf"), pdf())?; }
            return Ok(result);
        }
        assert_eq!(invocation.arguments, ["--no-addons", "--no-global-search-paths", "package/awc.js", "--source", "source.fountain", "--pdf", "output.pdf", "--config", "profile.json"]);
        assert_eq!(fs::read(invocation.working_directory.join("source.fountain"))?, SOURCE);
        assert_eq!(serde_json::from_slice::<Value>(&fs::read(invocation.working_directory.join("profile.json"))?)?, self.profile.settings());
        let output = invocation.expected_output.as_ref().unwrap();
        assert!(!output.exists());
        match self.behavior {
            Behavior::Missing => {}
            Behavior::Empty => fs::write(output, [])?,
            Behavior::Malformed => fs::write(output, b"%PDF-broken")?,
            #[cfg(unix)]
            Behavior::SymlinkOutput => {
                let target = invocation.working_directory.join("elsewhere.pdf");
                fs::write(&target, pdf())?;
                std::os::unix::fs::symlink(target, output)?;
            }
            _ => fs::write(output, pdf())?,
        }
        match self.behavior {
            Behavior::Nonzero => result.termination = ProcessTermination::Exited { code: 2 },
            Behavior::Timeout => result.termination = ProcessTermination::TimedOut,
            Behavior::Cancelled => result.termination = ProcessTermination::Cancelled,
            Behavior::Truncated => result.capture_truncated = true,
            Behavior::MutateNode => fs::write(&invocation.executable, b"changed node")?,
            Behavior::MutatePackage => fs::write(invocation.working_directory.join("package/awc.js"), b"changed package")?,
            Behavior::MutateSource => fs::write(invocation.working_directory.join("source.fountain"), b"changed source")?,
            Behavior::MutateConfig => fs::write(invocation.working_directory.join("profile.json"), b"{}")?,
            Behavior::MutateOriginal => fs::write(&self.original, b"changed original")?,
            _ => {}
        }
        Ok(result)
    }
}

#[test]
fn successful_candidate_binds_source_package_node_settings_and_independent_evidence() {
    let (_directory, request) = fixture();
    let runner = FakeRunner::new(Behavior::Good, &request);
    let result = render_with_runner(&request, &runner, ProcessCancellationToken::new()).unwrap();
    assert_eq!(runner.calls(), 2);
    assert_eq!(result.evidence.source_sha256, digest(SOURCE));
    assert_eq!(result.evidence.output_sha256, digest(&result.output));
    assert_eq!(result.evidence.inspection.sha256, result.evidence.output_sha256);
    assert_eq!(result.evidence.analysis_sha256, digest(&serde_json::to_vec(&result.analysis).unwrap()));
    assert_eq!(result.evidence.config_sha256, digest(&serde_json::to_vec(&result.evidence.settings).unwrap()));
    assert_eq!(result.evidence.argv_sha256, digest(&serde_json::to_vec(&result.evidence.argv).unwrap()));
    assert_eq!(result.evidence.node_sha256, request.config.node_sha256);
    assert_eq!(result.evidence.package_tree_sha256, request.config.package_tree_sha256);
    assert_eq!(result.analysis.source.sha256, digest(SOURCE));
    assert_eq!(result.evidence.source_element_count, 2);
    assert_eq!(result.evidence.qualification, "authored_unqualified");
    assert!(!result.evidence.executable_source_attested);
    assert!(!result.evidence.dependency_closure_attested);
    assert!(!result.evidence.inspection.source_text_equivalence_verified);
    assert!(!result.evidence.inspection.visual_fidelity_verified);
    assert!(!result.evidence.inspection.byte_reproducibility_verified);
    assert_eq!(fs::read(&request.input).unwrap(), SOURCE);
}

#[test]
fn failed_process_output_or_mutation_cannot_return_successful_evidence() {
    for behavior in [Behavior::Missing, Behavior::Empty, Behavior::Malformed, Behavior::Nonzero,
        Behavior::Timeout, Behavior::Cancelled, Behavior::Truncated, Behavior::WrongVersion,
        Behavior::VersionStderr, Behavior::VersionTruncated, Behavior::PrecreatedOutput,
        Behavior::MutateNode, Behavior::MutatePackage, Behavior::MutateSource,
        Behavior::MutateConfig, Behavior::MutateOriginal] {
        let (_directory, request) = fixture();
        let runner = FakeRunner::new(behavior, &request);
        assert!(render_with_runner(&request, &runner, ProcessCancellationToken::new()).is_err());
    }
}

#[test]
fn wrong_paper_is_refused_by_independent_output_inspection() {
    let (_directory, mut request) = fixture();
    request.config.profile = ScreenplayPdfProfile::A4;
    let runner = FakeRunner::new(Behavior::WrongPaper, &request);
    assert!(matches!(render_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(AfterwritingError::InvalidOutput(_))));
}

#[test]
fn pins_fail_before_a_process_can_run() {
    for field in ["node", "package", "source"] {
        let (_directory, mut request) = fixture();
        match field { "node" => request.config.node_sha256 = "0".repeat(64),
            "package" => request.config.package_tree_sha256 = "0".repeat(64),
            _ => request.expected_source_sha256 = Some("0".repeat(64)) }
        let runner = FakeRunner::new(Behavior::Good, &request);
        assert!(render_with_runner(&request, &runner, ProcessCancellationToken::new()).is_err());
        assert_eq!(runner.calls(), 0);
    }
}

#[test]
fn unsupported_fountain_is_refused_before_any_native_probe() {
    for source in ["Title: A title\n\nINT. LAB - DAY\n", "INT. LAB - DAY\n\nCaf\u{e9}.\n", "INT. LAB - DAY\n\nAn <?xml fragment.\n"] {
        let (_directory, mut request) = fixture();
        fs::write(&request.input, source).unwrap();
        request.expected_source_sha256 = None;
        let runner = FakeRunner::new(Behavior::Good, &request);
        assert!(matches!(render_with_runner(&request, &runner, ProcessCancellationToken::new()), Err(AfterwritingError::InvalidInput(_))));
        assert_eq!(runner.calls(), 0);
    }
}

#[test]
fn missing_dependency_entrypoint_or_font_metrics_blocks_preflight() {
    for relative in ["awc.js", "js/bootstrap.js", "js/client/awrequire.js", "node_modules/aw-parser/package.json", "node_modules/stdio/index.js",
        "node_modules/pdfkit/js/pdfkit.js", "node_modules/pdfkit/js/data/Courier.afm", "node_modules/pdfkit/js/data/Helvetica.afm"] {
        let (_directory, mut request) = fixture();
        fs::remove_file(request.config.package_root.join(relative)).unwrap();
        repin(&mut request);
        let runner = FakeRunner::new(Behavior::Good, &request);
        assert!(preflight_with_runner(&request.config, &runner, ProcessCancellationToken::new()).is_err());
        assert_eq!(runner.calls(), 0);
    }
}

#[test]
fn wrong_package_identity_is_not_repaired_by_declaring_a_new_tree_pin() {
    for field in ["name", "version", "bin", "dependencies"] {
        let (_directory, mut request) = fixture();
        let manifest_path = request.config.package_root.join("package.json");
        let mut value: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
        value[field] = json!("wrong");
        fs::write(manifest_path, serde_json::to_vec(&value).unwrap()).unwrap();
        repin(&mut request);
        let runner = FakeRunner::new(Behavior::Good, &request);
        assert!(preflight_with_runner(&request.config, &runner, ProcessCancellationToken::new()).is_err());
        assert_eq!(runner.calls(), 0);
    }
}

#[test]
fn configured_preflight_only_probes_node_and_never_renders() {
    let (_directory, request) = fixture();
    let runner = FakeRunner::new(Behavior::Good, &request);
    let result = preflight_with_runner(&request.config, &runner, ProcessCancellationToken::new()).unwrap();
    assert_eq!(runner.calls(), 1);
    assert_eq!(result.node_version, request.config.expected_node_version);
    assert_eq!(result.provider_version, AFTERWRITING_VERSION);
    assert_eq!(result.package_tree_sha256, request.config.package_tree_sha256);
}

#[test]
fn exact_node_versions_optin_and_time_bounds_are_required() {
    let (_directory, mut request) = fixture();
    for value in ["22.1.0", "v20.1.0", "v23.1.0", "v22.01.0", "v22.1.0-beta", "v24.x.y", "v22.1.0\n", "v22.4294967296.0"] {
        assert!(!supported_node_version(value));
    }
    for value in ["v22.0.0", "v24.1.2"] { assert!(supported_node_version(value)); }
    request.config.allow_unqualified = false;
    assert!(request.config.validate().is_err());
    request.config.allow_unqualified = true;
    for timeout in [0, 301] { request.config.timeout_seconds = timeout; assert!(request.config.validate().is_err()); }
}

#[test]
fn custom_fonts_and_provider_options_have_no_config_escape_hatch() {
    let (_directory, request) = fixture();
    for name in ["font_family", "provider_arguments", "environment", "download", "overwrite"] {
        let mut value = serde_json::to_value(&request.config).unwrap();
        value[name] = json!("unexpected");
        assert!(serde_json::from_value::<AfterwritingConfig>(value).is_err());
    }
}

#[test]
fn cancellation_precedes_files_and_processes() {
    let (_directory, request) = fixture();
    fs::remove_file(&request.input).unwrap();
    let cancellation = ProcessCancellationToken::new();
    cancellation.cancel();
    let runner = FakeRunner::new(Behavior::Good, &request);
    assert!(matches!(render_with_runner(&request, &runner, cancellation), Err(AfterwritingError::Cancelled)));
    assert_eq!(runner.calls(), 0);
}

#[test]
fn tree_digest_covers_paths_content_and_empty_files_but_only_documented_exclusions() {
    let (_directory, request) = fixture();
    let root = &request.config.package_root;
    let before = package_fingerprint(root, ProcessCancellationToken::new()).unwrap();
    write(root, ".git/config", b"excluded");
    write(root, "node_modules/.bin/afterwriting", b"excluded");
    assert_eq!(package_fingerprint(root, ProcessCancellationToken::new()).unwrap(), before);
    write(root, "fakenode_modules/.bin/entry", b"included");
    let additional = package_fingerprint(root, ProcessCancellationToken::new()).unwrap();
    assert_ne!(additional.tree_sha256, before.tree_sha256);
    write(root, "empty", b"");
    let empty = package_fingerprint(root, ProcessCancellationToken::new()).unwrap();
    assert_ne!(empty.tree_sha256, additional.tree_sha256);
    assert_eq!(empty.total_bytes, additional.total_bytes);
    assert_eq!(empty.file_count, additional.file_count + 1);
    fs::rename(root.join("empty"), root.join("renamed-empty")).unwrap();
    assert_ne!(package_fingerprint(root, ProcessCancellationToken::new()).unwrap().tree_sha256, empty.tree_sha256);
}

#[test]
fn staged_copy_is_snapshot_equivalent_and_original_additions_are_detected() {
    let (_directory, request) = fixture();
    let cancellation = ProcessCancellationToken::new();
    let snapshot = TreeSnapshot::read(&request.config.package_root, &cancellation).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let staged = snapshot.stage(&fs::canonicalize(temporary.path()).unwrap().join("package"), &cancellation).unwrap();
    assert_eq!(snapshot.fingerprint, staged.fingerprint);
    write(&request.config.package_root, "new-file.js", b"changed");
    assert!(snapshot.recheck(&cancellation).is_err());
}

#[cfg(unix)]
#[test]
fn symlink_files_roots_outputs_and_control_names_are_refused() {
    let (_directory, request) = fixture();
    let root = &request.config.package_root;
    let link = root.parent().unwrap().join("linked-package");
    std::os::unix::fs::symlink(root, &link).unwrap();
    assert!(package_fingerprint(&link, ProcessCancellationToken::new()).is_err());
    std::os::unix::fs::symlink(root.join("awc.js"), root.join("linked.js")).unwrap();
    assert!(package_fingerprint(root, ProcessCancellationToken::new()).is_err());
    fs::remove_file(root.join("linked.js")).unwrap();
    write(root, "newline\n.js", b"invalid name");
    assert!(package_fingerprint(root, ProcessCancellationToken::new()).is_err());
    fs::remove_file(root.join("newline\n.js")).unwrap();
    let runner = FakeRunner::new(Behavior::SymlinkOutput, &request);
    assert!(render_with_runner(&request, &runner, ProcessCancellationToken::new()).is_err());
}
