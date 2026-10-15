//! Receipt-last publication of scoped composition branches and verified cache
//! payload reuse. Creative approval is never a consequence of a cache hit.

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::{hash_bytes, hash_json, plan, render, CompositionBranchState, CompositionDiagnostic,
    CompositionPlan, CompositionRequest, DiagnosticSeverity, LoadedComposition,
    COMPOSITION_PROFILE, COMPOSITION_RUN_SCHEMA_V4};
use crate::artifact::{Artifact, ArtifactDescriptor, ArtifactId, ArtifactStorageClass, ArtifactStore,
    CanonicalFormat, MediaType};
use crate::evidence::{ArtifactEvidence, ArtifactManifest, ArtifactRole, CacheDisposition,
    FidelityDeclaration, ProducerEvidence, ValidationState, ARTIFACT_MANIFEST_SCHEMA_V1};
use crate::process::ProcessCancellationToken;

const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_BUNDLE_OUTPUT_BYTES: usize = 512 * 1024 * 1024;
const CACHE_SCHEMA: &str = "renderflow.composition-cache-entry/v4";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionRunState { Complete, Partial, Failed, Cancelled }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompositionBranchOutcome { Published, Blocked, Unavailable, Failed, Cancelled }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionBranchRun {
    pub target_id: String,
    pub outcome: CompositionBranchOutcome,
    pub cache: CacheDisposition,
    pub artifact_evidence_index: Option<usize>,
    pub artifact_id: Option<String>,
    pub locator: Option<String>,
    pub diagnostics: Vec<CompositionDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositionRun {
    pub schema: String,
    pub run_id: String,
    pub state: CompositionRunState,
    pub plan_sha256: String,
    pub plan: CompositionPlan,
    pub artifact_manifest: ArtifactManifest,
    pub branches: Vec<CompositionBranchRun>,
    pub runtime_qualified: bool,
    pub review_required: bool,
    pub visual_determinism_qualified: bool,
}

impl CompositionRun {
    pub fn successful(&self) -> bool { self.state == CompositionRunState::Complete }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CacheEntry {
    schema: String,
    key: String,
    profile: String,
    artifact: Artifact,
}

/// Execute the same plan exposed to SDK and CLI callers. Output must be a new
/// directory. Existing source paths are never used as output destinations.
pub fn execute(
    snapshot: &LoadedComposition,
    request: &CompositionRequest,
    output: &Path,
    cache_directory: Option<&Path>,
    cancellation: &ProcessCancellationToken,
) -> Result<CompositionRun> {
    if cancellation.is_cancelled() { bail!("composition.cancelled: cancelled before planning"); }
    let plan = plan(snapshot, request)?;
    let plan_bytes = json_bytes(&plan)?;
    let plan_sha256 = hash_bytes(&plan_bytes);
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
    let run_id = format!("run:sha256:{}", hash_json(&(plan_sha256.as_str(), now, std::process::id()))?);
    refuse_links(output)?;
    if fs::symlink_metadata(output).is_ok() { bail!("composition.output.exists: output directory must be new"); }
    let parent = output.parent().filter(|value| !value.as_os_str().is_empty()).unwrap_or(Path::new("."));
    if !parent.is_dir() { bail!("composition.output.parent: output parent must exist"); }
    if cancellation.is_cancelled() { bail!("composition.cancelled: cancelled before publication"); }
    fs::create_dir(output).context("composition.output.create: cannot create new output directory")?;
    let output = output.canonicalize()?;
    let cache_root = cache_directory.map(Path::to_path_buf).unwrap_or_else(|| output.join(".artifact-store"));
    refuse_links(&cache_root)?;
    fs::create_dir_all(&cache_root)?;
    let cache_root = cache_root.canonicalize()?;
    refuse_links(&cache_root.join("store"))?;
    refuse_links(&cache_root.join("entries"))?;
    fs::create_dir_all(cache_root.join("entries"))?;
    check_store_paths(&cache_root.join("store"), None)?;
    let store = ArtifactStore::new(cache_root.join("store"))?;
    fs::create_dir(output.join("inputs"))?;
    fs::create_dir(output.join("outputs"))?;
    write_new(&output.join("composition-plan.json"), &plan_bytes)?;
    let mut records = Vec::new();
    let mut source_ids = Vec::<ArtifactId>::new();
    let mut copied_inputs = BTreeSet::new();
    for input in &snapshot.inputs {
        if cancellation.is_cancelled() { break; }
        let key = format!("{}:{}", input.kind, input.id);
        let bytes = snapshot.input_bytes.get(&key).context("composition.input.snapshot_missing")?;
        if hash_bytes(bytes) != input.sha256 || bytes.len() as u64 != input.size_bytes {
            bail!("composition.input.snapshot_changed: captured input no longer matches its evidence");
        }
        let (format, media) = input_type(snapshot, &input.kind, &input.id);
        check_store_paths(store.root(), Some(&input.sha256))?;
        let artifact = store.put_bytes(bytes, ArtifactDescriptor::new(
            CanonicalFormat::new(format)?, MediaType::new(media)?, ArtifactStorageClass::Source))?;
        store.verify(&artifact)?;
        source_ids.push(artifact.id().clone());
        let relative = format!("inputs/{}", input.sha256);
        if copied_inputs.insert(input.sha256.clone()) { write_new(&output.join(&relative), bytes)?; }
        let mut evidence = ArtifactEvidence::from_artifact(&artifact, key, ArtifactRole::Source,
            format!("bundle:{relative}"), ProducerEvidence::source(), ValidationState::ValidWithWarnings,
            FidelityDeclaration::Lossless);
        evidence.warnings.push("composition.source.validation_scoped".into());
        evidence.metadata.insert("renderflow.composition.source_validation".into(), serde_json::json!({
            "captured_bytes_and_digest_verified": true,
            "validation_scope": "contract structure, confined bounded bytes, source-specific signature/metadata checks; not complete raster/font decoding, glyph coverage, visual fidelity or creative approval",
            "kind": input.kind, "id": input.id,
            "declared_pin": input.kind != "font_license_observed" && input.kind != "manifest",
            "visual_qualified": false,
        }));
        records.push(evidence);
    }
    source_ids.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    source_ids.dedup();
    let mut branches = Vec::new();
    let mut outputs = Vec::new();
    let mut total_bytes = 0usize;
    for branch in &plan.branches {
        let mut result = CompositionBranchRun {
            target_id: branch.target_id.clone(), outcome: CompositionBranchOutcome::Failed,
            cache: CacheDisposition::NotApplicable, artifact_evidence_index: None,
            artifact_id: None, locator: None, diagnostics: branch.diagnostics.clone(),
        };
        if cancellation.is_cancelled() { result.outcome = CompositionBranchOutcome::Cancelled; }
        else if branch.state == CompositionBranchState::Blocked { result.outcome = CompositionBranchOutcome::Blocked; }
        else if branch.state == CompositionBranchState::Unavailable { result.outcome = CompositionBranchOutcome::Unavailable; }
        else {
            let publish = (|| -> Result<()> {
                let cache_path = cache_root.join("entries").join(format!("{}.json", branch.cache_key));
                // Keep the full measured report in the embedded plan. Cache
                // records and terminal evidence retain its exact digest and
                // provider identity, not a second copy of every glyph.
                let typography = branch.typography.as_ref().map(|report| -> Result<_> {
                    Ok(serde_json::json!({
                        "schema": report.schema, "provider_profile": report.provider_profile,
                        "report_sha256": hash_json(report)?, "runtime_qualified": false,
                    }))
                }).transpose()?;
                let expected_metadata = serde_json::json!({
                    "schema": "renderflow.composition-artifact/v4", "profile": COMPOSITION_PROFILE,
                    "version": env!("CARGO_PKG_VERSION"), "target_id": branch.target_id,
                    "renderer": { "id": render::COMPOSITION_RENDERER_ID, "version": render::COMPOSITION_RENDERER_VERSION },
                    "locale": branch.locale, "content_revision": branch.content_revision,
                    "content_sha256": branch.content_sha256, "composition_sha256": snapshot.manifest_sha256,
                    "cache_key": branch.cache_key, "review_required": true, "runtime_qualified": false,
                    "font_resolution": plan.font_resolution, "provenance": snapshot.manifest.provenance,
                    "typography": typography,
                    "raster": branch.raster,
                    "pdf": branch.pdf,
                    "locale_overrides": snapshot.manifest.locale_overrides.get(&branch.locale),
                    "deterministic_serialization": true, "visual_determinism_qualified": false,
                    "losses": if branch.output == super::CompositionOutput::Pdf {
                        vec!["flattened_pixels_no_editable_layers_or_text", "font_and_vector_content_rasterized", "opaque_background_no_alpha", "assumed_srgb_no_profile_conversion", "no_icc_output_intent_or_pdfx_pdfa_conformance", "untagged_no_selectable_text", "printer_acceptance_and_visual_fidelity_unqualified"]
                    } else if branch.output == super::CompositionOutput::Png {
                        vec!["flattened_pixels_no_editable_layers_or_text", "assumed_srgb_no_profile_conversion", "raster_color_and_visual_appearance_unqualified"]
                    } else if branch.output.visual() { vec!["no_native_layered_round_trip", "visual_text_serialized_as_outlines", "raster_color_and_visual_appearance_unqualified"] }
                        else { vec!["visual_artwork_and_layout_omitted"] },
                });
                let cached = read_cache(&cache_path, &store, &branch.cache_key, branch.output.name(),
                    branch.output.media_type(), &source_ids, &expected_metadata, branch.raster.as_ref(), branch.pdf.as_ref());
                let (artifact, bytes, cache) = match cached {
                    Ok(Some((artifact, bytes))) => (artifact, bytes, CacheDisposition::Hit),
                    other => {
                        if let Err(error) = other {
                            result.diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Warning,
                                "composition.cache.rejected", format!("{error:#}"), Some(&branch.locale), None));
                        }
                        let rendered = render::render_with_plan(snapshot, &branch.locale, branch.output,
                            branch.typography.as_ref(), branch.raster.as_ref(), branch.pdf.as_ref())?;
                        if rendered.bytes.len() > MAX_OUTPUT_BYTES { bail!("composition.output.byte_limit: output exceeds 64 MiB"); }
                        if rendered.media_type != branch.output.media_type() || rendered.extension != branch.output.extension() {
                            bail!("composition.output.contract: renderer output disagrees with selected target");
                        }
                        let descriptor = ArtifactDescriptor::new(CanonicalFormat::new(branch.output.name())?,
                            MediaType::new(branch.output.media_type())?, ArtifactStorageClass::Terminal)
                            .with_sources(source_ids.clone()).with_metadata("renderflow.composition", expected_metadata.clone());
                        check_store_paths(store.root(), Some(&hash_bytes(&rendered.bytes)))?;
                        let artifact = store.put_bytes(&rendered.bytes, descriptor)?;
                        store.verify(&artifact)?;
                        let entry = CacheEntry { schema: CACHE_SCHEMA.into(), key: branch.cache_key.clone(),
                            profile: COMPOSITION_PROFILE.into(), artifact: artifact.clone() };
                        if let Err(error) = write_cache(&cache_path, &json_bytes(&entry)?) {
                            result.diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Warning,
                                "composition.cache.write_failed", format!("{error:#}"), Some(&branch.locale), None));
                        }
                        (artifact, rendered.bytes, CacheDisposition::Miss)
                    },
                };
                let new_total = total_bytes.checked_add(bytes.len()).context("composition.output.byte_limit")?;
                if new_total > MAX_BUNDLE_OUTPUT_BYTES { bail!("composition.output.byte_limit: total derivative output exceeds 512 MiB"); }
                if cancellation.is_cancelled() { bail!("composition.cancelled: cancelled before target publication"); }
                let directory = output.join("outputs").join(&branch.locale);
                fs::create_dir_all(&directory)?;
                let relative = format!("outputs/{}/page.{}", branch.locale, branch.output.extension());
                write_new(&output.join(&relative), &bytes)?;
                total_bytes = new_total;
                let locator = format!("bundle:{relative}");
                let mut evidence = ArtifactEvidence::from_artifact(&artifact, branch.target_id.clone(),
                    ArtifactRole::Terminal, &locator, ProducerEvidence {
                        system: "renderflow".into(), transform: Some(COMPOSITION_PROFILE.into()),
                        capability: Some(format!("composition.{}", branch.output.name())),
                        provider: Some("renderflow.core-composition".into()), version: Some(env!("CARGO_PKG_VERSION").into()),
                    }, ValidationState::ValidWithWarnings, FidelityDeclaration::Partial);
                evidence.cache = cache;
                evidence.warnings = result.diagnostics.iter().map(|item| item.code.clone()).collect();
                result.artifact_evidence_index = Some(records.len());
                result.artifact_id = Some(artifact.id().to_string());
                result.locator = Some(locator);
                result.outcome = CompositionBranchOutcome::Published;
                result.cache = cache;
                records.push(evidence);
                outputs.push(relative);
                Ok(())
            })();
            if let Err(error) = publish {
                result.outcome = if cancellation.is_cancelled() { CompositionBranchOutcome::Cancelled }
                    else { CompositionBranchOutcome::Failed };
                result.diagnostics.push(CompositionDiagnostic::new(DiagnosticSeverity::Error,
                    "composition.branch.failed", format!("{error:#}"), Some(&branch.locale), None));
            }
        }
        branches.push(result);
    }
    let published = branches.iter().filter(|branch| branch.outcome == CompositionBranchOutcome::Published).count();
    let state = if cancellation.is_cancelled() { CompositionRunState::Cancelled }
        else if published == branches.len() { CompositionRunState::Complete }
        else if published > 0 { CompositionRunState::Partial } else { CompositionRunState::Failed };
    let run = CompositionRun {
        schema: COMPOSITION_RUN_SCHEMA_V4.into(), run_id: run_id.clone(), state, plan_sha256, plan,
        artifact_manifest: ArtifactManifest { schema_version: ARTIFACT_MANIFEST_SCHEMA_V1.into(),
            run_id, output_dir: ".".into(), outputs, artifacts: records }, branches,
        runtime_qualified: false, review_required: true, visual_determinism_qualified: false,
    };
    // A failed/partial/cancelled receipt is still evidence. No receipt is created
    // if publication I/O fails before this final atomic write.
    write_new(&output.join("composition-run.json"), &json_bytes(&run)?)?;
    Ok(run)
}

fn input_type(snapshot: &LoadedComposition, kind: &str, id: &str) -> (String, String) {
    if kind == "asset" {
        if let Some(asset) = snapshot.assets.get(id) {
            let format = match asset.declaration.format {
                super::model::AssetFormat::Png => "png", super::model::AssetFormat::Jpeg => "jpeg",
                super::model::AssetFormat::Svg => "svg",
            };
            return (format.into(), asset.declaration.format.media_type().into());
        }
    }
    if kind == "font" {
        if let Some(font) = snapshot.fonts.registry.assets.iter().find(|font| font.id == id) {
            return (font.format.to_string(), font.format.media_type().into());
        }
    }
    if matches!(kind, "manifest" | "content" | "font_registry") {
        ("json".into(), "application/json".into())
    } else { ("binary".into(), "application/octet-stream".into()) }
}

fn read_cache(path: &Path, store: &ArtifactStore, key: &str, format: &str, media: &str,
    sources: &[ArtifactId], metadata: &serde_json::Value,
    raster: Option<&super::RasterPlan>, pdf: Option<&super::PdfPlan>) -> Result<Option<(Artifact, Vec<u8>)>> {
    if !path.try_exists()? { return Ok(None); }
    let entry: CacheEntry = serde_json::from_slice(&read_regular(path, 1024 * 1024)?)?;
    let digest = entry.artifact.digest().value();
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        bail!("cached digest is not canonical lowercase SHA-256");
    }
    let expected_payload = Path::new("objects").join("sha256").join(&digest[..2]).join(digest);
    if entry.schema != CACHE_SCHEMA || entry.key != key || entry.profile != COMPOSITION_PROFILE
        || entry.artifact.format().as_str() != format || entry.artifact.media_type().as_str() != media
        || entry.artifact.sources() != sources || entry.artifact.metadata().get("renderflow.composition") != Some(metadata)
        || entry.artifact.storage_class() != ArtifactStorageClass::Terminal
        || entry.artifact.payload().relative_path() != expected_payload
        || entry.artifact.id() != &ArtifactId::from_record(entry.artifact.digest(), entry.artifact.format(), sources)
        || entry.artifact.size_bytes() > MAX_OUTPUT_BYTES as u64 {
        bail!("cached record does not match frozen composition branch");
    }
    let bytes = read_regular(&store.payload_path(&entry.artifact)?, MAX_OUTPUT_BYTES)?;
    if hash_bytes(&bytes) != entry.artifact.digest().value() || bytes.len() as u64 != entry.artifact.size_bytes() {
        bail!("cached payload digest or size differs from its record");
    }
    // A self-consistent cache record cannot establish an encoded output's
    // dimensions, page boxes, or decodability. Apply the fresh-output contract
    // checks before accepting a hit; rejected entries are regenerated.
    if let Some(plan) = raster { super::raster::validate_output(&bytes, plan)?; }
    if let Some(plan) = pdf { super::pdf::validate_output(&bytes, plan)?; }
    Ok(Some((entry.artifact, bytes)))
}

/// The shared store is caller-supplied state. Refuse linked existing internals
/// before the generic store has any opportunity to create or write through them.
fn check_store_paths(root: &Path, digest: Option<&str>) -> Result<()> {
    for relative in [".tmp", "objects", "objects/sha256"] {
        refuse_links(&root.join(relative))?;
    }
    if let Some(digest) = digest {
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
            bail!("composition.store.digest: expected canonical lowercase SHA-256");
        }
        refuse_links(&root.join("objects/sha256").join(&digest[..2]).join(digest))?;
    }
    Ok(())
}

fn read_regular(path: &Path, maximum: usize) -> Result<Vec<u8>> {
    refuse_links(path)?;
    let before = fs::symlink_metadata(path)?;
    if !before.is_file() || before.len() > maximum as u64 { bail!("bounded regular file required"); }
    let file = File::open(path)?;
    if !file.metadata()?.is_file() { bail!("bounded regular file required"); }
    let mut bytes = Vec::new();
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum { bail!("file byte limit exceeded"); }
    Ok(bytes)
}

fn refuse_links(path: &Path) -> Result<()> {
    let mut current = std::path::PathBuf::new();
    for component in path.components() {
        if component == Component::ParentDir { bail!("composition.path.parent: parent traversal is refused"); }
        current.push(component.as_os_str());
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => bail!("composition.path.symlink: linked output/cache paths are refused"),
            Ok(_) => {}, Err(error) if error.kind() == std::io::ErrorKind::NotFound => {},
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("composition.output.parent")?;
    refuse_links(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(path).map_err(|error| error.error)?;
    Ok(())
}

fn write_cache(path: &Path, bytes: &[u8]) -> Result<()> {
    refuse_links(path)?;
    let parent = path.parent().context("composition.cache.parent")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}
