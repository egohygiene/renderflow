#!/usr/bin/env python3
"""Offline synthetic release receipt, tamper, and installer refusal tests."""

import base64
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
import unittest

import release_assets as release


SCRIPT = Path(__file__).resolve().parent / "release_assets.py"
REPO = SCRIPT.parents[2]
VERSION = "0.3.0-rc.1"
TAG = f"v{VERSION}"
COMMIT = "a" * 40


def call(action: str, root: Path, *extra: str, success: bool = True) -> subprocess.CompletedProcess[str]:
    cmd = [sys.executable, str(SCRIPT), action, "--artifact-dir", str(root / "assets"),
           "--version", VERSION, "--tag", TAG, "--commit", COMMIT,
           "--target", release.TARGET, *extra]
    result = subprocess.run(cmd, capture_output=True, text=True, check=False, timeout=60)
    if success:
        assert result.returncode == 0, result.stderr
    else:
        assert result.returncode != 0, result.stdout
    return result


def bundle(name: str, sha: str, sbom: dict | None = None) -> dict:
    statement = {"_type": "https://in-toto.io/Statement/v1", "subject": [{"name": name, "digest": {"sha256": sha}}],
                 "predicateType": "https://spdx.dev/Document/v2.3" if sbom is not None else "https://slsa.dev/provenance/v1",
                 "predicate": sbom if sbom is not None else {"buildDefinition": {"buildType": "fixture-only"}}}
    return {
        "mediaType": "application/vnd.dev.sigstore.bundle.v0.3+json",
        "dsseEnvelope": {"payloadType": "application/vnd.in-toto+json", "payload": base64.b64encode(json.dumps(statement).encode()).decode(), "signatures": [{"sig": "fixture-only-not-a-valid-signature"}]},
        "verificationMaterial": {"certificate": "fixture-only"},
    }


class ReleaseReceiptTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="renderflow-release-tests-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.source.mkdir()
        self.assets = self.root / "assets"
        self.assets.mkdir()
        (self.source / "schemas").mkdir()
        for _, filename in release.SCHEMAS.values():
            shutil.copyfile(REPO / "schemas" / filename, self.source / "schemas" / filename)
        shutil.copyfile(REPO / "Cargo.toml", self.source / "Cargo.toml")
        shutil.copyfile(REPO / "Cargo.lock", self.source / "Cargo.lock")
        self.assertEqual(release.tomllib.loads((self.source / "Cargo.toml").read_text())["workspace"]["package"]["version"], VERSION)
        manifest = self.root / "mock" / "Cargo.toml"
        manifest.parent.mkdir()
        manifest.write_text("[package]\nname=\"anyhow\"\nversion=\"1.0.0\"\n")
        (manifest.parent / "LICENSE-MIT").write_text("Synthetic MIT license text for parser test.\n")
        lock = release.tomllib.loads((self.source / "Cargo.lock").read_text())
        anyhow = next(item for item in lock["package"] if item["name"] == "anyhow")
        metadata = {"packages": [
            {"name": "renderflow-cli", "version": VERSION, "license": "MIT", "source": None, "manifest_path": str(self.source / "Cargo.toml")},
            {"name": "anyhow", "version": anyhow["version"], "license": "MIT OR Apache-2.0", "source": "registry+https://github.com/rust-lang/crates.io-index", "repository": "https://github.com/dtolnay/anyhow", "manifest_path": str(manifest)},
        ]}
        self.metadata = self.root / "metadata.json"
        self.metadata.write_text(json.dumps(metadata))
        self.name = f"renderflow-{release.TARGET}"
        binary = self.assets / self.name
        binary.write_text("#!/usr/bin/env sh\nif [ \"${1:-}\" = \"--version\" ]; then printf 'renderflow 0.3.0-rc.1\\n'; exit 0; fi\nexit 1\n")
        binary.chmod(0o755)

    def prepared(self) -> dict:
        call("prepare", self.root, "--source-dir", str(self.source), "--metadata-file", str(self.metadata))
        sha = release.digest(self.assets / self.name)
        (self.assets / release.ATTESTATION).write_text(json.dumps(bundle(self.name, sha)))
        sbom = json.loads((self.assets / release.SBOM).read_text())
        (self.assets / release.SBOM_ATTESTATION).write_text(json.dumps(bundle(self.name, sha, sbom)))
        call("manifest", self.root, "--source-dir", str(self.source),
             "--attestation-bundle", str(self.assets / release.ATTESTATION),
             "--sbom-attestation-bundle", str(self.assets / release.SBOM_ATTESTATION))
        return json.loads((self.assets / release.MANIFEST).read_text())

    def test_offline_receipt_is_self_consistent_and_portable(self) -> None:
        receipt = self.prepared()
        call("verify", self.root)
        self.assertEqual(receipt["binary"]["sha256"], release.digest(self.assets / self.name))
        self.assertFalse(any(c["source_mutation"] for c in receipt["contracts"]["capabilities"]))
        self.assertEqual(receipt["security"]["asset_signing"], "unsigned")
        self.assertEqual(len(receipt["contracts"]["schemas"]), 7)
        self.assertEqual(len(receipt["assets"]), 10)
        self.assertTrue(all((self.assets / f"{entry['name']}.sha256").is_file()
                            for entry in receipt["assets"] if entry["kind"] != "checksum"))
        self.assertIn("Synthetic MIT license text", (self.assets / release.NOTICES).read_text())
        inventory = json.loads((self.assets / release.SBOM).read_text())
        self.assertEqual(inventory["spdxVersion"], "SPDX-2.3")
        self.assertTrue(any(p["name"] == "anyhow" for p in inventory["packages"]))
        try:
            import jsonschema
        except ImportError:
            pass
        else:
            schema = json.loads((REPO / "schemas" / "renderflow-release-manifest-v1.schema.json").read_text())
            jsonschema.validate(receipt, schema)
        downloaded = self.root / "downloaded"
        shutil.copytree(self.assets, downloaded)
        renamed_root = self.root / "independent"
        renamed_root.mkdir()
        downloaded.rename(renamed_root / "assets")
        call("verify", renamed_root)

    def test_binary_tamper_and_checksum_record_refused(self) -> None:
        self.prepared()
        (self.assets / self.name).write_bytes(b"tampered")
        self.assertIn("asset digest/size mismatch", call("verify", self.root, success=False).stderr)
        self.assertIn("checksum mismatch", call("manifest", self.root, "--source-dir", str(self.source),
            "--attestation-bundle", str(self.assets / release.ATTESTATION),
            "--sbom-attestation-bundle", str(self.assets / release.SBOM_ATTESTATION), success=False).stderr)

    def test_missing_bundle_wrong_subject_and_wrong_commit_refused(self) -> None:
        self.prepared()
        (self.assets / release.SBOM_ATTESTATION).unlink()
        self.assertIn("missing or symlinked asset", call("verify", self.root, success=False).stderr)
        wrong = bundle(self.name, "0" * 64, json.loads((self.assets / release.SBOM).read_text()))
        (self.assets / release.SBOM_ATTESTATION).write_text(json.dumps(wrong))
        self.assertIn("asset digest/size mismatch", call("verify", self.root, success=False).stderr)
        call("verify", self.root, "--commit", "b" * 40, success=False)

    def test_attestation_subject_refused_before_manifest(self) -> None:
        call("prepare", self.root, "--source-dir", str(self.source), "--metadata-file", str(self.metadata))
        sha = release.digest(self.assets / self.name)
        (self.assets / release.ATTESTATION).write_text(json.dumps(bundle(self.name, sha)))
        (self.assets / release.SBOM_ATTESTATION).write_text(json.dumps(bundle(self.name, "0" * 64, json.loads((self.assets / release.SBOM).read_text()))))
        result = call("manifest", self.root, "--source-dir", str(self.source),
                      "--attestation-bundle", str(self.assets / release.ATTESTATION),
                      "--sbom-attestation-bundle", str(self.assets / release.SBOM_ATTESTATION), success=False)
        self.assertIn("attestation subject mismatch", result.stderr)

    def test_sbom_predicate_must_match_attached_inventory(self) -> None:
        call("prepare", self.root, "--source-dir", str(self.source), "--metadata-file", str(self.metadata))
        sha = release.digest(self.assets / self.name)
        (self.assets / release.ATTESTATION).write_text(json.dumps(bundle(self.name, sha)))
        signed_other = {"spdxVersion": "SPDX-2.3", "packages": []}
        (self.assets / release.SBOM_ATTESTATION).write_text(json.dumps(bundle(self.name, sha, signed_other)))
        result = call("manifest", self.root, "--source-dir", str(self.source),
                      "--attestation-bundle", str(self.assets / release.ATTESTATION),
                      "--sbom-attestation-bundle", str(self.assets / release.SBOM_ATTESTATION), success=False)
        self.assertIn("attached SBOM does not match signed predicate", result.stderr)

    def test_gh_verifier_is_bound_to_exact_commit_ref_workflow_and_predicate(self) -> None:
        self.prepared()
        fake_bin = self.root / "fake-tools"
        fake_bin.mkdir()
        gh = fake_bin / "gh"
        gh.write_text("#!/usr/bin/env sh\nprintf '%s\\n' \"$*\" >> \"$GH_ARGV_LOG\"\n")
        gh.chmod(0o755)
        log = self.root / "gh-argv.txt"
        env = os.environ.copy()
        env["PATH"] = f"{fake_bin}{os.pathsep}{env.get('PATH', '')}"
        env["GH_ARGV_LOG"] = str(log)
        command = [sys.executable, str(SCRIPT), "verify", "--artifact-dir", str(self.assets),
                   "--version", VERSION, "--tag", TAG, "--commit", COMMIT,
                   "--target", release.TARGET, "--verify-attestations"]
        result = subprocess.run(command, env=env, capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        invocations = log.read_text().splitlines()
        self.assertEqual(len(invocations), 2)
        for invocation in invocations:
            self.assertIn(f"--source-ref refs/tags/{TAG}", invocation)
            self.assertIn(f"--source-digest {COMMIT}", invocation)
            self.assertIn("--signer-workflow egohygiene/renderflow/.github/workflows/release.yml", invocation)
        self.assertNotIn("--predicate-type", invocations[0])
        self.assertIn("--predicate-type https://spdx.dev/Document/v2.3", invocations[1])

    @unittest.skipUnless(platform.system() == "Linux" and platform.machine() == "x86_64", "installer has one supported host")
    def test_file_installer_exact_version_and_checksum(self) -> None:
        self.prepared()
        call("verify", self.root, "--installer-script", str(REPO / "scripts" / "install.sh"))
        # A modified payload with the old checksum is rejected before installation.
        (self.assets / self.name).write_bytes(b"tampered")
        install = self.root / "install"
        env = os.environ.copy()
        env.update({"RENDERFLOW_VERSION": TAG, "RENDERFLOW_DOWNLOAD_BASE_URL": self.assets.as_uri(), "RENDERFLOW_INSTALL_DIR": str(install)})
        result = subprocess.run(["sh", str(REPO / "scripts" / "install.sh")], env=env, text=True, capture_output=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("checksum verification failed", result.stderr)
        self.assertFalse((install / "renderflow").exists())
        # A checksum-correct binary with a different version cannot overwrite an install.
        (self.assets / self.name).write_text("#!/usr/bin/env sh\nprintf 'renderflow 9.9.9\\n'\n")
        release.checksum(self.assets, self.name)
        result = subprocess.run(["sh", str(REPO / "scripts" / "install.sh")], env=env, text=True, capture_output=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("version differs", result.stderr)
        self.assertFalse((install / "renderflow").exists())
        env["RENDERFLOW_VERSION"] = "latest"
        result = subprocess.run(["sh", str(REPO / "scripts" / "install.sh")], env=env, text=True, capture_output=True, check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exact release tag", result.stderr)


if __name__ == "__main__":
    unittest.main()
