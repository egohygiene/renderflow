//! Synthetic, unconditional fixed-layout EPUB generation coverage. All source
//! artwork is tiny and redistributable; no external renderer is involved.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{atomic::AtomicBool, Arc};

use renderflow::ebook::{
    inspect_ebook, inspect_ebook_with_manifest, EbookCapabilityContract, EbookDiagnosticSeverity,
    EbookLayout, EbookProvenanceStatus,
};
use renderflow::evidence::{sha256_serialized, ArtifactRole, RunState, StepState};
use renderflow::planning::{execute, resolve, CanonicalExecutionResult, PlanningRequest};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

const PNG_FIRST: &[u8] = include_bytes!("fixtures/print-pdf/page-001.png");
const PNG_SECOND: &[u8] = include_bytes!("fixtures/print-pdf/page-002.png");
const JPEG_FIRST: &[u8] = include_bytes!("fixtures/print-pdf/page-001.jpg");
const JPEG_SECOND: &[u8] = include_bytes!("fixtures/print-pdf/page-002.jpg");
const SVG_FIRST: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect width="100" height="100"/></svg>"#;
const SVG_SECOND: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><circle cx="50" cy="50" r="30"/></svg>"#;

struct Fixture {
    dir: TempDir,
    config: PathBuf,
    originals: [Vec<u8>; 2],
    extension: &'static str,
}

impl Fixture {
    fn new(format: &str, direction: &str) -> Self {
        let (extension, media_type, pages): (&str, &str, [&[u8]; 2]) = match format {
            "png" => ("png", "image/png", [PNG_FIRST, PNG_SECOND]),
            "jpeg" => ("jpg", "image/jpeg", [JPEG_FIRST, JPEG_SECOND]),
            "svg" => ("svg", "image/svg+xml", [SVG_FIRST, SVG_SECOND]),
            other => panic!("unsupported synthetic format: {other}"),
        };
        let dir = tempfile::tempdir().unwrap();
        let originals = [pages[0].to_vec(), pages[1].to_vec()];
        let mut source_specs = String::new();
        let mut artwork = String::new();
        for (index, bytes) in pages.iter().enumerate() {
            let number = index + 1;
            let filename = format!("page-{number:03}.{extension}");
            fs::write(dir.path().join(&filename), bytes).unwrap();
            let digest = format!("{:x}", Sha256::digest(bytes));
            source_specs.push_str(&format!(
                "  - id: source.page{number:03}\n    path: {filename}\n    format: {format}\n    media_type: {media_type}\n    sha256: \"{digest}\"\n    geometry: {{ width: 90, height: 90, unit: mm }}\n"
            ));
            artwork.push_str(&format!(
                "    - role: page\n      path: {filename}\n      alt_text: Synthetic page {number} with a geometric mark.\n"
            ));
        }
        let config = dir.path().join("renderflow.yaml");
        fs::write(
            &config,
            format!(
                "schema: renderflow/v2\nsources:\n{source_specs}  - id: source.pages\n    kind: collection\n    members: [source.page001, source.page002]\npublication:\n  schema: renderflow.publication/v1\n  publication: Synthetic Fixture Editions\n  issue_id: synthetic-fixed-001\n  title: Synthetic Fixed EPUB\n  contributors:\n    - name: Renderflow Contributors\n      role: author\n  publication_date: \"2026-09-27\"\n  language: en-US\n  geometry: {{ width: 90, height: 90, unit: mm }}\n  artwork:\n{artwork}  rights:\n    license: CC0-1.0\n    rights_holder: Renderflow Contributors\n    reviewed: true\n  accessibility:\n    summary: Two synthetic geometric pages with alternative descriptions.\n    access_modes: [visual]\n    hazards: [none]\n  output_roles:\n    ebook:\n      format: epub\n      stage: candidate\ntargets:\n  exact:\n    - id: target.ebook\n      role: ebook\n      format: epub\n      requirement: required\nexecution:\n  fixed_layout_epub:\n    page_progression_direction: {direction}\n    spread: none\n    cover_member_id: source.page001\n    max_pages: 2\n    max_input_bytes: 1000000\n    max_output_bytes: 5000000\noutput:\n  bundle_root: \"{}\"\n  naming_template: \"{{source.id}}/{{target.role}}.{{ext}}\"\n",
                dir.path().join("dist").display()
            ),
        )
        .unwrap();
        Self {
            dir,
            config,
            originals,
            extension: match format {
                "jpeg" => "jpg",
                "svg" => "svg",
                _ => "png",
            },
        }
    }

    fn source(&self, index: usize) -> PathBuf {
        self.dir
            .path()
            .join(format!("page-{:03}.{}", index + 1, self.extension))
    }

    fn rewrite(&self, before: &str, after: &str) {
        let yaml = fs::read_to_string(&self.config).unwrap();
        assert!(yaml.contains(before), "test replacement missing: {before}");
        fs::write(&self.config, yaml.replacen(before, after, 1)).unwrap();
    }

    fn unchanged(&self) {
        for index in 0..2 {
            assert_eq!(
                fs::read(self.source(index)).unwrap(),
                self.originals[index],
                "source artwork was mutated"
            );
        }
    }
}

fn resolve_error(fixture: &Fixture, code: &str) {
    let error = format!(
        "{:#}",
        resolve(PlanningRequest::from_path(&fixture.config))
            .err()
            .expect("invalid fixture unexpectedly planned")
    );
    assert!(error.contains(code), "expected {code}, got: {error}");
    assert!(!fixture.dir.path().join("dist").exists());
    fixture.unchanged();
}

fn zip_text(archive: &mut ZipArchive<File>, name: &str) -> String {
    let mut text = String::new();
    archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("missing ZIP member {name}"))
        .read_to_string(&mut text)
        .unwrap();
    text
}

fn zip_bytes(archive: &mut ZipArchive<File>, name: &str) -> Vec<u8> {
    let mut bytes = Vec::new();
    archive
        .by_name(name)
        .unwrap_or_else(|_| panic!("missing ZIP member {name}"))
        .read_to_end(&mut bytes)
        .unwrap();
    bytes
}

fn published_epub(result: &CanonicalExecutionResult) -> &Path {
    let epubs = result
        .outputs
        .iter()
        .filter(|output| output.ends_with("/ebook.epub"))
        .collect::<Vec<_>>();
    assert_eq!(epubs.len(), 1, "expected exactly one EPUB output");
    let path = Path::new(epubs[0]);
    assert!(path.is_file(), "EPUB missing at {}", path.display());
    path
}

fn rewrite_member(members: &mut [(String, Vec<u8>)], name: &str, before: &str, after: &str) {
    let content = members
        .iter_mut()
        .find(|(path, _)| path == name)
        .unwrap_or_else(|| panic!("missing ZIP member {name}"));
    let text = String::from_utf8(content.1.clone()).unwrap();
    assert!(
        text.contains(before),
        "missing replacement marker in {name}: {before}"
    );
    content.1 = text.replacen(before, after, 1).into_bytes();
}

fn tampered_epub(
    source: &Path,
    destination: &Path,
    change: impl FnOnce(&mut Vec<(String, Vec<u8>)>),
) {
    let mut original = ZipArchive::new(File::open(source).unwrap()).unwrap();
    let mut members = Vec::new();
    for index in 0..original.len() {
        let mut entry = original.by_index(index).unwrap();
        let name = entry.name().to_string();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        members.push((name, bytes));
    }
    change(&mut members);
    let mut archive = ZipWriter::new(File::create(destination).unwrap());
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    for (name, bytes) in members {
        archive.start_file(name, options).unwrap();
        std::io::Write::write_all(&mut archive, &bytes).unwrap();
    }
    archive.finish().unwrap();
}

fn assert_publication_sidecars(result: &CanonicalExecutionResult, fixture: &Fixture) {
    let metadata = fixture.dir.path().join("dist/metadata");
    for name in [
        "publication.json",
        "manifest.json",
        "provenance.json",
        "preflight.json",
        "checksums.sha256",
    ] {
        let path = metadata.join(name);
        assert!(
            path.is_file(),
            "publication sidecar missing: {}",
            path.display()
        );
        assert!(
            result
                .outputs
                .iter()
                .any(|output| Path::new(output) == path),
            "publication sidecar absent from output evidence: {}",
            path.display()
        );
    }
}

fn expected_members(extension: &str) -> Vec<String> {
    let mut names = vec![
        "mimetype".to_string(),
        "META-INF/container.xml".to_string(),
        "EPUB/book.opf".to_string(),
        "EPUB/nav.xhtml".to_string(),
        "EPUB/styles.css".to_string(),
    ];
    for number in 1..=2 {
        names.push(format!("EPUB/pages/page-{number:04}.xhtml"));
        names.push(format!("EPUB/images/page-{number:04}.{extension}"));
    }
    names
}

fn inspect_generated_epub(path: &Path, fixture: &Fixture, direction: &str) {
    let mut archive = ZipArchive::new(File::open(path).unwrap()).unwrap();
    let members = (0..archive.len())
        .map(|index| {
            let entry = archive.by_index(index).unwrap();
            assert_eq!(entry.compression(), CompressionMethod::Stored);
            let modified = entry.last_modified().expect("ZIP timestamp is present");
            assert_eq!(modified.year(), 1980);
            assert_eq!(modified.month(), 1);
            assert_eq!(modified.day(), 1);
            entry.name().to_string()
        })
        .collect::<Vec<_>>();
    assert_eq!(members, expected_members(fixture.extension));
    assert_eq!(zip_bytes(&mut archive, "mimetype"), b"application/epub+zip");
    let container = zip_text(&mut archive, "META-INF/container.xml");
    assert!(container.contains("full-path=\"EPUB/book.opf\""));

    let opf = zip_text(&mut archive, "EPUB/book.opf");
    assert!(opf.contains("version=\"3.0\""), "{opf}");
    assert!(opf.contains("pre-paginated"), "{opf}");
    assert!(opf.contains("Synthetic Fixed EPUB"), "{opf}");
    assert!(opf.contains("synthetic-fixed-001"), "{opf}");
    assert!(opf.contains("en-US"), "{opf}");
    assert!(opf.contains("Renderflow Contributors"), "{opf}");
    assert!(opf.contains("CC0-1.0"), "{opf}");
    assert!(opf.contains("schema:accessMode"), "{opf}");
    assert!(opf.contains("schema:accessibilitySummary"), "{opf}");
    assert!(opf.contains(&format!("page-progression-direction=\"{direction}\"")));
    assert!(opf.contains("properties=\"nav\""), "{opf}");
    assert!(opf.contains("properties=\"cover-image\""), "{opf}");
    assert_eq!(opf.matches("properties=\"cover-image\"").count(), 1);
    assert!(opf.contains(&format!(
        "id=\"image-0001\" href=\"images/page-0001.{}\"",
        fixture.extension
    )));
    let first_opf = opf.find("pages/page-0001.xhtml").unwrap();
    let second_opf = opf.find("pages/page-0002.xhtml").unwrap();
    assert!(
        first_opf < second_opf,
        "manifest order differs from source order"
    );
    let first_spine = opf.find("<itemref").unwrap();
    let second_spine = opf[first_spine + 1..].find("<itemref").unwrap() + first_spine + 1;
    assert!(first_spine < second_spine && opf[second_spine + 1..].find("<itemref").is_none());
    assert!(
        opf[first_spine..second_spine].contains("idref=\"page-0001\""),
        "first spine item must identify page 1"
    );
    assert!(
        opf[second_spine..].contains("idref=\"page-0002\""),
        "second spine item must identify page 2"
    );

    let nav = zip_text(&mut archive, "EPUB/nav.xhtml");
    assert!(nav.contains("toc"), "{nav}");
    assert!(nav.contains("page-list"), "{nav}");
    let (toc, page_list) = nav.split_once("page-list").unwrap();
    for (name, section) in [("toc", toc), ("page-list", page_list)] {
        assert!(
            section.find("pages/page-0001.xhtml").unwrap()
                < section.find("pages/page-0002.xhtml").unwrap(),
            "{name} order differs from source order"
        );
    }
    for index in 0..2 {
        let number = index + 1;
        let xhtml = zip_text(&mut archive, &format!("EPUB/pages/page-{number:04}.xhtml"));
        assert!(xhtml.contains("viewport"), "{xhtml}");
        assert!(
            xhtml.contains("content=\"width=100, height=100\""),
            "{xhtml}"
        );
        assert!(xhtml.contains(&format!(
            "src=\"../images/page-{number:04}.{}\"",
            fixture.extension
        )));
        assert!(
            xhtml.contains(&format!("Synthetic page {number} with a geometric mark.")),
            "alt text missing in page {number}: {xhtml}"
        );
        assert_eq!(
            zip_bytes(
                &mut archive,
                &format!("EPUB/images/page-{number:04}.{}", fixture.extension)
            ),
            fixture.originals[index],
            "embedded image differs from the source artwork"
        );
    }
}

#[test]
fn generated_epub_is_deterministic_and_tracks_ordered_artwork() {
    for format in ["png", "jpeg"] {
        let mut outputs = Vec::new();
        for _ in 0..2 {
            let fixture = Fixture::new(format, "ltr");
            let resolved = resolve(PlanningRequest::from_path(&fixture.config)).unwrap();
            let collection = resolved.plan().source_collection.as_ref().unwrap();
            assert_eq!(collection.members.len(), 2);
            assert_eq!(collection.members[0].source_id, "source.page001");
            assert_eq!(collection.members[1].source_id, "source.page002");
            assert!(resolved.plan().edges.iter().any(|edge| {
                edge.capability_id.as_deref() == Some("ebook.generate.epub.fixed-layout")
            }));
            let dry_run = execute(
                resolve(PlanningRequest::from_path(&fixture.config)).unwrap(),
                true,
            )
            .unwrap();
            assert_eq!(dry_run.run_manifest.state, RunState::Planned);
            assert!(!fixture.dir.path().join("dist").exists());

            let result = execute(resolved, false).unwrap();
            assert_eq!(
                result.run_manifest.state,
                RunState::Complete,
                "{:?}",
                result.run_manifest.diagnostics
            );
            let epub = published_epub(&result);
            assert_eq!(result.outputs.len(), 6);
            assert_publication_sidecars(&result, &fixture);
            inspect_generated_epub(epub, &fixture, "ltr");
            let evidence = &result.run_manifest.artifact_manifest.artifacts;
            let source = evidence
                .iter()
                .filter(|entry| entry.lifecycle == ArtifactRole::Source)
                .collect::<Vec<_>>();
            assert_eq!(source.len(), 2);
            let ebook = evidence.iter().find(|entry| entry.role == "ebook").unwrap();
            assert_eq!(
                ebook.sources,
                source
                    .iter()
                    .map(|item| item.artifact_id.clone())
                    .collect::<Vec<_>>()
            );
            assert_eq!(source[0].metadata["renderflow.collection.index"], 0);
            assert_eq!(source[1].metadata["renderflow.collection.index"], 1);
            fixture.unchanged();
            outputs.push(fs::read(epub).unwrap());
        }
        assert_eq!(outputs[0], outputs[1], "{format} clean builds differ");
    }
}

#[test]
fn fixed_layout_refuses_unsupported_sources_geometry_metadata_and_bounds() {
    let svg = Fixture::new("svg", "ltr");
    resolve_error(&svg, "fixed_epub.input.format");
    for (before, after, code) in [
        ("width: 90", "width: 91", "fixed_epub.geometry"),
        (
            "max_input_bytes: 1000000",
            "max_input_bytes: 1",
            "fixed_epub.bounds",
        ),
        ("role: ebook", "role: web", "fixed_epub.target"),
        (
            "contributors:\n    - name: Renderflow Contributors\n      role: author",
            "contributors: []",
            "fixed_epub.metadata",
        ),
        (
            "alt_text: Synthetic page 1 with a geometric mark.",
            "alt_text: \"\"",
            "fixed_epub.metadata",
        ),
    ] {
        let fixture = Fixture::new("png", "ltr");
        fixture.rewrite(before, after);
        resolve_error(&fixture, code);
    }
}

#[test]
fn malformed_and_active_svg_sources_are_refused_without_parsing_or_publication() {
    // SVG is outside the native image-only EPUB route. These inputs remain
    // prohibited even if their source digest is honestly declared; this test
    // does not claim to validate or sanitize any SVG/XML payload.
    let cases: &[(&str, &[u8])] = &[
        ("malformed", b"<svg><path"),
        (
            "external_reference",
            br#"<svg xmlns="http://www.w3.org/2000/svg"><image href="https://example.invalid/remote.png"/></svg>"#,
        ),
        (
            "script",
            br#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#,
        ),
        (
            "external_entity",
            br#"<!DOCTYPE svg [<!ENTITY remote SYSTEM "file:///tmp/fixture-secret">]><svg xmlns="http://www.w3.org/2000/svg">&remote;</svg>"#,
        ),
    ];
    for (name, bytes) in cases {
        let fixture = Fixture::new("svg", "ltr");
        fs::write(fixture.source(0), bytes).unwrap();
        fixture.rewrite(
            &format!("{:x}", Sha256::digest(SVG_FIRST)),
            &format!("{:x}", Sha256::digest(bytes)),
        );
        let error = format!(
            "{:#}",
            resolve(PlanningRequest::from_path(&fixture.config))
                .err()
                .unwrap_or_else(|| panic!("{name} SVG unexpectedly planned"))
        );
        assert!(
            error.contains("fixed_epub.input.format") || error.contains("collection.member."),
            "{name} SVG should be rejected by the format boundary or typed intake: {error}"
        );
        assert!(!fixture.dir.path().join("dist").exists());
        assert_eq!(fs::read(fixture.source(0)).unwrap(), *bytes);
        assert_eq!(fs::read(fixture.source(1)).unwrap(), SVG_SECOND);
    }
}

#[test]
fn output_byte_limit_prevents_publication() {
    let fixture = Fixture::new("png", "ltr");
    fixture.rewrite("max_output_bytes: 5000000", "max_output_bytes: 1");
    let result = execute(
        resolve(PlanningRequest::from_path(&fixture.config)).unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(result.run_manifest.state, RunState::Failed);
    assert!(result
        .outputs
        .iter()
        .all(|output| !output.ends_with(".epub")));
    assert_publication_sidecars(&result, &fixture);
    assert!(!fixture
        .dir
        .path()
        .join("dist/source.pages/ebook.epub")
        .exists());
    assert!(result.run_manifest.steps.iter().any(|step| {
        step.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.starts_with("fixed_epub."))
    }));
    fixture.unchanged();
}

#[test]
fn changed_artwork_after_planning_never_publishes_epub() {
    let fixture = Fixture::new("png", "ltr");
    let resolved = resolve(PlanningRequest::from_path(&fixture.config)).unwrap();
    fs::write(fixture.source(1), b"changed after planning").unwrap();
    let result = execute(resolved, false).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Failed);
    assert!(result.outputs.is_empty());
    assert!(result
        .run_manifest
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == "execution.source_changed_after_intake" }));
    assert!(!fixture
        .dir
        .path()
        .join("dist/source.pages/ebook.epub")
        .exists());
    assert_eq!(fs::read(fixture.source(0)).unwrap(), fixture.originals[0]);
}

#[test]
fn cancellation_before_first_wave_records_no_epub() {
    let fixture = Fixture::new("png", "ltr");
    let cancellation = Arc::new(AtomicBool::new(true));
    let resolved = resolve(PlanningRequest::from_path(&fixture.config))
        .unwrap()
        .with_cancellation_flag(cancellation);
    let result = execute(resolved, false).unwrap();
    assert_eq!(result.run_manifest.state, RunState::Cancelled);
    assert!(result.outputs.iter().all(|path| !path.ends_with(".epub")));
    assert!(!fixture
        .dir
        .path()
        .join("dist/source.pages/ebook.epub")
        .exists());
    assert_eq!(result.run_manifest.steps.len(), 1);
    let step = &result.run_manifest.steps[0];
    assert_eq!(step.state, StepState::Cancelled);
    assert!(step
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "execution.step_cancelled"));
    assert!(Path::new(result.manifest_path.as_ref().unwrap()).is_file());
    fixture.unchanged();
}

#[test]
fn right_to_left_direction_is_structural_and_does_not_mirror_artwork() {
    let fixture = Fixture::new("png", "rtl");
    let result = execute(
        resolve(PlanningRequest::from_path(&fixture.config)).unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete);
    inspect_generated_epub(published_epub(&result), &fixture, "rtl");
    fixture.unchanged();
}

#[test]
fn metadata_and_page_order_change_the_execution_plan_identity() {
    let fixture = Fixture::new("png", "ltr");
    let initial = resolve(PlanningRequest::from_path(&fixture.config)).unwrap();
    let initial_digest = sha256_serialized(initial.plan()).unwrap().value;
    fixture.rewrite("Synthetic Fixed EPUB", "Synthetic Fixed EPUB Revised");
    let metadata_plan = resolve(PlanningRequest::from_path(&fixture.config)).unwrap();
    let metadata_digest = sha256_serialized(metadata_plan.plan()).unwrap().value;
    assert_ne!(
        initial_digest, metadata_digest,
        "metadata not bound to the plan"
    );

    let reordered = Fixture::new("png", "ltr");
    reordered.rewrite(
        "members: [source.page001, source.page002]",
        "members: [source.page002, source.page001]",
    );
    reordered.rewrite(
        "cover_member_id: source.page001",
        "cover_member_id: source.page002",
    );
    let reordered_plan = resolve(PlanningRequest::from_path(&reordered.config)).unwrap();
    let order_digest = sha256_serialized(reordered_plan.plan()).unwrap().value;
    assert_ne!(
        initial_digest, order_digest,
        "member order not bound to the plan"
    );
}

#[test]
fn native_inspection_proves_generated_package_and_honest_capabilities() {
    for (format, direction) in [("png", "ltr"), ("jpeg", "rtl")] {
        let fixture = Fixture::new(format, direction);
        let result = execute(
            resolve(PlanningRequest::from_path(&fixture.config)).unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(result.run_manifest.state, RunState::Complete);
        let inspection = inspect_ebook(published_epub(&result), false).unwrap();
        assert!(inspection.valid, "{:?}", inspection.diagnostics);
        assert_eq!(inspection.layout, EbookLayout::PrePaginated);
        assert_eq!(inspection.spine_items, 2);
        assert_eq!(inspection.xhtml_documents, 3);
        assert!(inspection.navigation);
        assert!(inspection.page_list);
        assert!(inspection.metadata.title);
        assert!(inspection.metadata.creator);
        assert!(inspection.metadata.language);
        assert!(inspection.metadata.identifier);
        assert!(inspection.metadata.rights);
        assert!(inspection.accessibility.accessibility_summary);
        assert!(inspection.accessibility.access_modes > 0);
        assert!(inspection.accessibility.accessibility_features > 0);
        assert!(inspection.accessibility.accessibility_hazards > 0);
        assert!(inspection.epubcheck.is_none());
        fixture.unchanged();
    }

    let capability = EbookCapabilityContract::builtin();
    assert_eq!(capability.schema, "renderflow.ebook-capabilities/v1");
    assert!(capability.epub_fixed_layout_generation);
    assert!(!capability.kepub_fixed_layout_generation);
    let route = capability.fixed_layout_epub_route.as_ref().unwrap();
    assert_eq!(route.capability, "ebook.generate.epub.fixed-layout");
    assert_eq!(route.provider_id, "tool.renderflow-epub");
    assert_eq!(
        route.source_formats,
        ["png".to_string(), "jpeg".to_string()]
    );
    assert_eq!(route.target_format, "epub");
    assert!(route.ordered_collection_required);
    assert!(route.homogeneous_local_sources_required);
    assert!(route.explicit_execution_policy_required);
    assert_eq!(
        route.page_progression_directions,
        ["ltr".to_string(), "rtl".to_string()]
    );
    assert_eq!(route.spread_policies, ["none".to_string()]);
    assert!(route.native_validation);
    assert!(route.optional_epubcheck_v5);
}

#[test]
fn native_inspection_rejects_adversarial_package_mutations() {
    let fixture = Fixture::new("png", "ltr");
    let result = execute(
        resolve(PlanningRequest::from_path(&fixture.config)).unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete);
    let generated = published_epub(&result);
    let cases = [
        ("duplicate_spine_item", "spine"),
        ("missing_page_list", "navigation"),
        ("reversed_page_list", "navigation"),
        ("missing_cover", "cover"),
        ("wrong_cover", "cover"),
        ("wrong_viewport", "viewport"),
        ("empty_alt_text", "accessibility"),
        ("missing_accessibility_summary", "accessibility"),
        ("active_page_head", "unsafe_markup"),
        ("upper_case_css_url", "unsafe_markup"),
        ("unsafe_script", "unsafe_markup"),
        ("external_image", "unsafe_markup"),
        ("external_srcset", "unsafe_markup"),
        ("external_xml_base", "unsafe_markup"),
        ("inline_style", "unsafe_markup"),
        ("malformed_xml", "xml"),
        ("external_entity", "unsafe_markup"),
        ("missing_image", "resource"),
        ("unreadable_image", "resource"),
        ("duplicate_zip_member", "member"),
        ("path_traversal", "member"),
        ("wrong_mimetype", "mimetype"),
        ("mimetype_not_first", "mimetype"),
    ];
    for (case, expected_code) in cases {
        let tampered = fixture.dir.path().join(format!("tampered-{case}.epub"));
        tampered_epub(generated, &tampered, |members| {
            match case {
            "duplicate_spine_item" => rewrite_member(
                members,
                "EPUB/book.opf",
                "<itemref idref=\"page-0002\"/>",
                "<itemref idref=\"page-0001\"/>",
            ),
            "missing_page_list" => rewrite_member(
                members,
                "EPUB/nav.xhtml",
                "<nav epub:type=\"page-list\">",
                "<nav epub:type=\"landmarks\">",
            ),
            "reversed_page_list" => rewrite_member(
                members,
                "EPUB/nav.xhtml",
                "<li><a href=\"pages/page-0001.xhtml\">1</a></li>\n<li><a href=\"pages/page-0002.xhtml\">2</a></li>",
                "<li><a href=\"pages/page-0002.xhtml\">2</a></li>\n<li><a href=\"pages/page-0001.xhtml\">1</a></li>",
            ),
            "missing_cover" => rewrite_member(
                members,
                "EPUB/book.opf",
                " properties=\"cover-image\"",
                "",
            ),
            "wrong_cover" => {
                rewrite_member(members, "EPUB/book.opf", " properties=\"cover-image\"", "");
                rewrite_member(
                    members,
                    "EPUB/book.opf",
                    "href=\"images/page-0002.png\" media-type=\"image/png\"",
                    "href=\"images/page-0002.png\" media-type=\"image/png\" properties=\"cover-image\"",
                );
            }
            "wrong_viewport" => rewrite_member(
                members,
                "EPUB/pages/page-0001.xhtml",
                "content=\"width=100, height=100\"",
                "content=\"width=900, height=100\"",
            ),
            "empty_alt_text" => rewrite_member(
                members,
                "EPUB/pages/page-0001.xhtml",
                "alt=\"Synthetic page 1 with a geometric mark.\"",
                "alt=\"\"",
            ),
            "missing_accessibility_summary" => rewrite_member(
                members,
                "EPUB/book.opf",
                "property=\"schema:accessibilitySummary\"",
                "property=\"schema:alternateName\"",
            ),
            "active_page_head" => rewrite_member(
                members,
                "EPUB/pages/page-0001.xhtml",
                "</head>",
                "<style>@import url(https://example.invalid/stylesheet.css);</style></head>",
            ),
            "upper_case_css_url" => rewrite_member(
                members,
                "EPUB/styles.css",
                "object-fit:contain;",
                "object-fit:contain; background:URL(https://example.invalid/tracker.png);",
            ),
            "unsafe_script" => rewrite_member(
                members,
                "EPUB/pages/page-0001.xhtml",
                "</body>",
                "<script>alert(1)</script></body>",
            ),
            "external_image" => rewrite_member(
                members,
                "EPUB/pages/page-0001.xhtml",
                "src=\"../images/page-0001.png\"",
                "src=\"https://example.invalid/tracker.png\"",
            ),
            "external_srcset" => rewrite_member(
                members,
                "EPUB/pages/page-0001.xhtml",
                "<img class=\"page\"",
                "<img srcset=\"https://example.invalid/tracker.png 1x\" class=\"page\"",
            ),
            "external_xml_base" => rewrite_member(
                members,
                "EPUB/pages/page-0001.xhtml",
                "<html xmlns=\"http://www.w3.org/1999/xhtml\"",
                "<html xml:base=\"https://example.invalid/\" xmlns=\"http://www.w3.org/1999/xhtml\"",
            ),
            "inline_style" => rewrite_member(
                members,
                "EPUB/pages/page-0001.xhtml",
                "<img class=\"page\"",
                "<img style=\"background:url(https://example.invalid/tracker.png)\" class=\"page\"",
            ),
            "malformed_xml" => rewrite_member(
                members,
                "EPUB/pages/page-0001.xhtml",
                "</html>",
                "</broken>",
            ),
            "external_entity" => rewrite_member(
                members,
                "EPUB/book.opf",
                "<package ",
                "<!DOCTYPE package [<!ENTITY leaked SYSTEM \"file:///tmp/fixture-secret\">]><package ",
            ),
            "missing_image" => members.retain(|(name, _)| name != "EPUB/images/page-0002.png"),
            "unreadable_image" => {
                members
                    .iter_mut()
                    .find(|(name, _)| name == "EPUB/images/page-0002.png")
                    .unwrap()
                    .1 = b"not a PNG image".to_vec();
            }
            "duplicate_zip_member" => {
                let original = members
                    .iter()
                    .find(|(name, _)| name == "EPUB/pages/page-0001.xhtml")
                    .unwrap()
                    .clone();
                members.push(("EPUB/pages/page-0099.xhtml".to_string(), original.1));
            }
            "path_traversal" => members.push(("../outside.xhtml".to_string(), b"<html/>".to_vec())),
            "wrong_mimetype" => members[0].1 = b"application/epub+zip\n".to_vec(),
            "mimetype_not_first" => members.swap(0, 1),
            _ => unreachable!(),
        }
        });
        if case == "duplicate_zip_member" {
            // ZipWriter refuses duplicate names. Rewrite the equal-length
            // local and central filenames in a completed, otherwise valid ZIP.
            let mut bytes = fs::read(&tampered).unwrap();
            let marker = b"EPUB/pages/page-0099.xhtml";
            let duplicate = b"EPUB/pages/page-0001.xhtml";
            let positions = bytes
                .windows(marker.len())
                .enumerate()
                .filter_map(|(index, window)| (window == marker).then_some(index))
                .collect::<Vec<_>>();
            assert_eq!(positions.len(), 2);
            for index in positions {
                bytes[index..index + marker.len()].copy_from_slice(duplicate);
            }
            fs::write(&tampered, bytes).unwrap();
        }
        let inspection = inspect_ebook(&tampered, false).unwrap();
        let errors = inspection
            .diagnostics
            .iter()
            .filter(|item| item.severity == EbookDiagnosticSeverity::Error)
            .collect::<Vec<_>>();
        assert!(
            !inspection.valid && !errors.is_empty(),
            "{case} unexpectedly passed native inspection: fixed={:?}, diagnostics={:?}",
            inspection.fixed_layout,
            inspection.diagnostics
        );
        assert!(
            errors
                .iter()
                .any(|item| item.code == format!("ebook.fixed_layout.{expected_code}")),
            "{case} lost distinct typed diagnostic identity: {errors:?}"
        );
        assert!(!fixture.dir.path().join("outside.xhtml").exists());
    }
    fixture.unchanged();
}

#[test]
fn run_manifest_binding_distinguishes_verified_stale_and_corrupt_evidence() {
    let fixture = Fixture::new("png", "ltr");
    let result = execute(
        resolve(PlanningRequest::from_path(&fixture.config)).unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete);
    let epub = published_epub(&result);
    let manifest = Path::new(result.manifest_path.as_ref().unwrap());
    let original = fs::read(epub).unwrap();

    let verified = inspect_ebook_with_manifest(epub, false, manifest).unwrap();
    assert!(verified.valid, "{:?}", verified.diagnostics);
    assert_eq!(
        verified.provenance.as_ref().unwrap().status,
        EbookProvenanceStatus::Verified
    );
    assert_eq!(
        verified.provenance.as_ref().unwrap().run_id.as_deref(),
        Some(result.run_manifest.run_id.as_str())
    );

    let altered = fixture
        .dir
        .path()
        .join("same-structure-different-digest.epub");
    tampered_epub(epub, &altered, |members| {
        rewrite_member(
            members,
            "EPUB/book.opf",
            "<dc:title>Synthetic Fixed EPUB</dc:title>",
            "<dc:title>Synthetic Fixed EPUB Revised</dc:title>",
        );
    });
    fs::write(epub, fs::read(altered).unwrap()).unwrap();
    let stale = inspect_ebook_with_manifest(epub, false, manifest).unwrap();
    assert!(!stale.valid);
    assert_eq!(
        stale.provenance.as_ref().unwrap().status,
        EbookProvenanceStatus::Stale
    );
    assert!(stale
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "ebook.provenance.stale"));
    assert_eq!(
        stale.fixed_layout.as_ref().unwrap().status,
        renderflow::ebook::FixedLayoutStatus::Validated
    );

    fs::write(epub, &original).unwrap();
    let mut altered_lineage: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest).unwrap()).unwrap();
    let source = altered_lineage["artifact_manifest"]["artifacts"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|artifact| artifact["lifecycle"] == "source")
        .unwrap();
    source["digest"]["value"] = serde_json::Value::String("0".repeat(64));
    let false_lineage = fixture.dir.path().join("false-source-lineage.json");
    fs::write(
        &false_lineage,
        serde_json::to_vec(&altered_lineage).unwrap(),
    )
    .unwrap();
    let lineage = inspect_ebook_with_manifest(epub, false, &false_lineage).unwrap();
    assert!(!lineage.valid);
    assert_eq!(
        lineage.provenance.as_ref().unwrap().status,
        EbookProvenanceStatus::Stale
    );
    assert!(lineage
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "ebook.provenance.stale"));

    let malformed_manifest = fixture.dir.path().join("corrupt-run.json");
    fs::write(&malformed_manifest, b"{not valid JSON").unwrap();
    let corrupt = inspect_ebook_with_manifest(epub, false, &malformed_manifest).unwrap();
    assert!(!corrupt.valid);
    assert_eq!(
        corrupt.provenance.as_ref().unwrap().status,
        EbookProvenanceStatus::Corrupt
    );
    assert!(corrupt
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "ebook.provenance.corrupt"));
    assert_eq!(fs::read(epub).unwrap(), original);
    fixture.unchanged();
}

#[test]
fn multiple_accessibility_hazards_are_inspected_as_declared_metadata() {
    let fixture = Fixture::new("png", "ltr");
    fixture.rewrite("hazards: [none]", "hazards: [flashing, sound]");
    let result = execute(
        resolve(PlanningRequest::from_path(&fixture.config)).unwrap(),
        false,
    )
    .unwrap();
    assert_eq!(result.run_manifest.state, RunState::Complete);
    let inspection = inspect_ebook(published_epub(&result), false).unwrap();
    assert!(inspection.valid, "{:?}", inspection.diagnostics);
    assert_eq!(inspection.accessibility.accessibility_hazards, 2);
    fixture.unchanged();
}

#[test]
fn unreadable_zip_is_a_typed_invalid_inspection() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("malformed.epub");
    fs::write(
        &source,
        b"PK\x03\x04truncated member and no central directory",
    )
    .unwrap();
    let inspection = inspect_ebook(&source, false).unwrap();
    assert!(!inspection.valid);
    assert!(inspection
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "ebook.container.invalid"));
    assert!(inspection.epubcheck.is_none());
}

#[test]
fn member_digest_and_declared_geometry_change_the_plan_identity() {
    let fixture = Fixture::new("png", "ltr");
    let initial = resolve(PlanningRequest::from_path(&fixture.config)).unwrap();
    let initial_digest = sha256_serialized(initial.plan()).unwrap().value;

    let altered_bytes = Fixture::new("png", "ltr");
    fs::write(altered_bytes.source(0), PNG_SECOND).unwrap();
    altered_bytes.rewrite(
        &format!("{:x}", Sha256::digest(PNG_FIRST)),
        &format!("{:x}", Sha256::digest(PNG_SECOND)),
    );
    let changed = resolve(PlanningRequest::from_path(&altered_bytes.config)).unwrap();
    assert_ne!(
        initial_digest,
        sha256_serialized(changed.plan()).unwrap().value,
        "member payload digest not bound to the plan"
    );

    let altered_geometry = Fixture::new("png", "ltr");
    let yaml = fs::read_to_string(&altered_geometry.config).unwrap();
    assert_eq!(yaml.matches("width: 90, height: 90").count(), 3);
    fs::write(
        &altered_geometry.config,
        yaml.replace("width: 90, height: 90", "width: 95, height: 95"),
    )
    .unwrap();
    let changed = resolve(PlanningRequest::from_path(&altered_geometry.config)).unwrap();
    assert_ne!(
        initial_digest,
        sha256_serialized(changed.plan()).unwrap().value,
        "declared page geometry not bound to the plan"
    );
    fixture.unchanged();
    altered_geometry.unchanged();
}

#[test]
fn planned_source_digests_match_original_artwork() {
    let fixture = Fixture::new("png", "ltr");
    let resolved = resolve(PlanningRequest::from_path(&fixture.config)).unwrap();
    let members = &resolved.plan().source_collection.as_ref().unwrap().members;
    let by_source = members
        .iter()
        .map(|member| (member.source_id.as_str(), member.artifact.digest.as_str()))
        .collect::<BTreeMap<_, _>>();
    for (index, id) in ["source.page001", "source.page002"].iter().enumerate() {
        assert_eq!(
            by_source[id]
                .strip_prefix("sha256:")
                .unwrap_or(by_source[id]),
            format!("{:x}", Sha256::digest(&fixture.originals[index]))
        );
    }
}
