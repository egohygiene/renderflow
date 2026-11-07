//! Renderer-neutral presentations and an explicit local Slidev publication profile.
//!
//! Standalone and v2 build entry points share frozen capture and execution.

pub(crate) mod render;
pub mod slidev;

use std::collections::BTreeSet;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use render::{build, plan, PresentationPlan, PresentationRequest, PresentationTarget};

pub const SCHEMA: &str = "renderflow.presentation/v1";
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_ASSET_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_TOTAL_ASSET_BYTES: usize = 64 * 1024 * 1024;
pub const PROFILE_VERSION: &str = "presentation-slidev-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRevision {
    pub repository: String,
    pub revision: String,
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState { Draft, Reviewed }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme { Paper, Night }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Citation { pub label: String, pub url: String }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    pub path: String,
    pub sha256: String,
    pub alt: String,
    #[serde(default)]
    pub caption: Option<String>,
    pub rights: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Slide {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub section: Option<String>,
    #[serde(default)]
    pub paragraphs: Vec<String>,
    #[serde(default)]
    pub bullets: Vec<String>,
    #[serde(default)]
    pub images: Vec<Image>,
    #[serde(default)]
    pub citations: Vec<Citation>,
    #[serde(default)]
    pub speaker_notes: Option<String>,
}

/// Text is literal Unicode text, never Vue/Markdown/JavaScript supplied to a renderer.
/// Review and rights fields are author declarations, not verified approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Presentation {
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub language: String,
    pub source: SourceRevision,
    /// Explicit source-freshness label supplied by the author; no clock inference.
    pub as_of: String,
    pub rights: String,
    pub review_state: ReviewState,
    pub theme: Theme,
    /// The initial profile supports a static 16:9 deck with no transition effects.
    pub transition: String,
    pub slides: Vec<Slide>,
}

pub fn parse(bytes: &[u8]) -> Result<Presentation> {
    ensure!(bytes.len() <= MAX_INPUT_BYTES, "presentation.input.limit: exceeds 4 MiB");
    let value: Presentation = serde_json::from_slice(bytes).context("presentation.input.json")?;
    validate(&value)?;
    Ok(value)
}

pub fn validate(value: &Presentation) -> Result<()> {
    ensure!(value.schema_version == SCHEMA, "presentation.schema: expected {SCHEMA}");
    identifier(&value.id)?;
    text(&value.title, 512)?;
    ensure!(value.language.len() <= 64 && value.language.split('-').next().is_some_and(|part|
        (2..=8).contains(&part.len()) && part.bytes().all(|ch| ch.is_ascii_alphabetic()))
        && value.language.split('-').all(|part|
        !part.is_empty() && part.chars().all(|ch| ch.is_ascii_alphanumeric())),
        "presentation.language: expected an explicit language tag");
    text(&value.as_of, 128)?;
    text(&value.rights, 4096)?;
    http_url(&value.source.repository)?;
    ensure!(value.source.revision.len() == 40 && value.source.revision.bytes().all(|ch| ch.is_ascii_hexdigit()),
        "presentation.source.revision: expected a full Git revision");
    relative_path(&value.source.path)?;
    digest(&value.source.sha256)?;
    ensure!(value.transition == "none", "presentation.transition: only none is implemented");
    ensure!(!value.slides.is_empty() && value.slides.len() <= 300, "presentation.slides.limit: expected 1..300 slides");
    let mut ids = BTreeSet::new();
    let mut image_count = 0;
    for slide in &value.slides {
        identifier(&slide.id)?;
        ensure!(ids.insert(&slide.id), "presentation.slide.id: duplicate ID");
        text(&slide.title, 512)?;
        if let Some(section) = &slide.section { text(section, 512)?; }
        ensure!(slide.paragraphs.len() + slide.bullets.len() <= 64, "presentation.slide.limit: at most 64 text blocks");
        for block in slide.paragraphs.iter().chain(&slide.bullets) { text(block, 16 * 1024)?; }
        if let Some(notes) = &slide.speaker_notes { text(notes, 32 * 1024)?; }
        ensure!(slide.citations.len() <= 32, "presentation.citations.limit");
        for citation in &slide.citations { text(&citation.label, 1024)?; http_url(&citation.url)?; }
        ensure!(slide.images.len() <= 8, "presentation.images.limit: at most 8 per slide");
        image_count += slide.images.len();
        for image in &slide.images {
            relative_path(&image.path)?;
            let extension = Path::new(&image.path).extension().and_then(|s| s.to_str()).unwrap_or("").to_ascii_lowercase();
            ensure!(matches!(extension.as_str(), "png" | "jpg" | "jpeg"), "presentation.image.format: PNG/JPEG only");
            digest(&image.sha256)?;
            text(&image.alt, 4096)?;
            text(&image.rights, 4096)?;
            if let Some(caption) = &image.caption { text(caption, 4096)?; }
        }
    }
    ensure!(image_count <= 128, "presentation.images.limit: at most 128 references");
    Ok(())
}

pub(crate) fn identifier(value: &str) -> Result<()> {
    ensure!(!value.is_empty() && value.len() <= 128 && value.bytes().all(|ch|
        ch.is_ascii_alphanumeric() || matches!(ch, b'-' | b'_' | b'.')), "presentation.id: invalid ID");
    Ok(())
}

pub(crate) fn text(value: &str, limit: usize) -> Result<()> {
    ensure!(!value.trim().is_empty() && value.len() <= limit && !value.chars().any(|ch|
        ch.is_control() && ch != '\n' && ch != '\t'), "presentation.text: empty, oversized, or control characters");
    Ok(())
}

pub(crate) fn http_url(value: &str) -> Result<()> {
    ensure!((value.starts_with("https://") || value.starts_with("http://")) && value.len() <= 4096
        && !value.chars().any(|ch| ch.is_whitespace() || ch.is_control() || matches!(ch, '<' | '>' | '"' | '\'' | '\\' | '`' | '{' | '}')),
        "presentation.url: expected explicit HTTP(S) URL");
    let authority = value.split_once("://").map(|(_, tail)| tail.split('/').next().unwrap_or("")).unwrap_or("");
    ensure!(!authority.is_empty() && !authority.contains('@'), "presentation.url: empty host or embedded credentials");
    Ok(())
}

pub(crate) fn relative_path(value: &str) -> Result<()> {
    ensure!(!value.is_empty() && value.len() <= 1024 && !value.contains(['\\', ':', '%', '#', '?'])
        && value.split('/').all(|part| !part.is_empty() && part != "." && part != "..")
        && Path::new(value).components().all(|part| matches!(part, Component::Normal(_))),
        "presentation.path: expected a confined relative POSIX path");
    Ok(())
}

pub(crate) fn digest(value: &str) -> Result<()> {
    ensure!(value.len() == 64 && value.bytes().all(|ch| ch.is_ascii_digit() || (b'a'..=b'f').contains(&ch)),
        "presentation.digest: expected lowercase SHA-256");
    Ok(())
}

pub(crate) fn sha256(bytes: &[u8]) -> String { format!("{:x}", Sha256::digest(bytes)) }

pub(crate) fn read_file(path: &Path, limit: usize) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path).context("presentation.file.metadata")?;
    ensure!(metadata.is_file() && !metadata.file_type().is_symlink() && metadata.len() <= limit as u64,
        "presentation.file: expected bounded regular file");
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= limit, "presentation.file.limit");
    Ok(bytes)
}

/// Reject symlink components and capture exact local image bytes. This is not
/// protection against a hostile concurrent filesystem; no OS sandbox is claimed.
pub(crate) fn asset_path(root: &Path, relative: &str) -> Result<PathBuf> {
    relative_path(relative)?;
    let mut path = root.to_path_buf();
    for part in Path::new(relative).components() {
        path.push(part.as_os_str());
        ensure!(!fs::symlink_metadata(&path)?.file_type().is_symlink(), "presentation.asset.symlink");
    }
    ensure!(fs::canonicalize(&path)?.starts_with(fs::canonicalize(root)?), "presentation.asset.escape");
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_model_refuses_duplicate_ids_unknown_fields_and_invented_transitions() {
        let fixture = include_bytes!("../../../../examples/presentation/synthetic.presentation.json");
        let mut model = parse(fixture).unwrap();
        model.slides[1].id = model.slides[0].id.clone();
        assert!(validate(&model).is_err());
        model.slides[1].id = "second".into();
        model.transition = "fade".into();
        assert!(validate(&model).is_err());
        let mut value: serde_json::Value = serde_json::from_slice(fixture).unwrap();
        value["javascript"] = serde_json::json!("alert(1)");
        assert!(parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }

    #[test]
    fn relative_asset_paths_and_citation_urls_cannot_escape_the_model_contract() {
        for path in ["../private.png", "/private.png", "C:\\private.png", "https://host/image.png", "assets/%2e.png"] {
            assert!(relative_path(path).is_err());
        }
        for url in ["javascript:alert(1)", "https://name:password@host/", "https://host/\"onclick"] {
            assert!(http_url(url).is_err());
        }
    }
}
