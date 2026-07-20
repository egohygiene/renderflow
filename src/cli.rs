use clap::{Parser, Subcommand};

use crate::optimization::OptimizationMode;

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
            renderflow build --all                  Build all reachable outputs via graph resolution")]
    Build {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Simulate execution: log intended actions without creating files or running commands
        #[arg(long)]
        dry_run: bool,

        /// Optimization mode: controls how transformation paths are selected.
        /// Overrides the value set in the config file when provided.
        /// Choices: speed (minimise cost), quality (maximise quality), balanced (default),
        /// pareto (return Pareto-optimal frontier of non-dominated paths).
        #[arg(long, value_name = "MODE")]
        optimization: Option<OptimizationMode>,

        /// Build only the specified output format using graph-based path resolution.
        /// The format must be reachable from the input format via the configured transforms.
        /// Requires a 'transforms' key in the config file.
        /// Cannot be combined with --all.
        #[arg(long, value_name = "FORMAT", conflicts_with = "all")]
        target: Option<String>,

        /// Build all reachable output formats using graph-based path resolution.
        /// Requires a 'transforms' key in the config file.
        /// Cannot be combined with --target.
        #[arg(long, conflicts_with = "target")]
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

    /// Visualize the transformation DAG and execution plan
    #[command(after_help = "Examples:\n  \
            renderflow inspect                          Show DAG tree for renderflow.yaml\n  \
            renderflow inspect --config custom.yaml    Show DAG tree for a custom config\n  \
            renderflow inspect --output-format dot     Emit Graphviz DOT output to stdout\n  \
            renderflow inspect --target pdf            Show execution plan for a single target\n  \
            renderflow inspect --all --export dag.dot  Export full DAG to a DOT file")]
    Inspect {
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

    /// Manage and inspect AI providers and the AI transform cache
    ///
    /// These commands help you discover configured AI providers, inspect
    /// available models, run connectivity diagnostics, and manage the AI
    /// response cache.
    #[command(
        subcommand_required = true,
        arg_required_else_help = true,
        after_help = "Examples:\n  \
            renderflow ai providers             List available AI providers\n  \
            renderflow ai models                List available models per provider\n  \
            renderflow ai doctor                Run AI provider diagnostics\n  \
            renderflow ai cache                 Show AI cache statistics"
    )]
    Ai {
        #[command(subcommand)]
        subcommand: AiCommands,
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

/// Subcommands for `renderflow ai`.
#[derive(Subcommand)]
pub enum AiCommands {
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
    #[command(after_help = "Examples:\n  renderflow ai cache\n  renderflow ai cache --path .renderflow-ai-cache.json")]
    Cache {
        /// Path to the AI cache file
        #[arg(long, default_value = ".renderflow-ai-cache.json", value_name = "FILE")]
        path: String,
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
            renderflow graph plan --target pdf")]
    Plan {
        /// Path to the renderflow configuration file
        #[arg(long, default_value = "renderflow.yaml", value_name = "FILE")]
        config: String,

        /// Output format.
        /// Choices: text (default), dot, mermaid, json, yaml, markdown.
        #[arg(long, default_value = "text", value_name = "FORMAT")]
        format: String,

        /// Limit the plan to this output format only.
        #[arg(long, value_name = "FORMAT")]
        target: Option<String>,

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
