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

        /// Simulate execution: log intended actions without creating files or running commands
        #[arg(long)]
        dry_run: bool,

        /// Resume from compatible validated node checkpoints.
        #[arg(long)]
        resume: bool,

        /// Optimization mode: controls how transformation paths are selected.
        /// Overrides the value set in the config file when provided.
        /// Choices: speed (minimise cost), quality (maximise quality), balanced (default),
        /// pareto (return Pareto-optimal frontier of non-dominated paths).
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,

        /// Build only the specified output format using the canonical capability graph.
        /// Built-in document/image/audio capabilities and optional configured transforms are
        /// resolved through the same planner. Cannot be combined with --all.
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
            renderflow plugin list              List all registered plugins\n  \
            renderflow plugin info my-plugin    Show details for 'my-plugin'\n  \
            renderflow plugin validate          Validate all plugin metadata\n  \
            renderflow plugin doctor            Run diagnostics on all plugins"
    )]
    Plugin {
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
    Doctor {
        /// Exit with non-zero status when required dependencies are missing
        #[arg(long)]
        strict: bool,
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

    /// Run diagnostics on all registered plugins
    ///
    /// Checks required external tools, validates metadata, and reports
    /// actionable issues.
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
