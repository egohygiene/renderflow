import type { RenderflowExample } from "./types";

export const examples: readonly RenderflowExample[] = [
	{
		identifier: "hello-world",
		title: "Minimal Markdown to HTML",
		useCase: "Verify a new Renderflow install with the smallest documented config.",
		inputSummary: "examples/hello-world/hello.md with a single HTML output target.",
		configuration: `input: "hello.md"
output_dir: "dist"
outputs:
  - type: html`,
		expectedOutputs: ["dist/hello.html"],
		command: "renderflow build",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/examples/hello-world/renderflow.yaml",
	},
	{
		identifier: "multi-output",
		title: "Multiple outputs from one spec",
		useCase:
			"Generate HTML, PDF, and DOCX artifacts from one Markdown source and shared variables.",
		inputSummary: "Document-oriented build using outputs[] with template support.",
		configuration: `input: report.md
output_dir: dist
variables:
  title: Annual Report
outputs:
  - type: html
    template: default
  - type: pdf
  - type: docx`,
		expectedOutputs: ["dist/report.html", "dist/report.pdf", "dist/report.docx"],
		command: "renderflow build",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/examples/multi-output.md",
	},
	{
		identifier: "transform-pipeline",
		title: "Built-in transform pipeline",
		useCase: "Apply variables, emoji handling, and syntax normalization before rendering HTML.",
		inputSummary:
			"examples/transforms/document.md with variables and mixed-case fenced code blocks.",
		configuration: `input: "document.md"
output_dir: "dist"
variables:
  title: "Transform Pipeline Demo"
  author: "Jane Smith"
  version: "1.0"
outputs:
  - type: html`,
		expectedOutputs: ["dist/document.html"],
		command: "renderflow build --config renderflow.yaml",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/examples/transforms/README.md",
	},
	{
		identifier: "graph-plan",
		title: "Dry-run graph planning",
		useCase: "Inspect a canonical execution plan before running graph-driven work.",
		inputSummary: "Config plus transforms.yaml loaded through the graph command family.",
		configuration: `input: report.md
optimization: balanced
transforms: transforms.yaml`,
		expectedOutputs: ["Execution plan in text, JSON, YAML, Mermaid, DOT, or Markdown"],
		command: "renderflow graph plan --format mermaid --optimization speed",
		status: "available",
		documentationPath:
			"https://github.com/egohygiene/renderflow/blob/main/docs/cli-reference/graph.md",
	},
	{
		identifier: "ai-transform",
		title: "Optional AI-assisted preprocessing",
		useCase:
			"Summarize, translate, or rewrite content as a configured transform before the normal render pipeline continues.",
		inputSummary:
			"AI transform definition with provider, model, prompt, cache path, and env-based API key configuration.",
		configuration: `transforms:
  - name: summarize
    ai: openai
    model: gpt-4o-mini
    prompt: |
      Summarize the following document for release notes:

      {input}
    api_key_env: OPENAI_API_KEY
    cache_path: .renderflow-ai-cache.json
    prompt_version: v1
    from: markdown
    to: markdown
    cost: 2.0
    quality: 0.9`,
		expectedOutputs: ["Markdown-to-markdown transform stage before final render"],
		command: "renderflow ai doctor --ollama-endpoint http://localhost:11434",
		status: "available",
		documentationPath: "https://github.com/egohygiene/renderflow/blob/main/docs/user-guide/ai.md",
	},
];
