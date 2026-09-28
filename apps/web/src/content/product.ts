import type { ExternalLink, InstallationMethod } from "./types";

export const productIdentity = {
	name: "renderflow",
	tagline: "Spec-driven document rendering engine",
	summary:
		"Transform Markdown into publication-ready PDF and HTML — defined once, rendered anywhere.",
	repositoryUrl: "https://github.com/egohygiene/renderflow",
	documentationUrl: "https://egohygiene.github.io/renderflow/",
	ecosystemUrl: "https://egohygiene.io/",
	configurationExample: `input: report.md
input_format: markdown
output_dir: dist
optimization: balanced
transforms: transforms.yaml
variables:
  title: Quarterly Report
  author: Jane Smith
outputs:
  - type: html
    template: default
  - type: pdf
  - type: docx`,
	configurationSource:
		"https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/configuration.md",
} as const;

export const productPipeline = [
	"Markdown and source assets",
	"renderflow.yaml",
	"Graph planning",
	"Transforms and optimization",
	"Validated output artifacts",
	"PDF, HTML, and supported targets",
] as const;

export const whyRenderflow = [
	"Define rendering behavior once in a versioned YAML spec.",
	"Produce multiple publication targets from one source document.",
	"Preview graph plans and dry runs before committing expensive work.",
	"Reuse the same engine through the Rust library or the standalone CLI.",
	"Keep builds reproducible with explicit optimization modes and cache metadata.",
	"Preserve provenance and diagnostics across planning, transforms, and rendering.",
] as const;

export const architectureLayers = [
	{
		title: "renderflow-core",
		description:
			"Owns config loading, transform orchestration, graph planning, caching, and output strategies.",
	},
	{
		title: "renderflow-cli",
		description:
			"Provides the terminal-oriented command surface without reimplementing rendering behavior.",
	},
	{
		title: "Planner and DAG executor",
		description:
			"Builds canonical execution plans, groups independent work into waves, and exports readable diagnostics.",
	},
	{
		title: "Transforms and adapters",
		description:
			"Combines built-in transforms, YAML-defined command transforms, AI providers, and library-side plugin boundaries.",
	},
	{
		title: "Artifact storage and diagnostics",
		description:
			"Writes outputs plus cache metadata so dry-run, watch, and incremental workflows stay inspectable.",
	},
] as const;

export const architectureLinks: readonly ExternalLink[] = [
	{
		href: "https://github.com/egohygiene/renderflow/blob/main/docs/architecture/overview.md",
		label: "Architecture overview",
	},
	{
		href: "https://github.com/egohygiene/renderflow/blob/main/docs/architecture/execution-plans.md",
		label: "Execution plan reference",
	},
	{
		href: "https://github.com/egohygiene/renderflow/blob/main/docs/architecture/plugin-architecture.md",
		label: "Plugin architecture",
	},
] as const;

export const installationMethods: readonly InstallationMethod[] = [
	{
		identifier: "source-checkout",
		title: "Reviewed source checkout",
		command: "cargo install --locked --path crates/renderflow-cli",
		notes:
			"Run from the reviewed repository root with Rust 1.94 or newer. This does not verify a downloaded release artifact or crates.io publication.",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/getting-started/installation.md",
	},
	{
		identifier: "linux-release",
		title: "Ubuntu 24.04 x86_64 GNU candidate",
		notes:
			"v0.3.0-rc.1 is proposed for glibc 2.39 or newer on Ubuntu 24.04. Other distro baselines are unverified. Install only after the exact tag, binary, checksum, manifest, and clean-host evidence are published.",
		status: "planned",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/getting-started/installation.md",
	},
	{
		identifier: "package-managers",
		title: "Package-manager channels",
		notes:
			"Homebrew, Scoop, Chocolatey, Snap, AUR, Debian/RPM, and crates.io are unpublished or unverified for the first integration candidate.",
		status: "planned",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/getting-started/installation.md",
	},
	{
		identifier: "container-images",
		title: "Docker / OCI images",
		notes: "Listed in the installation guide as a planned distribution target.",
		status: "planned",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/getting-started/installation.md",
	},
] as const;
