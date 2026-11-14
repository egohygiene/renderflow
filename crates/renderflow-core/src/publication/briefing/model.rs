use std::collections::BTreeSet;

use anyhow::{ensure, Result};
use serde::{Deserialize, Serialize};

use crate::evidence::files;
use crate::publication::local;

pub const SCHEMA: &str = "renderflow.briefing/v1";
pub const PROFILE: &str = "briefing-local-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewClaim {
    pub sha256: String,
    /// A declaration/reference, not proof of an authenticated human decision.
    pub reference: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: String,
    pub path: String,
    pub sha256: String,
    pub revision: String,
    pub canonical_url: Option<String>,
    pub rights: String,
    pub review: Option<ReviewClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceLocation {
    pub source_id: String,
    /// Inclusive, one-based line range in the exact pinned UTF-8 source.
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chapter {
    pub id: String,
    pub title: String,
    pub text: String,
    pub sources: Vec<SourceLocation>,
    /// Explicit operator-supplied audio position; never inferred from text.
    pub start_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NarrationOrigin {
    HumanRecording,
    SuppliedSynthetic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Narration {
    pub path: String,
    pub sha256: String,
    pub script_sha256: String,
    pub origin: NarrationOrigin,
    pub voice_reference: String,
    pub consent_reference: String,
    pub provider_reference: Option<String>,
    pub model_reference: Option<String>,
    pub rights: String,
    pub review: Option<ReviewClaim>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feed {
    pub title: String,
    pub description: String,
    pub site_url: String,
    /// Explicit final public location; generation does not upload to it.
    pub episode_url: String,
    pub narration_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Briefing {
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub language: String,
    pub as_of: String,
    pub rights: String,
    pub sources: Vec<Source>,
    pub chapters: Vec<Chapter>,
    pub script_review: Option<ReviewClaim>,
    pub narration: Option<Narration>,
    pub feed: Option<Feed>,
}

pub fn parse(bytes: &[u8]) -> Result<Briefing> {
    let model: Briefing = local::parse(bytes)?;
    validate(&model)?;
    Ok(model)
}

pub fn validate(model: &Briefing) -> Result<()> {
    ensure!(model.schema_version == SCHEMA, "briefing.schema");
    local::identifier(&model.id)?;
    local::text(&model.title, 512)?;
    local::language(&model.language)?;
    local::text(&model.as_of, 128)?;
    local::text(&model.rights, 4096)?;
    ensure!(
        !model.sources.is_empty() && model.sources.len() <= 16,
        "briefing.sources.count"
    );
    ensure!(
        !model.chapters.is_empty() && model.chapters.len() <= 64,
        "briefing.chapters.count"
    );
    let mut ids = BTreeSet::new();
    for source in &model.sources {
        local::identifier(&source.id)?;
        ensure!(ids.insert(source.id.as_str()), "briefing.source.duplicate");
        files::components(&source.path, 64)?;
        files::digest(&source.sha256)?;
        local::text(&source.revision, 256)?;
        local::text(&source.rights, 4096)?;
        if let Some(url) = &source.canonical_url {
            local::url(url)?;
        }
        if let Some(review) = &source.review {
            review_claim(review, &source.sha256)?;
        }
    }
    let mut chapters = BTreeSet::new();
    let mut previous = None;
    let timed = model
        .chapters
        .iter()
        .filter(|chapter| chapter.start_ms.is_some())
        .count();
    ensure!(
        timed == 0 || (model.narration.is_some() && timed == model.chapters.len()),
        "briefing.chapters.clock: all or none, with supplied narration"
    );
    for chapter in &model.chapters {
        local::identifier(&chapter.id)?;
        ensure!(chapters.insert(&chapter.id), "briefing.chapter.duplicate");
        local::text(&chapter.title, 512)?;
        local::text(&chapter.text, 32 * 1024)?;
        ensure!(
            !chapter.sources.is_empty() && chapter.sources.len() <= 32,
            "briefing.chapter.sources.count"
        );
        for location in &chapter.sources {
            ensure!(
                ids.contains(location.source_id.as_str())
                    && location.start_line > 0
                    && location.start_line <= location.end_line
                    && location.end_line <= 100_000,
                "briefing.chapter.source.range"
            );
        }
        if let Some(start) = chapter.start_ms {
            ensure!(
                previous.is_none_or(|last| start > last),
                "briefing.chapter.clock.order"
            );
            previous = Some(start);
        }
    }
    if let Some(narration) = &model.narration {
        files::components(&narration.path, 64)?;
        files::digest(&narration.sha256)?;
        files::digest(&narration.script_sha256)?;
        for value in [
            &narration.voice_reference,
            &narration.consent_reference,
            &narration.rights,
        ] {
            local::text(value, 4096)?;
        }
        if narration.origin == NarrationOrigin::SuppliedSynthetic {
            local::text(narration.provider_reference.as_deref().unwrap_or(""), 4096)?;
            local::text(narration.model_reference.as_deref().unwrap_or(""), 4096)?;
        }
        if let Some(review) = &narration.review {
            review_claim(review, &narration.sha256)?;
        }
    }
    if let Some(feed) = &model.feed {
        local::text(&feed.title, 512)?;
        local::text(&feed.description, 4096)?;
        local::url(&feed.site_url)?;
        local::url(&feed.episode_url)?;
        ensure!(
            feed.narration_url.is_some() == model.narration.is_some(),
            "briefing.feed.narration: supply explicit URL only with audio"
        );
        if let Some(url) = &feed.narration_url {
            local::url(url)?;
        }
    }
    Ok(())
}

pub(super) fn review_claim(review: &ReviewClaim, digest: &str) -> Result<()> {
    files::digest(&review.sha256)?;
    local::text(&review.reference, 4096)?;
    ensure!(
        review.sha256 == digest,
        "briefing.review.changed: declaration does not bind selected bytes"
    );
    Ok(())
}
