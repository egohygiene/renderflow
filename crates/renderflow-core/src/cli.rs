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
            renderflow build --profile magazine    Build a versioned magazine release bundle")]
    Build {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        resume: bool,
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
        #[arg(long, value_name = "FORMAT", conflicts_with_all = ["all", "profile"])]
        target: Option<String>,
        #[arg(long, value_name = "PROFILE", conflicts_with_all = ["target", "all"])]
        profile: Option<String>,
        #[arg(long, value_name = "SELECTOR")]
        exclude: Vec<String>,
        #[arg(long, conflicts_with_all = ["target", "profile"])]
        all: bool,
    },
    Watch {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, default_value = "500", value_name = "MS")]
        debounce: u64,
    },
    Audit,
    Inspect {
        #[arg(long, value_name = "FILE", conflicts_with_all = ["target", "all"])]
        input: Option<String>,
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, default_value = "tree", value_name = "FORMAT")]
        output_format: String,
        #[arg(long, value_name = "FORMAT", conflicts_with = "all")]
        target: Option<String>,
        #[arg(long, conflicts_with = "target")]
        all: bool,
        #[arg(long, value_name = "FILE")]
        export: Option<String>,
        #[arg(long, value_name = "TYPE", requires = "input")]
        media_type: Option<String>,
        #[arg(long, requires = "input")]
        extract: bool,
        #[arg(long, requires = "extract")]
        recursive: bool,
        #[arg(long, default_value = ".renderflow/intake-artifacts", value_name = "DIR", requires = "input")]
        store: String,
        #[arg(long, default_value_t = 3, requires = "input")]
        max_depth: u32,
        #[arg(long, default_value_t = 1000, requires = "input")]
        max_artifacts: u64,
        #[arg(long, default_value_t = 536_870_912, requires = "input")]
        max_extracted_bytes: u64,
        #[arg(long, default_value_t = 100.0, requires = "input")]
        max_expansion_ratio: f64,
    },
    Plugin {
        #[command(subcommand)]
        subcommand: PluginCommands,
    },
    Ai {
        #[command(subcommand)]
        subcommand: AiCommands,
    },
    Dna {
        #[command(subcommand)]
        subcommand: DnaCommands,
    },
    Font {
        #[command(subcommand)]
        subcommand: FontCommands,
    },
    Graph {
        #[command(subcommand)]
        subcommand: GraphCommands,
    },
    Tools {
        #[command(subcommand)]
        subcommand: ToolCommands,
    },
    Ebook {
        #[command(subcommand)]
        subcommand: EbookCommands,
    },
    Publication {
        #[command(subcommand)]
        subcommand: PublicationCommands,
    },
    Video {
        #[command(subcommand)]
        subcommand: VideoCommands,
    },
    Capabilities {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        transforms: Option<String>,
        #[arg(long)]
        matrix: bool,
    },
    Spec {
        #[command(subcommand)]
        subcommand: SpecCommands,
    },
    Version,
    Env,
    Doctor {
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
    Capabilities {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
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
    /// Derive deterministic, review-required magazine asset and metadata guidance from validated Artifact DNA.
    MagazineGuidance {
        #[arg(long, value_name = "FILE")]
        dna: String,
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        /// Plan optional provider-neutral AI skill candidates without executing them.
        #[arg(long)]
        plan_ai: bool,
        /// Explicitly permit remote candidates; local/open models remain preferred.
        #[arg(long, requires = "plan_ai")]
        allow_remote: bool,
        /// Explicitly approve sanitized source evidence for model exposure.
        #[arg(long, requires = "plan_ai")]
        source_approved_for_ai: bool,
        /// Explicit privacy approval required before remote model exposure.
        #[arg(long, requires = "allow_remote")]
        privacy_approved_for_remote: bool,
    },
    Lulu {
        #[command(subcommand)]
        subcommand: LuluCommands,
    },
}

#[derive(Subcommand)]
pub enum LuluCommands {
    Rules {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
    },
    Preflight {
        #[arg(long, value_name = "FILE")]
        request: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        #[arg(long)]
        epubcheck: bool,
    },
}

#[derive(Subcommand)]
pub enum EbookCommands {
    Inspect {
        #[arg(long, value_name = "FILE")]
        input: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long)]
        epubcheck: bool,
    },
    Capabilities {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum SpecCommands {
    Validate {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    Migrate {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, short = 'o', value_name = "FILE")]
        output: Option<String>,
    },
    Schema {
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
        #[arg(long, short = 'o', value_name = "FILE")]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum PluginCommands {
    List,
    Info { name: String },
    Validate,
    Doctor,
}

#[derive(Subcommand)]
pub enum ToolCommands {
    Ecosystem {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "CAPABILITY")]
        capability: Option<String>,
        #[arg(long, value_name = "ADAPTER", requires = "capability")]
        preferred: Vec<String>,
        #[arg(long)]
        available_only: bool,
    },
    List {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        transforms: Option<String>,
    },
    Inspect {
        id: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        transforms: Option<String>,
    },
    Variants {
        id: String,
        #[arg(long, value_name = "DIR")]
        models_dir: Option<String>,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum AiCommands {
    Matrix {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        catalog: Option<String>,
    },
    Resolve {
        #[arg(long, value_name = "ID")]
        skill: String,
        #[arg(long, value_name = "VERSION")]
        skill_version: Option<String>,
        #[arg(long, default_value = "local-preferred", value_name = "PREFERENCE")]
        execution_preference: String,
        #[arg(long)]
        allow_remote: bool,
        #[arg(long)]
        allow_unverified: bool,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FILE")]
        catalog: Option<String>,
    },
    Skills {
        #[command(subcommand)]
        subcommand: AiSkillCommands,
    },
    Providers,
    Models,
    Doctor {
        #[arg(long, default_value = "http://localhost:11434", value_name = "URL")]
        ollama_endpoint: String,
    },
    Cache {
        #[arg(long, default_value = ".renderflow-ai-cache.json", value_name = "FILE")]
        path: String,
    },
}

#[derive(Subcommand)]
pub enum AiSkillCommands {
    List {
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    Inspect {
        id: String,
        #[arg(long, value_name = "VERSION")]
        version: Option<String>,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    Validate {
        #[arg(long, value_name = "FILE")]
        path: Option<String>,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum DnaCommands {
    Extract {
        #[arg(long, value_name = "FILE")]
        input: String,
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        #[arg(long, default_value = ".renderflow/dna-artifacts", value_name = "DIR")]
        store: String,
        #[arg(long, value_name = "TYPE")]
        media_type: Option<String>,
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
        #[arg(long, default_value_t = 67_108_864, value_name = "BYTES")]
        max_source_bytes: u64,
        #[arg(long, default_value_t = 512, value_name = "COUNT")]
        max_observations: usize,
        #[arg(long)]
        allow_ai: bool,
        #[arg(long)]
        allow_network: bool,
        #[arg(long, requires_all = ["allow_ai", "allow_network"])]
        allow_remote: bool,
        #[arg(long, value_name = "TERM")]
        protected_reference: Vec<String>,
    },
    Validate {
        #[arg(long, value_name = "FILE")]
        input: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    Compare {
        #[arg(long, value_name = "FILE")]
        left: String,
        #[arg(long, value_name = "FILE")]
        right: String,
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
    },
}

#[derive(Subcommand)]
pub enum FontCommands {
    Validate {
        #[arg(long, value_name = "FILE")]
        registry: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
    },
    Resolve {
        #[arg(long, value_name = "FILE")]
        registry: String,
        #[arg(long, value_name = "TARGET")]
        target: String,
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
    },
    Css {
        #[arg(long, value_name = "FILE")]
        registry: String,
        #[arg(long, value_name = "FILE")]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum GraphCommands {
    Plan {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FORMAT", conflicts_with = "profile")]
        target: Option<String>,
        #[arg(long, value_name = "PROFILE", conflicts_with = "target")]
        profile: Option<String>,
        #[arg(long, value_name = "SELECTOR")]
        exclude: Vec<String>,
        #[arg(long, short = 'o', value_name = "FILE")]
        export: Option<String>,
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },
    Render {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, default_value = "mermaid", value_name = "FORMAT")]
        format: String,
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,
        #[arg(long, value_name = "FILE")]
        export: Option<String>,
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },
    Explain {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },
    Export {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, default_value = "json", value_name = "FORMAT")]
        format: String,
        #[arg(long, short = 'o', value_name = "FILE", required = true)]
        output: String,
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },
    Doctor {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },
    Stats {
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,
    },
}
