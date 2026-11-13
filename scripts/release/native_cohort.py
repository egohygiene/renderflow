"""Native v2 adapter for the existing release owner; no publication side effect."""
from __future__ import annotations
import argparse
import json
import os
from pathlib import Path
import platform
import stat
import subprocess
import tomllib
from types import SimpleNamespace
import cohort_assets as assets

TARGETS = ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-apple-darwin")
MANIFEST = "renderflow-release-manifest-v2.json"


def host_target():
    system, machine = platform.system(), platform.machine().lower()
    if system == "Linux" and machine in {"x86_64", "amd64"} and platform.libc_ver()[0] == "glibc":
        return TARGETS[0]
    if system == "Darwin" and machine in {"arm64", "aarch64"}:
        return TARGETS[1]
    if system == "Darwin" and machine in {"x86_64", "amd64"}:
        return TARGETS[2]
    raise ValueError("no native cohort adapter for this host; Windows remains a retained objective")


def binary_identity(binary, target):
    if target == TARGETS[0]:
        assets.require(len(binary) >= 64 and binary[:6] == b"\x7fELF\x02\x01" and int.from_bytes(binary[18:20], "little") == 62, "native ELF CPU/class differs")
    else:
        cpu = 0x100000c if target == TARGETS[1] else 0x1000007
        assets.require(target in TARGETS and len(binary) >= 32 and binary[:4] == b"\xcf\xfa\xed\xfe" and int.from_bytes(binary[4:8], "little") == cpu, "native Mach-O CPU/container differs")


def command(argv, source, environment=None, timeout=600):
    result = subprocess.run([str(value) for value in argv], cwd=source, env=environment,
                            capture_output=True, check=False, timeout=timeout)
    assets.require(result.returncode == 0 and len(result.stdout) + len(result.stderr) <= 16777216,
                   "native release preparation command failed or exceeded capture bounds")
    return result.stdout


def cargo_binary(path):
    # Cargo may hard-link its public target to deps/. Both are inside the fresh
    # owned build root; final distributable files are copied into new inodes.
    assets.require(path.resolve() == path, "canonical regular Cargo output required")
    before = path.stat()
    assets.require(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= assets.MAX_FILE, "bounded Cargo binary required")
    with path.open("rb") as reader:
        data = reader.read(assets.MAX_FILE + 1)
    after = path.stat()
    identity = lambda m: (m.st_dev, m.st_ino, m.st_size, m.st_mtime_ns, m.st_ctime_ns)
    assets.require(identity(before) == identity(after) and len(data) == before.st_size, "Cargo output changed during capture")
    return data


def source_identity(source, revision, version):
    git = ["git", "-c", "core.hooksPath=/dev/null"]
    assets.require(command(git + ["rev-parse", "HEAD"], source).decode().strip() == revision, "source HEAD differs")
    assets.require(not command(git + ["status", "--porcelain", "--untracked-files=all"], source), "clean source required")
    cargo = tomllib.loads(assets.read(source / "Cargo.toml", assets.MAX_JSON).decode())
    assets.require(cargo["workspace"]["package"]["version"] == version, "Cargo version differs")


def put(path, data, mode=0o644):
    with path.open("xb") as writer:
        writer.write(data)
        writer.flush()
        os.fsync(writer.fileno())
    path.chmod(mode)


def put_json(path, value):
    put(path, (json.dumps(value, sort_keys=True, indent=2) + "\n").encode())


def fresh(path, source=None):
    path = Path(path).absolute()
    assets.require(path.parent.resolve() == path.parent and path.parent.is_dir(), "canonical existing output parent required")
    assets.require(source is None or not path.is_relative_to(source), "native outputs must be outside source")
    path.mkdir(mode=0o700)
    return path


def build(args):
    import release_assets as legacy
    source = Path(args.source_dir).resolve()
    legacy.identity(SimpleNamespace(version=args.version, tag="v" + args.version, commit=args.commit,
                                    target=args.target, repository=legacy.REPOSITORY), TARGETS)
    assets.require(host_target() == args.target, "native build requires its real target host")
    source_identity(source, args.commit, args.version)
    rustc = command(["rustup", "run", "1.94.0", "rustc", "--version", "--verbose"], source).decode()
    assets.require(rustc.startswith("rustc 1.94.") and "host: " + args.target + "\n" in rustc, "installed native Rust toolchain differs")
    work = fresh(args.work_directory, source)
    output = Path(args.artifact_dir).absolute()
    assets.require(not output.is_relative_to(work) and not work.is_relative_to(output), "separate build/output roots required")
    binaries, descriptors = [], []
    for index in [1, 2]:
        target = work / ("build-" + str(index))
        environment = dict(os.environ)
        environment.pop("RUSTFLAGS", None)
        environment.update({"RUSTUP_TOOLCHAIN":"1.94.0", "RUSTUP_AUTO_INSTALL":"0", "CARGO_TARGET_DIR":str(target), "CARGO_NET_OFFLINE":"true", "CARGO_INCREMENTAL":"0",
                            "CARGO_ENCODED_RUSTFLAGS":"--remap-path-prefix="+str(source)+"=/renderflow/source\x1f--remap-path-prefix="+str(target)+"=/renderflow/target"})
        command(["cargo", "build", "--frozen", "--release", "--package", "renderflow-cli", "--bin", "renderflow", "--target", args.target], source, environment, 2400)
        executable = target / args.target / "release/renderflow"
        binary = cargo_binary(executable)
        binary_identity(binary, args.target)
        assets.require(command([executable, "--version"], source).decode().strip() == "renderflow " + args.version, "compiled CLI version differs")
        descriptor = json.loads(command([executable, "release-contract"], source))
        binaries.append(binary)
        descriptors.append((json.dumps(descriptor, sort_keys=True, indent=2) + "\n").encode())
    assets.require(binaries[0] == binaries[1] and descriptors[0] == descriptors[1], "isolated native binaries/contracts differ")
    source_identity(source, args.commit, args.version)
    output = fresh(output, source)
    binary_name = "renderflow-" + args.target
    descriptor_name = binary_name + ".contract.json"
    build_name = binary_name + ".build.json"
    put(output / binary_name, binaries[0], 0o755)
    put(output / descriptor_name, descriptors[0])
    descriptor_pin = assets.asset(output, descriptor_name)
    assets.descriptor(output, descriptor_pin, args.version)
    put_json(output / build_name, {"schema_version":"renderflow.native-build/v1", "target":args.target,
        "host_target":host_target(), "version":args.version, "source_revision":args.commit,
        "binary_sha256":assets.sha(binaries[0]), "release_contract_sha256":assets.sha(descriptors[0]),
        "host_system":platform.system(), "host_release":platform.release(), "host_libc":list(platform.libc_ver()),
        "qualification":"build_and_contract_capture_only", "isolated_builds":2})
    metadata_file = args.metadata_file
    if metadata_file is None:
        metadata = command(["rustup", "run", "1.94.0", "cargo", "metadata", "--frozen", "--format-version", "1", "--filter-platform", args.target], source, environment)
        metadata_file = str(work / "cargo-metadata.json")
        put(Path(metadata_file), metadata)
    legacy.prepare(SimpleNamespace(artifact_dir=str(output), source_dir=str(source), version=args.version,
        tag="v" + args.version, commit=args.commit, target=args.target, repository=legacy.REPOSITORY, metadata_file=metadata_file), TARGETS)
    source_identity(source, args.commit, args.version)
    names = sorted(p.name for p in output.iterdir() if p.is_file())
    put_json(output / "native-assets.json", {"schema_version":"renderflow.native-assets/v1", "target":args.target,
        "version":args.version,"source_revision":args.commit,"binary":binary_name,"descriptor":descriptor_name,
        "native_build_evidence":build_name,"assets":[assets.asset(output, name) for name in names]})


def assemble(args):
    import release_assets as legacy
    source = Path(args.source_dir).resolve()
    request = json.loads(assets.read(Path(args.parts_request), assets.MAX_JSON))
    assets.require(request["schema_version"] == "renderflow.native-parts/v1" and 0 < len(request["parts"]) <= 3, "native parts request shape/bound")
    source_identity(source, request["source_revision"], request["version"])
    output = fresh(args.artifact_dir, source)
    targets, seen = [], set()
    for part in request["parts"]:
        directory = Path(part["directory"]).absolute()
        receipt = json.loads(assets.read(directory / "native-assets.json", assets.MAX_JSON))
        target = receipt["target"]
        assets.require(receipt["schema_version"] == "renderflow.native-assets/v1" and target in TARGETS and target not in seen and receipt["version"] == request["version"] and receipt["source_revision"] == request["source_revision"], "native part identity/coverage differs")
        seen.add(target)
        assets.require(0 < len(receipt["assets"]) <= 64 and len({p["name"] for p in receipt["assets"]}) == len(receipt["assets"]), "native part inventory bounds/identity")
        assets.require({p.name for p in directory.iterdir()} == {p["name"] for p in receipt["assets"]} | {"native-assets.json"}, "native part membership differs")
        for entry in receipt["assets"]:
            assets.require(assets.asset(directory, entry["name"]) == entry, "native part bytes changed")
        mapping = {}
        for kind in ["binary", "descriptor", "native_build_evidence"]:
            name = receipt[kind]
            assets.require(name in {entry["name"] for entry in receipt["assets"]}, "native part points outside its exact inventory")
            assets.asset(directory, name)
            mapping[kind] = name
            put(output / name, assets.read(directory / name))
        support = []
        for name in [legacy.SBOM, legacy.NOTICES]:
            selected = "renderflow-" + target + "-" + name
            put(output / selected, assets.read(directory / name))
            support.append(selected)
        targets.append({"target":target, **mapping, "support_assets":support})
    manifest = assets.prepare(output, source, {"schema_version":"renderflow.release-assets-request/v2",
        "version":request["version"], "source_revision":request["source_revision"], "targets":targets})
    assets.verify(output, manifest)
    put_json(output / MANIFEST, manifest)
    put(output / (MANIFEST + ".sha256"), (assets.sha(assets.read(output / MANIFEST)) + "  " + MANIFEST + "\n").encode())


def pinned(args):
    data = assets.read(Path(args.manifest), assets.MAX_JSON)
    assets.require(assets.sha(data) == args.manifest_sha256, "selected v2 manifest digest differs")
    manifest = json.loads(data)
    root = Path(args.artifact_dir).absolute()
    assets.require(root.resolve() == root and root.is_dir() and Path(args.manifest).absolute() == root / MANIFEST, "canonical cohort root and its exact manifest required")
    assets.verify(root, manifest)
    expected = {MANIFEST, MANIFEST + ".sha256"}
    for row in manifest["targets"]:
        expected.update(row[kind]["name"] for kind in ["binary", "descriptor", "native_build_evidence"])
        expected.update(item["name"] for item in row.get("support_assets", []))
    assets.require({p.name for p in root.iterdir()} == expected, "cohort membership contains missing or unreferenced assets")
    assets.require(assets.read(root / (MANIFEST + ".sha256"), 1024) == (args.manifest_sha256 + "  " + MANIFEST + "\n").encode(), "cohort manifest checksum differs")
    return root, manifest


def install(args):
    assets.require(args.acknowledge_unqualified_candidate, "explicit candidate installation acknowledgement required")
    root, manifest = pinned(args)
    target = host_target()
    selected = [row for row in manifest["targets"] if row["target"] == target]
    assets.require(len(selected) == 1, "no prepared native asset for this host")
    selected = selected[0]
    output = Path(args.install_directory).absolute()
    assets.require(not output.is_relative_to(root) and not root.is_relative_to(output), "install/source asset directories overlap")
    output = fresh(output)
    binary = assets.read(root / selected["binary"]["name"])
    descriptor = assets.read(root / selected["descriptor"]["name"], assets.MAX_JSON)
    assets.require(assets.sha(binary) == selected["binary"]["sha256"] and len(binary) == selected["binary"]["byte_count"] and assets.sha(descriptor) == selected["descriptor"]["sha256"] and len(descriptor) == selected["descriptor"]["byte_count"], "selected install bytes changed after manifest observation")
    binary_identity(binary, target)
    put(output / "renderflow", binary, 0o755)
    put(output / "release-contract.json", descriptor)
    put_json(output / "renderflow-install.json", {"schema_version":"renderflow.installation/v1", "version":manifest["version"],
        "source_revision":manifest["source_revision"], "target":target, "manifest_sha256":args.manifest_sha256,
        "binary_sha256":assets.sha(binary), "descriptor_sha256":assets.sha(descriptor),
        "provider_invoked":False, "release_authenticated":False, "native_qualified":False,
        "state":"operator_pinned_candidate_bytes_installed"})


def arguments(commands):
    build_parser = commands.add_parser("cohort-build")
    for name in ["artifact-dir", "work-directory", "target", "version", "commit"]:
        build_parser.add_argument("--" + name, required=True)
    build_parser.add_argument("--source-dir", default=str(Path(__file__).resolve().parents[2]))
    build_parser.add_argument("--metadata-file")
    assembly = commands.add_parser("cohort-assemble")
    assembly.add_argument("--artifact-dir", required=True)
    assembly.add_argument("--parts-request", required=True)
    assembly.add_argument("--source-dir", default=str(Path(__file__).resolve().parents[2]))
    for name in ["cohort-verify", "cohort-install", "cohort-handoff", "cohort-report"]:
        parser = commands.add_parser(name)
        parser.add_argument("--artifact-dir", required=True)
        parser.add_argument("--manifest", required=True)
        parser.add_argument("--manifest-sha256", required=True)
        if name == "cohort-report":
            parser.add_argument("--request", required=True)
            parser.add_argument("--output-directory", required=True)
            parser.add_argument("--kind", choices=["publisher", "qualified"], required=True)
        if name == "cohort-install":
            parser.add_argument("--install-directory", required=True)
            parser.add_argument("--acknowledge-unqualified-candidate", action="store_true")


def dispatch(args):
    if args.command == "cohort-build":
        build(args)
    elif args.command == "cohort-assemble":
        assemble(args)
    elif args.command == "cohort-install":
        install(args)
    elif args.command == "cohort-report":
        report(args)
    else:
        root, manifest = pinned(args)
        result = {"verification":"exact_supplied_asset_bytes_only", "native_qualified":False, "publication_performed":False}
        if args.command == "cohort-handoff":
            result["required_before_publication"] = "retain the existing exact-source native, supply-chain, installation and release gates; this command does not satisfy them"
            result["manual_argv"] = ["gh","release","create","v"+manifest["version"],"--repo","egohygiene/renderflow","--target",manifest["source_revision"],"--draft","--prerelease","--latest=false"] + [str(p) for p in sorted(root.iterdir()) if p.is_file()]
        print(json.dumps(result, sort_keys=True))


def report(args):
    """Bind the selected existing v2 cohort before offline owner report assembly."""
    import importlib.util
    root, manifest = pinned(args)
    source = Path(__file__).resolve().parents[2] / "scripts/owner_reports.py"
    spec = importlib.util.spec_from_file_location("renderflow_owner_reports", source)
    adapter = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(adapter)
    request_file = adapter.file_pin(Path(args.request).absolute(), adapter.MAX_JSON)
    request = adapter.document(request_file)
    selected = request["publisher_request"] if args.kind == "qualified" else request
    row, _, _ = adapter.select(selected)
    represented = [r for r in manifest["targets"] if r["target"] == row["target"]]
    assets.require(len(represented) == 1, "report target absent from selected v2 cohort")
    native = represented[0]
    assets.require(adapter.native_path(selected["archive"]["path"]) == root / native["binary"]["name"] and selected["archive"]["sha256"] == native["binary"]["sha256"] and selected.get("asset_kind") == "raw_binary", "report does not bind selected raw v2 binary")
    assets.require(any(c.get("identifier") == "renderflow.release-contract/v2" and c.get("file", {}).get("sha256") == native["descriptor"]["sha256"] and adapter.native_path(c["file"]["path"]) == root / native["descriptor"]["name"] for c in selected["contract_members"]), "report omits exact v2 descriptor")
    output = Path(args.output_directory).absolute()
    assets.require(not output.is_relative_to(root) and not root.is_relative_to(output), "report output overlaps immutable cohort")
    result = adapter.emit(args.kind, request, output)
    assets.require(adapter.file_pin(Path(args.request).absolute(), adapter.MAX_JSON) == request_file, "report request changed")
    pinned(args)
    print(json.dumps(result, sort_keys=True))
