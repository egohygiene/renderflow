//! Thin CLI; source capture, generation, review and export remain SDK-owned.
use crate::ai::profile::{load_catalog, RuntimeProfile};
use crate::artifact::ArtifactStore;
use crate::cli::SummaryCommands;
use crate::process::ProcessCancellationToken;
use crate::publication::{local, summary};
use anyhow::{ensure, Context, Result};
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use summary::review;

pub fn run(command: SummaryCommands) -> Result<()> {
    let token = ProcessCancellationToken::new();
    let signal = token.clone();
    ctrlc::set_handler(move || signal.cancel())?;
    match command {
        SummaryCommands::Plan {
            recipe,
            profile,
            catalog,
        } => {
            let profile = RuntimeProfile::load(profile)?;
            let catalog = load_catalog(catalog.as_deref().map(Path::new))?;
            let temporary = tempfile::tempdir()?;
            let store = ArtifactStore::new(temporary.path())?;
            let captured = summary::capture(Path::new(&recipe), &store, token)?;
            emit(&serde_json::to_value(captured.plan(&profile, &catalog)?)?)?;
        }
        SummaryCommands::Generate {
            recipe,
            profile,
            catalog,
            store,
            output_dir,
        } => {
            // Refuse an obviously invalid destination before any inference.
            // The shared publisher still owns the final create-new admission.
            preflight_output(Path::new(&output_dir))?;
            let profile = RuntimeProfile::load(profile)?;
            let catalog = load_catalog(catalog.as_deref().map(Path::new))?;
            let store = ArtifactStore::new(store)?;
            let captured = summary::capture(Path::new(&recipe), &store, token.clone())?;
            let outcome =
                summary::generate(&captured, &profile, &catalog, &store, token.clone(), None)?;
            emit(&review::publish_outcome(
                &outcome,
                &store,
                Path::new(&output_dir),
                token,
            )?)?;
            ensure!(
                matches!(
                    outcome.status,
                    summary::SummaryStatus::Disabled | summary::SummaryStatus::AwaitingReview
                ),
                "summary.generate.no_candidate; inspect the written outcome and receipt"
            );
        }
        SummaryCommands::Review {
            candidate,
            decision,
            store,
            output_dir,
        } => {
            let candidate = local::read_file(Path::new(&candidate), 131072)?;
            let decision = local::read_file(Path::new(&decision), 65536)?;
            let decision: review::ReviewDecision = crate::evidence::json::object(&decision, 65536)?;
            let store = ArtifactStore::new(store)?;
            let reviewed = review::apply(&candidate, &decision, &store, token.clone())?;
            emit(&review::publish_review(
                &reviewed,
                &store,
                Path::new(&output_dir),
                token,
            )?)?;
        }
        SummaryCommands::Export {
            review,
            review_sha256,
            store,
            output_dir,
        } => {
            let bytes = local::read_file(Path::new(&review), 65536)?;
            let store = ArtifactStore::new(store)?;
            emit(&review::export(
                &bytes,
                &review_sha256,
                &store,
                Path::new(&output_dir),
                token,
            )?)?;
        }
    }
    Ok(())
}
fn preflight_output(output: &Path) -> Result<()> {
    match std::fs::symlink_metadata(output) {
        Ok(_) => anyhow::bail!("summary.output: must not exist"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("summary.output.metadata"),
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    ensure!(parent.is_dir(), "summary.output.parent: must exist");
    Ok(())
}
fn emit(value: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    std::io::stdout().write_all(&bytes)?;
    Ok(())
}
