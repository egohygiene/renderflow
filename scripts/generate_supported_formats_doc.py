#!/usr/bin/env python3

from __future__ import annotations

import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
DOC_PATH = ROOT / "docs" / "user-guide" / "supported-formats.md"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def extract_block(text: str, anchor: str) -> str:
    start = text.index(anchor)
    brace = text.index("{", start)
    depth = 0
    for index in range(brace, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text[brace + 1 : index]
    raise ValueError(f"unclosed block for {anchor!r}")


def extract_display_values(text: str, type_name: str) -> list[tuple[str, str]]:
    block = extract_block(text, f"impl fmt::Display for {type_name}")
    return re.findall(rf"{re.escape(type_name)}::(\w+)\s*=>\s*\"([^\"]+)\"", block)


def extract_primary_values(block: str, type_name: str) -> list[tuple[str, str]]:
    values: list[tuple[str, str]] = []
    for names, variant in re.findall(
        rf'((?:"[^"]+"\s*\|\s*)*"[^"]+")\s*=>\s*Ok\({re.escape(type_name)}::(\w+)\)',
        block,
    ):
        aliases = re.findall(r'"([^"]+)"', names)
        values.append((variant, aliases[0]))
    return values


def extract_string_arms(block: str, type_name: str) -> dict[str, str]:
    mapping: dict[str, str] = {}
    for variants, value in re.findall(
        rf"((?:{re.escape(type_name)}::\w+\s*\|\s*)*{re.escape(type_name)}::\w+)\s*=>\s*\"([^\"]+)\"",
        block,
    ):
        for variant in re.findall(rf"{re.escape(type_name)}::(\w+)", variants):
            mapping[variant] = value
    return mapping


def extract_option_arms(block: str, type_name: str) -> dict[str, bool]:
    support: dict[str, bool] = {}
    for variants, value in re.findall(
        rf"((?:{re.escape(type_name)}::\w+\s*\|\s*)*{re.escape(type_name)}::\w+)\s*=>\s*(Some\([^)]+\)|None)",
        block,
    ):
        encodable = value.startswith("Some(")
        for variant in re.findall(rf"{re.escape(type_name)}::(\w+)", variants):
            support[variant] = encodable
    return support


def extract_input_extensions(text: str) -> dict[str, list[str]]:
    block = extract_block(text, "pub fn from_extension(path: &str) -> Option<Self>")
    extensions: dict[str, list[str]] = {}
    for names, variant in re.findall(
        r'((?:"[^"]+"\s*\|\s*)*"[^"]+")\s*=>\s*Some\(InputFormat::(\w+)\)',
        block,
    ):
        extensions[variant] = re.findall(r'"([^"]+)"', names)
    return extensions


def render_table(headers: list[str], rows: list[list[str]]) -> list[str]:
    lines = [
        "| " + " | ".join(headers) + " |",
        "| " + " | ".join("---" for _ in headers) + " |",
    ]
    for row in rows:
        lines.append("| " + " | ".join(row) + " |")
    return lines


def main() -> None:
    input_text = read(ROOT / "src" / "input_format.rs")
    graph_text = read(ROOT / "src" / "graph" / "format.rs")
    audio_text = read(ROOT / "src" / "audio" / "format.rs")
    image_text = read(ROOT / "src" / "image" / "format.rs")

    input_extensions = extract_input_extensions(input_text)

    graph_names = extract_display_values(graph_text, "Format")

    input_names = re.findall(
        r'InputFormat::(\w+)\s*=>\s*"([^"]+)"',
        extract_block(input_text, "pub fn as_pandoc_format(&self) -> &str"),
    )

    audio_names = extract_primary_values(
        extract_block(audio_text, "fn from_str(s: &str) -> Result<Self>"),
        "AudioFormat",
    )
    audio_extensions = extract_string_arms(
        extract_block(audio_text, "pub fn file_extension(self) -> &'static str"),
        "AudioFormat",
    )
    audio_support = extract_option_arms(
        extract_block(audio_text, "pub fn ffmpeg_codec(self) -> Option<&'static str>"),
        "AudioFormat",
    )

    image_names = extract_primary_values(
        extract_block(image_text, "fn from_str(s: &str) -> Result<Self>"),
        "ImageFormat",
    )
    image_extensions = extract_string_arms(
        extract_block(image_text, "pub fn file_extension(self) -> &'static str"),
        "ImageFormat",
    )
    image_support = extract_option_arms(
        extract_block(image_text, "pub fn ffmpeg_codec(self) -> Option<&'static str>"),
        "ImageFormat",
    )

    audio_values = {value for _, value in audio_names}
    graph_rows = [
        [f"`{value}`"]
        for _, value in graph_names
        if value not in audio_values
    ]

    input_rows = [
        [
            f"`{value}`",
            ", ".join(f"`.{extension}`" for extension in input_extensions.get(variant, [])),
        ]
        for variant, value in input_names
    ]
    audio_rows = [
        [
            f"`{value}`",
            f"`.{audio_extensions[variant]}`",
            "Yes" if audio_support.get(variant, False) else "No",
        ]
        for variant, value in audio_names
    ]
    image_rows = [
        [
            f"`{value}`",
            f"`.{image_extensions[variant]}`",
            "Yes" if image_support.get(variant, False) else "No",
        ]
        for variant, value in image_names
    ]

    content = [
        "# Supported Formats",
        "",
        "!!! info",
        "    This page is generated from `src/input_format.rs`, `src/graph/format.rs`,",
        "    `src/audio/format.rs`, and `src/image/format.rs` by",
        "    `scripts/generate_supported_formats_doc.py`. Do not edit it by hand.",
        "",
        "Renderflow recognizes format identifiers in four places:",
        "",
        "- `input_format` in `renderflow.yaml`,",
        "- `outputs[].type` for standard, audio, and image rendering,",
        "- transform-graph definitions, and",
        "- `renderflow graph ...` / `renderflow inspect ...` target selection.",
        "",
        "For audio and image formats, **Encodable = No** means Renderflow recognizes the",
        "identifier but cannot emit that format through the built-in FFmpeg-backed",
        "strategy.",
        "",
        "## Input formats",
        "",
        *render_table(["Config value", "Recognized extensions"], input_rows),
        "",
        "## Transform graph identifiers",
        "",
        "These are the canonical node names used in transform YAML files and graph output.",
        "",
        *render_table(["Identifier"], graph_rows),
        "",
        "## Audio format identifiers",
        "",
        *render_table(["Identifier", "Default extension", "Encodable"], audio_rows),
        "",
        "## Image format identifiers",
        "",
        *render_table(["Identifier", "Default extension", "Encodable"], image_rows),
        "",
    ]

    DOC_PATH.write_text("\n".join(content), encoding="utf-8")


if __name__ == "__main__":
    main()
