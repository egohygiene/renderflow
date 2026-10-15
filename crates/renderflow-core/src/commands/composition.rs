//! Public CLI adapters for the shared localized-composition planner and executor.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde::Serialize;

use crate::cli::{CompositionRenderArguments, CompositionSelectionArguments};
use crate::composition::{self, CompositionOutput, CompositionRequest, LocaleSelector, RasterSettings, BindingSide, PdfSettings};
use crate::planning::{resolve_composition, execute_composition, CompositionPlanningRequest};
use crate::process::ProcessCancellationToken;

/// Inspection is a read-only plan for every declared locale and the default targets.
pub fn run_inspect(input: &str) -> Result<()> {
    let loaded = composition::load::load_composition(Path::new(input))?;
    let plan = composition::plan(&loaded, &CompositionRequest::default())?;
    emit_json(&plan)
}

pub fn run_plan(arguments: CompositionSelectionArguments) -> Result<()> {
    let request = request_from_arguments(&arguments)?;
    let resolved = resolve_composition(CompositionPlanningRequest {
        input: arguments.input.into(), request,
    })?;
    // An unavailable branch is a useful plan result, not a failed serialization.
    // Execution treats every selected branch as required and returns nonzero.
    emit_json(resolved.plan())
}

pub fn run_render(arguments: CompositionRenderArguments) -> Result<()> {
    let request = request_from_arguments(&arguments.selection)?;
    let resolved = resolve_composition(CompositionPlanningRequest {
        input: arguments.selection.input.into(), request,
    })?;
    let cancellation = ProcessCancellationToken::new();
    let signal = cancellation.clone();
    ctrlc::set_handler(move || signal.cancel())
        .context("cannot install page-composition cancellation handler")?;
    let run = execute_composition(
        &resolved,
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
    request.pdf = PdfSettings {
        dpi: arguments.pdf_dpi,
        background: arguments.pdf_background.clone(),
        binding_side: match arguments.pdf_binding_side.as_str() {
            "none" => BindingSide::None,
            "left" => BindingSide::Left,
            "right" => BindingSide::Right,
            _ => bail!("--pdf-binding-side must be none, left, or right"),
        },
        color_policy: arguments.pdf_color_policy.clone(),
        conformance: arguments.pdf_conformance.clone(),
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
        target.parse::<CompositionOutput>()
            .with_context(|| format!("unknown page-composition target {target}"))
    }).collect::<Result<Vec<_>>>()?;
    Ok(CompositionRequest { locales, targets, raster: RasterSettings::default(), pdf: PdfSettings::default() })
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
            "--locales", "en,ru", "--targets", "svg,ora", "--output-dir", "dist/page",
        ]).unwrap();
        let Some(Commands::Composition {
            subcommand: CompositionCommands::Render(arguments),
        }) = cli.command else { panic!("expected composition render"); };
        let request = selection_request(&arguments.selection.locales, &arguments.selection.targets).unwrap();
        assert!(matches!(request.locales, LocaleSelector::Subset { ref locales } if locales == &["en", "ru"]));
        assert!(matches!(request.targets.as_slice(), [CompositionOutput::Svg, CompositionOutput::Ora]));
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
        assert_eq!(legacy.pdf, PdfSettings::default());
    }

    #[test]
    fn pdf_options_reach_the_shared_request_without_changing_png_settings() {
        let cli = Cli::try_parse_from([
            "renderflow", "composition", "plan", "--input", "page.composition.json",
            "--locales", "en", "--targets", "pdf,png,transcript_json",
            "--pdf-dpi", "144", "--pdf-background", "#AAbbCC",
            "--pdf-binding-side", "left",
            "--pdf-color-policy", "assumed_srgb_no_profile_conversion",
            "--pdf-conformance", "none",
        ]).unwrap();
        let Some(Commands::Composition {
            subcommand: CompositionCommands::Plan(arguments),
        }) = cli.command else { panic!("expected composition plan"); };
        let request = request_from_arguments(&arguments).unwrap();
        assert_eq!(request.pdf.dpi, 144);
        assert_eq!(request.pdf.background, "#AAbbCC");
        assert_eq!(request.pdf.binding_side, BindingSide::Left);
        assert_eq!(request.pdf.color_policy, "assumed_srgb_no_profile_conversion");
        assert_eq!(request.pdf.conformance, "none");
        assert_eq!(request.raster, RasterSettings::default());
        assert_eq!(request.targets, [CompositionOutput::Pdf, CompositionOutput::Png, CompositionOutput::TranscriptJson]);
    }

    #[test]
    fn pdf_policy_requests_are_not_silently_normalized_to_supported_values() {
        let cli = Cli::try_parse_from([
            "renderflow", "composition", "plan", "--input", "page.composition.json",
            "--targets", "pdf,transcript_json", "--pdf-background", "transparent",
            "--pdf-color-policy", "cmyk", "--pdf-conformance", "pdfx",
        ]).unwrap();
        let Some(Commands::Composition {
            subcommand: CompositionCommands::Plan(arguments),
        }) = cli.command else { panic!("expected composition plan"); };
        let request = request_from_arguments(&arguments).unwrap();
        assert_eq!(request.pdf.background, "transparent");
        assert_eq!(request.pdf.color_policy, "cmyk");
        assert_eq!(request.pdf.conformance, "pdfx");
        assert_eq!(request.targets, [CompositionOutput::Pdf, CompositionOutput::TranscriptJson]);
    }

    #[test]
    fn composition_defaults_keep_pdf_opt_in_and_use_fixed_pdf_defaults() {
        let cli = Cli::try_parse_from([
            "renderflow", "composition", "plan", "--input", "page.composition.json",
        ]).unwrap();
        let Some(Commands::Composition {
            subcommand: CompositionCommands::Plan(arguments),
        }) = cli.command else { panic!("expected composition plan"); };
        let request = request_from_arguments(&arguments).unwrap();
        assert_eq!(request, CompositionRequest::default());
        assert_eq!(request.pdf.dpi, 300);
        assert_eq!(request.pdf.background, "#ffffff");
        assert_eq!(request.pdf.binding_side, BindingSide::None);
        assert!(!request.targets.contains(&CompositionOutput::Pdf));
        assert!(Cli::try_parse_from([
            "renderflow", "composition", "plan", "--input", "page.composition.json",
            "--pdf-binding-side", "auto",
        ]).is_err());
    }

}
