#!/usr/bin/env python3
"""Publish creative capability declarations without probing or running tools.

The same route catalog is embedded by creative_doctor.rs. This generator uses
only Python's standard library and writes documentation plus a JSON declaration
matrix; it does not qualify providers, routes, or configuration.
"""

from __future__ import annotations

import hashlib
import json
import posixpath
from collections import Counter
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CATALOG_PATH = Path("crates/renderflow-core/data/creative-routes.json")
DOC_PATH = Path("docs/user-guide/creative-capabilities.md")
JSON_PATH = Path("docs/user-guide/creative-capabilities.json")
REPOSITORY_SOURCE_URL = "https://github.com/egohygiene/renderflow/blob/main/"


def source_link(path: str, label: str | None = None) -> str:
    return f"[{label or path}]({REPOSITORY_SOURCE_URL}{path})"


def guide_link(path: str, label: str = "Guide") -> str:
    target, separator, anchor = path.partition("#")
    relative = posixpath.relpath(target, DOC_PATH.parent.as_posix())
    return f"[{label}]({relative}{separator}{anchor})"


def code_list(values: list[str], empty: str) -> str:
    return ", ".join(f"`{value}`" for value in values) or empty


def main() -> None:
    catalog_bytes = (ROOT / CATALOG_PATH).read_bytes()
    catalog = json.loads(catalog_bytes)
    routes = sorted(catalog["routes"], key=lambda route: route["id"])
    matrix = {
        "schema_version": "renderflow.creative-capabilities/v1",
        "catalog": {
            "path": CATALOG_PATH.as_posix(),
            "sha256": hashlib.sha256(catalog_bytes).hexdigest(),
            "schema_version": catalog["schema_version"],
            "profile": catalog["profile"],
        },
        "runtime_qualified": catalog["runtime_qualified"],
        "scope": catalog["scope"],
        "routes": [
            {**route, "documentation": catalog["documentation"][route["id"]]}
            for route in routes
        ],
        "unsupported_directions": catalog["unsupported_directions"],
    }
    (ROOT / JSON_PATH).write_text(json.dumps(matrix, indent=2) + "\n", encoding="utf-8")

    counts = Counter(route["implementation"] for route in routes)
    count_text = "; ".join(f"{count} `{status}`" for status, count in sorted(counts.items()))
    lines = [
        "# Creative capability matrix",
        "",
        "This page and its [machine-readable JSON](creative-capabilities.json) are",
        f"generated from {source_link(CATALOG_PATH.as_posix(), 'the shared route catalog')}.",
        f"{source_link('crates/renderflow-core/src/creative_doctor.rs', 'Creative doctor')}",
        "embeds the same declarations. Edit the catalog and regenerate these files;",
        "do not maintain a separate capability table by hand.",
        "",
        f"The catalog declares **{len(routes)} routes**: {count_text}.",
        catalog["scope"],
        "",
        "`authored_unqualified` means code exists for the stated scope; runtime",
        "qualification is deferred. An installed provider, successful doctor",
        "discovery, or a recognized format identifier does not select a provider,",
        "enable a conversion, or establish fidelity. Some rows are standalone",
        "operations rather than graph edges. The guides describe the applicable",
        "surface, full configuration contract, bounds, and evidence to retain.",
        "",
        "## Declared routes",
        "",
        "| Route | Implementation | Declared providers | Usage |",
        "| --- | --- | --- | --- |",
    ]
    for route in routes:
        documentation = catalog["documentation"][route["id"]]
        lines.append("| " + " | ".join([
            f"`{route['id']}`",
            f"`{route['implementation']}`",
            code_list(route["providers"], "No external provider"),
            guide_link(documentation["guide"]),
        ]) + " |")
    lines.extend([
        "",
        "Provider IDs refer to the selected declarations in the",
        source_link("crates/renderflow-core/data/tool-registry.yaml", "tool registry") + ".",
        "This generated document has no live availability results. Doctor combines",
        "these shared route requirements with current registry discovery observations;",
        "runtime configuration separately selects provider bytes and execution policy.",
        "An in-process provider's availability does not qualify its outputs.",
        "See [doctor](creative-toolchain.md) and",
        "[setup and qualification](creative-toolchain-setup.md).",
        "",
        "## Scope and configuration requirements",
        "",
        "The following identifiers are the doctor's declared configuration and",
        "input-policy requirements. Some identify prerequisites rather than literal",
        "configuration keys. They are not an exhaustive configuration schema; use",
        "the linked guide for an executable request.",
    ])
    for route in routes:
        documentation = catalog["documentation"][route["id"]]
        lines.extend([
            "",
            f"### `{route['id']}`",
            "",
            route["detail"],
            "",
            "Declared requirements:",
            "",
            *[f"- `{requirement}`" for requirement in route["required_configuration"]],
            "",
            "Sources: " + "; ".join(source_link(path) for path in documentation["sources"]) + ".",
        ])
    lines.extend([
        "",
        "## Explicitly unsupported claims and directions",
        "",
        "These restrictions do not introduce conversion edges or replace the route",
        "policy. They distinguish rejected paths from authored, unqualified paths.",
    ])
    for unsupported in catalog["unsupported_directions"]:
        lines.extend([
            "",
            f"### `{unsupported['id']}`",
            "",
            unsupported["detail"],
            "",
            "Sources: " + "; ".join(source_link(path) for path in unsupported["sources"]) + ".",
        ])
    lines.extend([
        "",
        "## Regenerate",
        "",
        "From the repository root:",
        "",
        "```bash",
        'python3 "scripts/generate_creative_capabilities.py"',
        "```",
        "",
        "Generation reads the catalog and writes only this Markdown file and its",
        "JSON counterpart. It does not execute Renderflow, probe tools, run native",
        "conversions, validate schemas, install dependencies, or establish that",
        "tests pass. The JSON includes the catalog's SHA-256 so downstream",
        "consumers can identify the declaration snapshot.",
        "",
    ])
    (ROOT / DOC_PATH).write_text("\n".join(lines), encoding="utf-8")


if __name__ == "__main__":
    main()
