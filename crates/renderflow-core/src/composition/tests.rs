//! Authored integration regressions for the scoped composition boundary.
//!
//! The measured example carries a licensed real TTF. These authored, unrun
//! regressions concern contracts, immutable bytes and receipts; they do not
//! establish native visual fidelity or publication readiness.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tempfile::TempDir;

use super::execution::CompositionBranchOutcome;
use super::*;
use crate::evidence::{ArtifactRole, CacheDisposition, ValidationState};
use crate::process::ProcessCancellationToken;

fn example_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/page-composition/measured-typography")
        .canonicalize()
        .unwrap()
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

fn input_fixture(directory: &TempDir) -> PathBuf {
    let root = directory.path().canonicalize().unwrap().join("input");
    copy_tree(&example_root(), &root);
    root.join("page.composition.json")
}

fn file_snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, current: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(current).unwrap() {
            let entry = entry.unwrap();
            if entry.file_type().unwrap().is_dir() {
                collect(root, &entry.path(), output);
            } else {
                output.insert(entry.path().strip_prefix(root).unwrap().to_path_buf(), fs::read(entry.path()).unwrap());
            }
        }
    }
    let mut output = BTreeMap::new();
    collect(root, root, &mut output);
    output
}

fn request(locales: LocaleSelector, targets: Vec<CompositionOutput>) -> CompositionRequest {
    CompositionRequest { locales, targets, raster: raster::RasterSettings::default(), pdf: pdf::PdfSettings::default() }
}

fn exact(locale: &str, targets: Vec<CompositionOutput>) -> CompositionRequest {
    request(LocaleSelector::Exact { locale: locale.into() }, targets)
}

fn identities(plan: &CompositionPlan) -> BTreeMap<(String, CompositionOutput), (String, String)> {
    plan.branches.iter().map(|branch| (
        (branch.locale.clone(), branch.output),
        (branch.target_id.clone(), branch.cache_key.clone()),
    )).collect()
}

#[test]
fn exact_subset_and_all_declared_keep_the_same_branch_identities() {
    let loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let outputs = vec![CompositionOutput::Svg, CompositionOutput::TranscriptJson];
    let source = plan(&loaded, &exact(&loaded.manifest().source_locale, outputs.clone())).unwrap();
    let default = plan(&loaded, &exact(&loaded.manifest().default_locale, outputs.clone())).unwrap();
    let subset = plan(&loaded, &request(
        LocaleSelector::Subset { locales: vec!["de".into(), "en".into()] }, outputs.clone(),
    )).unwrap();
    let all = plan(&loaded, &request(LocaleSelector::AllDeclared, outputs)).unwrap();

    assert_eq!(identities(&source), identities(&default));
    assert_eq!(source.branches.len(), 2);
    assert_eq!(subset.branches.len(), 4);
    assert_eq!(all.branches.len(), loaded.manifest().locales.len() * 2);
    let all_identities = identities(&all);
    for (key, identity) in identities(&subset) {
        assert_eq!(all_identities.get(&key), Some(&identity));
    }
    for (key, identity) in identities(&source) {
        assert_eq!(identities(&subset).get(&key), Some(&identity));
    }
    assert!(subset.branches.iter().all(|branch| branch.state == CompositionBranchState::Planned));
    assert!(all.branches.iter().filter(|branch| matches!(branch.locale.as_str(), "ar" | "ja"))
        .all(|branch| branch.state == CompositionBranchState::Blocked));
    assert!(!all.runtime_qualified && !all.visual_determinism_qualified && all.review_required);
}

#[test]
fn visual_branches_share_one_measured_report_and_commit_its_identity() {
    let mut loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let request = exact("en", vec![CompositionOutput::Svg, CompositionOutput::Html,
        CompositionOutput::Png, CompositionOutput::TranscriptJson, CompositionOutput::Pdf]);
    let before = plan(&loaded, &request).unwrap();
    assert_eq!(before.schema, "renderflow.composition-plan/v5");
    assert_eq!(before.profile, "renderflow.layered-composition/v5");
    let svg = before.branches.iter().find(|branch| branch.output == CompositionOutput::Svg).unwrap();
    let html = before.branches.iter().find(|branch| branch.output == CompositionOutput::Html).unwrap();
    let png = before.branches.iter().find(|branch| branch.output == CompositionOutput::Png).unwrap();
    let pdf = before.branches.iter().find(|branch| branch.output == CompositionOutput::Pdf).unwrap();
    let report = serde_json::to_value(svg.typography.as_ref().unwrap()).unwrap();
    assert_eq!(report, serde_json::to_value(html.typography.as_ref().unwrap()).unwrap());
    assert_eq!(report, serde_json::to_value(png.typography.as_ref().unwrap()).unwrap());
    assert_eq!(report, serde_json::to_value(pdf.typography.as_ref().unwrap()).unwrap());
    assert_eq!(pdf.state, CompositionBranchState::Planned);
    assert!(pdf.raster.is_none() && pdf.pdf.is_some());
    assert_eq!(hash_json(svg.typography.as_ref().unwrap()).unwrap(),
        hash_json(html.typography.as_ref().unwrap()).unwrap());
    assert_ne!(svg.target_id, html.target_id);
    assert_ne!(svg.cache_key, html.cache_key);
    assert!(before.branches.iter().filter(|branch| !branch.output.visual())
        .all(|branch| branch.typography.is_none()));

    // Unit-only mutation isolates the measured-report contribution from the
    // captured-input digests. Public SDK snapshots remain immutable.
    let layer = loaded.manifest.layers.iter_mut().find(|layer| layer.id == "caption.text.one").unwrap();
    let model::LayerContent::Text { typography, .. } = &mut layer.content else { panic!("text fixture required") };
    typography.font_size_pt = 13.0;
    let after = plan(&loaded, &request).unwrap();
    for previous in &before.branches {
        let current = after.branches.iter().find(|branch| branch.target_id == previous.target_id).unwrap();
        if previous.output.visual() {
            assert_ne!(hash_json(previous.typography.as_ref().unwrap()).unwrap(),
                hash_json(current.typography.as_ref().unwrap()).unwrap());
            assert_ne!(previous.cache_key, current.cache_key);
        } else {
            assert_eq!(previous.cache_key, current.cache_key);
        }
    }
}

#[test]
fn aggregate_visual_text_budget_counts_whitespace_and_refuses_before_publication() {
    let mut loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let reference = loaded.manifest.locales["en"].clone();
    let template = loaded.contents["en"].clone();
    let locales = vec!["en", "de", "fr", "es", "it"];
    for locale in &locales {
        // Private fixture mutation creates several otherwise small locales to
        // isolate the aggregate budget; production snapshots must be reloaded.
        let mut content = template.clone();
        content.document.locale = (*locale).to_string();
        content.document.content.insert("panel.one.caption".into(), " ".repeat(3500));
        content.document.content.insert("panel.two.caption".into(), String::new());
        loaded.contents.insert((*locale).to_string(), content);
        loaded.manifest.locales.insert((*locale).to_string(), reference.clone());
    }
    let selector = LocaleSelector::Subset { locales: locales.into_iter().map(String::from).collect() };
    let visual = request(selector.clone(), vec![CompositionOutput::Svg]);
    let error = plan(&loaded, &visual).unwrap_err();
    assert!(error.to_string().contains("composition.typography.plan_budget"));
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("not-created");
    let cache = directory.path().join("cache-not-created");
    assert!(execute(&loaded, &visual, &output, Some(&cache), &ProcessCancellationToken::new()).is_err());
    assert!(!output.exists() && !cache.exists());
    let transcript = plan(&loaded, &request(selector, vec![CompositionOutput::TranscriptJson])).unwrap();
    assert!(transcript.branches.iter().all(|branch|
        branch.state == CompositionBranchState::Planned && branch.typography.is_none()));
}

#[test]
fn raster_settings_change_only_the_png_branch_and_invalid_settings_stay_scoped() {
    let loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let mut request = exact("en", vec![CompositionOutput::Svg, CompositionOutput::Html,
        CompositionOutput::Png, CompositionOutput::Pdf, CompositionOutput::TranscriptJson]);
    let initial = plan(&loaded, &request).unwrap();
    request.raster = raster::RasterSettings { dpi: 72, background: "#A1B2C3".into() };
    let changed = plan(&loaded, &request).unwrap();
    for before in &initial.branches {
        let after = changed.branches.iter().find(|branch| branch.locale == before.locale && branch.output == before.output).unwrap();
        assert_eq!(before.state, CompositionBranchState::Planned);
        assert_eq!(after.state, CompositionBranchState::Planned);
        if before.output == CompositionOutput::Png {
            assert_ne!(before.target_id, after.target_id);
            assert_ne!(before.cache_key, after.cache_key);
            let raster = after.raster.as_ref().unwrap();
            assert_eq!((raster.width_px, raster.height_px), (596, 420));
            assert_eq!(raster.settings.dpi, 72);
            assert_eq!(raster.settings.background, "#a1b2c3");
            assert_eq!(serde_json::to_value(&before.typography).unwrap(), serde_json::to_value(&after.typography).unwrap());
        } else {
            assert_eq!(before.target_id, after.target_id);
            assert_eq!(before.cache_key, after.cache_key);
            assert!(before.raster.is_none() && after.raster.is_none());
        }
    }
    request.raster.background = "#a1b2c3".into();
    let lowercase = plan(&loaded, &request).unwrap();
    assert_eq!(identities(&changed), identities(&lowercase));
    for invalid in [
        raster::RasterSettings { dpi: 35, background: "transparent".into() },
        raster::RasterSettings { dpi: 601, background: "transparent".into() },
        raster::RasterSettings { dpi: 144, background: "#ff00ff80".into() },
    ] {
        request.raster = invalid;
        let blocked = plan(&loaded, &request).unwrap();
        for branch in &blocked.branches {
            if branch.output == CompositionOutput::Png {
                assert_eq!(branch.state, CompositionBranchState::Blocked);
                assert!(branch.raster.is_none());
                assert!(branch.diagnostics.iter().any(|item| item.severity == DiagnosticSeverity::Error));
            } else {
                assert_eq!(branch.state, CompositionBranchState::Planned);
            }
        }
    }
}

#[test]
fn png_receipts_commit_raster_settings_and_reuse_other_outputs_without_changing_sources() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let input = root.join("input");
    copy_tree(&example_root().parent().unwrap().join("raster-composition"), &input);
    let before = file_snapshot(&input);
    let loaded = LoadedComposition::load(input.join("page.composition.json")).unwrap();
    let mut request = exact("en", vec![CompositionOutput::Svg, CompositionOutput::Png,
        CompositionOutput::TranscriptJson]);
    request.raster = raster::RasterSettings { dpi: 72, background: "#204060".into() };
    let cache = root.join("cache");
    let cancellation = ProcessCancellationToken::new();
    let first = execute(&loaded, &request, &root.join("first"), Some(&cache), &cancellation).unwrap();
    assert_eq!(first.state, CompositionRunState::Complete);
    request.raster.dpi = 144;
    let second = execute(&loaded, &request, &root.join("second"), Some(&cache), &cancellation).unwrap();
    assert_eq!(second.state, CompositionRunState::Complete);
    assert_eq!(file_snapshot(&input), before);
    for run in [&first, &second] {
        for branch in &run.branches {
            let planned = run.plan.branches.iter().find(|plan| plan.target_id == branch.target_id).unwrap();
            let artifact = &run.artifact_manifest.artifacts[branch.artifact_evidence_index.unwrap()];
            let metadata = &artifact.metadata["renderflow.composition"];
            if planned.output == CompositionOutput::Png {
                assert_eq!(metadata["raster"], serde_json::to_value(planned.raster.as_ref().unwrap()).unwrap());
                assert_eq!(metadata["typography"]["report_sha256"], hash_json(planned.typography.as_ref().unwrap()).unwrap());
                assert_eq!(branch.cache, CacheDisposition::Miss);
            } else {
                assert!(metadata["raster"].is_null());
            }
        }
    }
    for branch in &second.branches {
        let planned = second.plan.branches.iter().find(|plan| plan.target_id == branch.target_id).unwrap();
        if planned.output != CompositionOutput::Png { assert_eq!(branch.cache, CacheDisposition::Hit); }
    }
    let png_branch = |run: &CompositionRun| run.branches.iter().find(|branch|
        run.plan.branches.iter().any(|plan| plan.target_id == branch.target_id && plan.output == CompositionOutput::Png))
        .unwrap().artifact_id.clone();
    assert_ne!(png_branch(&first), png_branch(&second));
    let receipt: CompositionRun = serde_json::from_slice(&fs::read(root.join("second/composition-run.json")).unwrap()).unwrap();
    assert_eq!(receipt.plan_sha256, hash_bytes(&fs::read(root.join("second/composition-plan.json")).unwrap()));
    assert_eq!(receipt.artifact_manifest, second.artifact_manifest);
    assert!(!receipt.runtime_qualified && !receipt.visual_determinism_qualified && receipt.review_required);
}

#[test]
fn malformed_captured_png_cannot_publish_an_incomplete_picture_but_transcripts_survive() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let input = root.join("input");
    copy_tree(&example_root().parent().unwrap().join("raster-composition"), &input);
    let manifest_path = input.join("page.composition.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let asset = manifest["assets"].as_array_mut().unwrap().iter_mut()
        .find(|asset| asset["id"] == "raster.png-alpha").unwrap();
    let relative_path = asset["file"]["path"].as_str().unwrap().to_string();
    let asset_path = input.join(&relative_path);
    let mut bytes = fs::read(&asset_path).unwrap();
    // A pinned signature and IHDR are valid source-header evidence; they do not
    // supply a decodable image. Retain the header and remove all image data.
    bytes.truncate(33);
    fs::write(&asset_path, &bytes).unwrap();
    asset["file"]["sha256"] = Value::String(hash_bytes(&bytes));
    for original in manifest["provenance"]["originals"].as_array_mut().unwrap() {
        if original["path"] == relative_path {
            original["sha256"] = Value::String(hash_bytes(&bytes));
        }
    }
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();
    let before = file_snapshot(&input);
    let loaded = LoadedComposition::load(&manifest_path).unwrap();
    let output = root.join("partial");
    let run = execute(&loaded, &exact("en", vec![CompositionOutput::Png, CompositionOutput::Pdf, CompositionOutput::TranscriptJson]),
        &output, None, &ProcessCancellationToken::new()).unwrap();
    assert_eq!(run.state, CompositionRunState::Partial);
    for output in [CompositionOutput::Png, CompositionOutput::Pdf] {
        let branch = run.branches.iter().find(|branch| run.plan.branches.iter().any(|plan|
            plan.target_id == branch.target_id && plan.output == output)).unwrap();
        assert!(matches!(branch.outcome, CompositionBranchOutcome::Blocked | CompositionBranchOutcome::Failed));
        assert!(branch.artifact_id.is_none() && branch.artifact_evidence_index.is_none() && branch.locator.is_none());
    }
    assert_eq!(run.artifact_manifest.outputs, vec!["outputs/en/page.transcript.json".to_string()]);
    assert!(!output.join("outputs/en/page.png").exists());
    assert!(!output.join("outputs/en/page.pdf").exists());
    assert!(output.join("composition-run.json").is_file());
    assert_eq!(file_snapshot(&input), before);
}

#[test]
fn shared_cache_verifies_records_and_refuses_corrupted_payloads() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = input_fixture(&directory);
    let root = directory.path().canonicalize().unwrap();
    let loaded = LoadedComposition::load(&manifest).unwrap();
    let request = exact("en", vec![CompositionOutput::Svg]);
    let cache = root.join("cache");
    let cancellation = ProcessCancellationToken::new();
    let first = execute(&loaded, &request, &root.join("first"), Some(&cache), &cancellation).unwrap();
    let second = execute(&loaded, &request, &root.join("second"), Some(&cache), &cancellation).unwrap();
    assert_eq!(first.state, CompositionRunState::Complete);
    assert_eq!(first.branches[0].cache, CacheDisposition::Miss);
    assert_eq!(second.state, CompositionRunState::Complete);
    assert_eq!(second.branches[0].cache, CacheDisposition::Hit);
    assert_eq!(first.branches[0].artifact_id, second.branches[0].artifact_id);
    assert_eq!(fs::read(root.join("first/outputs/en/page.svg")).unwrap(), fs::read(root.join("second/outputs/en/page.svg")).unwrap());
    let report_digest = hash_json(first.plan.branches[0].typography.as_ref().unwrap()).unwrap();
    for run in [&first, &second] {
        assert_eq!(run.schema, "renderflow.composition-run/v5");
        let artifact = &run.artifact_manifest.artifacts[run.branches[0].artifact_evidence_index.unwrap()];
        assert_eq!(artifact.metadata["renderflow.composition"]["typography"]["report_sha256"], report_digest);
        assert_eq!(artifact.metadata["renderflow.composition"]["renderer"]["version"], render::COMPOSITION_RENDERER_VERSION);
        assert_eq!(artifact.metadata["renderflow.composition"]["typography"]["runtime_qualified"], false);
    }

    // Hashing payload bytes alone cannot validate a tampered artifact identity.
    let entry_path = cache.join("entries").join(format!("{}.json", first.plan.branches[0].cache_key));
    let mut entry: Value = serde_json::from_slice(&fs::read(&entry_path).unwrap()).unwrap();
    entry["artifact"]["id"] = Value::String("artifact:sha256:forged".into());
    fs::write(&entry_path, serde_json::to_vec(&entry).unwrap()).unwrap();
    let repaired = execute(&loaded, &request, &root.join("repaired"), Some(&cache), &cancellation).unwrap();
    assert_eq!(repaired.state, CompositionRunState::Complete);
    assert_eq!(repaired.branches[0].cache, CacheDisposition::Miss);
    assert_eq!(repaired.branches[0].artifact_id, first.branches[0].artifact_id);
    assert!(repaired.branches[0].diagnostics.iter().any(|item| item.code == "composition.cache.rejected"));

    // Valid output bytes cannot validate a forged measured-layout receipt.
    let mut entry: Value = serde_json::from_slice(&fs::read(&entry_path).unwrap()).unwrap();
    entry["artifact"]["metadata"]["renderflow.composition"]["typography"]["report_sha256"] =
        Value::String("0".repeat(64));
    fs::write(&entry_path, serde_json::to_vec(&entry).unwrap()).unwrap();
    let report_repaired = execute(&loaded, &request, &root.join("report-repaired"), Some(&cache), &cancellation).unwrap();
    assert_eq!(report_repaired.state, CompositionRunState::Complete);
    assert_eq!(report_repaired.branches[0].cache, CacheDisposition::Miss);
    assert_eq!(report_repaired.branches[0].artifact_id, first.branches[0].artifact_id);
    assert!(report_repaired.branches[0].diagnostics.iter().any(|item| item.code == "composition.cache.rejected"));

    // A corrupted existing immutable object is refused, not silently overwritten
    // or published under its old digest when regeneration finds that object.
    let index = repaired.branches[0].artifact_evidence_index.unwrap();
    let digest = &repaired.artifact_manifest.artifacts[index].digest.value;
    let payload = cache.join("store/objects/sha256").join(&digest[..2]).join(digest);
    fs::write(&payload, b"deliberately corrupted derivative").unwrap();
    let rejected = execute(&loaded, &request, &root.join("rejected"), Some(&cache), &cancellation).unwrap();
    assert_eq!(rejected.state, CompositionRunState::Failed);
    assert_eq!(rejected.branches[0].outcome, CompositionBranchOutcome::Failed);
    assert!(rejected.branches[0].artifact_id.is_none());
    assert!(rejected.branches[0].locator.is_none());
    assert!(rejected.artifact_manifest.outputs.is_empty());
    assert!(rejected.branches[0].diagnostics.iter().any(|item| item.code == "composition.cache.rejected"));
    assert!(!root.join("rejected/outputs/en/page.svg").exists());
    assert!(root.join("rejected/composition-run.json").is_file());
    assert_eq!(fs::read(payload).unwrap(), b"deliberately corrupted derivative");
}

#[test]
fn reloading_changed_content_invalidates_cache_without_mutating_the_old_snapshot() {
    let directory = tempfile::tempdir().unwrap();
    let manifest_path = input_fixture(&directory);
    let old = LoadedComposition::load(&manifest_path).unwrap();
    let request = exact("de", vec![CompositionOutput::TranscriptJson]);
    let before = plan(&old, &request).unwrap();
    let old_render = render::render(&old, "de", CompositionOutput::TranscriptJson).unwrap().bytes;
    let mut manifest: Value = serde_json::from_slice(&fs::read(&manifest_path).unwrap()).unwrap();
    let content_path = manifest_path.parent().unwrap().join(manifest["locales"]["de"]["path"].as_str().unwrap());
    let mut content: Value = serde_json::from_slice(&fs::read(&content_path).unwrap()).unwrap();
    content["revision"] = Value::String("de-2".into());
    content["content"]["panel.one.caption"] = Value::String("Ein neuer Satz.".into());
    let bytes = serde_json::to_vec_pretty(&content).unwrap();
    fs::write(&content_path, &bytes).unwrap();
    manifest["locales"]["de"]["sha256"] = Value::String(hash_bytes(&bytes));
    fs::write(&manifest_path, serde_json::to_vec_pretty(&manifest).unwrap()).unwrap();

    let changed = LoadedComposition::load(&manifest_path).unwrap();
    let after = plan(&changed, &request).unwrap();
    assert_eq!(before.branches[0].target_id, after.branches[0].target_id);
    assert_ne!(before.branches[0].cache_key, after.branches[0].cache_key);
    assert_ne!(before.branches[0].content_sha256, after.branches[0].content_sha256);
    assert_eq!(after.branches[0].content_revision.as_deref(), Some("de-2"));
    assert_eq!(old_render, render::render(&old, "de", CompositionOutput::TranscriptJson).unwrap().bytes);
    assert_ne!(old_render, render::render(&changed, "de", CompositionOutput::TranscriptJson).unwrap().bytes);
}

#[test]
fn partial_receipt_names_only_published_outputs_and_preserves_source_files() {
    let directory = tempfile::tempdir().unwrap();
    let manifest = input_fixture(&directory);
    let input_root = manifest.parent().unwrap();
    let before = file_snapshot(input_root);
    let loaded = LoadedComposition::load(&manifest).unwrap();
    let output = directory.path().canonicalize().unwrap().join("partial");
    let run = execute(&loaded, &exact("en", vec![CompositionOutput::Svg, CompositionOutput::Ora]),
        &output, None, &ProcessCancellationToken::new()).unwrap();
    assert_eq!(run.state, CompositionRunState::Partial);
    assert_eq!(run.artifact_manifest.outputs, vec!["outputs/en/page.svg".to_string()]);
    assert!(!output.join("outputs/en/page.ora").exists());
    assert_eq!(file_snapshot(input_root), before);

    let published = run.branches.iter().find(|branch| branch.outcome == CompositionBranchOutcome::Published).unwrap();
    let unavailable = run.branches.iter().find(|branch| branch.outcome == CompositionBranchOutcome::Unavailable).unwrap();
    assert!(unavailable.artifact_evidence_index.is_none() && unavailable.artifact_id.is_none() && unavailable.locator.is_none());
    let artifact = &run.artifact_manifest.artifacts[published.artifact_evidence_index.unwrap()];
    assert_eq!(artifact.lifecycle, ArtifactRole::Terminal);
    assert_eq!(Some(artifact.artifact_id.as_str()), published.artifact_id.as_deref());
    assert_eq!(Some(artifact.locator.as_str()), published.locator.as_deref());
    let bytes = fs::read(output.join(artifact.locator.strip_prefix("bundle:").unwrap())).unwrap();
    assert_eq!(artifact.digest.value, hash_bytes(&bytes));
    assert_eq!(artifact.size_bytes, bytes.len() as u64);
    let sources = run.artifact_manifest.artifacts.iter().filter(|item| item.lifecycle == ArtifactRole::Source).collect::<Vec<_>>();
    assert_eq!(sources.len(), loaded.inputs().len());
    assert!(sources.iter().all(|item| item.validation == ValidationState::ValidWithWarnings
        && item.warnings.iter().any(|warning| warning == "composition.source.validation_scoped")));
    assert_eq!(artifact.sources.iter().cloned().collect::<BTreeSet<_>>(), sources.iter().map(|item| item.artifact_id.clone()).collect::<BTreeSet<_>>());
    for source in sources {
        let retained = fs::read(output.join(source.locator.strip_prefix("bundle:").unwrap())).unwrap();
        assert_eq!(source.digest.value, hash_bytes(&retained));
    }
    let receipt: CompositionRun = serde_json::from_slice(&fs::read(output.join("composition-run.json")).unwrap()).unwrap();
    assert_eq!(receipt.state, CompositionRunState::Partial);
    assert_eq!(receipt.run_id, run.run_id);
    assert_eq!(receipt.artifact_manifest, run.artifact_manifest);
    assert_eq!(receipt.plan_sha256, hash_bytes(&fs::read(output.join("composition-plan.json")).unwrap()));
    let published_plan = receipt.plan.branches.iter().find(|branch| branch.target_id == published.target_id).unwrap();
    assert_eq!(artifact.metadata["renderflow.composition"]["typography"]["report_sha256"],
        hash_json(published_plan.typography.as_ref().unwrap()).unwrap());
    assert!(!receipt.runtime_qualified && !receipt.visual_determinism_qualified && receipt.review_required);
}

#[cfg(unix)]
#[test]
fn linked_cache_internals_are_refused_before_writing_through_them() {
    for (index, relative) in [".tmp", "objects/sha256"].into_iter().enumerate() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = input_fixture(&directory);
        let loaded = LoadedComposition::load(&manifest).unwrap();
        let root = directory.path().canonicalize().unwrap();
        let cache = root.join("cache");
        let outside = root.join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel.txt"), b"preserve me").unwrap();
        let before = file_snapshot(&outside);
        let link = cache.join("store").join(relative);
        fs::create_dir_all(link.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        let output = root.join(format!("refused-{index}"));
        let error = execute(&loaded, &exact("en", vec![CompositionOutput::Svg]), &output,
            Some(&cache), &ProcessCancellationToken::new()).unwrap_err();
        assert!(error.to_string().contains("composition.path.symlink"));
        assert_eq!(file_snapshot(&outside), before);
        assert!(!output.join("composition-run.json").exists());
    }
}


#[test]
fn legacy_requests_keep_existing_targets_and_gain_explicit_pdf_defaults() {
    let request: CompositionRequest = serde_json::from_value(serde_json::json!({
        "locales": { "mode": "exact", "locale": "en" },
        "targets": ["svg", "transcript_json"],
    })).unwrap();
    assert_eq!(request.targets, vec![CompositionOutput::Svg, CompositionOutput::TranscriptJson]);
    assert_eq!(request.raster, raster::RasterSettings::default());
    assert_eq!(request.pdf, pdf::PdfSettings::default());
    assert_eq!(request.pdf.dpi, 300);
    assert_eq!(request.pdf.background, "#ffffff");
    assert_eq!(request.pdf.binding_side, pdf::BindingSide::None);
    assert_eq!(request.pdf.color_policy, "assumed_srgb_no_profile_conversion");
    assert_eq!(request.pdf.conformance, "none");
    assert!(!CompositionRequest::default().targets.contains(&CompositionOutput::Pdf));
}

#[test]
fn pdf_settings_are_canonical_and_change_only_the_pdf_branch() {
    let loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let mut request = exact("en", vec![CompositionOutput::Svg, CompositionOutput::Html,
        CompositionOutput::Png, CompositionOutput::Pdf, CompositionOutput::TranscriptJson]);
    let initial = plan(&loaded, &request).unwrap();
    request.pdf = pdf::PdfSettings {
        dpi: 72, background: "#A1B2C3".into(), ..pdf::PdfSettings::default()
    };
    let changed = plan(&loaded, &request).unwrap();
    for before in &initial.branches {
        let after = changed.branches.iter().find(|branch| branch.locale == before.locale && branch.output == before.output).unwrap();
        assert_eq!(before.state, CompositionBranchState::Planned);
        assert_eq!(after.state, CompositionBranchState::Planned);
        assert_eq!(serde_json::to_value(&before.typography).unwrap(),
            serde_json::to_value(&after.typography).unwrap());
        if before.output == CompositionOutput::Pdf {
            assert_ne!(before.target_id, after.target_id);
            assert_ne!(before.cache_key, after.cache_key);
            assert!(before.raster.is_none() && after.raster.is_none());
            let pdf = after.pdf.as_ref().unwrap();
            assert_eq!(pdf.settings.dpi, 72);
            assert_eq!(pdf.settings.background, "#a1b2c3");
            assert_eq!((pdf.raster.width_px, pdf.raster.height_px), (596, 420));
            assert_eq!(pdf.raster.settings.dpi, pdf.settings.dpi);
            assert_eq!(pdf.raster.settings.background, pdf.settings.background);
            assert!(!pdf.runtime_qualified);
        } else {
            assert_eq!(before.target_id, after.target_id);
            assert_eq!(before.cache_key, after.cache_key);
            assert!(before.pdf.is_none() && after.pdf.is_none());
        }
    }
    request.pdf.background = "#a1b2c3".into();
    let lowercase = plan(&loaded, &request).unwrap();
    assert_eq!(identities(&changed), identities(&lowercase));

    // A binding declaration belongs to PDF output even when no gutter is needed.
    request.pdf.binding_side = pdf::BindingSide::Right;
    let binding = plan(&loaded, &request).unwrap();
    for before in &lowercase.branches {
        let after = binding.branches.iter().find(|branch| branch.locale == before.locale && branch.output == before.output).unwrap();
        assert_eq!(after.state, CompositionBranchState::Planned);
        if before.output == CompositionOutput::Pdf {
            assert_ne!(before.target_id, after.target_id);
            assert_ne!(before.cache_key, after.cache_key);
        } else {
            assert_eq!(before.target_id, after.target_id);
            assert_eq!(before.cache_key, after.cache_key);
        }
    }
}

#[test]
fn invalid_pdf_settings_block_only_pdf_and_do_not_rewrite_other_outputs() {
    let loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let mut request = exact("en", vec![CompositionOutput::Svg, CompositionOutput::Png,
        CompositionOutput::Pdf, CompositionOutput::TranscriptJson]);
    let valid = plan(&loaded, &request).unwrap();
    for invalid in [
        pdf::PdfSettings { dpi: 35, ..pdf::PdfSettings::default() },
        pdf::PdfSettings { dpi: 601, ..pdf::PdfSettings::default() },
        pdf::PdfSettings { background: "transparent".into(), ..pdf::PdfSettings::default() },
        pdf::PdfSettings { background: "#11223380".into(), ..pdf::PdfSettings::default() },
        pdf::PdfSettings { color_policy: "convert_to_cmyk".into(), ..pdf::PdfSettings::default() },
        pdf::PdfSettings { conformance: "pdf_x_4".into(), ..pdf::PdfSettings::default() },
    ] {
        request.pdf = invalid;
        let blocked = plan(&loaded, &request).unwrap();
        for branch in &blocked.branches {
            if branch.output == CompositionOutput::Pdf {
                assert_eq!(branch.state, CompositionBranchState::Blocked);
                assert!(branch.pdf.is_none());
                assert!(branch.diagnostics.iter().any(|item| item.code == "composition.pdf.plan"
                    && item.severity == DiagnosticSeverity::Error));
            } else {
                assert_eq!(branch.state, CompositionBranchState::Planned);
                let previous = valid.branches.iter().find(|item| item.target_id == branch.target_id).unwrap();
                assert_eq!(previous.cache_key, branch.cache_key);
            }
        }
    }
}

#[test]
fn gutter_requires_an_explicit_pdf_binding_and_never_moves_canonical_layers() {
    let mut loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    loaded.manifest.page.gutter_mm = 2.0;
    let before = serde_json::to_value(&loaded.manifest).unwrap();
    let mut request = exact("en", vec![CompositionOutput::Svg, CompositionOutput::Png,
        CompositionOutput::Pdf, CompositionOutput::TranscriptJson]);
    let unresolved = plan(&loaded, &request).unwrap();
    for branch in &unresolved.branches {
        if branch.output == CompositionOutput::Pdf {
            assert_eq!(branch.state, CompositionBranchState::Blocked);
            assert!(branch.pdf.is_none());
        } else {
            assert_eq!(branch.state, CompositionBranchState::Planned);
        }
    }
    request.pdf.binding_side = pdf::BindingSide::Left;
    let resolved = plan(&loaded, &request).unwrap();
    for branch in &resolved.branches {
        assert_eq!(branch.state, CompositionBranchState::Planned);
        if branch.output == CompositionOutput::Pdf {
            assert_eq!(branch.pdf.as_ref().unwrap().settings.binding_side, pdf::BindingSide::Left);
            assert!(!branch.diagnostics.iter().any(|item| item.code == "composition.page.gutter_unresolved"));
        } else if branch.output.visual() {
            assert!(branch.diagnostics.iter().any(|item| item.code == "composition.page.gutter_unresolved"));
        }
    }
    assert_eq!(serde_json::to_value(&loaded.manifest).unwrap(), before);

    // A declared binding does not permit a gutter wider than the authored safe inset.
    loaded.manifest.page.gutter_mm = 5.0;
    let too_wide = plan(&loaded, &request).unwrap();
    for branch in &too_wide.branches {
        assert_eq!(branch.state, if branch.output == CompositionOutput::Pdf {
            CompositionBranchState::Blocked
        } else {
            CompositionBranchState::Planned
        });
    }
}

#[test]
fn pdf_receipts_pin_settings_and_cache_reuses_unchanged_outputs_without_mutating_sources() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let input = root.join("input");
    copy_tree(&example_root().parent().unwrap().join("raster-composition"), &input);
    let before = file_snapshot(&input);
    let loaded = LoadedComposition::load(input.join("page.composition.json")).unwrap();
    let mut request = exact("en", vec![CompositionOutput::Svg, CompositionOutput::Png,
        CompositionOutput::Pdf, CompositionOutput::TranscriptJson]);
    request.pdf = pdf::PdfSettings {
        dpi: 72, background: "#204060".into(), ..pdf::PdfSettings::default()
    };
    let cache = root.join("cache");
    let cancellation = ProcessCancellationToken::new();
    let first = execute(&loaded, &request, &root.join("first"), Some(&cache), &cancellation).unwrap();
    request.pdf.dpi = 144;
    let second = execute(&loaded, &request, &root.join("second"), Some(&cache), &cancellation).unwrap();
    let third = execute(&loaded, &request, &root.join("third"), Some(&cache), &cancellation).unwrap();
    assert_eq!(file_snapshot(&input), before);
    for (index, run) in [&first, &second, &third].into_iter().enumerate() {
        assert_eq!(run.state, CompositionRunState::Complete);
        assert_eq!(run.schema, "renderflow.composition-run/v5");
        for branch in &run.branches {
            let planned = run.plan.branches.iter().find(|plan| plan.target_id == branch.target_id).unwrap();
            let artifact = &run.artifact_manifest.artifacts[branch.artifact_evidence_index.unwrap()];
            let metadata = &artifact.metadata["renderflow.composition"];
            assert_eq!(branch.outcome, CompositionBranchOutcome::Published);
            if planned.output == CompositionOutput::Pdf {
                assert_eq!(metadata["pdf"], serde_json::to_value(planned.pdf.as_ref().unwrap()).unwrap());
                assert!(metadata["raster"].is_null());
                assert_eq!(metadata["typography"]["report_sha256"],
                    hash_json(planned.typography.as_ref().unwrap()).unwrap());
                assert!(artifact.locator.ends_with("/page.pdf"));
            } else {
                assert!(metadata["pdf"].is_null());
            }
            let expected_cache = if index == 0 || (index == 1 && planned.output == CompositionOutput::Pdf) {
                CacheDisposition::Miss
            } else {
                CacheDisposition::Hit
            };
            assert_eq!(branch.cache, expected_cache);
        }
        assert!(!run.runtime_qualified && !run.visual_determinism_qualified && run.review_required);
    }
    let pdf_branch = |run: &CompositionRun| run.branches.iter().find(|branch|
        run.plan.branches.iter().any(|plan| plan.target_id == branch.target_id && plan.output == CompositionOutput::Pdf))
        .unwrap().artifact_id.clone();
    assert_ne!(pdf_branch(&first), pdf_branch(&second));
    assert_eq!(pdf_branch(&second), pdf_branch(&third));
    assert_eq!(fs::read(root.join("second/outputs/en/page.pdf")).unwrap(),
        fs::read(root.join("third/outputs/en/page.pdf")).unwrap());
    let receipt: CompositionRun = serde_json::from_slice(&fs::read(root.join("third/composition-run.json")).unwrap()).unwrap();
    assert_eq!(receipt.plan_sha256, hash_bytes(&fs::read(root.join("third/composition-plan.json")).unwrap()));
    assert_eq!(receipt.artifact_manifest, third.artifact_manifest);
    assert_eq!(receipt.plan.schema, "renderflow.composition-plan/v5");
    assert_eq!(receipt.plan.profile, "renderflow.layered-composition/v5");
}

// Authored, unrun orchestration regressions. A shared-stage observation proves
// successful accesses in the invocation; it is not a persistent cache hit or a
// publication promise for a downstream adapter.
fn assert_common_bundle(output: &Path, native: &CompositionRun) {
    use crate::evidence::{RunManifest, RunState};
    use crate::graph::execution_plan::ForestTargetOutcome;

    let common: RunManifest = serde_json::from_slice(&fs::read(output.join("run-manifest.json")).unwrap()).unwrap();
    assert_eq!(common, native.run_manifest().unwrap());
    assert_eq!(common.run_id, native.run_id);
    assert_eq!(common.artifact_manifest, native.artifact_manifest);
    assert_eq!(common.started_at_unix_ms, native.started_at_unix_ms);
    assert_eq!(common.completed_at_unix_ms, native.completed_at_unix_ms);
    assert_eq!(common.execution_plan_digest.algorithm, "sha256");
    assert_eq!(common.execution_plan_digest.value,
        hash_bytes(&fs::read(output.join("execution-plan.json")).unwrap()));
    assert_eq!(common.source_spec_digest.value, native.plan.composition_sha256);
    assert_eq!(common.state, match native.state {
        CompositionRunState::Complete => RunState::Complete,
        CompositionRunState::Partial => RunState::Partial,
        CompositionRunState::Failed => RunState::Failed,
        CompositionRunState::Cancelled => RunState::Cancelled,
    });
    let forest = common.artifact_forest.as_ref().unwrap();
    let rows = &forest.execution.as_ref().unwrap().targets;
    assert_eq!(forest.branches.len(), native.plan.branches.len());
    assert_eq!(rows.len(), native.branches.len());
    assert_eq!(common.steps.len(), native.branches.len());
    for (((planned, branch), row), step) in native.plan.branches.iter().zip(&native.branches).zip(rows).zip(&common.steps) {
        assert_eq!(planned.target_id, branch.target_id);
        assert_eq!(row.target_id, branch.target_id);
        assert_eq!(row.artifact_evidence_index, branch.artifact_evidence_index);
        assert_eq!(row.artifact_id, branch.artifact_id);
        assert_eq!(row.locator, branch.locator);
        assert!(!row.source_identity);
        assert_eq!(step.step_id, format!("composition.branch:{}", branch.target_id));
        assert_eq!(step.started_at_unix_ms, branch.started_at_unix_ms);
        assert_eq!(step.completed_at_unix_ms, branch.completed_at_unix_ms);
        assert_eq!(step.cache, branch.cache);
        if branch.outcome == CompositionBranchOutcome::Published {
            assert_eq!(row.outcome, ForestTargetOutcome::Published);
            assert_eq!(row.producing_step_id.as_deref(), Some(step.step_id.as_str()));
            let record = &common.artifact_manifest.artifacts[row.artifact_evidence_index.unwrap()];
            assert_eq!(Some(record.artifact_id.as_str()), row.artifact_id.as_deref());
            assert_eq!(Some(record.locator.as_str()), row.locator.as_deref());
            assert_eq!(record.metadata["renderflow.composition"]["target_id"], planned.target_id);
            assert_eq!(record.metadata["renderflow.composition"]["locale"], planned.locale);
            assert_eq!(step.output_artifacts, vec![record.artifact_id.clone()]);
        } else {
            assert!(row.artifact_evidence_index.is_none() && row.artifact_id.is_none() && row.locator.is_none());
            assert!(row.producing_step_id.is_none() && step.output_artifacts.is_empty());
            assert_ne!(row.outcome, ForestTargetOutcome::Published);
        }
    }
    let published = native.branches.iter().filter_map(|branch| branch.artifact_id.clone()).collect::<BTreeSet<_>>();
    assert_eq!(forest.produced_artifacts.iter().cloned().collect::<BTreeSet<_>>(), published);
    assert_eq!(common.diagnostics.iter().filter(|item| item.code == "composition.shared_stage.observed").count(),
        native.shared_intermediates.len());
}

fn stage_consumers(run: &CompositionRun, locale: &str, outputs: &[CompositionOutput]) -> Vec<String> {
    run.plan.branches.iter().filter(|branch| branch.locale == locale && outputs.contains(&branch.output))
        .map(|branch| branch.target_id.clone()).collect::<BTreeSet<_>>().into_iter().collect()
}

#[test]
fn common_receipts_keep_native_publication_rows_and_observe_distinct_locale_work() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let output = directory.path().canonicalize().unwrap().join("localized");
    let selected = request(LocaleSelector::Subset { locales: vec!["en".into(), "de".into()] },
        vec![CompositionOutput::Svg, CompositionOutput::Html, CompositionOutput::TranscriptJson, CompositionOutput::Ora]);
    let run = execute(&loaded, &selected, &output, None, &ProcessCancellationToken::new()).unwrap();
    assert_eq!(run.state, CompositionRunState::Partial);
    assert_common_bundle(&output, &run);
    assert_eq!(run.shared_intermediates.len(), 4);
    for locale in ["de", "en"] {
        let transcript = run.shared_intermediates.iter().find(|stage| stage.locale == locale && stage.kind == "transcript").unwrap();
        let svg = run.shared_intermediates.iter().find(|stage| stage.locale == locale && stage.kind == "svg").unwrap();
        assert_eq!((transcript.materializations, transcript.reuse_count), (1, 2));
        assert_eq!((svg.materializations, svg.reuse_count), (1, 1));
        assert_eq!(transcript.consumer_target_ids, stage_consumers(&run, locale,
            &[CompositionOutput::Svg, CompositionOutput::Html, CompositionOutput::TranscriptJson]));
        assert_eq!(svg.consumer_target_ids, stage_consumers(&run, locale,
            &[CompositionOutput::Svg, CompositionOutput::Html]));
        for (observed, extension) in [(transcript, "transcript.json"), (svg, "svg")] {
            let bytes = fs::read(output.join(format!("outputs/{locale}/page.{extension}"))).unwrap();
            assert_eq!(observed.sha256, hash_bytes(&bytes));
            assert_eq!(observed.size_bytes, bytes.len() as u64);
        }
    }
    for kind in ["transcript", "svg"] {
        let german = run.shared_intermediates.iter().find(|stage| stage.locale == "de" && stage.kind == kind).unwrap();
        let english = run.shared_intermediates.iter().find(|stage| stage.locale == "en" && stage.kind == kind).unwrap();
        assert_ne!(german.sha256, english.sha256);
        assert!(german.consumer_target_ids.iter().all(|id| !english.consumer_target_ids.contains(id)));
    }
}

#[test]
fn terminal_cache_hits_do_not_claim_shared_stage_work_and_mixed_runs_observe_only_misses() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let cache = root.join("cache");
    let cancellation = ProcessCancellationToken::new();
    execute(&loaded, &exact("en", vec![CompositionOutput::Svg]), &root.join("prime"), Some(&cache), &cancellation).unwrap();
    let mut selected = exact("en", vec![CompositionOutput::Svg, CompositionOutput::Html,
        CompositionOutput::TranscriptJson, CompositionOutput::Png]);
    selected.raster.dpi = 36;
    let mixed = execute(&loaded, &selected, &root.join("mixed"), Some(&cache), &cancellation).unwrap();
    assert_eq!(mixed.state, CompositionRunState::Complete);
    assert_common_bundle(&root.join("mixed"), &mixed);
    for branch in &mixed.branches {
        let planned = mixed.plan.branches.iter().find(|item| item.target_id == branch.target_id).unwrap();
        assert_eq!(branch.cache, if planned.output == CompositionOutput::Svg { CacheDisposition::Hit } else { CacheDisposition::Miss });
    }
    assert_eq!(mixed.shared_intermediates.len(), 2);
    let transcript = mixed.shared_intermediates.iter().find(|stage| stage.kind == "transcript").unwrap();
    let svg = mixed.shared_intermediates.iter().find(|stage| stage.kind == "svg").unwrap();
    assert_eq!((transcript.materializations, transcript.reuse_count), (1, 1));
    assert_eq!((svg.materializations, svg.reuse_count), (1, 1));
    assert_eq!(transcript.consumer_target_ids, stage_consumers(&mixed, "en",
        &[CompositionOutput::Html, CompositionOutput::TranscriptJson]));
    assert_eq!(svg.consumer_target_ids, stage_consumers(&mixed, "en",
        &[CompositionOutput::Html, CompositionOutput::Png]));
    assert_eq!(transcript.sha256, hash_bytes(&fs::read(root.join("mixed/outputs/en/page.transcript.json")).unwrap()));
    assert_eq!(svg.sha256, hash_bytes(&fs::read(root.join("mixed/outputs/en/page.svg")).unwrap()));

    let cached = execute(&loaded, &selected, &root.join("cached"), Some(&cache), &cancellation).unwrap();
    assert_eq!(cached.state, CompositionRunState::Complete);
    assert!(cached.branches.iter().all(|branch| branch.cache == CacheDisposition::Hit));
    assert!(cached.shared_intermediates.is_empty());
    assert_common_bundle(&root.join("cached"), &cached);
    let common = cached.run_manifest().unwrap();
    assert!(common.steps.iter().all(|step| step.state == crate::evidence::StepState::Reused));
}

#[test]
fn blocked_pdf_settings_remain_local_in_common_execution_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let output = directory.path().canonicalize().unwrap().join("blocked-pdf");
    let mut selected = exact("en", vec![CompositionOutput::Svg, CompositionOutput::Pdf, CompositionOutput::TranscriptJson]);
    selected.pdf.conformance = "pdf_x_4".into();
    let run = execute(&loaded, &selected, &output, None, &ProcessCancellationToken::new()).unwrap();
    assert_eq!(run.state, CompositionRunState::Partial);
    assert_common_bundle(&output, &run);
    let common = run.run_manifest().unwrap();
    let rows = &common.artifact_forest.as_ref().unwrap().execution.as_ref().unwrap().targets;
    for ((planned, branch), row) in run.plan.branches.iter().zip(&run.branches).zip(rows) {
        if planned.output == CompositionOutput::Pdf {
            assert_eq!(branch.outcome, CompositionBranchOutcome::Blocked);
            assert_eq!(row.outcome, crate::graph::execution_plan::ForestTargetOutcome::NotSelected);
            assert!(run.shared_intermediates.iter().all(|stage| !stage.consumer_target_ids.contains(&planned.target_id)));
        } else {
            assert_eq!(branch.outcome, CompositionBranchOutcome::Published);
        }
    }
    assert!(!output.join("outputs/en/page.pdf").exists());
    assert_eq!(run.artifact_manifest.outputs.len(), 2);
}

#[test]
fn common_projection_rejects_mismatched_native_publication_evidence() {
    let directory = tempfile::tempdir().unwrap();
    let loaded = LoadedComposition::load(example_root().join("page.composition.json")).unwrap();
    let output = directory.path().canonicalize().unwrap().join("receipt");
    let run = execute(&loaded, &exact("en", vec![CompositionOutput::TranscriptJson]),
        &output, None, &ProcessCancellationToken::new()).unwrap();
    assert_common_bundle(&output, &run);
    let record_index = run.branches[0].artifact_evidence_index.unwrap();
    assert!(record_index > 0, "captured source records precede terminal records");
    for mutation in [
        "source_index", "missing_index", "artifact_id", "locator", "target_id", "metadata", "outputs", "cache",
        "plan_qualification", "validation_promotion", "fidelity_promotion", "engine_metadata", "renderer_metadata",
        "typography_metadata", "cached_stage_consumer", "correlated_forged_id", "stage_digest", "stage_size",
    ] {
        let mut altered = run.clone();
        match mutation {
            "source_index" => altered.branches[0].artifact_evidence_index = Some(0),
            "missing_index" => altered.branches[0].artifact_evidence_index = Some(altered.artifact_manifest.artifacts.len()),
            "artifact_id" => altered.branches[0].artifact_id = Some("artifact:sha256:forged".into()),
            "locator" => altered.branches[0].locator = Some("bundle:outputs/de/page.transcript.json".into()),
            "target_id" => altered.branches[0].target_id = "target-forged".into(),
            "metadata" => altered.artifact_manifest.artifacts[record_index].metadata
                .get_mut("renderflow.composition").unwrap()["locale"] = Value::String("de".into()),
            "outputs" => altered.artifact_manifest.outputs.push("outputs/en/unpublished.pdf".into()),
            "cache" => altered.artifact_manifest.artifacts[record_index].cache = CacheDisposition::Hit,
            "plan_qualification" => {
                altered.plan.runtime_qualified = true;
                // Keep the receipt digest consistent so the qualification gate
                // is exercised independently of ordinary plan corruption.
                let mut bytes = serde_json::to_vec_pretty(&altered.plan).unwrap();
                bytes.push(b'\n');
                altered.plan_sha256 = hash_bytes(&bytes);
            },
            "validation_promotion" => altered.artifact_manifest.artifacts[record_index].validation = ValidationState::Valid,
            "fidelity_promotion" => altered.artifact_manifest.artifacts[record_index].fidelity = crate::evidence::FidelityDeclaration::Lossless,
            "engine_metadata" => altered.artifact_manifest.artifacts[record_index].metadata
                .get_mut("renderflow.composition").unwrap()["version"] = Value::String("forged".into()),
            "renderer_metadata" => altered.artifact_manifest.artifacts[record_index].metadata
                .get_mut("renderflow.composition").unwrap()["renderer"]["version"] = serde_json::json!(999),
            "typography_metadata" => altered.artifact_manifest.artifacts[record_index].metadata
                .get_mut("renderflow.composition").unwrap()["typography"] = serde_json::json!({
                    "schema": "renderflow.composition-typography/v1", "report_sha256": "0".repeat(64),
                }),
            "cached_stage_consumer" => {
                // Agree on a terminal hit while retaining the impossible claim
                // that this invocation materialized its transcript stage.
                altered.branches[0].cache = CacheDisposition::Hit;
                altered.artifact_manifest.artifacts[record_index].cache = CacheDisposition::Hit;
            },
            "correlated_forged_id" => {
                let forged = format!("artifact:sha256:{}", "0".repeat(64));
                altered.branches[0].artifact_id = Some(forged.clone());
                altered.artifact_manifest.artifacts[record_index].artifact_id = forged;
            },
            "stage_digest" => altered.shared_intermediates[0].sha256 = "0".repeat(64),
            "stage_size" => altered.shared_intermediates[0].size_bytes += 1,
            _ => unreachable!(),
        }
        assert!(altered.run_manifest().is_err(), "must reject {mutation} mismatch");
    }
}
