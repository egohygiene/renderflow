#!/usr/bin/env python3
"""Additive v2 multi-target inventory over already prepared native assets.

No builds, CLI invocation, probes, downloads, signing or publication. The native
descriptor must have been captured from the exact separately built binary. This
tool binds the supplied bytes; existing native/supply-chain qualification gates
still require independent evidence. The older v1 release path stays unchanged.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import re
import stat
import tomllib

SCHEMA = "renderflow.release-manifest/v2"
CONTRACT = "renderflow.release-contract/v2"
OBJECTIVES = ["aarch64-apple-darwin", "x86_64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-pc-windows-msvc"]
MAX_FILE = 536870912
MAX_JSON = 16777216


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read(path, maximum=MAX_FILE):
    path = Path(path).absolute()
    require(not any(p.is_symlink() for p in [path, *path.parents]), "symlinked asset path")
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and 0 < before.st_size <= maximum, "regular bounded asset required")
    with path.open("rb") as handle:
        data = handle.read(maximum + 1)
    after = path.stat()
    require((before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) ==
            (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns) and len(data) == before.st_size,
            "asset changed during observation")
    return data


def sha(data):
    return hashlib.sha256(data).hexdigest()


def asset(root, name):
    require(isinstance(name, str) and re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_.-]*", name), "unsafe asset name")
    data = read(root / name)
    return {"name": name, "sha256": sha(data), "byte_count": len(data)}


def descriptor(root, record, version):
    data = read(root / record["name"], MAX_JSON)
    require(sha(data) == record["sha256"] and len(data) == record["byte_count"], "descriptor bytes changed")
    value = json.loads(data)
    require(value["schema_version"] == CONTRACT and value["tool"] == "renderflow" and value["version"] == version
            and value["library_version"] == version, "descriptor identity mismatch")
    require(value["qualification"] == "not_established_by_contract_description", "source descriptor cannot qualify a release")
    require(value["routes"] and len(value["routes"]) <= 100000 and value["profiles"], "missing or oversized interface inventory")
    return value


def prepare(root, source, request):
    require(request["schema_version"] == "renderflow.release-assets-request/v2", "unknown request")
    version = request["version"]
    require(re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?", version), "exact version required")
    require(re.fullmatch(r"[0-9a-f]{40}", request["source_revision"]), "exact source revision required")
    require(tomllib.loads(read(source / "Cargo.toml", MAX_JSON).decode())["workspace"]["package"]["version"] == version,
            "source Cargo version differs")
    rows = []
    seen = set()
    for entry in request["targets"]:
        target = entry["target"]
        require(target in OBJECTIVES and target not in seen, "duplicate or unknown target")
        seen.add(target)
        binary = asset(root, entry["binary"])
        contract = asset(root, entry["descriptor"])
        value = descriptor(root, contract, version)
        build = asset(root, entry["native_build_evidence"])
        evidence = json.loads(read(root / build["name"], MAX_JSON))
        require(evidence["target"] == target and evidence["source_revision"] == request["source_revision"]
                and evidence["binary_sha256"] == binary["sha256"] and evidence["release_contract_sha256"] == contract["sha256"],
                "native build evidence does not bind exact target/source/binary/descriptor")
        for schema in value["schemas"]:
            path = source / "schemas" / (schema["name"] + ".schema.json")
            require(sha(read(path, MAX_JSON)) == schema["sha256"], "compiled contract schema differs from represented source")
        rows.append({"target": target, "state": "prepared_bytes_only", "binary": binary, "descriptor": contract,
                     "native_build_evidence": build, "contracts": value})
    require(rows, "no target assets selected")
    return {"schema_version": SCHEMA, "repository": "egohygiene/renderflow", "version": version,
            "source_revision": request["source_revision"], "targets": rows,
            "retained_target_objectives": OBJECTIVES,
            "unavailable_targets": [t for t in OBJECTIVES if t not in seen],
            "qualification": "not_established_by_asset_inventory", "publication": "not_established",
            "owner_issue": "https://github.com/egohygiene/renderflow/issues/419"}


def verify(root, manifest):
    require(manifest["schema_version"] == SCHEMA and manifest["repository"] == "egohygiene/renderflow", "unknown release manifest")
    require(manifest["retained_target_objectives"] == OBJECTIVES and manifest["qualification"] == "not_established_by_asset_inventory"
            and manifest["publication"] == "not_established", "inventory cannot assert publication or qualification")
    seen = set()
    for row in manifest["targets"]:
        require(row["target"] in OBJECTIVES and row["target"] not in seen and row["state"] == "prepared_bytes_only", "invalid target inventory")
        seen.add(row["target"])
        for kind in ["binary", "descriptor", "native_build_evidence"]:
            require(asset(root, row[kind]["name"]) == row[kind], "release asset digest/size differs")
        value = descriptor(root, row["descriptor"], manifest["version"])
        require(value == row["contracts"], "release descriptor projection differs")
        evidence = json.loads(read(root / row["native_build_evidence"]["name"], MAX_JSON))
        require(evidence["target"] == row["target"] and evidence["source_revision"] == manifest["source_revision"] and
                evidence["binary_sha256"] == row["binary"]["sha256"] and evidence["release_contract_sha256"] == row["descriptor"]["sha256"], "native evidence binding differs")
    require(seen and manifest["unavailable_targets"] == [t for t in OBJECTIVES if t not in seen], "omitted platform coverage")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["prepare", "verify"])
    parser.add_argument("--asset-directory", required=True)
    parser.add_argument("--input", required=True)
    parser.add_argument("--source-directory")
    parser.add_argument("--output")
    args = parser.parse_args()
    root = Path(args.asset_directory).absolute()
    value = json.loads(read(Path(args.input), MAX_JSON))
    if args.action == "prepare":
        require(args.source_directory and args.output, "prepare requires source directory and fresh output")
        result = prepare(root, Path(args.source_directory).absolute(), value)
        with Path(args.output).open("x", encoding="utf-8") as handle:
            handle.write(json.dumps(result, indent=2, sort_keys=True) + "\n")
    else:
        verify(root, value)
        print(json.dumps({"schema_version": SCHEMA, "verification": "exact_supplied_asset_bytes_only", "qualified": False}))


if __name__ == "__main__":
    main()
