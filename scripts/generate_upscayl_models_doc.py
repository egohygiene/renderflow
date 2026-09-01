#!/usr/bin/env python3
"""Generate the Upscayl model reference from the canonical model catalog."""

from __future__ import annotations

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
CATALOG_PATH = ROOT / "crates" / "renderflow-core" / "data" / "upscayl-models.yaml"
OUTPUT_PATH = ROOT / "docs" / "user-guide" / "upscayl-models.md"


def escape(value: object) -> str:
    """Escape Markdown table separators in generated values."""
    return str(value).replace("|", "\\|").replace("\n", " ")


def main() -> None:
    """Generate the checked-in model reference page."""
    catalog = yaml.safe_load(CATALOG_PATH.read_text(encoding="utf-8"))
    models = sorted(catalog.get("models", []), key=lambda model: model["variant_id"])

    lines = [
        "<!-- GENERATED FILE: run scripts/generate_upscayl_models_doc.py -->",
        "# Upscayl model reference",
        "",
        "This page is generated from the canonical model catalog at",
        "`crates/renderflow-core/data/upscayl-models.yaml`. Do not edit it by hand.",
        "",
        f"Catalog schema: `{catalog['schema']}`",
        "",
        "| Variant ID | CLI model | Native scale | Commercial use | Notes |",
        "| --- | --- | ---: | --- | --- |",
    ]
    for model in models:
        lines.append(
            "| "
            + " | ".join(
                [
                    f"`{escape(model['variant_id'])}`",
                    f"`{escape(model['model_name'])}`",
                    f"x{escape(model['native_scale'])}",
                    escape(model["commercial_use"]),
                    escape(model["license_notes"]),
                ]
            )
            + " |"
        )

    lines.extend(
        [
            "",
            "!!! warning",
            "    `unknown` means Renderflow does not assert commercial-use permission.",
            "    Verify the model's own terms before commercial publication. Models marked",
            "    `prohibited` are currently labeled Non-Commercial by Upscayl and should not",
            "    be selected for commercial output without separately establishing permission.",
            "",
        ]
    )
    OUTPUT_PATH.write_text("\n".join(lines), encoding="utf-8")
    print(f"Generated {OUTPUT_PATH.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
