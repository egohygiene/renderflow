#!/usr/bin/env python3
"""Create and independently verify the bounded Renderflow release receipt.

Only prepare/manifest reads the source checkout. verify consumes downloaded assets.
Checksum verification is separate from optional GitHub/Sigstore trust verification.
"""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import tomllib
import zlib


REPOSITORY = "egohygiene/renderflow"
TARGET = "x86_64-unknown-linux-gnu"
SCHEMA = "renderflow.release-manifest/v1"
MANIFEST = "renderflow-release-manifest-v1.json"
SBOM = "renderflow-sbom.spdx.json"
NOTICES = "THIRD_PARTY_NOTICES.txt"
ATTESTATION = "renderflow-attestation.json"
SBOM_ATTESTATION = "renderflow-sbom-attestation.json"
SCHEMAS = {
    "execution_spec": ("renderflow/v2", "renderflow-v2.schema.json"),
    "run_evidence": ("renderflow.run/v1", "renderflow-run-v1.schema.json"),
    "provider": ("renderflow.provider/v1", "renderflow-provider-v1.schema.json"),
    "plugin": ("renderflow.plugin/v2alpha1", "renderflow-plugin-v2alpha1.schema.json"),
    "ebook_evidence": ("renderflow.ebook-evidence/v1", "renderflow-ebook-evidence-v1.schema.json"),
    "ebook_capabilities": ("renderflow.ebook-capabilities/v1", "renderflow-ebook-capabilities-v1.schema.json"),
    "release_manifest": (SCHEMA, "renderflow-release-manifest-v1.schema.json"),
}
HEX40 = re.compile(r"[0-9a-f]{40}\Z")
HEX64 = re.compile(r"[0-9a-f]{64}\Z")
VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+-[0-9A-Za-z.-]+\Z")
NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.-]*\Z")
MAX_ASSET = 512 * 1024 * 1024


def fail(message: str) -> None:
    raise ValueError(message)


def require(condition: bool, message: str) -> None:
    if not condition:
        fail(message)


def digest(path: Path) -> str:
    hasher = hashlib.sha256()
    with path.open("rb") as reader:
        for block in iter(lambda: reader.read(1024 * 1024), b""):
            hasher.update(block)
    return hasher.hexdigest()


def asset(root: Path, name: str) -> Path:
    require(bool(NAME.fullmatch(name)), f"unsafe asset name: {name}")
    path = root / name
    require(path.is_file() and not path.is_symlink(), f"missing or symlinked asset: {name}")
    require(0 < path.stat().st_size <= MAX_ASSET, f"empty or oversized asset: {name}")
    return path


def put_text(path: Path, content: str) -> None:
    path.write_text(content, encoding="utf-8")


def put_json(path: Path, value: object) -> None:
    put_text(path, json.dumps(value, sort_keys=True, indent=2, ensure_ascii=False) + "\n")


def checksum(root: Path, name: str) -> None:
    put_text(root / f"{name}.sha256", f"{digest(asset(root, name))}  {name}\n")


def check_checksum(root: Path, name: str) -> None:
    line = asset(root, f"{name}.sha256").read_text(encoding="ascii")
    require(line == f"{digest(asset(root, name))}  {name}\n", f"checksum mismatch or malformed record: {name}")


def identity(args: argparse.Namespace) -> None:
    require(bool(VERSION.fullmatch(args.version)), "candidate version must be SemVer prerelease")
    require(args.tag == f"v{args.version}", "version/tag mismatch")
    require(bool(HEX40.fullmatch(args.commit)), "commit must be an exact lowercase 40-hex SHA")
    require(args.target == TARGET, "only the verified Linux x86_64 GNU target is supported")
    require(args.repository == REPOSITORY, "repository mismatch")


def source_version(source: Path, version: str) -> None:
    cargo = tomllib.loads((source / "Cargo.toml").read_text(encoding="utf-8"))
    require(cargo["workspace"]["package"]["version"] == version, "workspace Cargo version differs from release")


def metadata_for(source: Path, metadata_file: str | None) -> dict:
    if metadata_file:
        return json.loads(Path(metadata_file).read_text(encoding="utf-8"))
    command = ["cargo", "metadata", "--locked", "--offline", "--format-version", "1", "--manifest-path", str(source / "Cargo.toml")]
    proc = subprocess.run(command, capture_output=True, text=True, check=False, timeout=180)
    require(proc.returncode == 0, f"offline cargo metadata failed: {proc.stderr[:1000]}")
    return json.loads(proc.stdout)


def locked_packages(source: Path, metadata: dict) -> list[dict]:
    locked = tomllib.loads((source / "Cargo.lock").read_text(encoding="utf-8"))["package"]
    lock_index = {(p["name"], p["version"]): p for p in locked}
    packages = metadata.get("packages")
    require(isinstance(packages, list) and packages, "cargo metadata has no packages")
    result = []
    for item in packages:
        name, version = item["name"], item["version"]
        lock = lock_index.get((name, version))
        require(lock is not None, f"metadata package is absent from Cargo.lock: {name} {version}")
        if lock.get("checksum"):
            require(bool(HEX64.fullmatch(lock["checksum"])), f"bad lock checksum: {name}")
        result.append({
            "name": name,
            "version": version,
            "license": item.get("license") or "NOASSERTION",
            "repository": item.get("repository") or "",
            "source": item.get("source") or "workspace",
            "checksum": lock.get("checksum"),
            "manifest_path": item.get("manifest_path") or "",
        })
    return sorted(result, key=lambda p: (p["name"], p["version"], p["source"]))


def license_texts(package: dict) -> list[tuple[str, str]]:
    """Include bounded license texts when cached; otherwise do not invent them."""
    path = Path(package["manifest_path"])
    if not path.is_file():
        return []
    root = path.parent.resolve()
    names = sorted(p for p in root.iterdir() if re.match(r"(?i)^(license|licence|copying|notice)([._-].*)?$", p.name))
    result = []
    for candidate in names:
        if candidate.is_symlink() or not candidate.is_file() or candidate.stat().st_size > 128 * 1024:
            continue
        if candidate.resolve().parent != root:
            continue
        result.append((candidate.name, candidate.read_text(encoding="utf-8", errors="replace")))
    return result


def prepare(args: argparse.Namespace) -> None:
    identity(args)
    source = Path(args.source_dir).resolve()
    source_version(source, args.version)
    root = Path(args.artifact_dir).resolve()
    root.mkdir(parents=True, exist_ok=True)
    binary = f"renderflow-{args.target}"
    asset(root, binary)
    metadata = metadata_for(source, args.metadata_file)
    packages = locked_packages(source, metadata)
    spdx_packages = []
    relationships = []
    for index, package in enumerate(packages, 1):
        spdx_id = f"SPDXRef-Package-{index}"
        entry = {
            "SPDXID": spdx_id,
            "name": package["name"],
            "versionInfo": package["version"],
            "downloadLocation": "NOASSERTION",
            "filesAnalyzed": False,
            "licenseConcluded": "NOASSERTION",
            "licenseDeclared": package["license"],
            "copyrightText": "NOASSERTION",
            "externalRefs": [{
                "referenceCategory": "PACKAGE-MANAGER",
                "referenceType": "purl",
                "referenceLocator": f"pkg:cargo/{package['name']}@{package['version']}",
            }],
        }
        if package["checksum"]:
            entry["checksums"] = [{"algorithm": "SHA256", "checksumValue": package["checksum"]}]
        spdx_packages.append(entry)
        relationships.append({"spdxElementId": "SPDXRef-DOCUMENT", "relatedSpdxElement": spdx_id, "relationshipType": "DESCRIBES"})
    created = dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z")
    if os.environ.get("SOURCE_DATE_EPOCH"):
        created = dt.datetime.fromtimestamp(int(os.environ["SOURCE_DATE_EPOCH"]), dt.timezone.utc).isoformat().replace("+00:00", "Z")
    put_json(root / SBOM, {
        "spdxVersion": "SPDX-2.3", "dataLicense": "CC0-1.0", "SPDXID": "SPDXRef-DOCUMENT",
        "name": f"Renderflow workspace Cargo lock {args.version}",
        "documentNamespace": f"https://github.com/{REPOSITORY}/releases/tag/{args.tag}/spdx-{args.commit}",
        "creationInfo": {"created": created, "creators": ["Tool: scripts/release/release_assets.py"]},
        "comment": "Cargo workspace locked dependency inventory, including optional and development dependencies. This is not a binary composition claim.",
        "packages": spdx_packages, "relationships": relationships,
    })
    notices = [
        f"Renderflow {args.version} — Cargo workspace dependency/license notices",
        f"Source commit: {args.commit}",
        "This inventory includes optional and development dependencies from Cargo.lock.",
        "License expressions are declared by upstream packages, not independently adjudicated.",
        "Bundled cached license text is included below when available; NOASSERTION or",
        "missing text requires upstream review. This file does not grant new rights.",
        "",
    ]
    total_license_bytes = 0
    for package in packages:
        if package["source"] == "workspace":
            continue
        notices += [f"{package['name']} {package['version']}", f"Declared license: {package['license']}", f"Source: {package['repository'] or package['source']}"]
        for filename, content in license_texts(package):
            total_license_bytes += len(content.encode("utf-8"))
            require(total_license_bytes <= 8 * 1024 * 1024, "cached license notice total is too large")
            notices += [f"--- {filename} ---", content.rstrip(), f"--- end {filename} ---"]
        notices += [""]
    put_text(root / NOTICES, "\n".join(notices))
    for name in (binary, SBOM, NOTICES):
        checksum(root, name)
    print(f"Prepared {binary}, checksum, SPDX inventory, and dependency notices in {root}")


def release_url(tag: str, name: str) -> str:
    return f"https://github.com/{REPOSITORY}/releases/download/{tag}/{name}"


def attestation_bundle(root: Path, provided: str, fixed_name: str, expected_subject: str, expected_digest: str) -> None:
    original = Path(provided).resolve()
    require(original.is_file() and original.stat().st_size > 0, f"missing attestation bundle: {provided}")
    path = root / fixed_name
    require(not path.is_symlink(), f"symlinked destination bundle refused: {fixed_name}")
    if original != path:
        shutil.copyfile(original, path)
    bundle = json.loads(asset(root, fixed_name).read_text(encoding="utf-8"))
    require(isinstance(bundle, dict), f"invalid attestation bundle: {fixed_name}")
    envelope = bundle.get("dsseEnvelope") or bundle.get("dsse_envelope")
    require(isinstance(envelope, dict) and envelope.get("payload") and envelope.get("signatures"), f"missing signed DSSE envelope: {fixed_name}")
    require(bundle.get("verificationMaterial") or bundle.get("verification_material"), f"missing verification material: {fixed_name}")
    statement = json.loads(base64.b64decode(envelope["payload"], validate=True))
    subjects = statement.get("subject", [])
    require(any(
        isinstance(s, dict)
        and isinstance(s.get("name"), str)
        and (s["name"] == expected_subject or s["name"].endswith(f"/{expected_subject}"))
        and isinstance(s.get("digest"), dict)
        and s["digest"].get("sha256") == expected_digest
        for s in subjects
    ), f"attestation subject mismatch: {fixed_name}")
    if fixed_name == SBOM_ATTESTATION:
        require(statement.get("predicateType") == "https://spdx.dev/Document/v2.3", "SBOM attestation predicate type mismatch")
        require(statement.get("predicate") == json.loads(asset(root, SBOM).read_text(encoding="utf-8")), "attached SBOM does not match signed predicate")
    else:
        require(statement.get("predicateType") == "https://slsa.dev/provenance/v1", "build provenance predicate type mismatch")
    # The contents remain untrusted until gh attestation verify checks signatures.


def manifest(args: argparse.Namespace) -> None:
    identity(args)
    source = Path(args.source_dir).resolve()
    source_version(source, args.version)
    root = Path(args.artifact_dir).resolve()
    binary = f"renderflow-{args.target}"
    for name in (binary, SBOM, NOTICES):
        check_checksum(root, name)
    attestation_bundle(root, args.attestation_bundle, ATTESTATION, binary, digest(asset(root, binary)))
    attestation_bundle(root, args.sbom_attestation_bundle, SBOM_ATTESTATION, binary, digest(asset(root, binary)))
    checksum(root, ATTESTATION)
    checksum(root, SBOM_ATTESTATION)
    names = [binary, f"{binary}.sha256", SBOM, f"{SBOM}.sha256", NOTICES, f"{NOTICES}.sha256", ATTESTATION, f"{ATTESTATION}.sha256", SBOM_ATTESTATION, f"{SBOM_ATTESTATION}.sha256"]
    kinds = ["cli-binary", "checksum", "spdx-sbom", "checksum", "dependency-notices", "checksum", "provenance-bundle", "checksum", "sbom-attestation-bundle", "checksum"]
    items = []
    for name, kind in zip(names, kinds, strict=True):
        path = asset(root, name)
        items.append({"name": name, "url": release_url(args.tag, name), "kind": kind, "sha256": digest(path), "size": path.stat().st_size})
    schemas = []
    for name, (identifier, filename) in SCHEMAS.items():
        path = source / "schemas" / filename
        require(path.is_file(), f"missing source schema: {filename}")
        schemas.append({"name": name, "identifier": identifier, "sha256": digest(path), "source_path": f"schemas/{filename}"})
    receipt = {
        "schema": SCHEMA, "repository": REPOSITORY, "version": args.version,
        "tag": args.tag, "commit": args.commit, "channel": "integration-candidate",
        "binary": {"name": binary, "url": release_url(args.tag, binary), "target": args.target, "sha256": digest(root / binary), "size": (root / binary).stat().st_size},
        "contracts": {
            "cli_version": args.version,
            "core_crate_version": args.version,
            "plugin_sdk_crate_version": args.version,
            "plugin_contract": "renderflow.plugin/v2alpha1",
            "provider_contract": "renderflow.provider/v1",
            "artifact_manifest_contract": "renderflow.artifact-manifest/v1",
            "flow_artifact_contract": "flow.artifact/v1",
            "tool_registry_contract": "renderflow.tool-registry/v1",
            "capabilities": [
                {"id": "publication.generate.pdf.interior", "provider_id": "tool.img2pdf", "availability": "requires_external_tool", "input_kind": "collection", "ordered_collection": True, "source_mutation": False, "output_format": "pdf", "required_external_tools": ["img2pdf 0.6.3"]},
                {"id": "ebook.generate.epub.fixed-layout", "provider_id": "tool.renderflow-epub", "availability": "native", "input_kind": "collection", "ordered_collection": True, "source_mutation": False, "output_format": "epub", "required_external_tools": []},
            ],
            "schemas": schemas,
        },
        "compatibility": {
            "supported_platforms": [TARGET], "verified_host": "ubuntu-24.04-x86_64",
            "gnu_libc_minimum": "2.39", "other_platforms": "unsupported_unverified",
            "external_tools": [
                {"name": "img2pdf", "constraint": "==0.6.3", "required_for": "publication.generate.pdf.interior"},
                {"name": "epubcheck", "constraint": "v5, optional; exact version recorded per inspection", "required_for": "optional independent EPUBCheck evidence"},
                {"name": "pandoc", "constraint": ">=2.0.0, optional", "required_for": "provider-backed document conversion"},
            ],
        },
        "assets": items,
        "security": {
            "asset_signing": "unsigned", "provenance": "github_sigstore_bundle_attached_verify_separately",
            "attestation_bundles": [{"subject": binary, "bundle": ATTESTATION}, {"subject": binary, "bundle": SBOM_ATTESTATION}],
        },
    }
    put_json(root / MANIFEST, receipt)
    checksum(root, MANIFEST)
    print(f"Created {MANIFEST} and checksum; binary SHA-256 {receipt['binary']['sha256']}")


def run_binary(binary: Path, cwd: Path, args: list[str], expect_success: bool = True) -> subprocess.CompletedProcess[str]:
    env = os.environ.copy()
    env["HOME"] = str(cwd / "home")
    command = [str(binary), *args]
    result = subprocess.run(command, cwd=cwd, env=env, text=True, capture_output=True, timeout=60, check=False)
    require((result.returncode == 0) == expect_success, f"smoke {args}: unexpected exit {result.returncode}: {result.stderr[:1000]}")
    return result


def png(color: tuple[int, int, int]) -> bytes:
    """Small, reproducible RGB synthetic art; no source tree fixture dependency."""
    def chunk(kind: bytes, data: bytes) -> bytes:
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xffffffff)
    pixel_rows = b"".join(b"\0" + bytes(color) * 100 for _ in range(100))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", 100, 100, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(pixel_rows, 9)) + chunk(b"IEND", b"")


def smoke_pdf(binary: Path, root: Path) -> None:
    """Exercise the advertised exact provider route on copied synthetic files."""
    provider = shutil.which("img2pdf")
    require(provider is not None, "release PDF smoke requires real img2pdf 0.6.3")
    version = subprocess.run([provider, "--version"], capture_output=True, text=True, timeout=10, check=False)
    require(version.returncode == 0 and version.stdout.strip() == "img2pdf 0.6.3", "PDF provider must be exact img2pdf 0.6.3")
    provider = str(Path(provider).resolve())
    pdf_work = root / "pdf-fixture"
    pdf_work.mkdir()
    (pdf_work / "home").mkdir()
    images = [png((30, 70, 130)), png((180, 90, 40))]
    for index, data in enumerate(images):
        (pdf_work / f"page-{index:03}.png").write_bytes(data)
    sources = "".join(
        f"  - id: source.page{index:03}\n    path: page-{index:03}.png\n"
        f"    format: png\n    media_type: image/png\n    sha256: \"{hashlib.sha256(data).hexdigest()}\"\n"
        "    geometry: { width: 90, height: 90, unit: mm, bleed: 5 }\n"
        for index, data in enumerate(images)
    )
    config = (
        "schema: renderflow/v2\nsources:\n" + sources +
        "  - id: source.pages\n    kind: collection\n    members: [source.page000, source.page001]\n"
        "targets:\n  exact:\n    - id: target.interior\n      role: interior\n"
        "      format: pdf\n      requirement: required\n"
        "execution:\n  print_pdf_interior:\n"
        f"    executable: \"{provider}\"\n    provider_version: \"0.6.3\"\n"
        "    box_policy: media_bleed_trim_inset\n    rotation: none\n    scaling: fit\n"
        "    color_policy: preserve_rgb_gray\n    max_pages: 2\n"
        "    max_input_bytes: 1000000\n    max_output_bytes: 5000000\n"
        "    timeout_seconds: 15\noutput:\n  bundle_root: dist\n"
        "  naming_template: \"{source.id}/{target.role}.{ext}\"\n"
    )
    (pdf_work / "renderflow.yaml").write_text(config, encoding="utf-8")
    run_binary(binary, pdf_work, ["spec", "validate", "--config", "renderflow.yaml"])
    run_binary(binary, pdf_work, ["build", "--config", "renderflow.yaml", "--dry-run"])
    run_binary(binary, pdf_work, ["build", "--config", "renderflow.yaml"])
    output = pdf_work / "dist" / "source.pages" / "interior.pdf"
    require(output.is_file() and output.read_bytes().startswith(b"%PDF-"), "PDF fixture output missing or malformed")
    evidence = json.loads((pdf_work / "dist" / "renderflow-run.json").read_text(encoding="utf-8"))
    require(evidence.get("state") == "complete", "PDF run manifest not complete")
    artifacts = evidence["artifact_manifest"]["artifacts"]
    sources = [item for item in artifacts if item.get("lifecycle") == "source"]
    interior = next((item for item in artifacts if item.get("role") == "interior"), None)
    require(len(sources) == 2 and interior is not None, "PDF source/output lineage missing")
    require([source["metadata"]["renderflow.collection.index"] for source in sources] == [0, 1], "PDF source order mismatch")
    require(interior["sources"] == [source["artifact_id"] for source in sources], "PDF artifact lineage mismatch")
    require(interior["validation"] == "valid", "PDF artifact was not validated")
    inspected = interior["metadata"]["renderflow.print_pdf.inspection"]
    require(inspected["page_count"] == 2 and len(inspected["pages"]) == 2, "independent PDF inspection page count mismatch")
    require(inspected["sha256"] == digest(output), "independent PDF inspection digest mismatch")
    require(inspected["pages"][0]["image_stream_sha256"] != inspected["pages"][1]["image_stream_sha256"], "PDF page streams not distinct")
    require(all((pdf_work / f"page-{i:03}.png").read_bytes() == original for i, original in enumerate(images)), "PDF source bytes mutated")
    try:
        import pikepdf
    except ImportError as error:
        fail(f"independent PDF parser unavailable with img2pdf installation: {error}")
    with pikepdf.open(output) as document:
        require(len(document.pages) == 2, "pikepdf found a different page count")
        for page in document.pages:
            require(len(page.MediaBox) == 4 and len(page.TrimBox) == 4 and len(page.BleedBox) == 4, "PDF page boxes missing")
    # A changed frozen source must be refused before publishing another PDF.
    (pdf_work / "page-001.png").write_bytes(b"source changed after plan")
    (pdf_work / "dist").rename(pdf_work / "completed-dist")
    run_binary(binary, pdf_work, ["build", "--config", "renderflow.yaml"], expect_success=False)
    require(not (pdf_work / "dist" / "source.pages" / "interior.pdf").exists(), "stale PDF source published an artifact")


def smoke(binary: Path, version: str) -> None:
    with tempfile.TemporaryDirectory(prefix="renderflow-release-smoke-") as temporary:
        root = Path(temporary)
        install = root / "install"
        install.mkdir()
        # Move executable into an otherwise empty installation: no worktree paths.
        installed = install / "renderflow"
        shutil.copy2(binary, installed)
        installed.chmod(0o755)
        work = root / "fixture"
        work.mkdir()
        (work / "home").mkdir()
        require(run_binary(installed, work, ["--version"]).stdout.strip() == f"renderflow {version}", "binary version mismatch")
        require("build" in run_binary(installed, work, ["--help"]).stdout, "top-level help missing build")
        require("Renderflow Doctor" in run_binary(installed, work, ["doctor"]).stdout, "doctor evidence missing")
        hashes = []
        for index, color in enumerate(((30, 70, 130), (180, 90, 40)), 1):
            filename = f"page-{index:03}.png"
            data = png(color)
            (work / filename).write_bytes(data)
            hashes.append(hashlib.sha256(data).hexdigest())
        sources = "".join(f"  - id: source.page{i:03}\n    path: page-{i:03}.png\n    format: png\n    media_type: image/png\n    sha256: \"{hashes[i-1]}\"\n    geometry: {{ width: 90, height: 90, unit: mm }}\n" for i in (1, 2))
        artwork = "".join(f"    - role: page\n      path: page-{i:03}.png\n      alt_text: Synthetic colored geometric page {i}.\n" for i in (1, 2))
        spec = (
            "schema: renderflow/v2\nsources:\n" + sources +
            "  - id: source.pages\n    kind: collection\n    members: [source.page001, source.page002]\n"
            "publication:\n  publication: Synthetic release smoke\n  issue_id: release-smoke\n"
            "  title: Synthetic release pages\n  contributors:\n    - name: Renderflow contributors\n      role: author\n"
            "  publication_date: \"2026-09-28\"\n  language: en-US\n"
            "  geometry: { width: 90, height: 90, unit: mm }\n  artwork:\n" + artwork +
            "  rights:\n    license: CC0-1.0\n    rights_holder: Renderflow contributors\n"
            "  accessibility:\n    summary: Two synthetic colored pages, each with a description.\n"
            "    access_modes: [visual]\n    hazards: [none]\n"
            "targets:\n  exact:\n    - id: target.ebook\n      role: ebook\n"
            "      format: epub\n      requirement: required\n"
            "execution:\n  fixed_layout_epub:\n    page_progression_direction: ltr\n"
            "    spread: none\n    cover_member_id: source.page001\n    max_pages: 2\n"
            "    max_input_bytes: 1000000\n    max_output_bytes: 5000000\n"
            "output:\n  bundle_root: dist\n"
        )
        (work / "renderflow.yaml").write_text(spec, encoding="utf-8")
        run_binary(installed, work, ["spec", "validate", "--config", "renderflow.yaml"])
        preview = run_binary(installed, work, ["build", "--config", "renderflow.yaml", "--dry-run"])
        require(preview.stdout.strip(), "planning did not emit a dry-run plan")
        run_binary(installed, work, ["build", "--config", "renderflow.yaml"])
        epub = work / "dist" / "source.pages" / "ebook.epub"
        require(epub.is_file(), "native fixture execution did not generate EPUB")
        inspected = run_binary(installed, work, ["ebook", "inspect", "--input", str(epub), "--run-manifest", str(work / "dist" / "renderflow-run.json"), "--fixed-layout", "--format", "json"])
        evidence = json.loads(inspected.stdout)
        require(evidence.get("valid") is True and evidence.get("fixed_layout", {}).get("status") == "validated" and evidence.get("provenance", {}).get("status") == "verified", "native EPUB inspection or provenance failed")
        run_binary(installed, work, ["spec", "validate", "--config", "missing.yaml"], expect_success=False)
        smoke_pdf(installed, root)
        print("Clean installation smoke: version, help, doctor, plan, native EPUB, exact-provider PDF, independent inspections, and failure exits passed")


def verify(args: argparse.Namespace) -> None:
    identity(args)
    root = Path(args.artifact_dir).resolve()
    check_checksum(root, MANIFEST)
    receipt = json.loads(asset(root, MANIFEST).read_text(encoding="utf-8"))
    for key, expected in (("schema", SCHEMA), ("repository", args.repository), ("version", args.version), ("tag", args.tag), ("commit", args.commit), ("channel", "integration-candidate")):
        require(receipt.get(key) == expected, f"release manifest {key} mismatch")
    binary = f"renderflow-{TARGET}"
    binary_info = receipt["binary"]
    require(binary_info["name"] == binary and binary_info["target"] == TARGET and binary_info["url"] == release_url(args.tag, binary), "binary lock mismatch")
    require(
        receipt["compatibility"]["supported_platforms"] == [TARGET]
        and receipt["compatibility"]["verified_host"] == "ubuntu-24.04-x86_64"
        and receipt["compatibility"]["gnu_libc_minimum"] == "2.39"
        and receipt["compatibility"]["other_platforms"] == "unsupported_unverified",
        "platform/libc baseline mismatch",
    )
    expected_tools = [
        {"name": "img2pdf", "constraint": "==0.6.3", "required_for": "publication.generate.pdf.interior"},
        {"name": "epubcheck", "constraint": "v5, optional; exact version recorded per inspection", "required_for": "optional independent EPUBCheck evidence"},
        {"name": "pandoc", "constraint": ">=2.0.0, optional", "required_for": "provider-backed document conversion"},
    ]
    require(receipt["compatibility"]["external_tools"] == expected_tools, "external-tool compatibility mismatch")
    require(receipt["security"]["asset_signing"] == "unsigned" and receipt["security"]["provenance"] == "github_sigstore_bundle_attached_verify_separately", "signing/provenance status mismatch")
    require(all(receipt["contracts"][field] == args.version for field in ("cli_version", "core_crate_version", "plugin_sdk_crate_version")), "contract version mismatch")
    require(receipt["contracts"]["plugin_contract"] == "renderflow.plugin/v2alpha1" and receipt["contracts"]["provider_contract"] == "renderflow.provider/v1", "contract identifier mismatch")
    for field, expected in (("artifact_manifest_contract", "renderflow.artifact-manifest/v1"),
                            ("flow_artifact_contract", "flow.artifact/v1"),
                            ("tool_registry_contract", "renderflow.tool-registry/v1")):
        require(receipt["contracts"][field] == expected, f"{field} mismatch")
    required_capabilities = {"publication.generate.pdf.interior": ("tool.img2pdf", "pdf", "requires_external_tool", ["img2pdf 0.6.3"]), "ebook.generate.epub.fixed-layout": ("tool.renderflow-epub", "epub", "native", [])}
    capabilities = {item["id"]: item for item in receipt["contracts"]["capabilities"]}
    require(set(capabilities) == set(required_capabilities), "release capability set mismatch")
    for name, (provider, output, availability, tools) in required_capabilities.items():
        item = capabilities[name]
        require(item["provider_id"] == provider and item["output_format"] == output and item["availability"] == availability and item["required_external_tools"] == tools and item["ordered_collection"] is True and item["source_mutation"] is False, f"capability effects/availability mismatch: {name}")
    schemas = {item["name"]: item for item in receipt["contracts"]["schemas"]}
    require(set(schemas) == set(SCHEMAS), "schema set mismatch")
    for name, (identifier, filename) in SCHEMAS.items():
        item = schemas[name]
        require(item["identifier"] == identifier and item["source_path"] == f"schemas/{filename}" and bool(HEX64.fullmatch(item["sha256"])), f"schema contract mismatch: {name}")
    expected_names = {binary, f"{binary}.sha256", SBOM, f"{SBOM}.sha256", NOTICES, f"{NOTICES}.sha256", ATTESTATION, f"{ATTESTATION}.sha256", SBOM_ATTESTATION, f"{SBOM_ATTESTATION}.sha256"}
    expected_kinds = dict(zip(
        [binary, f"{binary}.sha256", SBOM, f"{SBOM}.sha256", NOTICES, f"{NOTICES}.sha256", ATTESTATION, f"{ATTESTATION}.sha256", SBOM_ATTESTATION, f"{SBOM_ATTESTATION}.sha256"],
        ["cli-binary", "checksum", "spdx-sbom", "checksum", "dependency-notices", "checksum", "provenance-bundle", "checksum", "sbom-attestation-bundle", "checksum"], strict=True,
    ))
    entries = receipt["assets"]
    require(len(entries) == len(expected_names) and {entry["name"] for entry in entries} == expected_names, "asset list incomplete or duplicated")
    for entry in entries:
        path = asset(root, entry["name"])
        require(entry["kind"] == expected_kinds[entry["name"]], f"asset role mismatch: {entry['name']}")
        require(entry["url"] == release_url(args.tag, entry["name"]), f"asset URL mismatch: {entry['name']}")
        require(entry["size"] == path.stat().st_size and entry["sha256"] == digest(path), f"asset digest/size mismatch: {entry['name']}")
    require(binary_info["sha256"] == digest(asset(root, binary)) and binary_info["size"] == (root / binary).stat().st_size, "binary lock digest mismatch")
    for name in (binary, SBOM, NOTICES, ATTESTATION, SBOM_ATTESTATION):
        check_checksum(root, name)
    for bundle, subject in ((ATTESTATION, binary), (SBOM_ATTESTATION, binary)):
        attestation_bundle(root, str(root / bundle), bundle, subject, binary_info["sha256"])
    require(receipt["security"]["attestation_bundles"] == [{"subject": binary, "bundle": ATTESTATION}, {"subject": binary, "bundle": SBOM_ATTESTATION}], "attestation link mismatch")
    if args.verify_attestations:
        for bundle in (ATTESTATION, SBOM_ATTESTATION):
            command = [
                "gh", "attestation", "verify", str(root / binary),
                "--repo", REPOSITORY,
                "--bundle", str(root / bundle),
                "--source-ref", f"refs/tags/{args.tag}",
                "--source-digest", args.commit,
                "--signer-workflow", f"{REPOSITORY}/.github/workflows/release.yml",
            ]
            if bundle == SBOM_ATTESTATION:
                command += ["--predicate-type", "https://spdx.dev/Document/v2.3"]
            result = subprocess.run(command, capture_output=True, text=True, timeout=120, check=False)
            require(result.returncode == 0, f"Sigstore/GitHub attestation verification failed ({bundle}): {result.stderr[:1000]}")
        print("GitHub/Sigstore signatures verified for both attached bundles")
    if args.smoke:
        smoke(root / binary, args.version)
    if args.installer_script:
        installer = Path(args.installer_script).resolve()
        require(installer.is_file(), "installer script missing")
        with tempfile.TemporaryDirectory(prefix="renderflow-installer-smoke-") as temporary:
            install = Path(temporary) / "bin"
            env = os.environ.copy()
            env.update({"RENDERFLOW_VERSION": args.tag, "RENDERFLOW_DOWNLOAD_BASE_URL": root.as_uri(), "RENDERFLOW_INSTALL_DIR": str(install)})
            result = subprocess.run(["sh", str(installer)], env=env, capture_output=True, text=True, timeout=60, check=False)
            require(result.returncode == 0, f"file:// pinned installer smoke failed: {result.stderr[:1000]}")
            require(digest(install / "renderflow") == binary_info["sha256"], "installer binary differs from pinned release")
    print(f"Verified downloaded release assets and Flow provider lock: {args.tag} {args.commit} {binary_info['sha256']}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    for action in ("prepare", "manifest", "verify"):
        cmd = commands.add_parser(action)
        cmd.add_argument("--artifact-dir", required=True)
        cmd.add_argument("--version", required=True)
        cmd.add_argument("--tag", required=True)
        cmd.add_argument("--commit", required=True)
        cmd.add_argument("--target", required=True)
        cmd.add_argument("--repository", default=REPOSITORY)
        if action != "verify":
            cmd.add_argument("--source-dir", default=str(Path(__file__).resolve().parents[2]))
        if action == "prepare":
            cmd.add_argument("--metadata-file", help="Fixture-only, or a precomputed cargo metadata JSON file")
        if action == "manifest":
            cmd.add_argument("--attestation-bundle", required=True)
            cmd.add_argument("--sbom-attestation-bundle", required=True)
        if action == "verify":
            cmd.add_argument("--verify-attestations", action="store_true", help="Cryptographically verify both bundles via gh (network/trust root required)")
            cmd.add_argument("--smoke", action="store_true", help="Run isolated installed binary and a native synthetic EPUB fixture")
            cmd.add_argument("--installer-script", help="Test scripts/install.sh via file:// pinned assets in a clean temporary install")
    args = parser.parse_args()
    try:
        {"prepare": prepare, "manifest": manifest, "verify": verify}[args.command](args)
    except (ValueError, KeyError, OSError, json.JSONDecodeError, subprocess.TimeoutExpired) as error:
        print(f"release receipt {args.command}: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
