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
    CompositionRequest { locales, targets }
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
        CompositionOutput::TranscriptJson, CompositionOutput::Pdf]);
    let before = plan(&loaded, &request).unwrap();
    assert_eq!(before.schema, "renderflow.composition-plan/v2");
    assert_eq!(before.profile, "renderflow.layered-svg/v2");
    let svg = before.branches.iter().find(|branch| branch.output == CompositionOutput::Svg).unwrap();
    let html = before.branches.iter().find(|branch| branch.output == CompositionOutput::Html).unwrap();
    let report = serde_json::to_value(svg.typography.as_ref().unwrap()).unwrap();
    assert_eq!(report, serde_json::to_value(html.typography.as_ref().unwrap()).unwrap());
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
        assert_eq!(run.schema, "renderflow.composition-run/v2");
        let artifact = &run.artifact_manifest.artifacts[run.branches[0].artifact_evidence_index.unwrap()];
        assert_eq!(artifact.metadata["renderflow.composition"]["typography"]["report_sha256"], report_digest);
        assert_eq!(artifact.metadata["renderflow.composition"]["renderer"]["version"], 2);
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
    let run = execute(&loaded, &exact("en", vec![CompositionOutput::Svg, CompositionOutput::Pdf]),
        &output, None, &ProcessCancellationToken::new()).unwrap();
    assert_eq!(run.state, CompositionRunState::Partial);
    assert_eq!(run.artifact_manifest.outputs, vec!["outputs/en/page.svg".to_string()]);
    assert!(!output.join("outputs/en/page.pdf").exists());
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
