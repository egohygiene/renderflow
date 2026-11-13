//! Publication wiring in the existing comic recipe; domain work stays in core.

use super::*;
use renderflow::composition::RasterPlan;
use renderflow::evidence::{RunManifest, RunState};
use renderflow::publication::raster::{
    prepare_composition_rgb, RgbPreparationEvidence, RGB_POLICY,
};
use renderflow::{EngineBuilder, ExecutionRequest};

pub(super) struct Capture {
    pub manifest: RunManifest,
    pub root: PathBuf,
    pub raster: RasterPlan,
}

#[derive(Debug, Serialize)]
pub(super) struct AggregateEvidence {
    pub role: String,
    pub native_target_role: String,
    pub format: String,
    pub page_ids: Vec<String>,
    pub expected_page_count: usize,
    pub observed_page_count: Option<usize>,
    pub status: String,
    pub reason: Option<String>,
    pub output_sha256: Option<String>,
    pub run_id: Option<String>,
    pub provider_id: String,
    pub runtime_qualified: bool,
    pub publication_approval_inferred: bool,
}

pub(super) struct Outcome {
    pub outputs: Vec<AggregateEvidence>,
    pub preparation: BTreeMap<String, RgbPreparationEvidence>,
    pub hashes: BTreeMap<String, String>,
    pub observed: usize,
}

fn groups(generated: &Generated) -> Vec<(&'static str, &'static str, Vec<&Page>)> {
    vec![
        (
            "interior",
            "pdf",
            generated
                .pages
                .iter()
                .filter(|page| page.role == "interior")
                .collect(),
        ),
        (
            "cover_pages",
            "pdf",
            generated
                .pages
                .iter()
                .filter(|page| page.role == "cover")
                .collect(),
        ),
        ("ebook", "epub", generated.pages.iter().collect()),
    ]
}

fn contract(pages: &[&Page], case: &PackCase, title: &str) -> Value {
    json!({"schema":"renderflow.publication/v1","publication":"Synthetic Fixture Editions",
        "issue_id":"lantern-map","title":title,"publication_date":"2026-11-04","language":"en",
        "status":"draft","contributors":[{"name":"Renderflow Contributors","role":"author"}],
        "geometry":{"width":case.parameters.width_mm,"height":case.parameters.height_mm,"unit":"mm"},
        "rights":{"license":"CC0-1.0","rights_holder":"Renderflow Contributors","reviewed":true},
        "artwork":pages.iter().map(|page|json!({"role":"page","path":format!("{}.png",page.id),"alt_text":page.alt})).collect::<Vec<_>>(),
        "accessibility":{"summary":"Original geometric comic fixture; image descriptions are present, separate page transcripts retain dialogue.","access_modes":["visual"],"hazards":["none"]}})
}

fn spec(
    pages: &[&Page],
    case: &PackCase,
    role: &str,
    format: &str,
    output: &Path,
    digests: &BTreeMap<String, String>,
    executable: Option<&Path>,
) -> Result<Value> {
    let mut sources = pages.iter().map(|page| {
        let mut geometry = json!({"width":case.parameters.width_mm,"height":case.parameters.height_mm,"unit":"mm"});
        if format == "pdf" { geometry["bleed"] = json!(0); }
        json!({"id":page.id,"path":format!("{}.png",page.id),"format":"png","media_type":"image/png",
            "sha256":digests[&page.id],"geometry":geometry})
    }).collect::<Vec<_>>();
    sources.push(json!({"id":"ordered.pages","kind":"collection","members":pages.iter().map(|page|&page.id).collect::<Vec<_>>()}));
    let (native_role, provider, capability) = if format == "epub" {
        (
            "ebook",
            "tool.renderflow-epub",
            "ebook.generate.epub.fixed-layout",
        )
    } else {
        (
            "interior",
            "tool.img2pdf",
            "publication.generate.pdf.interior",
        )
    };
    let mut execution = json!({"network":"deny","ai":"deny","tools":{"allow":[provider]},"transforms":{"allow":[capability]}});
    if format == "epub" {
        execution["fixed_layout_epub"] = json!({"page_progression_direction":"ltr","spread":"none",
            "cover_member_id":pages[0].id,"max_pages":pages.len(),"max_input_bytes":16*1024*1024,"max_output_bytes":32*1024*1024});
    } else {
        execution["print_pdf_interior"] = json!({"executable":executable.context("print provider was not explicitly selected")?.to_str().context("print executable must be UTF-8")?,
            "provider_version":"0.6.3","box_policy":"media_bleed_trim_inset","rotation":"none","scaling":"fit","color_policy":"preserve_rgb_gray",
            "max_pages":pages.len(),"max_input_bytes":16*1024*1024,"max_output_bytes":32*1024*1024,"timeout_seconds":30});
    }
    let mut value = json!({"schema":"renderflow/v2","sources":sources,
        "targets":{"exact":[{"id":format!("target.{role}"),"role":native_role,"format":format,"requirement":"required"}]},
        "execution":execution,"output":{"bundle_root":output.to_str().context("output must be UTF-8")?,"naming_template":format!("{role}.{{ext}}")}});
    if format == "epub" {
        value["publication"] = contract(pages, case, "Lantern Map");
    }
    Ok(value)
}

pub(super) fn run(
    generated: &Generated,
    case: &PackCase,
    root: &Path,
    output_root: &Path,
    captures: &BTreeMap<String, Capture>,
    requested_rank: usize,
    execute: bool,
    started: Instant,
) -> Result<Outcome> {
    let groups = groups(generated);
    let plan = json!({"schema_version":"renderflow.fixture-publication-plan/v1","source":"native page candidates",
        "normalization_policy":RGB_POLICY,"groups":groups.iter().map(|(role,format,pages)|json!({"role":role,"format":format,
            "native_target_role":if *format=="epub" {"ebook"} else {"interior"},
            "page_ids":pages.iter().map(|page|&page.id).collect::<Vec<_>>(),"requires_rendered_pages":true})).collect::<Vec<_>>(),
        "cover_scope":"Two ordered cover pages; no spread, spine, wrap, PDF/X or printer acceptance is implied.",
        "runtime_qualified":false,"publication_approved":false});
    write_json(&root.join("publication-plan.json"), &plan)?;
    let mut outcome = Outcome {
        outputs: Vec::new(),
        preparation: BTreeMap::new(),
        hashes: BTreeMap::new(),
        observed: 0,
    };
    if !execute {
        for (role, format, pages) in groups {
            outcome.outputs.push(AggregateEvidence {
                role: role.into(),
                native_target_role: if format == "epub" {
                    "ebook"
                } else {
                    "interior"
                }
                .into(),
                format: format.into(),
                page_ids: pages.iter().map(|page| page.id.clone()).collect(),
                status: "recipe_only".into(),
                expected_page_count: pages.len(),
                observed_page_count: None,
                reason: Some(
                    "No raster source was generated; no aggregate SDK plan or output is inferred"
                        .into(),
                ),
                output_sha256: None,
                run_id: None,
                provider_id: if format == "epub" {
                    "tool.renderflow-epub"
                } else {
                    "tool.img2pdf"
                }
                .into(),
                runtime_qualified: false,
                publication_approval_inferred: false,
            });
        }
        return Ok(outcome);
    }
    ensure!(
        captures.len() == generated.pages.len(),
        "comic complete native capture set differs"
    );
    let inputs = root.join("publication-inputs");
    fs::create_dir(&inputs)?;
    let mut digests = BTreeMap::new();
    let mut images = BTreeMap::new();
    for page in &generated.pages {
        check_run_budget(root, output_root, case, started)?;
        let capture = captures
            .get(&page.id)
            .context("comic native capture missing")?;
        let publications = capture.manifest.published_targets()?;
        let png = publications
            .iter()
            .find(|publication| publication.artifact.format == "png")
            .context("comic PNG was not published")?;
        let observed = verification::read_publication(
            &capture.manifest,
            &png.branch.target_id,
            &capture.root,
            &VerificationLimits::default(),
            token(),
        )?;
        let prepared = prepare_composition_rgb(
            &observed.bytes,
            &observed.observation.sha256,
            &capture.raster,
            RGB_POLICY,
            &token(),
        )?;
        let path = inputs.join(format!("{}.png", page.id));
        write(&path, &prepared.bytes)?;
        let image = renderflow::print_pdf_image::inspect_print_image(
            &path,
            renderflow::graph::Format::Png,
        )?;
        ensure!(
            image.width_px == 500 && image.height_px == 700,
            "comic integer-pixel publication geometry differs"
        );
        digests.insert(page.id.clone(), prepared.evidence.output_sha256.clone());
        images.insert(page.id.clone(), image);
        outcome
            .preparation
            .insert(page.id.clone(), prepared.evidence);
    }
    write_json(
        &inputs.join("rgb-preparation.json"),
        &serde_json::to_value(&outcome.preparation)?,
    )?;
    let before = snapshot(&inputs, 64, 16 * 1024 * 1024)?;
    let configured = std::env::var_os("RENDERFLOW_TEST_IMG2PDF").map(PathBuf::from);
    let executable = if requested_rank >= 1 {
        configured.as_deref()
    } else {
        None
    };
    for (role, format, pages) in groups {
        check_run_budget(root, output_root, case, started)?;
        let mut evidence = AggregateEvidence {
            role: role.into(),
            native_target_role: if format == "epub" {
                "ebook"
            } else {
                "interior"
            }
            .into(),
            format: format.into(),
            page_ids: pages.iter().map(|page| page.id.clone()).collect(),
            expected_page_count: pages.len(),
            observed_page_count: None,
            status: "not_attempted".into(),
            reason: None,
            output_sha256: None,
            run_id: None,
            provider_id: if format == "epub" {
                "tool.renderflow-epub"
            } else {
                "tool.img2pdf"
            }
            .into(),
            runtime_qualified: false,
            publication_approval_inferred: false,
        };
        if format == "pdf" && executable.is_none() {
            evidence.reason=Some("Requires full/maximal tier and explicit RENDERFLOW_TEST_IMG2PDF=img2pdf-0.6.3; fast tier never probes external tools".into());
            outcome.outputs.push(evidence);
            continue;
        }
        let output = output_root.join(format!("aggregate-{role}"));
        let config = inputs.join(format!("{role}.renderflow.json"));
        write_json(
            &config,
            &spec(&pages, case, role, format, &output, &digests, executable)?,
        )?;
        let engine = EngineBuilder::new().build()?;
        let resolved = engine.resolve_execution(ExecutionRequest::from_path(&config))?;
        let result = engine.execute_resolved(resolved, false)?;
        check_run_budget(root, output_root, case, started)?;
        ensure!(
            result.run_manifest.state == RunState::Complete,
            "comic aggregate did not complete; no provider success is inferred"
        );
        let actual = verification::verify(
            &result.run_manifest,
            &output,
            &VerificationLimits::default(),
            token(),
        )?;
        ensure!(
            actual.len() == 1,
            "comic aggregate publication membership differs"
        );
        let publication = &actual[0];
        let observation = publication.observation();
        if format == "epub" {
            let path = output.join(format!("{role}.epub"));
            let inspected = renderflow::ebook::inspect_ebook(&path, false)?;
            let fixed = inspected
                .fixed_layout
                .context("comic EPUB exact-route evidence missing")?;
            ensure!(
                inspected.valid
                    && fixed.status == renderflow::ebook::FixedLayoutStatus::Validated
                    && inspected.spine_items == pages.len()
                    && fixed.pages.len() == pages.len(),
                "comic EPUB structure/order count differs"
            );
            for (actual, page) in fixed.pages.iter().zip(&pages) {
                ensure!(
                    actual.image_sha256 == digests[&page.id],
                    "comic EPUB image order or bytes differ"
                );
            }
        } else {
            let expected = pages
                .iter()
                .map(|page| {
                    let image = &images[&page.id];
                    renderflow::print_pdf_inspect::ExpectedPrintPage {
                        media_width_pt: case.parameters.width_mm * 72.0 / 25.4,
                        media_height_pt: case.parameters.height_mm * 72.0 / 25.4,
                        trim_inset_pt: 0.0,
                        pixel_width: image.width_px,
                        pixel_height: image.height_px,
                        rotation: 0,
                        color_space: "DeviceRGB".into(),
                        image_filter: "FlateDecode".into(),
                        image_stream_sha256: image.image_stream_sha256.clone(),
                    }
                })
                .collect::<Vec<_>>();
            let inspected = renderflow::print_pdf_inspect::inspect_print_pdf(
                &output.join(format!("{role}.pdf")),
                &expected,
            )?;
            ensure!(
                inspected.page_count == pages.len() && inspected.sha256 == observation.sha256,
                "comic aggregate PDF count/hash differs"
            );
        }
        evidence.observed_page_count = Some(pages.len());
        evidence.status = "observed_candidate".into();
        evidence.output_sha256 = Some(observation.sha256.clone());
        evidence.run_id = Some(result.run_manifest.run_id.clone());
        outcome.hashes.insert(
            format!("aggregate/{role}.{format}"),
            observation.sha256.clone(),
        );
        outcome.observed += 1;
        outcome.outputs.push(evidence);
    }
    // New configs are intentional recipe products; immutable raster inputs must
    // retain their exact pre-execution bytes. Output checks never bless approval.
    for (path, hash) in &before.0 {
        ensure!(
            digest(&fs::read(inputs.join(path))?) == *hash,
            "comic normalized source mutated"
        );
    }
    write_json(
        &root.join("publication-results.json"),
        &serde_json::to_value(&outcome.outputs)?,
    )?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn large_case() -> PackCase {
        let corpus: Corpus = serde_json::from_slice(
            &fs::read(repository().join("tests/fixtures/golden-artifacts/v1/corpus.json")).unwrap(),
        )
        .unwrap();
        corpus
            .packs
            .into_iter()
            .next()
            .unwrap()
            .cases
            .into_iter()
            .find(|case| case.id == "publication-44")
            .unwrap()
    }
    fn pages() -> Generated {
        let page = |id: String, role: &str| Page {
            id,
            role: role.into(),
            manifest: PathBuf::from("unused.json"),
            text: ["a".into(), "b".into()],
            alt: "Synthetic declaration".into(),
        };
        let mut pages = vec![page("cover-front".into(), "cover")];
        pages.extend((1..=44).map(|n| page(format!("page-{n:03}"), "interior")));
        pages.push(page("cover-back".into(), "cover"));
        Generated {
            source: PathBuf::from("unused"),
            pages,
            model: json!({}),
        }
    }

    #[test]
    fn full_size_groups_keep_interior_sequence_and_separate_cover_packet() {
        let generated = pages();
        let grouped = groups(&generated);
        assert_eq!(grouped[0].2.len(), 44);
        assert_eq!(grouped[1].2.len(), 2);
        assert_eq!(grouped[2].2.len(), 46);
        assert_eq!(grouped[0].2[0].id, "page-001");
        assert_eq!(grouped[0].2[43].id, "page-044");
        assert_eq!(
            grouped[1]
                .2
                .iter()
                .map(|page| page.id.as_str())
                .collect::<Vec<_>>(),
            vec!["cover-front", "cover-back"]
        );
        assert_eq!(grouped[2].2[0].id, "cover-front");
        assert_eq!(grouped[2].2[45].id, "cover-back");
    }

    #[test]
    fn unselected_large_execution_reports_recipes_without_aggregate_outputs() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("outputs");
        fs::create_dir(&output).unwrap();
        let outcome = run(
            &pages(),
            &large_case(),
            root.path(),
            &output,
            &BTreeMap::new(),
            2,
            false,
            Instant::now(),
        )
        .unwrap();
        assert_eq!(outcome.observed, 0);
        assert!(outcome.preparation.is_empty() && outcome.hashes.is_empty());
        assert_eq!(
            outcome
                .outputs
                .iter()
                .map(|v| v.expected_page_count)
                .collect::<Vec<_>>(),
            vec![44, 2, 46]
        );
        assert!(outcome.outputs.iter().all(|v| v.status == "recipe_only"
            && v.observed_page_count.is_none()
            && v.output_sha256.is_none()));
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
        assert!(!root.path().join("publication-inputs").exists());
    }

    #[test]
    fn incomplete_native_capture_set_cannot_start_aggregate_generation() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("outputs");
        fs::create_dir(&output).unwrap();
        let error = run(
            &pages(),
            &large_case(),
            root.path(),
            &output,
            &BTreeMap::new(),
            2,
            true,
            Instant::now(),
        )
        .err()
        .unwrap();
        assert!(error.to_string().contains("complete native capture"));
        assert!(!root.path().join("publication-inputs").exists());
        assert_eq!(fs::read_dir(&output).unwrap().count(), 0);
    }

    #[test]
    fn large_case_keeps_a_finite_cooperative_output_budget() {
        let root = tempfile::tempdir().unwrap();
        let output = root.path().join("outputs");
        fs::create_dir(&output).unwrap();
        let mut case = large_case();
        assert!(
            case.limits.max_output_bytes > 0 && case.limits.max_output_bytes <= 128 * 1024 * 1024
        );
        case.limits.max_output_bytes = 1;
        fs::write(output.join("oversize"), b"xx").unwrap();
        assert!(check_run_budget(root.path(), &output, &case, Instant::now()).is_err());
    }
}
