#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/run-artifact-gallery.sh --output-dir DIR [--img2pdf PATH]

Run the explicit local, real-provider artifact gallery and retain its output.
Requires Pandoc 2.0.0 or newer and img2pdf 0.6.3. The output directory must
be absent or empty. This script never installs tools or removes existing files.
EOF
}

unavailable() {
  printf 'artifact gallery unavailable: %s\n' "$1" >&2
  exit 2
}

output_dir=""
img2pdf_executable=""
while (($#)); do
  case "$1" in
    --output-dir)
      if (($# < 2)) || [[ -z "$2" ]]; then
        usage >&2
        exit 2
      fi
      output_dir="$2"
      shift 2
      ;;
    --img2pdf)
      if (($# < 2)) || [[ -z "$2" ]]; then
        usage >&2
        exit 2
      fi
      img2pdf_executable="$2"
      shift 2
      ;;
    --help)
      usage
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

if [[ -z "$output_dir" ]]; then
  usage >&2
  exit 2
fi

if ! command -v cargo >/dev/null 2>&1; then
  unavailable "Cargo is missing (Rust 1.94 or newer is required for this repository)"
fi
if ! command -v pandoc >/dev/null 2>&1; then
  unavailable "Pandoc is missing (version 2.0.0 or newer is required)"
fi
pandoc_version="$(pandoc --version 2>/dev/null)" || unavailable "Pandoc version probe failed"
pandoc_version="${pandoc_version%%$'\n'*}"
if [[ ! "$pandoc_version" =~ ^pandoc[[:space:]]+([0-9]+)\.([0-9]+)(\.[0-9]+)?([[:space:]]|$) ]]; then
  unavailable "cannot parse Pandoc version: $pandoc_version"
fi
if ((10#${BASH_REMATCH[1]} < 2)); then
  unavailable "Pandoc 2.0.0 or newer is required; observed: $pandoc_version"
fi

if [[ -z "$img2pdf_executable" ]]; then
  img2pdf_executable="$(command -v img2pdf || true)"
fi
if [[ -z "$img2pdf_executable" ]]; then
  unavailable "img2pdf is missing (version 0.6.3 is required); pass --img2pdf PATH"
fi
if [[ ! -x "$img2pdf_executable" ]]; then
  unavailable "img2pdf executable is missing or not executable: $img2pdf_executable"
fi
img2pdf_version="$("$img2pdf_executable" --version 2>/dev/null)" || unavailable "img2pdf version probe failed"
if [[ "$img2pdf_version" != "img2pdf 0.6.3" ]]; then
  unavailable "img2pdf 0.6.3 is required; observed: $img2pdf_version"
fi
img2pdf_executable="$(cd "$(dirname "$img2pdf_executable")" && pwd -P)/$(basename "$img2pdf_executable")"

if [[ -e "$output_dir" && ! -d "$output_dir" ]]; then
  printf 'artifact gallery: output path is not a directory: %s\n' "$output_dir" >&2
  exit 2
fi
if [[ -d "$output_dir" ]]; then
  for entry in "$output_dir"/* "$output_dir"/.[!.]* "$output_dir"/..?*; do
    if [[ -e "$entry" || -L "$entry" ]]; then
      printf 'artifact gallery: output directory must be empty: %s\n' "$output_dir" >&2
      exit 2
    fi
  done
fi
mkdir -p "$output_dir"
output_dir="$(cd "$output_dir" && pwd -P)"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd -P)"
cd "$script_dir/.."
printf 'artifact gallery: generating retained artifacts in %s\n' "$output_dir"
RENDERFLOW_GALLERY_OUTPUT_DIR="$output_dir" \
RENDERFLOW_TEST_IMG2PDF="$img2pdf_executable" \
cargo test --package renderflow-cli --test artifact_gallery_cli --locked -- --ignored --nocapture
printf 'artifact gallery: inspect %s\n' "$output_dir"
