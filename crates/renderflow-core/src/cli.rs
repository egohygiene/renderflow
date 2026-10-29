use clap::{Parser, Subcommand, ValueEnum};

use crate::optimization::OptimizationMode;
use crate::video::HandBrakePreset;

/// Spec-driven document rendering engine
#[derive(Parser)]
#[command(
    name = "renderflow",
    version,
    about = "Spec-driven document rendering engine",
    long_about = "renderflow — Spec-driven document rendering engine\n\n\
        Transform structured YAML configurations into rendered documents\n\
        (PDF, HTML, LaTeX) using Pandoc, Tectonic, and Jinja2 templates.",
    after_help = "Examples:\n  \
        renderflow build                        Build using renderflow.yaml\n  \
        renderflow build --config custom.yaml   Build with a custom config file\n  \
        renderflow build --dry-run              Preview what would be built\n  \
        renderflow watch                        Watch using renderflow.yaml\n  \
        renderflow watch --config custom.yaml   Watch with a custom config file\n  \
        renderflow audit                        Generate an optimization audit report\n  \
        renderflow inspect                      Visualize the transformation DAG\n  \
        renderflow inspect --output-format dot  Export DAG as Graphviz DOT\n  \
        renderflow plugin list                  List registered plugins\n  \
        renderflow plugin info <name>           Show details for a plugin\n  \
        renderflow plugin validate              Validate all plugin metadata\n  \
        renderflow plugin doctor                Run plugin diagnostics\n  \
        renderflow version                      Print the installed version\n  \
        renderflow env                          Print installation environment details\n  \
        renderflow doctor                       Run installation diagnostics\n  \
        renderflow tools list                    List runtime tool providers\n  \
        renderflow tools inspect tool.ffmpeg     Inspect one runtime provider\n  \
        renderflow capabilities                  List provider capability IDs\n  \
        renderflow spec validate                 Validate v1/v2 execution specifications\n  \
        renderflow spec migrate                  Migrate unversioned v1 config to v2\n  \
        renderflow my-project.yaml              Shorthand: run build on the given config"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to the renderflow configuration file (used when no subcommand is provided)
    pub input: Option<String>,

    /// Enable verbose logging (DEBUG level)
    #[arg(long, global = true)]
    pub verbose: bool,

    /// Enable debug logging (TRACE level); takes precedence over --verbose
    #[arg(long, global = true)]
    pub debug: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build rendered documents from a renderflow configuration file
    #[command(after_help = "Examples:\n  \
            renderflow build                        Build using renderflow.yaml\n  \
            renderflow build --config custom.yaml   Build with a custom config file\n  \
            renderflow build --dry-run              Preview what would be built\n  \
            renderflow build --optimization speed   Build using speed optimization mode\n  \
            renderflow build --optimization pareto  Build with Pareto-optimal path selection\n  \
            renderflow build --target pdf           Build only the PDF output via graph resolution\n  \
            renderflow build --profile everything  Build the maximal available artifact forest
  \
            renderflow build --profile magazine    Build a versioned magazine release bundle
  \
            renderflow build --profile coloring-book Build a rights-aware coloring-book bundle")]
    Build {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Resolve and print the plan without publishing outputs. Ordinary graph providers may run bounded probes.
        #[arg(long)]
        dry_run: bool,

        /// Resume from compatible validated node checkpoints (not supported for composition sources).
        #[arg(long)]
        resume: bool,

        /// Optimization mode: controls how transformation paths are selected.
        /// Overrides the value set in the config file when provided.
        /// Choices: speed (minimise cost), quality (maximise quality), balanced (default),
        /// pareto (return Pareto-optimal frontier of non-dominated paths).
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,

        /// Build only the specified output format using the source's canonical executor.
        /// Built-in document/image/audio capabilities and optional configured transforms are
        /// resolved through the same planner. Composition accepts its native output names.
        /// Cannot be combined with --all.
        #[arg(long, value_name = "FORMAT", conflicts_with_all = ["all", "profile"])]
        target: Option<String>,

        /// Build a named, versioned derivative profile. `everything`, `magazine`, and `coloring-book` are bundled.
        #[arg(long, value_name = "PROFILE", conflicts_with_all = ["target", "all"])]
        profile: Option<String>,

        /// Exclude a branch selector (for example `family:video` or `provider:tool.ffmpeg`).
        #[arg(long, value_name = "SELECTOR")]
        exclude: Vec<String>,

        /// Build all policy-allowed output formats reachable through the canonical capability graph.
        /// Built-in capabilities and optional configured transforms participate equally.
        /// Cannot be combined with --target.
        #[arg(long, conflicts_with_all = ["target", "profile"])]
        all: bool,
    },

    /// Watch for file changes and automatically rebuild
    #[command(after_help = "Examples:\n  \
            renderflow watch                                    Watch using renderflow.yaml\n  \
            renderflow watch --config custom.yaml               Watch with a custom config file\n  \
            renderflow watch --config custom.yaml --debounce 300   Watch with a 300 ms debounce delay")]
    Watch {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Debounce delay in milliseconds: wait this long after the last change before rebuilding
        #[arg(long, default_value = "500", value_name = "MS")]
        debounce: u64,
    },

    /// Generate an optimization audit report covering performance, memory, and Rust best practices
    #[command(after_help = "Examples:\n  \
            renderflow audit   Generate an audit report in the audits/ directory")]
    Audit,

    /// Inspect an input artifact or visualize a configured transformation DAG
    #[command(after_help = "Examples:\n  \
            renderflow inspect                          Show DAG tree for renderflow.yaml\n  \
            renderflow inspect --config custom.yaml    Show DAG tree for a custom config\n  \
            renderflow inspect --output-format dot     Emit Graphviz DOT output to stdout\n  \
            renderflow inspect --target pdf            Show execution plan for a single target\n  \
            renderflow inspect --all --export dag.dot  Export full DAG to a DOT file\n  \
            renderflow inspect --input file.bin        Inspect arbitrary bytes as JSON\n  \
            renderflow inspect --input book.epub --extract --recursive  Extract safe child artifacts")]
    Inspect {
        /// Arbitrary source file to inspect instead of a Renderflow config.
        #[arg(long, value_name = "FILE", conflicts_with_all = ["target", "all"])]
        input: Option<String>,

        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Output format for the DAG visualization: 'tree' (default) or 'dot' (Graphviz)
        #[arg(long, default_value = "tree", value_name = "FORMAT")]
        output_format: String,

        /// Visualize only the execution plan targeting this output format.
        /// Cannot be combined with --all.
        #[arg(long, value_name = "FORMAT", conflicts_with = "all")]
        target: Option<String>,

        /// Visualize the execution plan for all reachable output formats.
        /// Cannot be combined with --target.
        #[arg(long, conflicts_with = "target")]
        all: bool,

        /// Write the visualization output to a file instead of stdout.
        /// Useful for saving DOT files for later rendering with Graphviz.
        #[arg(long, value_name = "FILE")]
        export: Option<String>,

        /// Source-reported media type used as one detection signal.
        #[arg(long, value_name = "TYPE", requires = "input")]
        media_type: Option<String>,

        /// Extract provider-declared child artifacts into the artifact store.
        #[arg(long, requires = "input")]
        extract: bool,

        /// Recursively inspect/extract supported nested containers.
        #[arg(long, requires = "extract")]
        recursive: bool,

        /// Artifact-store root for input inspection.
        #[arg(
            long,
            default_value = ".renderflow/intake-artifacts",
            value_name = "DIR",
            requires = "input"
        )]
        store: String,

        /// Maximum recursive extraction depth.
        #[arg(long, default_value_t = 3, requires = "input")]
        max_depth: u32,

        /// Maximum source-plus-child artifact count.
        #[arg(long, default_value_t = 1000, requires = "input")]
        max_artifacts: u64,

        /// Maximum total extracted bytes.
        #[arg(long, default_value_t = 536_870_912, requires = "input")]
        max_extracted_bytes: u64,

        /// Maximum uncompressed/compressed ratio for one archive entry.
        #[arg(long, default_value_t = 100.0, requires = "input")]
        max_expansion_ratio: f64,
    },

    /// Manage and inspect plugins
    ///
    /// Plugins extend the renderflow transform pipeline at runtime without
    /// modifying the core codebase.
    #[command(
        subcommand_required = true,
        arg_required_else_help = true,
        after_help = "Examples:\n  \
            renderflow plugin list --transforms transforms.yaml              List declared local plugins\n  \
            renderflow plugin info my-plugin --transforms transforms.yaml    Inspect one declaration\n  \
            renderflow plugin validate --transforms transforms.yaml          Validate declarations without running code\n  \
            renderflow plugin doctor --transforms transforms.yaml            Report metadata and qualification limits"
    )]
    Plugin {
        /// Explicit transform registry containing local command plugin declarations
        #[arg(long, global = true, value_name = "FILE")]
        transforms: Option<String>,
        /// Metadata output format for configured plugins
        #[arg(long, global = true, default_value = "text", value_parser = ["text", "json"])]
        format: String,
        #[command(subcommand)]
        subcommand: PluginCommands,
    },

    /// Resolve model-aware AI skills and inspect providers, policy, and cache state
    ///
    /// These commands help you discover configured AI providers, inspect
    /// available models, run connectivity diagnostics, and manage the AI
    /// response cache.
    #[command(
        subcommand_required = true,
        arg_required_else_help = true,
        after_help = "Examples:\n  \
            renderflow ai matrix                Inspect model-specific compatibility\n  \
            renderflow ai resolve --skill skill.metadata.extract\n  \
            renderflow ai skills validate       Validate bundled skill contracts\n  \
            renderflow ai providers             List legacy provider-wide capabilities\n  \
            renderflow ai doctor                Run AI provider diagnostics\n  \
            renderflow ai cache                 Show AI cache statistics"
    )]
    Ai {
        #[command(subcommand)]
        subcommand: AiCommands,
    },

    /// Extract, validate, and compare versioned Artifact DNA
    #[command(
        subcommand_required = true,
        arg_required_else_help = true,
        after_help = "Examples:\n  \
            renderflow dna extract --input cover.svg --output cover.dna.json\n  \
            renderflow dna validate --input cover.dna.json\n  \
            renderflow dna compare --left cover.dna.json --right divider.dna.json"
    )]
    Dna {
        #[command(subcommand)]
        subcommand: DnaCommands,
    },

    /// Validate and resolve pinned local font assets by semantic role
    #[command(
        subcommand_required = true,
        arg_required_else_help = true,
        after_help = "Examples:\n  \
            renderflow font validate --registry fonts.yaml\n  \
            renderflow font resolve --registry fonts.yaml --target pdf\n  \
            renderflow font css --registry fonts.yaml --output fonts.css"
    )]
    Font {
        #[command(subcommand)]
        subcommand: FontCommands,
    },

    /// Inspect, plan, and render explicitly authored localized page compositions.
    #[command(
        subcommand_required = true,
        arg_required_else_help = true,
        after_help = "Examples:\n  \
            renderflow composition inspect --input page.composition.json\n  \
            renderflow composition plan --input page.composition.json --locales en,ru\n  \
            renderflow composition render --input page.composition.json --output-dir dist/page\n\n\
            Authored, unqualified SVG/HTML/PNG/PDF/transcript profile. PNG is opt-in\n\
            with --targets png; --raster-dpi and --raster-background control its\n\
            pixels. PDF is opt-in with --targets pdf and independent --pdf-* settings.\n\
            ORA, PSD, and IDML remain unsupported target branches. Plan emits the\n\
            native composition, common execution forest, and shared-stage wrapper;\n\
            render retains both native and common run receipts."
    )]
    Composition {
        #[command(subcommand)]
        subcommand: CompositionCommands,
    },

    /// Inspect, visualize, and export the transformation execution plan
    ///
    /// These commands expose the canonical execution plan that the planner
    /// produces before running any transforms.  Use them to understand exactly
    /// what work will be performed, in which order, and why.
    #[command(
        subcommand_required = true,
        arg_required_else_help = true,
        after_help = "Examples:\n  \
            renderflow graph plan                           Show the execution plan (text)\n  \
            renderflow graph plan --format mermaid          Render the plan as Mermaid\n  \
            renderflow graph plan --format json             Export plan as JSON\n  \
            renderflow graph render --format dot            Emit Graphviz DOT output\n  \
            renderflow graph explain                        Show planner diagnostics\n  \
            renderflow graph export --format markdown -o plan.md  Export Markdown report\n  \
            renderflow graph doctor                         Run graph health checks\n  \
            renderflow graph stats                          Print graph statistics"
    )]
    Graph {
        #[command(subcommand)]
        subcommand: GraphCommands,
    },

    /// Inspect the runtime external-tool/provider registry.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Tools {
        #[command(subcommand)]
        subcommand: ToolCommands,
    },

    /// Inspect and validate EPUB/KEPUB publication derivatives.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Ebook {
        #[command(subcommand)]
        subcommand: EbookCommands,
    },

    /// Inspect pinned commercial-publication provider rule packs and candidates.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Publication {
        #[command(subcommand)]
        subcommand: PublicationCommands,
    },

    /// Plan and execute bounded whole-file video transforms.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Video {
        #[command(subcommand)]
        subcommand: VideoCommands,
    },

    /// Inspect local subtitle streams and convert the supported plain-text profile.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Subtitles {
        #[command(subcommand)]
        subcommand: SubtitleCommands,
    },

    /// Inspect local speech, transcribe offline, or map a retained transcript clock.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Speech {
        #[command(subcommand)]
        subcommand: SpeechCommands,
    },

    /// Inspect, analyze, recover, convert, or render explicitly configured screenplay profiles.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Screenplay {
        #[command(subcommand)]
        subcommand: ScreenplayCommands,
    },

    /// List stable provider capability IDs and their implementations.
    Capabilities {
        /// Output format: text (default), json, or yaml.
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        /// Optional transform YAML whose dynamic providers should be included.
        #[arg(long, value_name = "FILE")]
        transforms: Option<String>,
        /// Emit the generated artifact capability conformance matrix.
        #[arg(long)]
        matrix: bool,
    },

    /// Validate, migrate, and export the Renderflow execution specification.
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Spec {
        #[command(subcommand)]
        subcommand: SpecCommands,
    },

    /// Print the installed Renderflow version
    Version,

    /// Print installation environment details for troubleshooting
    Env,

    /// Run installation diagnostics
    #[command(after_help = "Examples:\n  renderflow doctor\n  renderflow doctor --profile creative-publishing\n  renderflow doctor --profile creative-publishing --format json --strict")]
    Doctor {
        /// Fail for missing required core tools, or failed supported discovery probes in a profile.
        #[arg(long)]
        strict: bool,

        /// Inspect an optional provider stack without installing tools or enabling conversion routes.
        #[arg(long, value_enum)]
        profile: Option<DoctorProfile>,

        /// Profile report format (defaults to text); requires --profile.
        #[arg(long, value_enum, requires = "profile")]
        format: Option<DoctorReportFormat>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DoctorProfile {
    CreativePublishing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DoctorReportFormat {
    Text,
    Json,
}

#[cfg(test)]
mod doctor_arguments_tests {
    use super::{Cli, Commands, DoctorProfile, DoctorReportFormat};
    use clap::Parser;

    #[test]
    fn legacy_doctor_does_not_select_an_optional_profile() {
        let cli = Cli::try_parse_from(["renderflow", "doctor", "--strict"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Doctor {
            strict: true, profile: None, format: None,
        })));
    }

    #[test]
    fn creative_doctor_accepts_default_text_and_explicit_json() {
        let default = Cli::try_parse_from([
            "renderflow", "doctor", "--profile", "creative-publishing",
        ]).unwrap();
        assert!(matches!(default.command, Some(Commands::Doctor {
            strict: false, profile: Some(DoctorProfile::CreativePublishing), format: None,
        })));
        let json = Cli::try_parse_from([
            "renderflow", "doctor", "--profile", "creative-publishing",
            "--format", "json", "--strict",
        ]).unwrap();
        assert!(matches!(json.command, Some(Commands::Doctor {
            strict: true, profile: Some(DoctorProfile::CreativePublishing),
            format: Some(DoctorReportFormat::Json),
        })));
    }

    #[test]
    fn doctor_rejects_format_without_profile_and_unknown_values() {
        for arguments in [
            vec!["renderflow", "doctor", "--format", "json"],
            vec!["renderflow", "doctor", "--profile", "everything"],
            vec!["renderflow", "doctor", "--profile", "creative-publishing", "--format", "yaml"],
        ] {
            assert!(Cli::try_parse_from(arguments).is_err());
        }
    }
}

/// Local page composition commands use the same planner as the public SDK.
#[derive(Subcommand)]
pub enum CompositionCommands {
    /// Inspect the declared locales, pinned sources, layout diagnostics, and default targets.
    /// The JSON report uses the default plan; no outputs or cache entries are written.
    Inspect {
        /// Canonical renderflow.page-composition/v1 JSON file; sources are read only.
        #[arg(long, value_name = "FILE")]
        input: String,
    },
    /// Emit a JSON plan with per-locale and per-target diagnostics, without writes.
    Plan(CompositionSelectionArguments),
    /// Render selected branches into a new evidence bundle and emit its JSON receipt.
    Render(CompositionRenderArguments),
}

#[derive(clap::Args)]
pub struct CompositionSelectionArguments {
    /// Canonical renderflow.page-composition/v1 JSON file; sources are read only.
    #[arg(long, value_name = "FILE")]
    pub input: String,
    /// all_declared, one locale, or a comma-separated subset. No implicit locale fallback.
    #[arg(long, default_value = "all_declared", value_name = "LOCALES")]
    pub locales: String,
    /// Comma-separated target formats. Unsupported known targets retain diagnostic branches.
    #[arg(long, default_value = "svg,html,transcript_json", value_name = "FORMATS")]
    pub targets: String,
    /// PNG pixel density, from 36 to 600 DPI. Ignored for other target formats.
    #[arg(long, default_value = "144", value_name = "DPI")]
    pub raster_dpi: u32,
    /// PNG canvas background: transparent or a six-digit #RRGGBB color.
    #[arg(long, default_value = "transparent", value_name = "COLOR")]
    pub raster_background: String,
    /// PDF raster density, from 36 to 600 DPI, subject to bounded pixel limits.
    #[arg(long, default_value = "300", value_name = "DPI")]
    pub pdf_dpi: u32,
    /// Opaque PDF canvas background: a six-digit #RRGGBB color.
    #[arg(long, default_value = "#ffffff", value_name = "COLOR")]
    pub pdf_background: String,
    /// Explicit PDF binding edge. A nonzero gutter requires left or right.
    #[arg(long, default_value = "none", value_parser = ["none", "left", "right"], value_name = "SIDE")]
    pub pdf_binding_side: String,
    /// PDF color policy. Only assumed_srgb_no_profile_conversion is supported.
    #[arg(long, default_value = "assumed_srgb_no_profile_conversion", value_name = "POLICY")]
    pub pdf_color_policy: String,
    /// PDF conformance request. Only none is supported; PDF/X and PDF/A are refused.
    #[arg(long, default_value = "none", value_name = "CONFORMANCE")]
    pub pdf_conformance: String,
}

#[derive(clap::Args)]
pub struct CompositionRenderArguments {
    #[command(flatten)]
    pub selection: CompositionSelectionArguments,
    /// New bundle directory; its parent must already exist. Existing paths are refused.
    #[arg(long, value_name = "DIRECTORY")]
    pub output_dir: String,
    /// Optional local cache directory. Cached payloads are verified before reuse.
    #[arg(long, value_name = "DIRECTORY")]
    pub cache_dir: Option<String>,
}

#[derive(Subcommand)]
pub enum ScreenplayCommands {
    /// Validate canonical screenplay JSON and report structure and retained evidence.
    #[command(after_help = "Only the renderflow.screenplay/v1 JSON model is accepted.\n\
        Native conversion uses the separate explicitly configured convert command; inspect\n\
        does not invoke tools, convert a screenplay, or verify upstream source bytes.")]
    Inspect {
        /// Canonical screenplay JSON file (16 MiB maximum); input is read only.
        #[arg(long, value_name = "FILE")]
        input: String,
        /// Report format; output is written only to standard output.
        #[arg(long, default_value = "json", value_parser = ["json", "text"], value_name = "FORMAT")]
        format: String,
    },
    /// Convert a supported native screenplay body with an explicitly pinned scripttool.
    #[command(after_help = "Authored, unqualified import routes only: FDX/OSF/FadeIn to Fountain.\n\
        Reverse native conversions and scripttool JSON are rejected. Unsupported\n\
        structure or changed semantic content causes failure; no silent fallback.")]
    Convert(ScreenplayConvertArguments),
    /// Recover a review-required screenplay candidate from PDF or unstructured text.
    #[command(after_help = "Authored, unqualified deterministic recovery only; every result is lossy and requires review.\n\
        PDF extraction uses explicitly pinned local tools, with bounded OCR only when needed.\n\
        The new bundle always includes canonical JSON, source-page text, and recovery evidence.\n\
        Optional native derivatives retain the recovery lineage; no AI or installation is implicit.")]
    Recover(ScreenplayRecoverArguments),
    /// Analyze the strict ASCII Fountain body profile without launching a provider.
    #[command(after_help = "Counts are independently derived by Renderflow, not afterwriting statistics.\n\
        The authored profile rejects title pages, styling, multiline paragraphs,\n\
        XML markers, and non-ASCII text. No PDF is created or page count estimated.")]
    Analyze {
        /// Local Fountain input, at most 16 MiB; JSON report goes to stdout.
        #[arg(long, value_name = "FILE")]
        input: String,
    },
    /// Render a strict Fountain body to PDF using a pinned afterwriting installation.
    #[command(after_help = "Authored, unqualified Fountain-to-PDF profile only.\n\
        Requires a JSON config with explicit Node/package pins and allow_unqualified=true.\n\
        Fixed A4 or US Letter, Courier base14, no title page or native FDX input.\n\
        PDF structure is checked independently; visual fidelity and reproducibility remain unqualified.")]
    Pdf(ScreenplayPdfArguments),
    /// Hash a local afterwriting installation without launching Node or installing anything.
    #[command(after_help = "Reports the bounded package-content fingerprint needed by the PDF config.\n\
        A fingerprint identifies local bytes; it does not establish trust, dependency\n\
        completeness, source provenance, or runtime compatibility.")]
    PackageFingerprint {
        /// Absolute path to an already installed afterwriting package directory.
        #[arg(long, value_name = "DIRECTORY")]
        package_root: String,
    },
}

#[derive(clap::Args)]
pub struct ScreenplayRecoverArguments {
    /// Local PDF or UTF-8 text source; the original is read only.
    #[arg(long, value_name = "FILE")]
    pub input: String,
    /// Explicit source format; filename extensions do not select a recovery method.
    #[arg(long, value_parser = ["pdf", "text"], value_name = "FORMAT")]
    pub source_format: String,
    /// RecoveryExtractionConfig JSON with explicit bounds and allow_unqualified=true.
    #[arg(long, value_name = "FILE")]
    pub config: String,
    /// New bundle directory; its parent must exist. Existing paths are refused.
    #[arg(long, value_name = "DIRECTORY")]
    pub output: String,
    /// Optional native derivative; repeat for multiple formats. Canonical JSON is always kept.
    #[arg(long = "target", value_parser = ["fountain", "fdx", "fadein", "osf"], value_name = "FORMAT")]
    pub targets: Vec<String>,
    /// Optional expected lowercase SHA-256 of the original source bytes.
    #[arg(long, value_name = "SHA256")]
    pub source_sha256: Option<String>,
}

#[derive(clap::Args)]
pub struct ScreenplayPdfArguments {
    /// Local Fountain source; the original is read only.
    #[arg(long, value_name = "FILE")]
    pub input: String,
    /// JSON AfterwritingConfig containing explicit runtime, installation, and profile pins.
    #[arg(long, value_name = "FILE")]
    pub config: String,
    /// New bundle directory; its parent must already exist.
    #[arg(long, value_name = "DIRECTORY")]
    pub output_dir: String,
    /// Optional expected lowercase SHA-256 of the source bytes.
    #[arg(long, value_name = "SHA256")]
    pub source_sha256: Option<String>,
}

#[derive(clap::Args)]
pub struct ScreenplayConvertArguments {
    /// Local native screenplay input; the original is read only.
    #[arg(long, value_name = "FILE")]
    pub input: String,
    /// Explicit native source format.
    #[arg(long, value_parser = ["fountain", "fdx", "osf", "fadein"], value_name = "FORMAT")]
    pub input_format: String,
    /// Explicit native target; this checkpoint adopts only Fountain outputs.
    #[arg(long, value_parser = ["fountain", "fdx", "osf", "fadein"], value_name = "FORMAT")]
    pub output_format: String,
    /// New bundle directory; its parent must exist. Existing paths are refused.
    #[arg(long, value_name = "DIRECTORY")]
    pub output_dir: String,
    /// Absolute path to the selected external scripttool executable.
    #[arg(long, value_name = "FILE")]
    pub scripttool_path: String,
    /// Expected lowercase SHA-256 of that exact executable.
    #[arg(long, value_name = "SHA256")]
    pub scripttool_sha256: String,
    /// Expected numeric provider version; only 0.0.10 is adopted by this source profile.
    #[arg(long, default_value = "0.0.10", value_name = "VERSION")]
    pub expected_version: String,
    /// Optional expected SHA-256 of the original source bytes.
    #[arg(long, value_name = "SHA256")]
    pub source_sha256: Option<String>,
    /// Maximum provider invocation duration in seconds.
    #[arg(long, default_value_t = 30, value_name = "SECONDS")]
    pub timeout_seconds: u64,
    /// Explicitly accept that this authored provider profile has not been runtime-qualified.
    #[arg(long)]
    pub allow_unqualified: bool,
}

#[derive(Subcommand)]
pub enum SpeechCommands {
    /// List audio streams using an explicitly pinned local FFprobe executable.
    Inspect(SpeechInspectArguments),
    /// Produce an unreviewed transcript bundle from WAV or an explicit audio stream.
    Transcribe(SpeechTranscribeArguments),
    /// Assess transcript clock readiness from a pinned saved-result bundle.
    Timeline(SpeechTimelineArguments),
    /// Export a new transcript using an explicit, pinned operator-supplied time map.
    Align(SpeechAlignArguments),
}

#[derive(clap::Args)]
pub struct SpeechAlignArguments {
    /// Saved-result receipt published by a prepared-media speech graph run.
    #[arg(long, value_name = "FILE")]
    pub receipt: String,
    /// Expected lowercase SHA-256 of the selected receipt.
    #[arg(long, value_name = "SHA256")]
    pub receipt_sha256: String,
    /// Explicit mapping from prepared-audio milliseconds to original-media milliseconds.
    #[arg(long, value_name = "FILE")]
    pub mapping: String,
    /// Expected lowercase SHA-256 of the exact mapping file.
    #[arg(long, value_name = "SHA256")]
    pub mapping_sha256: String,
    /// New bundle directory; its parent must exist. Existing paths are refused.
    #[arg(long, value_name = "DIRECTORY")]
    pub output_dir: String,
}

#[derive(clap::Args)]
pub struct SpeechTimelineArguments {
    /// Saved-result receipt published by a speech graph run.
    #[arg(long, value_name = "FILE")]
    pub receipt: String,
    /// Expected lowercase SHA-256 of the selected receipt.
    #[arg(long, value_name = "SHA256")]
    pub receipt_sha256: String,
    /// Print the assessment, then fail if original-media clock mapping is missing.
    #[arg(long)]
    pub require_original_media: bool,
}

#[derive(clap::Args)]
pub struct SpeechInspectArguments {
    #[arg(long, value_name = "FILE")]
    pub input: String,
    #[arg(long, value_name = "FILE")]
    pub ffprobe: String,
    #[arg(long, value_name = "SHA256")]
    pub ffprobe_sha256: String,
}

#[derive(clap::Args)]
pub struct SpeechTranscribeArguments {
    #[arg(long, value_name = "FILE")]
    pub input: String,
    /// Global audio stream index from speech inspect; never selected implicitly.
    #[arg(long, requires_all = ["input_sha256", "ffprobe", "ffprobe_sha256", "ffmpeg", "ffmpeg_sha256"])]
    pub audio_stream: Option<u32>,
    /// Source digest from speech inspect, binding the choice to that exact file.
    #[arg(long, value_name = "SHA256", requires = "audio_stream")]
    pub input_sha256: Option<String>,
    #[arg(long, value_name = "FILE", requires = "audio_stream")]
    pub ffprobe: Option<String>,
    #[arg(long, value_name = "SHA256", requires = "audio_stream")]
    pub ffprobe_sha256: Option<String>,
    #[arg(long, value_name = "FILE", requires = "audio_stream")]
    pub ffmpeg: Option<String>,
    #[arg(long, value_name = "SHA256", requires = "audio_stream")]
    pub ffmpeg_sha256: Option<String>,
    /// Audio preparation timeout (1–3600 seconds); default 300 for media input.
    #[arg(long, requires = "audio_stream")]
    pub audio_timeout_seconds: Option<u64>,
    /// Local whisper-cli executable; never downloaded or selected implicitly.
    #[arg(long, value_name = "FILE")]
    pub provider: String,
    /// Expected lowercase SHA-256 of the exact executable.
    #[arg(long, value_name = "SHA256")]
    pub provider_sha256: String,
    #[arg(long, value_name = "FILE")]
    pub model: String,
    /// Operator-assigned identity for the local model artifact.
    #[arg(long, value_name = "ID")]
    pub model_id: String,
    /// Expected lowercase SHA-256 of the exact model file.
    #[arg(long, value_name = "SHA256")]
    pub model_sha256: String,
    /// Language code, or auto to request language detection.
    #[arg(long, default_value = "en")]
    pub language: String,
    #[arg(long, default_value_t = 4)]
    pub threads: u32,
    #[arg(long, default_value_t = 900)]
    pub timeout_seconds: u64,
    /// New bundle directory; its parent must exist. Existing paths are refused.
    #[arg(long, value_name = "DIRECTORY")]
    pub output_dir: String,
}

#[derive(Subcommand)]
pub enum SubtitleCommands {
    /// Enumerate embedded subtitle streams with FFprobe; emits JSON evidence.
    Inspect {
        #[arg(long, value_name = "FILE")]
        input: String,
    },
    /// Convert a strict plain-text SRT/WebVTT or timed-transcript JSON file.
    Convert {
        #[arg(long, value_name = "FILE")]
        input: String,
        /// Explicit source format; JSON must use renderflow.timed-transcript/v1.
        #[arg(long, value_parser = ["srt", "vtt", "json"])]
        input_format: String,
        /// New destination file; existing files are never replaced.
        #[arg(long, value_name = "FILE")]
        output: String,
        #[arg(long, default_value = "json", value_parser = ["srt", "vtt", "json", "text"])]
        output_format: String,
    },
    /// Extract one explicitly selected supported stream; emits JSON evidence.
    Extract {
        #[arg(long, value_name = "FILE")]
        input: String,
        /// Absolute stream index reported by `subtitles inspect`.
        #[arg(long)]
        stream_index: u32,
        /// New destination file; existing files are never replaced.
        #[arg(long, value_name = "FILE")]
        output: String,
        #[arg(long, default_value = "json", value_parser = ["srt", "vtt", "json", "text"])]
        output_format: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum VideoPresetArgument {
    #[value(name = "fast-720p30")]
    Fast720p30,
    #[value(name = "fast-1080p30")]
    Fast1080p30,
    #[value(name = "creator-1080p60")]
    Creator1080p60,
    #[value(name = "production-standard")]
    ProductionStandard,
}

impl From<VideoPresetArgument> for HandBrakePreset {
    fn from(value: VideoPresetArgument) -> Self {
        match value {
            VideoPresetArgument::Fast720p30 => Self::Fast720p30,
            VideoPresetArgument::Fast1080p30 => Self::Fast1080p30,
            VideoPresetArgument::Creator1080p60 => Self::Creator1080p60,
            VideoPresetArgument::ProductionStandard => Self::ProductionStandard,
        }
    }
}

#[derive(Subcommand)]
pub enum VideoCommands {
    /// Print typed presets, ownership boundaries, and interchange contracts.
    Capabilities {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    /// Normalize and hash a whole-file HandBrake request without executing it.
    Plan {
        #[arg(long, value_name = "FILE")]
        input: String,
        #[arg(long, value_name = "FILE")]
        output: String,
        #[arg(long, value_enum, default_value_t = VideoPresetArgument::Fast1080p30)]
        preset: VideoPresetArgument,
        #[arg(long, default_value_t = 7_200)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 262_144)]
        capture_limit_bytes: usize,
        #[arg(long, default_value_t = 1_000)]
        progress_interval_ms: u64,
        #[arg(long, default_value_t = 21_474_836_480)]
        maximum_output_bytes: u64,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    /// Execute a bounded HandBrakeCLI whole-file transform.
    Transcode {
        #[arg(long, value_name = "FILE")]
        input: String,
        #[arg(long, value_name = "FILE")]
        output: String,
        #[arg(long, value_enum, default_value_t = VideoPresetArgument::Fast1080p30)]
        preset: VideoPresetArgument,
        #[arg(long, default_value_t = 7_200)]
        timeout_seconds: u64,
        #[arg(long, default_value_t = 262_144)]
        capture_limit_bytes: usize,
        #[arg(long, default_value_t = 1_000)]
        progress_interval_ms: u64,
        #[arg(long, default_value_t = 21_474_836_480)]
        maximum_output_bytes: u64,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum PublicationCommands {
    /// Validate a rights-aware coloring-book contract entirely offline.
    ColoringBookPreflight {
        /// Coloring-book source contract in YAML or JSON.
        #[arg(long, value_name = "FILE")]
        contract: String,
        /// Optional validation report output file.
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        /// Output format: text (default), json, or yaml.
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        /// Acknowledge review of provenance from remote providers. No provider is invoked.
        #[arg(long)]
        allow_remote: bool,
    },
    /// Produce candidate-only magazine briefs and metadata from Artifact DNA.
    MagazineCandidates {
        /// Renderflow v2 publication specification.
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        /// Artwork role whose Artifact DNA sidecar should be consumed.
        #[arg(long, value_name = "ROLE")]
        asset_role: String,
        /// Optional candidate output file.
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        /// Output format: json (default) or yaml.
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
        /// Request an optional model-generated candidate through the AI skill runtime.
        #[arg(long)]
        ai: bool,
        /// AI model catalog with observed availability and exact model provenance.
        #[arg(long, value_name = "FILE")]
        ai_catalog: Option<String>,
        /// Provider-neutral model selection preference.
        #[arg(long, default_value = "local-only", value_name = "PREFERENCE")]
        ai_preference: String,
        /// Explicitly permit selection of a remote provider.
        #[arg(long)]
        allow_remote: bool,
        /// Permit selection of catalog entries whose availability is unverified.
        #[arg(long)]
        allow_unverified: bool,
        /// Confirm that source and rights evidence permit model exposure.
        #[arg(long)]
        source_approved_for_ai: bool,
        /// Confirm that privacy review permits remote exposure.
        #[arg(long)]
        privacy_approved_for_remote: bool,
        /// Optional OpenAI-compatible endpoint; used only after remote opt-in.
        #[arg(long, value_name = "URL")]
        openai_endpoint: Option<String>,
        /// Environment variable containing an OpenAI-compatible API key.
        #[arg(long, default_value = "OPENAI_API_KEY", value_name = "NAME")]
        openai_api_key_env: String,
    },
    /// Evaluate candidates with the bundled, offline Lulu provider pack.
    Lulu {
        #[command(subcommand)]
        subcommand: LuluCommands,
    },
}

#[derive(Subcommand)]
pub enum LuluCommands {
    /// Emit the exact pinned Lulu rule pack and official source observations.
    Rules {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
    },
    /// Validate local upload candidates without uploading or allocating identifiers.
    Preflight {
        #[arg(long, value_name = "FILE")]
        request: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        /// Run the optional local EPUBCheck v5 provider.
        #[arg(long)]
        epubcheck: bool,
    },
}

/// Subcommands for provider-neutral e-book derivatives.
#[derive(Subcommand)]
pub enum EbookCommands {
    /// Inspect EPUB 3 structure, metadata, navigation, layout, and accessibility evidence.
    Inspect {
        #[arg(long, value_name = "FILE")]
        input: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        /// Run the optional local EPUBCheck provider and include its report.
        #[arg(long)]
        epubcheck: bool,
        /// Require exact native validation of the ordered PNG/JPEG fixed-layout route.
        #[arg(long)]
        fixed_layout: bool,
        /// Bind the inspected EPUB to an explicit completed run manifest.
        #[arg(long, value_name = "FILE")]
        run_manifest: Option<String>,
    },
    /// Print the honest built-in EPUB/KEPUB capability contract.
    Capabilities {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
}

/// Subcommands for `renderflow spec`.
#[derive(Subcommand)]
pub enum SpecCommands {
    /// Validate an unversioned v1 config or a versioned v2 spec.
    Validate {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },

    /// Migrate an unversioned v1 config to the v2 execution specification.
    Migrate {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, short = 'o', value_name = "FILE")]
        output: Option<String>,
    },

    /// Emit the canonical v2 JSON Schema used by the runtime.
    Schema {
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
        #[arg(long, short = 'o', value_name = "FILE")]
        output: Option<String>,
    },
}

/// Subcommands for `renderflow plugin`.
#[derive(Subcommand)]
pub enum PluginCommands {
    /// List all registered plugins
    #[command(after_help = "Examples:\n  renderflow plugin list")]
    List,

    /// Print detailed information about a named plugin
    #[command(after_help = "Examples:\n  renderflow plugin info my-plugin")]
    Info {
        /// Name of the plugin to inspect
        name: String,
    },

    /// Validate all registered plugin metadata and report any issues
    #[command(after_help = "Examples:\n  renderflow plugin validate")]
    Validate,

    /// Inspect configured declarations, or diagnose a legacy host registry
    ///
    /// With --transforms, never runs plugin code or probes programs. Legacy
    /// host registries retain their existing required-tool diagnostics.
    #[command(after_help = "Examples:\n  renderflow plugin doctor")]
    Doctor,
}

/// Subcommands for `renderflow tools`.
#[derive(Subcommand)]
pub enum ToolCommands {
    /// Inspect the versioned adapter-pack catalog and adopt/adapt/reject matrix.
    Ecosystem {
        /// Output format: text (default), json, or yaml.
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        /// Limit output to providers for one stable capability ID.
        #[arg(long, value_name = "CAPABILITY")]
        capability: Option<String>,
        /// Prefer an adapter ID for capability selection; repeat for fallback order.
        #[arg(long, value_name = "ADAPTER", requires = "capability")]
        preferred: Vec<String>,
        /// Only include providers available on this host.
        #[arg(long)]
        available_only: bool,
    },

    /// List registered providers and live availability/version state.
    List {
        /// Output format: text (default), json, or yaml.
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        /// Optional transform YAML whose dynamic providers should be included.
        #[arg(long, value_name = "FILE")]
        transforms: Option<String>,
    },

    /// Inspect one stable provider ID.
    Inspect {
        /// Stable provider/tool identifier, for example `tool.ffmpeg`.
        id: String,
        /// Output format: text (default), json, or yaml.
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        /// Optional transform YAML whose dynamic providers should be included.
        #[arg(long, value_name = "FILE")]
        transforms: Option<String>,
    },

    /// List stable provider variants/models and optional runtime material evidence.
    Variants {
        /// Stable provider/tool identifier, for example `tool.upscayl-ncnn`.
        id: String,
        /// Optional provider model/material directory used for runtime discovery.
        #[arg(long, value_name = "DIR")]
        models_dir: Option<String>,
        /// Output format: text (default), json, or yaml.
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
}

/// Subcommands for `renderflow ai`.
#[derive(Subcommand)]
pub enum AiCommands {
    /// Inspect the versioned provider/model compatibility matrix
    #[command(
        after_help = "Examples:\n  renderflow ai matrix\n  renderflow ai matrix --format json"
    )]
    Matrix {
        /// Output format: text (default), json, or yaml
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        /// Optional path to a versioned catalog JSON file
        #[arg(long, value_name = "FILE")]
        catalog: Option<String>,
    },

    /// Resolve a reviewed AI skill to a compatible provider/model
    #[command(
        after_help = "Examples:\n  renderflow ai resolve --skill skill.metadata.extract\n  renderflow ai resolve --skill skill.metadata.extract --allow-unverified --format json"
    )]
    Resolve {
        /// Stable skill ID
        #[arg(long, value_name = "ID")]
        skill: String,
        /// Optional exact skill version
        #[arg(long, value_name = "VERSION")]
        skill_version: Option<String>,
        /// Execution preference
        #[arg(long, default_value = "local-preferred", value_name = "PREFERENCE")]
        execution_preference: String,
        /// Explicitly allow remote candidates when the skill policy also permits them
        #[arg(long)]
        allow_remote: bool,
        /// Permit planning against catalog entries whose live availability is unverified
        #[arg(long)]
        allow_unverified: bool,
        /// Output format: text (default), json, or yaml
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        /// Optional path to a versioned catalog JSON file
        #[arg(long, value_name = "FILE")]
        catalog: Option<String>,
    },

    /// Inspect and validate reviewed, versioned AI skills
    Skills {
        #[command(subcommand)]
        subcommand: AiSkillCommands,
    },

    /// List available AI providers and their capabilities
    ///
    /// Prints a table of all known providers (Ollama, OpenAI) with their
    /// locality (local/remote) and supported capabilities.
    #[command(after_help = "Examples:\n  renderflow ai providers")]
    Providers,

    /// List available AI models per provider
    ///
    /// Prints the default model list for each known provider.  For Ollama
    /// the list is the built-in set of common models; run `ollama list` to
    /// see models actually installed locally.
    #[command(after_help = "Examples:\n  renderflow ai models")]
    Models,

    /// Run AI provider connectivity diagnostics
    ///
    /// Checks whether each provider's endpoint is reachable and prints
    /// actionable guidance for any issues found.  Always returns `Ok` so
    /// callers can use the output as advisory information.
    #[command(after_help = "Examples:\n  renderflow ai doctor")]
    Doctor {
        /// Base URL of the Ollama server to probe (default: http://localhost:11434)
        #[arg(long, default_value = "http://localhost:11434", value_name = "URL")]
        ollama_endpoint: String,
    },

    /// Show AI response cache statistics
    ///
    /// Reads the AI cache file (if it exists) and prints summary statistics:
    /// number of cached entries, total size, and per-provider/model counts.
    #[command(
        after_help = "Examples:\n  renderflow ai cache\n  renderflow ai cache --path .renderflow-ai-cache.json"
    )]
    Cache {
        /// Path to the AI cache file
        #[arg(long, default_value = ".renderflow-ai-cache.json", value_name = "FILE")]
        path: String,
    },
}

/// Subcommands for versioned Renderflow AI skills.
#[derive(Subcommand)]
pub enum AiSkillCommands {
    /// List bundled AI skills
    List {
        /// Output format: text (default), json, or yaml
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    /// Inspect one bundled AI skill
    Inspect {
        /// Stable skill ID
        id: String,
        /// Optional exact skill version
        #[arg(long, value_name = "VERSION")]
        version: Option<String>,
        /// Output format: text (default), json, or yaml
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    /// Validate all bundled skills or one external skill file
    Validate {
        /// Optional path to a skill JSON file
        #[arg(long, value_name = "FILE")]
        path: Option<String>,
        /// Output format: text (default), json, or yaml
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
}

/// Subcommands for optional, versioned Artifact DNA.
#[derive(Subcommand)]
pub enum DnaCommands {
    /// Extract local deterministic DNA from an immutable source artifact
    Extract {
        /// Source artifact to inspect
        #[arg(long, value_name = "FILE")]
        input: String,
        /// Optional output file; omit to print to standard output
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        /// Content-addressed artifact-store root
        #[arg(long, default_value = ".renderflow/dna-artifacts", value_name = "DIR")]
        store: String,
        /// Source-reported media type used as an intake signal
        #[arg(long, value_name = "TYPE")]
        media_type: Option<String>,
        /// Serialization format: json (default) or yaml
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
        /// Maximum source size permitted for extraction
        #[arg(long, default_value_t = 67_108_864, value_name = "BYTES")]
        max_source_bytes: u64,
        /// Maximum number of observations permitted in the result
        #[arg(long, default_value_t = 512, value_name = "COUNT")]
        max_observations: usize,
        /// Explicitly allow registered AI-assisted extractors
        #[arg(long)]
        allow_ai: bool,
        /// Explicitly allow extractors that require network access
        #[arg(long)]
        allow_network: bool,
        /// Explicitly allow registered remote extractors
        #[arg(long, requires_all = ["allow_ai", "allow_network"])]
        allow_remote: bool,
        /// Protected artist, creator, brand, franchise, or work name to omit
        #[arg(long, value_name = "TERM")]
        protected_reference: Vec<String>,
    },
    /// Validate a versioned Artifact DNA JSON document
    Validate {
        /// Artifact DNA JSON document
        #[arg(long, value_name = "FILE")]
        input: String,
        /// Output format: text (default), json, or yaml
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    /// Compare shared, similarity-eligible descriptive dimensions
    Compare {
        /// Left Artifact DNA JSON document
        #[arg(long, value_name = "FILE")]
        left: String,
        /// Right Artifact DNA JSON document
        #[arg(long, value_name = "FILE")]
        right: String,
        /// Optional report output; omit to print to standard output
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        /// Serialization format: json (default) or yaml
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
    },
}

/// Subcommands for versioned local font registries.
#[derive(Subcommand)]
pub enum FontCommands {
    /// Validate registry structure, local bytes, digests, and license artifacts
    Validate {
        /// Font registry YAML or JSON file
        #[arg(long, value_name = "FILE")]
        registry: String,
        /// Output format: text (default), json, or yaml
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    /// Resolve every configured semantic role for one renderer target
    Resolve {
        /// Font registry YAML or JSON file
        #[arg(long, value_name = "FILE")]
        registry: String,
        /// Renderer target: html, latex, pdf, epub, or docx
        #[arg(long, value_name = "TARGET")]
        target: String,
        /// Optional report output file
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        /// Output format: json (default) or yaml
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
    },
    /// Emit deterministic HTML/EPUB @font-face CSS for resolved local assets
    Css {
        /// Font registry YAML or JSON file
        #[arg(long, value_name = "FILE")]
        registry: String,
        /// Optional CSS output file
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
    },
}

/// Subcommands for `renderflow graph`.
#[derive(Subcommand)]
pub enum GraphCommands {
    /// Display the canonical execution plan
    ///
    /// Loads the transform graph, computes the optimal DAG for the configured
    /// outputs, and prints the execution plan.
    #[command(after_help = "Examples:\n  \
            renderflow graph plan\n  \
            renderflow graph plan --format mermaid\n  \
            renderflow graph plan --format json --export plan.json\n  \
            renderflow graph plan --target pdf\n  \
            renderflow graph plan --profile everything")]
    Plan {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Output format.
        /// Choices: text (default), dot, mermaid, json, yaml, markdown.
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,

        /// Limit the plan to this output format only.
        #[arg(long, value_name = "FORMAT", conflicts_with = "profile")]
        target: Option<String>,

        /// Resolve a named, versioned derivative profile. `everything`, `magazine`, and `coloring-book` are bundled.
        #[arg(long, value_name = "PROFILE", conflicts_with = "target")]
        profile: Option<String>,

        /// Exclude a branch selector such as `family:video`.
        #[arg(long, value_name = "SELECTOR")]
        exclude: Vec<String>,

        /// Write the output to a file instead of stdout.
        #[arg(long, short = 'o', value_name = "FILE")]
        export: Option<String>,

        /// Optimization mode override.
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },

    /// Render the transformation graph as a visual diagram
    ///
    /// Produces a visual representation of the execution graph.
    /// Defaults to Mermaid output suitable for embedding in GitHub Markdown.
    #[command(after_help = "Examples:\n  \
            renderflow graph render\n  \
            renderflow graph render --format dot\n  \
            renderflow graph render --format mermaid --export graph.mmd")]
    Render {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Output format.
        /// Choices: mermaid (default), dot, text, json, yaml, markdown.
        #[arg(long, default_value = "mermaid", value_name = "FORMAT")]
        format: String,

        /// Limit the graph to this output format only.
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,

        /// Write the output to a file instead of stdout.
        #[arg(long, short = 'o', value_name = "FILE")]
        export: Option<String>,

        /// Optimization mode override.
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },

    /// Explain planner decisions and transformation trade-offs
    ///
    /// Prints human-readable diagnostics that describe why the planner chose
    /// particular paths, lists lossy transforms, and explains optimization
    /// trade-offs.
    #[command(after_help = "Examples:\n  \
            renderflow graph explain\n  \
            renderflow graph explain --target pdf")]
    Explain {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Limit diagnostics to this output format only.
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,

        /// Optimization mode override.
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },

    /// Export the execution plan to a file
    ///
    /// Serializes the full execution plan to the requested format and writes it
    /// to a file.  Suitable for use as a CI artifact.
    #[command(after_help = "Examples:\n  \
            renderflow graph export --format json -o plan.json\n  \
            renderflow graph export --format markdown -o plan.md\n  \
            renderflow graph export --format dot -o graph.dot")]
    Export {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Export format.
        /// Choices: json (default), yaml, mermaid, dot, markdown, text.
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,

        /// Path of the output file (required).
        #[arg(long, short = 'o', value_name = "FILE", required = true)]
        output: String,

        /// Limit the export to this output format only.
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,

        /// Optimization mode override.
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },

    /// Run health checks on the transformation graph
    ///
    /// Checks the execution plan for issues such as lossy transforms and
    /// reports any error-level diagnostics.  Exits with a non-zero status
    /// when errors are found.
    #[command(after_help = "Examples:\n  renderflow graph doctor")]
    Doctor {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Limit diagnostics to this output format only.
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,

        /// Optimization mode override.
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },

    /// Print graph statistics
    ///
    /// Outputs node count, edge count, execution depth, wave count, estimated
    /// cost, and estimated quality for the planned execution graph.
    #[command(after_help = "Examples:\n  renderflow graph stats")]
    Stats {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Limit statistics to this output format only.
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,

        /// Optimization mode override.
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },
}

#[cfg(test)]
mod screenplay_cli_tests {
    use super::*;

    #[test]
    fn screenplay_inspect_defaults_to_json_report() {
        let cli = Cli::try_parse_from([
            "renderflow", "screenplay", "inspect", "--input", "script.screenplay.json",
        ]).unwrap();
        match cli.command {
            Some(Commands::Screenplay {
                subcommand: ScreenplayCommands::Inspect { input, format },
            }) => {
                assert_eq!(input, "script.screenplay.json");
                assert_eq!(format, "json");
            }
            _ => panic!("expected screenplay inspect command"),
        }
    }

    #[test]
    fn screenplay_inspect_accepts_text_report() {
        let cli = Cli::try_parse_from([
            "renderflow", "screenplay", "inspect", "--input", "script.screenplay.json",
            "--format", "text",
        ]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Screenplay {
            subcommand: ScreenplayCommands::Inspect { format, .. },
        }) if format == "text"));
    }

    #[test]
    fn screenplay_requires_an_inspect_command_and_explicit_input() {
        assert!(Cli::try_parse_from(["renderflow", "screenplay"]).is_err());
        assert!(Cli::try_parse_from(["renderflow", "screenplay", "inspect"]).is_err());
    }

    #[test]
    fn screenplay_inspect_rejects_unsupported_format_and_output_flags() {
        assert!(Cli::try_parse_from([
            "renderflow", "screenplay", "inspect", "--input", "script.screenplay.json",
            "--format", "yaml",
        ]).is_err());
        assert!(Cli::try_parse_from([
            "renderflow", "screenplay", "inspect", "--input", "script.screenplay.json",
            "--output", "converted.fdx",
        ]).is_err());
        assert!(Cli::try_parse_from(["renderflow", "screenplay", "convert"]).is_err());
    }

    fn conversion_arguments() -> Vec<&'static str> {
        vec!["renderflow", "screenplay", "convert", "--input", "scene.fdx",
            "--input-format", "fdx", "--output-format", "fountain",
            "--output-dir", "new-bundle", "--scripttool-path", "/opt/scripttool",
            "--scripttool-sha256", "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]
    }

    #[test]
    fn screenplay_conversion_requires_explicit_unqualified_opt_in() {
        let cli = Cli::try_parse_from(conversion_arguments()).unwrap();
        match cli.command {
            Some(Commands::Screenplay { subcommand: ScreenplayCommands::Convert(arguments) }) => {
                assert!(!arguments.allow_unqualified);
                assert_eq!(arguments.expected_version, "0.0.10");
                assert_eq!(arguments.timeout_seconds, 30);
            }
            _ => panic!("expected screenplay convert"),
        }
        let mut argv = conversion_arguments();
        argv.push("--allow-unqualified");
        let cli = Cli::try_parse_from(argv).unwrap();
        assert!(matches!(cli.command, Some(Commands::Screenplay {
            subcommand: ScreenplayCommands::Convert(arguments),
        }) if arguments.allow_unqualified));
    }

    #[test]
    fn screenplay_conversion_rejects_provider_json_and_missing_binary_pin() {
        let mut argv = conversion_arguments();
        let index = argv.iter().position(|arg| *arg == "--output-format").unwrap() + 1;
        argv[index] = "json";
        assert!(Cli::try_parse_from(argv).is_err());
        let mut argv = conversion_arguments();
        argv.truncate(argv.len() - 2);
        assert!(Cli::try_parse_from(argv).is_err());
    }

    #[test]
    fn screenplay_pdf_requires_explicit_config_and_new_bundle_destination() {
        let argv = ["renderflow", "screenplay", "pdf", "--input", "scene.fountain",
            "--config", "afterwriting.json", "--output-dir", "new-pdf-bundle"];
        let cli = Cli::try_parse_from(argv).unwrap();
        assert!(matches!(cli.command, Some(Commands::Screenplay {
            subcommand: ScreenplayCommands::Pdf(arguments),
        }) if arguments.input == "scene.fountain" && arguments.config == "afterwriting.json"
            && arguments.output_dir == "new-pdf-bundle" && arguments.source_sha256.is_none()));
        assert!(Cli::try_parse_from(&argv[..7]).is_err());
        assert!(Cli::try_parse_from(["renderflow", "screenplay", "pdf", "--input", "scene.fountain",
            "--output-dir", "new-bundle"]).is_err());
    }

    #[test]
    fn screenplay_recovery_requires_source_format_and_config_and_accepts_repeated_targets() {
        let argv = ["renderflow", "screenplay", "recover", "--input", "scene.pdf",
            "--source-format", "pdf", "--config", "recovery.json", "--output", "new-recovery",
            "--target", "fountain", "--target", "fdx"];
        let cli = Cli::try_parse_from(argv).unwrap();
        assert!(matches!(cli.command, Some(Commands::Screenplay {
            subcommand: ScreenplayCommands::Recover(arguments),
        }) if arguments.input == "scene.pdf" && arguments.source_format == "pdf"
            && arguments.config == "recovery.json" && arguments.output == "new-recovery"
            && arguments.targets == vec!["fountain", "fdx"] && arguments.source_sha256.is_none()));
        for arguments in [
            vec!["renderflow", "screenplay", "recover", "--input", "scene.pdf", "--output", "new-recovery"],
            vec!["renderflow", "screenplay", "recover", "--input", "scene.pdf", "--source-format", "pdf", "--output", "new-recovery"],
            vec!["renderflow", "screenplay", "recover", "--input", "scene.pdf", "--source-format", "pdf", "--config", "recovery.json", "--output", "new-recovery", "--target", "pdf"],
            vec!["renderflow", "screenplay", "recover", "--input", "scene.fdx", "--source-format", "fdx", "--config", "recovery.json", "--output", "new-recovery"],
        ] {
            assert!(Cli::try_parse_from(arguments).is_err());
        }
    }

    #[test]
    fn screenplay_analysis_is_separate_from_provider_pdf_generation() {
        let cli = Cli::try_parse_from(["renderflow", "screenplay", "analyze", "--input", "scene.fountain"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Screenplay {
            subcommand: ScreenplayCommands::Analyze { input },
        }) if input == "scene.fountain"));
        assert!(Cli::try_parse_from(["renderflow", "screenplay", "analyze", "--input", "scene.fountain",
            "--config", "afterwriting.json"]).is_err());
    }

    #[test]
    fn screenplay_package_fingerprint_requires_explicit_root() {
        let cli = Cli::try_parse_from(["renderflow", "screenplay", "package-fingerprint",
            "--package-root", "/opt/afterwriting"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Screenplay {
            subcommand: ScreenplayCommands::PackageFingerprint { package_root },
        }) if package_root == "/opt/afterwriting"));
        assert!(Cli::try_parse_from(["renderflow", "screenplay", "package-fingerprint"]).is_err());
    }
}
