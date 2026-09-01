#!/usr/bin/env python3
"""Generate the Renderflow spec v2 reference from the runtime JSON Schema."""

from __future__ import annotations

import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCHEMA_PATH = ROOT / "schemas" / "renderflow-v2.schema.json"
OUTPUT_PATH = ROOT / "docs" / "user-guide" / "spec-v2-reference.md"
EXAMPLE_PATH = ROOT / "examples" / "renderflow-v2.yaml"


def format_default(schema: dict[str, object]) -> str:
    """Return a compact Markdown representation of a JSON Schema default."""
    if "default" not in schema:
        return "—"
    return f"`{json.dumps(schema['default'], separators=(',', ':'))}`"


def property_type(schema: dict[str, object]) -> str:
    """Render a Markdown-safe type or enum summary for a schema property."""
    if "$ref" in schema:
        return f"`{str(schema['$ref']).split('/')[-1]}`"
    if "const" in schema:
        return f"`{json.dumps(schema['const'])}`"
    if "enum" in schema:
        return " / ".join(f"`{value}`" for value in schema["enum"])
    value = schema.get("type", "object")
    if isinstance(value, list):
        return " / ".join(f"`{item}`" for item in value)
    return f"`{value}`"


def render_properties(title: str, schema: dict[str, object]) -> list[str]:
    """Render one JSON Schema object's properties as a Markdown table."""
    properties = schema.get("properties", {})
    required = set(schema.get("required", []))
    lines = [
        f"## {title}",
        "",
        "| Field | Type | Required | Default |",
        "| --- | --- | --- | --- |",
    ]
    for name, definition in properties.items():
        definition = dict(definition)
        lines.append(
            f"| `{name}` | {property_type(definition)} | "
            f"{'yes' if name in required else 'no'} | {format_default(definition)} |"
        )
    lines.append("")
    return lines


def main() -> None:
    """Generate the checked-in spec v2 reference page."""
    schema = json.loads(SCHEMA_PATH.read_text(encoding="utf-8"))
    example = EXAMPLE_PATH.read_text(encoding="utf-8").rstrip()
    definitions = schema["$defs"]

    lines = [
        "<!-- GENERATED FILE: run scripts/generate_spec_v2_reference.py -->",
        "# Renderflow spec v2 reference",
        "",
        "This page is generated from the JSON Schema emitted by the Renderflow runtime.",
        "Do not edit it by hand.",
        "",
        f"**Schema identifier:** `{schema['properties']['schema']['const']}`",
        "",
        "Spec v2 describes source intent, derivative selection, execution policy, and deterministic output layout. Planning resolves this intent into an execution plan; the specification itself does not encode a resolved DAG.",
        "",
    ]
    lines.extend(render_properties("Top-level fields", schema))
    lines.extend(render_properties("Source", definitions["source"]))
    lines.extend(render_properties("Target selection", definitions["targetSelection"]))
    lines.extend(render_properties("Execution policy", definitions["executionPolicy"]))
    lines.extend(render_properties("Output layout", definitions["outputLayout"]))
    lines.extend(
        [
            "## Compatibility",
            "",
            "Unversioned configuration files are treated as the explicit v1 compatibility format. Use `renderflow spec migrate` to produce a v2 document. Unsupported declared schema identifiers are rejected rather than reinterpreted.",
            "",
            "## Example",
            "",
            "```yaml",
            example,
            "```",
            "",
        ]
    )

    OUTPUT_PATH.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT_PATH.write_text("\n".join(lines), encoding="utf-8")
    print(f"Generated {OUTPUT_PATH.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
