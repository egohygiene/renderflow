#!/usr/bin/env python3
"""Read-only checks for the workspace CLI packaging declarations.

No Cargo process, installer, package builder, release or network is invoked.
This is a source-declaration check, not native packaging qualification.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
CLI_MANIFEST = "crates/renderflow-cli/Cargo.toml"
CLI_PACKAGE = "renderflow-cli"
CLI_BINARY = "renderflow"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def workspace(root):
    data = tomllib.loads((root / "Cargo.toml").read_text())
    version = data["workspace"]["package"]["version"]
    minimum = data["workspace"]["package"]["rust-version"]
    packages = {}
    for member in data["workspace"]["members"]:
        manifest = f"{member}/Cargo.toml"
        package = tomllib.loads((root / manifest).read_text())["package"]
        require(package["name"] not in packages, "duplicate workspace package")
        require(package["version"] == {"workspace": True}, "workspace package must inherit version authority")
        packages[package["name"]] = manifest
    require(packages.get(CLI_PACKAGE) == CLI_MANIFEST, "CLI package/manifest differs")
    cli = tomllib.loads((root / CLI_MANIFEST).read_text())
    require([(entry["name"], entry["path"]) for entry in cli["bin"]] == [(CLI_BINARY, "src/main.rs")],
            "CLI binary target differs")
    return {"version": version, "rust_toolchain": minimum if minimum.count(".") == 2 else minimum + ".0",
            "packages": packages, "cli_package": CLI_PACKAGE,
            "cli_manifest": CLI_MANIFEST, "cli_binary": CLI_BINARY}


def check(root):
    data = workspace(root)
    locked = tomllib.loads((root / "Cargo.lock").read_text())["package"]
    for name in data["packages"]:
        entries = [p for p in locked if p["name"] == name and "source" not in p]
        require(len(entries) == 1 and entries[0]["version"] == data["version"],
                "workspace package lock/version differs: " + name)
    snap = (root / "snap/snapcraft.yaml").read_text()
    require(re.search(r"(?m)^adopt-info: renderflow$", snap), "Snap must adopt the CLI's Cargo version")
    require(not re.search(r"(?m)^version:", snap), "Snap cannot override Cargo version authority")
    require(re.search(r'(?m)^    rust-channel: "' + re.escape(data["rust_toolchain"]) + r'"$', snap),
            "Snap Rust toolchain differs")
    require('rust-path: ["crates/renderflow-cli"]' in snap, "Snap must select the CLI manifest")
    require("--package renderflow-cli --bin renderflow" in snap and "cargo build --locked" in snap,
            "Snap must explicitly build the locked CLI target")
    stable = (root / "pkg/aur/renderflow/PKGBUILD").read_text()
    match = re.search(r"(?m)^_source_version=([^\s]+)$", stable)
    require(match is not None and match[1] == data["version"], "AUR source tag template differs from Cargo version")
    require('cd "$pkgname-$_source_version"' in stable, "AUR archive root must match the exact source tag")
    data.update({"schema_version": "renderflow.packaging-source-check/v1",
                 "qualification": "source_metadata_only", "native_build_observed": False,
                 "installer_observed": False, "snap_observed": False})
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["version", "check"])
    parser.add_argument("--source-dir", type=Path, default=ROOT)
    args = parser.parse_args()
    if args.operation == "version":
        print(workspace(args.source_dir)["version"])
    else:
        print(json.dumps(check(args.source_dir), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
