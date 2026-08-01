import type { RenderflowCapability } from "./types";

export const capabilities: readonly RenderflowCapability[] = [
	{
		identifier: "yaml-spec",
		title: "Configuration-driven builds",
		description:
			"Renderflow loads a YAML spec, validates it, and uses it as the repeatable contract for each build.",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/configuration.md",
		evidence: "docs/user-guide/configuration.md",
	},
	{
		identifier: "document-outputs",
		title: "Markdown to publication outputs",
		description:
			"The documented document workflow supports HTML, PDF, and DOCX targets from a single source.",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/examples/multi-output.md",
		evidence: "README.md, docs/examples/multi-output.md",
	},
	{
		identifier: "graph-planning",
		title: "Graph-planned execution",
		description:
			"The graph command family produces canonical execution plans, diagnostics, and exports in text, JSON, YAML, Mermaid, DOT, and Markdown.",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/cli-reference/graph.md",
		evidence: "docs/cli-reference/graph.md",
	},
	{
		identifier: "optimization",
		title: "Deterministic optimization modes",
		description:
			"Path selection is controlled by explicit speed, quality, balanced, and pareto optimization modes.",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/optimization.md",
		evidence: "docs/user-guide/optimization.md",
	},
	{
		identifier: "incremental-caching",
		title: "Incremental caching",
		description:
			"Build, output, dependency, graph, and AI caches are content-hash based so unchanged work can be reused safely.",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/caching.md",
		evidence: "docs/user-guide/caching.md",
	},
	{
		identifier: "watch-mode",
		title: "Watch mode",
		description:
			"The watch command performs an initial build, watches config/input/template changes, and rebuilds with resilient transform handling.",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/cli-reference/watch.md",
		evidence: "docs/cli-reference/watch.md",
	},
	{
		identifier: "plugins",
		title: "Library-side plugins",
		description:
			"Embedding applications can register runtime plugin executors and metadata without changing Renderflow core.",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/plugins.md",
		evidence: "docs/user-guide/plugins.md",
	},
	{
		identifier: "ai-transforms",
		title: "Optional AI transforms",
		description:
			"AI-backed transforms are supported through Ollama and OpenAI-compatible providers with prompt versioning and cache support when configured.",
		status: "available",
		documentationPath: "https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/ai.md",
		evidence: "docs/user-guide/ai.md",
	},
	{
		identifier: "container-distribution",
		title: "Container distribution",
		description:
			"Docker and OCI images are documented as future distribution targets rather than a shipping installation path today.",
		status: "planned",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/getting-started/installation.md",
		evidence: "docs/getting-started/installation.md",
	},
];
