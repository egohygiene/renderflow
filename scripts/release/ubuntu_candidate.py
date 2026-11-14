#!/usr/bin/env python3
"""Run the selected offline Ubuntu candidate build and smoke, never a release.

Docker, the pinned image, Rust 1.94 and public registry/provider caches must
already be prepared. No image pull, installer, socket, model or secret is used.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tomllib

IMAGE = "ghcr.io/catthehacker/ubuntu:act-24.04@sha256:19619a010b204f44c5340bff7d39b68551109fe6a56a53f75201ea91e05d4270"
TARGET = "x86_64-unknown-linux-gnu"
LIMIT = 16 * 1024 * 1024


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def directory(value):
    path = Path(value).absolute()
    require(path.is_dir() and path.resolve() == path and "," not in str(path),
            "canonical existing directory without Docker mount separators required")
    return path


def run(argv, output, timeout=120):
    with output.open("xb") as capture:
        result = subprocess.run([str(item) for item in argv], stdout=capture,
                                stderr=subprocess.STDOUT, check=False, timeout=timeout)
    require(output.stat().st_size <= LIMIT, "captured command output exceeds 16 MiB")
    require(result.returncode == 0, f"selected command failed ({output.name}); retained log identifies the failure")
    return output.read_bytes()


def source_identity(source):
    git = ["git", "-c", "core.hooksPath=/dev/null", "-C", str(source)]
    def read(*args):
        return subprocess.check_output(git + list(args), text=True).strip()
    require(not read("status", "--porcelain", "--untracked-files=all"), "clean selected source required")
    return {"revision": read("rev-parse", "HEAD"), "tree": read("rev-parse", "HEAD^{tree}"),
            "cargo_lock_sha256": sha(source / "Cargo.lock")}


def candidate_inventory(assets, binary_name):
    payloads = {binary_name, "renderflow-sbom.spdx.json", "THIRD_PARTY_NOTICES.txt"}
    expected = payloads | {name + ".sha256" for name in payloads} | {binary_name + ".contract.json"}
    require({path.name for path in assets.iterdir()} == expected,
            "local candidate must contain exactly the seven selected files")
    result = []
    for path in sorted(assets.iterdir()):
        require(path.is_file() and not path.is_symlink() and 0 < path.stat().st_size <= 512 * 1024 * 1024,
                "bounded regular nonlink candidate file required")
        result.append({"name": path.name, "bytes": path.stat().st_size, "sha256": sha(path)})
    for name in payloads:
        require((assets / (name + ".sha256")).read_text() == sha(assets / name) + "  " + name + "\n",
                "local asset checksum differs")
    return result


def portable_summary(report, raw_sha):
    # Keep the local full ledger separately; portable evidence exposes only
    # explicit receipt fields and commands within the selected container.
    selected = ("source", "image", "baseline", "target", "maximum_glibc_symbol", "providers",
                "assets", "verifier_modules", "helper_sha256", "local_smoke", "scope", "target_cache",
                "selected_build_invocations", "independent_rebuilds", "kernel_scope", "network",
                "docker_socket_mounted", "release_authenticated", "supported_matrix_qualified",
                "publication_performed", "logs")
    result = {key: report[key] for key in selected}
    result["schema_version"] = "renderflow.ubuntu-candidate-summary/v1"
    result["local_full_report_sha256"] = raw_sha
    result["verifier_staging"] = {"execution_host": "host_python", "execution_command": "release_assets.py stage-verifier"}
    result["container_limits"] = {"network": "none", "read_only_root": True, "dropped_capabilities": "all",
        "no_new_privileges": True, "build_jobs": 2, "cpus": 2, "memory_bytes": 6 * 1024**3,
        "processes": 256, "tmpfs_bytes": 1024**3, "tmpfs_options": ["nosuid", "nodev", "exec"]}
    result["commands"] = [{"phase": command["phase"],
        "container_argv": command["argv"][command["argv"].index(IMAGE) + 1:],
        "timeout_seconds": command["timeout_seconds"]} for command in report["commands"]]
    return result


def main():
    helper_sha = sha(Path(__file__))
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("source-dir", "toolchain", "registry", "provider-site", "target-directory", "work-directory"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--reuse-target-cache", action="store_true",
                        help="Explicitly admit a nonempty caller cache without independent build provenance")
    args = parser.parse_args()
    source, toolchain, registry, providers, target = [directory(value) for value in
        (args.source_dir, args.toolchain, args.registry, args.provider_site, args.target_directory)]
    require(registry.name == "registry", "mount the public registry directory, never the whole Cargo home")
    work = Path(args.work_directory).absolute()
    require(work.parent.resolve() == work.parent and work.parent.is_dir(), "canonical work parent required")
    protected = (source, toolchain, registry, providers)
    for writable in (target, work):
        require(all(not writable.is_relative_to(root) and not root.is_relative_to(writable)
                    for root in protected), "writable roots must not overlap read-only source/tools/provider roots")
    require(not target.is_relative_to(work) and not work.is_relative_to(target),
            "separate target/work roots required")
    fresh_target = not any(target.iterdir())
    require(fresh_target or args.reuse_target_cache,
            "nonempty target requires explicit --reuse-target-cache; cache origin is not authenticated")
    work.mkdir(mode=0o700)
    before = source_identity(source)
    version = tomllib.loads((source / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    require(re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+-[0-9A-Za-z.-]+", version), "source prerelease version required")
    image = json.loads(run(["docker", "image", "inspect", IMAGE], work / "image.json"))[0]
    require(image["Os"] == "linux" and image["Architecture"] == "amd64" and IMAGE.replace(":act-24.04@", "@") in image["RepoDigests"],
            "prepared image identity/architecture differs")
    for name in ("cargo-home", "assets", "runtime", "provider-bin"):
        (work / name).mkdir(mode=0o700)
    wrapper = work / "provider-bin/img2pdf"
    wrapper.write_text("#!/usr/bin/python3\nimport img2pdf\nraise SystemExit(img2pdf.main())\n")
    wrapper.chmod(0o700)
    common = ["docker", "run", "--rm", "--pull", "never", "--platform", "linux/amd64",
              "--network", "none", "--read-only", "--cap-drop", "ALL",
              "--security-opt", "no-new-privileges", "--user", f"{os.getuid()}:{os.getgid()}",
              "--memory", "6g", "--cpus", "2", "--pids-limit", "256",
              "--tmpfs", "/tmp:rw,nosuid,nodev,exec,size=1g"]
    sequence = 0
    commands = []
    def container(phase, argv, mounts, env=None, timeout=120):
        nonlocal sequence
        sequence += 1
        name = f"renderflow-ubuntu-{os.getpid()}-{sequence}"
        command = common + ["--name", name]
        for host, destination, readonly in mounts:
            command += ["--mount", f"type=bind,source={host},target={destination}" + (",readonly" if readonly else "")]
        for key, value in (env or {}).items():
            command += ["--env", f"{key}={value}"]
        command += ["--workdir", "/tmp", IMAGE, *argv]
        commands.append({"phase": phase, "argv": command, "timeout_seconds": timeout})
        try:
            return run(command, work / (phase + ".log"), timeout)
        finally:
            subprocess.run(["docker", "rm", "--force", name], stdout=subprocess.DEVNULL,
                           stderr=subprocess.DEVNULL, timeout=30, check=False)

    builder_mounts = [(source, "/source", True), (toolchain, "/toolchain", True),
                      (work / "cargo-home", "/cargo-home", False), (registry, "/cargo-home/registry", True),
                      (target, "/target", False), (work / "assets", "/assets", False),
                      (work / "runtime", "/runtime", False)]
    builder_env = {"PATH": "/toolchain/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                   "CARGO_HOME": "/cargo-home", "CARGO_TARGET_DIR": "/target", "CARGO_BUILD_JOBS": "2",
                   "CARGO_NET_OFFLINE": "true", "CARGO_INCREMENTAL": "0",
                   "RUSTC": "/toolchain/bin/rustc", "RUSTDOC": "/toolchain/bin/rustdoc",
                   "PYTHONDONTWRITEBYTECODE": "1"}
    preflight_code = (
        "import json,platform,subprocess;from pathlib import Path;"
        "r=lambda *a:subprocess.check_output(a,text=True).strip();"
        "print(json.dumps(dict(os_release=Path('/etc/os-release').read_text(),libc=r('getconf','GNU_LIBC_VERSION'),"
        "rustc=r('/toolchain/bin/rustc','--version','--verbose'),cargo=r('/toolchain/bin/cargo','--version'),"
        "gcc=r('gcc','--version').splitlines()[0],python=platform.python_version(),kernel=platform.release())))")
    preflight = json.loads(container("preflight", ["python3", "-B", "-c", preflight_code], builder_mounts, builder_env))
    require('ID=ubuntu\n' in preflight["os_release"] and 'VERSION_ID="24.04"' in preflight["os_release"]
            and preflight["libc"] == "glibc 2.39" and preflight["rustc"].startswith("rustc 1.94.0 "),
            "selected Ubuntu/libc/compiler baseline differs")
    container("release-build", ["/toolchain/bin/cargo", "build", "--manifest-path", "/source/Cargo.toml",
              "--frozen", "--release", "--package", "renderflow-cli", "--bin", "renderflow",
              "--target", TARGET, "--jobs", "2"], builder_mounts, builder_env, 2400)
    built = target / TARGET / "release/renderflow"
    binary = work / "assets" / ("renderflow-" + TARGET)
    require(built.is_file() and not built.is_symlink() and 0 < built.stat().st_size <= 512 * 1024 * 1024,
            "bounded regular compiled binary required")
    shutil.copyfile(built, binary)
    binary.chmod(0o755)
    with binary.open("rb") as stream:
        header = stream.read(64)
    require(header[:6] == b"\x7fELF\x02\x01" and int.from_bytes(header[18:20], "little") == 62,
            "compiled binary CPU/class differs")
    elf = container("elf-requirements", ["readelf", "--version-info", "/assets/" + binary.name],
                    [(work / "assets", "/assets", True)])
    versions = {tuple(map(int, value.split(b".")))
                for value in re.findall(rb"GLIBC_([0-9]+(?:\.[0-9]+)+)", elf)}
    require(versions and max(versions) <= (2, 39), "compiled ELF exceeds GNU libc 2.39")
    metadata = container("cargo-metadata", ["/toolchain/bin/cargo", "metadata", "--manifest-path", "/source/Cargo.toml",
                         "--frozen", "--format-version", "1"], builder_mounts, builder_env)
    json.loads(metadata)
    (work / "runtime/cargo-metadata.json").write_bytes(metadata)
    contract = container("compiled-contract", ["/assets/" + binary.name, "release-contract"],
                         [(work / "assets", "/assets", True)])
    json.loads(contract)
    (work / "assets" / (binary.name + ".contract.json")).write_bytes(contract)
    container("package-prepare", ["python3", "-B", "/source/scripts/release/release_assets.py", "prepare",
              "--artifact-dir", "/assets", "--source-dir", "/source", "--version", version,
              "--tag", "v" + version, "--commit", before["revision"], "--target", TARGET,
              "--metadata-file", "/runtime/cargo-metadata.json"], builder_mounts, builder_env)
    assets_before_smoke = candidate_inventory(work / "assets", binary.name)
    run([sys.executable, "-B", source / "scripts/release/release_assets.py", "stage-verifier",
         "--output-directory", work / "verifier"], work / "stage-verifier.log")
    verifier_names = {"release_assets.py", "native_cohort.py", "cohort_assets.py"}
    require({path.name for path in (work / "verifier").iterdir()} == verifier_names,
            "staged verifier is not the closed three-module bundle")
    verifier = []
    for name in sorted(verifier_names):
        path = work / "verifier" / name
        require(path.is_file() and not path.is_symlink()
                and sha(path) == sha(source / "scripts/release" / name),
                "staged verifier differs from selected clean source")
        verifier.append({"name": name, "bytes": path.stat().st_size, "sha256": sha(path)})
    runtime_mounts = [(work / "assets", "/assets", True), (work / "verifier", "/verifier", True),
                      (providers, "/provider-site", True), (work / "provider-bin", "/provider-bin", True),
                      (work / "runtime", "/runtime", False)]
    runtime_env = {"PATH": "/provider-bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
                   "PYTHONPATH": "/provider-site", "PYTHONDONTWRITEBYTECODE": "1"}
    provider_code = (
        "import json,importlib.metadata as m,img2pdf,pikepdf;"
        "print(json.dumps(dict(img2pdf=m.version('img2pdf'),pikepdf=m.version('pikepdf'),qpdf=pikepdf.__libqpdf_version__)))")
    provider = json.loads(container("provider-preflight", ["python3", "-B", "-c", provider_code], runtime_mounts, runtime_env))
    require(provider["img2pdf"] == "0.6.3", "selected PDF provider differs")
    container("local-smoke", ["python3", "-B", "/verifier/release_assets.py", "local-smoke", "--binary",
              "/assets/" + binary.name, "--version", version, "--output", "/runtime/local-smoke.json"],
              runtime_mounts, runtime_env, 300)
    smoke = json.loads((work / "runtime/local-smoke.json").read_text())
    require(smoke["binary"]["sha256"] == sha(binary) and smoke["host"]["libc"] == ["glibc", "2.39"],
            "smoke bytes/host differ from the selected candidate")
    for name in ("source_authenticated", "release_authenticated", "supported_host_qualified",
                 "native_matrix_qualified", "publication_performed"):
        require(smoke[name] is False, "local smoke cannot grant release or whole-host authority")
    require(source_identity(source) == before, "selected source changed during the candidate trial")
    assets = candidate_inventory(work / "assets", binary.name)
    require(assets == assets_before_smoke, "candidate files changed during the selected smoke")
    require(sha(Path(__file__)) == helper_sha, "selected candidate helper changed during execution")
    report = {"schema_version": "renderflow.ubuntu-candidate-proof/v1", "source": before,
              "image": {"reference": IMAGE, "id": image["Id"], "preparation": "existing_pinned_image"},
              "baseline": preflight, "target": TARGET, "maximum_glibc_symbol": ".".join(map(str, max(versions))),
              "providers": provider, "assets": assets, "verifier_modules": verifier,
              "verifier_staging": {"execution_host": "host_python", "python_executable": sys.executable,
                                   "execution_command": "release_assets.py stage-verifier"},
              "helper_sha256": helper_sha,
              "local_smoke": smoke, "commands": commands,
              "scope": "selected_ubuntu_userland_build_and_offline_smoke",
              "target_cache": {"fresh_target_at_start": fresh_target,
                               "cache_origin": "fresh_empty_directory" if fresh_target else "caller_supplied_existing_cache",
                               "prior_cache_origin_authenticated": False},
              "selected_build_invocations": 1, "independent_rebuilds": 0,
              "kernel_scope": "container_uses_host_kernel; not_a_full_native_Ubuntu_host",
              "network": "none", "docker_socket_mounted": False, "release_authenticated": False,
              "supported_matrix_qualified": False, "publication_performed": False,
              "logs": [{"name": path.name, "bytes": path.stat().st_size, "sha256": sha(path)}
                       for path in sorted(work.glob("*.log"))]}
    (work / "proof.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    (work / "portable-summary.json").write_text(json.dumps(portable_summary(report, sha(work / "proof.json")),
                                                         indent=2, sort_keys=True) + "\n")
    print(json.dumps({"source": before, "binary_sha256": sha(binary), "image": image["Id"],
                      "proof": str(work / "proof.json"), "release_authenticated": False}))


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print("Ubuntu candidate proof refused: " + str(error)[:1500], file=sys.stderr)
        raise SystemExit(1)
