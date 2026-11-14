"""Admission checks do not invoke Docker, Rust, providers or product media."""
import hashlib
import importlib.util
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[1] / "ubuntu_candidate.py"


class CandidateAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name).resolve()
        for name in ("source", "toolchain", "registry", "providers", "target"):
            (self.root / name).mkdir()

    def tearDown(self):
        self.temporary.cleanup()

    def refuse(self, target, work, expected, *flags):
        argv = [sys.executable, "-B", str(SCRIPT)]
        for name, value in (("source-dir", self.root / "source"),
                            ("toolchain", self.root / "toolchain"),
                            ("registry", self.root / "registry"),
                            ("provider-site", self.root / "providers"),
                            ("target-directory", target), ("work-directory", work)):
            argv += ["--" + name, str(value)]
        result = subprocess.run([*argv, *flags], capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(result.stdout, "")
        self.assertEqual(result.stderr, "Ubuntu candidate proof refused: " + expected + "\n")

    def test_nonempty_target_requires_explicit_cache_admission(self):
        (self.root / "target" / "old-object").write_bytes(b"old")
        work = self.root / "new-work"
        self.refuse(self.root / "target", work,
                    "nonempty target requires explicit --reuse-target-cache; cache origin is not authenticated")
        self.assertFalse(work.exists())

    def test_equal_and_descendant_target_cannot_write_source_alias(self):
        source = self.root / "source"
        (source / "child").mkdir()
        for target in (source, source / "child"):
            with self.subTest(target=target):
                work = self.root / "new-work"
                self.refuse(target, work, "writable roots must not overlap read-only source/tools/provider roots",
                            "--reuse-target-cache")
                self.assertFalse(work.exists())

    def test_target_ancestor_cannot_write_protected_tools(self):
        self.refuse(self.root, self.root / "new-work",
                    "writable roots must not overlap read-only source/tools/provider roots", "--reuse-target-cache")

    def test_work_ancestor_cannot_write_protected_source(self):
        self.refuse(self.root / "target", self.root,
                    "writable roots must not overlap read-only source/tools/provider roots")


class CandidateInventoryTests(unittest.TestCase):
    def test_closed_seven_file_bundle_rejects_extra_symlink_and_bad_checksum(self):
        spec = importlib.util.spec_from_file_location("ubuntu_candidate_under_test", SCRIPT)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = "renderflow-x86_64-unknown-linux-gnu"
            for name in (binary, "renderflow-sbom.spdx.json", "THIRD_PARTY_NOTICES.txt"):
                data = (name + " synthetic test").encode()
                (root / name).write_bytes(data)
                (root / (name + ".sha256")).write_text(hashlib.sha256(data).hexdigest() + "  " + name + "\n")
            (root / (binary + ".contract.json")).write_text("{}\n")
            self.assertEqual(len(module.candidate_inventory(root, binary)), 7)
            extra = root / "unexpected"
            extra.write_bytes(b"unselected")
            with self.assertRaisesRegex(ValueError, "exactly the seven"):
                module.candidate_inventory(root, binary)
            extra.unlink()
            contract = root / (binary + ".contract.json")
            contract.unlink()
            contract.symlink_to(root / binary)
            with self.assertRaisesRegex(ValueError, "regular nonlink"):
                module.candidate_inventory(root, binary)
            contract.unlink()
            contract.write_text("{}\n")
            (root / (binary + ".sha256")).write_text("0" * 64 + "  " + binary + "\n")
            with self.assertRaisesRegex(ValueError, "checksum differs"):
                module.candidate_inventory(root, binary)


if __name__ == "__main__":
    unittest.main()
