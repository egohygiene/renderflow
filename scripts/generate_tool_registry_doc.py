#!/usr/bin/env python3

from __future__ import annotations

from pathlib import Path

import yaml


ROOT = Path(__file__).resolve().parents[1]
REGISTRY_PATH = ROOT / "crates" / "renderflow-core" / "data" / "tool-registry.yaml"
DOC_PATH = ROOT / "docs" / "user-guide" / "tool-registry.md"


def render_table(headers: list[str], rows: list[list[str]]) -> list[str]:
    lines = [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    for row in rows:
        lines.append("| " + " | ".join(row) + " |")
    return lines


def discovery_label(discovery: dict[str, object]) -> str:
    kind = str(discovery.get("kind", "unknown"))
    if kind == "executable":
        candidates = discovery.get("candidates", [])
        return "executable: " + ", ".join(f"`{value}`" for value in candidates)
    if kind == "runtime_service":
        return f"service: `{discovery.get('service', '-')}`"
    return kind


def main() -> None:
    registry = yaml.safe_load(REGISTRY_PATH.read_text(encoding="utf-8"))
    schema = registry["schema"]
    tools = sorted(registry.get("tools", []), key=lambda tool: tool["id"])

    tool_rows: list[list[str]] = []
    capability_rows: list[list[str]] = []
    for tool in tools:
        tool_rows.append(
            [
                f"`{tool['id']}`",
                str(tool["name"]),
                discovery_label(tool["discovery"]),
                str(tool.get("support_tier", "optional")),
                str(tool.get("determinism", "-")),
                str(tool.get("locality", "-")),
            ]
        )
        for capability in sorted(tool.get("capabilities", [])):
            capability_rows.append([f"`{capability}`", f"`{tool['id']}`"])

    content = [
        "# Tool Registry",
        "",
        "!!! info",
        "    This page is generated from the canonical built-in catalog at",
        "    `crates/renderflow-core/data/tool-registry.yaml` by",
        "    `scripts/generate_tool_registry_doc.py`. Do not edit it by hand.",
        "",
        f"Registry schema: `{schema}`",
        "",
        "The built-in catalog defines stable provider IDs, discovery/version probes,",
        "platform constraints, capability IDs, determinism/locality/fidelity metadata,",
        "runtime requirements, fallback relationships, and distribution/licensing notes.",
        "YAML command transforms and plugins can register additional runtime providers",
        "without editing this built-in catalog.",
        "",
        "## Built-in providers",
        "",
        *render_table(
            ["Provider ID", "Name", "Discovery", "Tier", "Determinism", "Locality"],
            tool_rows,
        ),
        "",
        "## Capability matrix",
        "",
        *render_table(["Capability ID", "Provider ID"], capability_rows),
        "",
        "## Runtime inspection",
        "",
        "Use the CLI to inspect the live host using the same model:",
        "",
        "```text",
        "renderflow tools list",
        "renderflow tools inspect tool.ffmpeg",
        "renderflow capabilities",
        "renderflow doctor",
        "```",
        "",
        "Add `--format json` or `--format yaml` to the tools/capabilities commands",
        "when machine-readable evidence is needed.",
        "",
    ]

    DOC_PATH.write_text("\n".join(content), encoding="utf-8")


if __name__ == "__main__":
    main()
