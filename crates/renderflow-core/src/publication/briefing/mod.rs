//! Local, reviewable repository briefings from pinned text and supplied scripts.
//! Standalone and common-build entry points share this publication owner.

mod model;
mod render;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::process::ProcessCancellationToken;
use crate::publication::local::{self, Capture};

pub use model::{parse, validate, Briefing, Chapter, Feed, Narration, NarrationOrigin,
    ReviewClaim, Source, SourceLocation, PROFILE, SCHEMA};
pub use crate::publication::local::FileSubject;

#[derive(Debug, Clone)]
pub struct BriefingRequest {
    pub input: PathBuf,
    pub output_dir: PathBuf,
    /// Explicit candidate packaging; does not approve source, narration or publication.
    pub allow_draft: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceStatus {
    pub source_id: String,
    pub revision: String,
    pub declared_sha256: String,
    pub observed_sha256: String,
    pub current: bool,
    pub review_declared: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BriefingPlan {
    pub schema_version: String,
    pub profile: String,
    pub engine_version: String,
    pub episode_id: String,
    pub input_sha256: String,
    pub script_sha256: String,
    pub sources: Vec<SourceStatus>,
    pub freshness: String,
    pub reviews_declared: bool,
    pub narration_duration_ms: Option<u64>,
    pub narration_script_current: Option<bool>,
    pub expected_files: Vec<FileSubject>,
    pub runtime_qualified: bool,
    pub publication_approval: String,
}

pub(crate) struct Captured {
    pub(crate) plan: BriefingPlan,
    pub(crate) files: BTreeMap<String, Vec<u8>>,
}

/// Shared CLI/SDK resolution. It reads bounded local snapshots, never fetches
/// repository revisions, invokes a provider or creates the output directory.
pub fn plan(input: &Path, token: ProcessCancellationToken) -> Result<BriefingPlan> {
    Ok(capture(input, token)?.plan)
}

pub(crate) fn capture(input: &Path, token: ProcessCancellationToken) -> Result<Captured> {
    ensure!(!token.is_cancelled(), "briefing.cancelled");
    let bytes = local::read_file(input, local::MAX_JSON)?;
    let model = parse(&bytes)?;
    let parent = input.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let mut capture = Capture::new(parent, token)?;
    let mut files = BTreeMap::new();
    let mut sources = Vec::new();
    let mut line_counts = BTreeMap::new();
    for source in &model.sources {
        let payload = capture.read(&source.path, 1024 * 1024)?;
        let text = std::str::from_utf8(&payload).context("briefing.source.utf8")?;
        line_counts.insert(source.id.as_str(), text.lines().count());
        let observed = local::sha256(&payload);
        sources.push(SourceStatus {source_id: source.id.clone(), revision: source.revision.clone(),
            declared_sha256: source.sha256.clone(), current: source.sha256 == observed,
            observed_sha256: observed, review_declared: source.review.is_some()});
        files.insert(format!("sources/{}.txt", source.id), payload);
    }
    for chapter in &model.chapters {
        for location in &chapter.sources {
            ensure!(location.end_line <= *line_counts.get(location.source_id.as_str()).context("briefing.source.id")?,
                "briefing.chapter.source.bounds: selected lines do not exist");
        }
    }
    let script = json!({"schema_version":"renderflow.briefing-script/v1", "episode_id":model.id,
        "title":model.title, "language":model.language, "as_of":model.as_of, "rights":model.rights,
        "sources":model.sources, "chapters":model.chapters});
    let script_bytes = serde_json::to_vec_pretty(&script)?;
    let script_sha256 = local::sha256(&script_bytes);
    if let Some(review) = &model.script_review { model::review_claim(review, &script_sha256)?; }
    let duration = if let Some(narration) = &model.narration {
        let audio = capture.read(&narration.path, 32 * 1024 * 1024)?;
        ensure!(local::sha256(&audio) == narration.sha256, "briefing.narration.changed");
        // Reuse the existing strict ten-minute mono/16 kHz/signed PCM16 inspector.
        let duration = crate::speech::provider::inspect_pcm_wav(&audio)?;
        for chapter in &model.chapters {
            ensure!(chapter.start_ms.is_none_or(|start| start < duration), "briefing.chapter.clock.duration");
        }
        files.insert("narration.wav".into(), audio);
        Some(duration)
    } else { None };
    let current = sources.iter().all(|source| source.current);
    let reviewed = model.sources.iter().all(|source| source.review.is_some())
        && model.script_review.is_some() && model.narration.as_ref().is_none_or(|audio| audio.review.is_some());
    files.insert("briefing.json".into(), bytes.clone());
    files.insert("script.json".into(), script_bytes);
    files.insert("transcript.txt".into(), render::text(&model, current).into_bytes());
    files.insert("transcript.html".into(), render::html(&model, current).into_bytes());
    files.insert("chapters.json".into(), serde_json::to_vec_pretty(&json!({
        "schema_version":"renderflow.briefing-chapters/v1", "script_sha256":script_sha256,
        "timing_authority":if model.chapters.iter().any(|chapter|chapter.start_ms.is_some()) {"operator_assertion"} else {"untimed"},
        "chapters":model.chapters.iter().map(|chapter|json!({"id":chapter.id,"title":chapter.title,
            "start_ms":chapter.start_ms,"sources":chapter.sources})).collect::<Vec<_>>()
    }))?);
    if let Some(feed) = &model.feed {
        files.insert("feed.rss".into(), render::feed(&model, feed, &local::sha256(&bytes), current,
            files.get("narration.wav").map(|audio|audio.len())).into_bytes());
    }
    capture.check()?;
    let expected_files = files.iter().map(|(path, bytes)| FileSubject {path:path.clone(),
        sha256:local::sha256(bytes),size_bytes:bytes.len() as u64}).collect();
    let narration_script_current = model.narration.as_ref().map(|audio|audio.script_sha256 == script_sha256);
    Ok(Captured {plan: BriefingPlan {schema_version:"renderflow.briefing-plan/v1".into(),
        profile:PROFILE.into(),engine_version:env!("CARGO_PKG_VERSION").into(),
        episode_id:model.id,input_sha256:local::sha256(&bytes),script_sha256,
        sources,freshness:if current {"current_local_snapshot"} else {"stale"}.into(),
        reviews_declared:reviewed,narration_duration_ms:duration,
        narration_script_current,expected_files,
        runtime_qualified:false,publication_approval:"not_granted".into()},files})
}

pub fn build(request: &BriefingRequest, token: ProcessCancellationToken) -> Result<Value> {
    let captured = capture(&request.input, token.clone())?;
    build_captured(request, &captured, token)
}

/// Publish the exact bounded snapshot resolved by the common build facade.
/// Review declarations remain declarations; no source/provider is reloaded.
pub(crate) fn build_captured(request: &BriefingRequest, captured: &Captured, token: ProcessCancellationToken) -> Result<Value> {
    ensure!(captured.plan.freshness != "stale", "briefing.source.stale: update the source binding and review before building");
    ensure!(captured.plan.narration_script_current != Some(false), "briefing.narration.script.changed: plan reports the current script digest");
    ensure!(captured.plan.reviews_declared || request.allow_draft,
        "briefing.review.required: supply exact review declarations or explicitly select draft packaging");
    let receipt = json!({"schema_version":"renderflow.briefing-run/v1", "state":"complete",
        "profile":PROFILE,"plan":captured.plan,"review_state":"candidate",
        "publication_approval":"not_granted","draft_packaging_selected":request.allow_draft,
        "alignment":"Any supplied audio/script binding and chapter times are operator assertions, not measured alignment.",
        "provider_executed":false,"remote_publication_performed":false,"runtime_qualified":false});
    local::publish(&request.output_dir, &captured.files, "briefing-run.json", receipt, token)
}
