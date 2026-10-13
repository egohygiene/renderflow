//! Public CLI adapters for the shared localized-composition planner and executor.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::cli::{CompositionRenderArguments, CompositionSelectionArguments};
use crate::composition::{self, CompositionOutput, CompositionRequest, LocaleSelector, RasterSettings};
use crate::process::ProcessCancellationToken;

/// Inspection is a read-only plan for every declared locale and the default targets.
pub fn run_inspect(input: &str) -> Result<()> {
    let loaded = composition::load::load_composition(Path::new(input))?;
    let plan = composition::plan(&loaded, &CompositionRequest::default())?;
    emit_json(&plan)
}

pub fn run_plan(arguments: CompositionSelectionArguments) -> Result<()> {
    let request = request_from_arguments(&arguments)?;
    let loaded = composition::load::load_composition(Path::new(&arguments.input))?;
    let plan = composition::plan(&loaded, &request)?;
    // An unavailable branch is a useful plan result, not a failed serialization.
    // Execution treats every selected branch as required and returns nonzero.
    emit_json(&plan)
}

pub fn run_render(arguments: CompositionRenderArguments) -> Result<()> {
    let request = request_from_arguments(&arguments.selection)?;
    let loaded = composition::load::load_composition(Path::new(&arguments.selection.input))?;
    let cancellation = ProcessCancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())
        .context("cannot install page-composition cancellation handler")?;
    let run = composition::execute(
        &loaded,
        &request,
        Path::new(&arguments.output_dir),
        arguments.cache_dir.as_deref().map(Path::new),
        &cancellation,
    )?;
    // Preserve the complete receipt on stdout before reporting partial/failure status.
    emit_json(&run)?;
    if !run.successful() {
        bail!("page composition did not publish every selected branch; inspect the JSON receipt");
    }
    Ok(())
}

fn request_from_arguments(arguments: &CompositionSelectionArguments) -> Result<CompositionRequest> {
    let mut request = selection_request(&arguments.locales, &arguments.targets)?;
    request.raster = RasterSettings {
        dpi: arguments.raster_dpi,
        background: arguments.raster_background.clone(),
    };
    Ok(request)
}

fn selection_request(locales: &str, targets: &str) -> Result<CompositionRequest> {
    let locale_values = comma_values(locales, "locales")?;
    let locales = if locale_values == ["all_declared"] {
        LocaleSelector::AllDeclared
    } else {
        if locale_values.iter().any(|locale| locale == "all_declared") {
            bail!("all_declared cannot be combined with individual locales");
        }
        match locale_values.as_slice() {
            [locale] => LocaleSelector::Exact { locale: locale.clone() },
            _ => LocaleSelector::Subset { locales: locale_values },
        }
    };
    let targets = comma_values(targets, "targets")?.iter().map(|target| {
        match target.as_str() {
            "svg" => Ok(CompositionOutput::Svg),
            "html" => Ok(CompositionOutput::Html),
            "transcript_json" => Ok(CompositionOutput::TranscriptJson),
            "transcript_text" => Ok(CompositionOutput::TranscriptText),
            "png" => Ok(CompositionOutput::Png),
            "pdf" => Ok(CompositionOutput::Pdf),
            "ora" => Ok(CompositionOutput::Ora),
            "psd" => Ok(CompositionOutput::Psd),
            "idml" => Ok(CompositionOutput::Idml),
            _ => bail!("unknown page-composition target {target}; expected svg, html, transcript_json, transcript_text, png, pdf, ora, psd, or idml"),
        }
    }).collect::<Result<Vec<_>>>()?;
    Ok(CompositionRequest { locales, targets, raster: RasterSettings::default() })
}

fn comma_values(value: &str, argument: &str) -> Result<Vec<String>> {
    let mut seen = BTreeSet::new();
    let mut values = Vec::new();
    for item in value.split(',') {
        let item = item.trim();
        if item.is_empty() {
            bail!("--{argument} must contain nonempty comma-separated values");
        }
        if !seen.insert(item.to_owned()) {
            bail!("--{argument} contains the repeated value {item}");
        }
        values.push(item.to_owned());
    }
    Ok(values)
}

fn emit_json(value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    std::io::stdout().lock().write_all(&bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    use crate::cli::{Cli, Commands, CompositionCommands};

    #[test]
    fn render_uses_explicit_subset_and_retains_unsupported_target() {
        let cli = Cli::try_parse_from([
            "renderflow", "composition", "render", "--input", "page.composition.json",
            "--locales", "en,ru", "--targets", "svg,pdf", "--output-dir", "dist/page",
        ]).unwrap();
        let Some(Commands::Composition {
            subcommand: CompositionCommands::Render(arguments),
        }) = cli.command else { panic!("expected composition render"); };
        let request = selection_request(&arguments.selection.locales, &arguments.selection.targets).unwrap();
        assert!(matches!(request.locales, LocaleSelector::Subset { ref locales } if locales == &["en", "ru"]));
        assert!(matches!(request.targets.as_slice(), [CompositionOutput::Svg, CompositionOutput::Pdf]));
        assert!(arguments.cache_dir.is_none());
    }

    #[test]
    fn selection_never_silently_broadens_or_deduplicates() {
        for locales in ["", "en,", "en,en", "all_declared,en"] {
            assert!(selection_request(locales, "svg").is_err());
        }
        for targets in ["", "svg,", "svg,svg", "jpg"] {
            assert!(selection_request("en", targets).is_err());
        }
        assert!(matches!(selection_request("en", "svg").unwrap().locales, LocaleSelector::Exact { locale } if locale == "en"));
        assert!(matches!(selection_request("all_declared", "svg").unwrap().locales, LocaleSelector::AllDeclared));
    }

    #[test]
    fn png_options_reach_the_shared_request_without_changing_selected_targets() {
        let cli = Cli::try_parse_from([
            "renderflow", "composition", "plan", "--input", "page.composition.json",
            "--locales", "en", "--targets", "png,transcript_json",
            "--raster-dpi", "300", "--raster-background", "#FfCc00",
        ]).unwrap();
        let Some(Commands::Composition {
            subcommand: CompositionCommands::Plan(arguments),
        }) = cli.command else { panic!("expected composition plan"); };
        let request = request_from_arguments(&arguments).unwrap();
        assert_eq!(request.raster.dpi, 300);
        assert_eq!(request.raster.background, "#FfCc00");
        assert_eq!(request.targets, [CompositionOutput::Png, CompositionOutput::TranscriptJson]);
        let legacy: CompositionRequest = serde_json::from_str(
            r#"{"locales":{"mode":"exact","locale":"en"},"targets":["png"]}"#,
        ).unwrap();
        assert_eq!(legacy.raster, RasterSettings::default());
    }
}
