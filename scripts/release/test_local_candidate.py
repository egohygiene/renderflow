"""Synthetic local candidate admission tests; no product binary is executed."""
from argparse import Namespace
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import release_assets as release


class LocalCandidateTest(unittest.TestCase):
    def setUp(self):
        self.private = tempfile.TemporaryDirectory()
        self.addCleanup(self.private.cleanup)
        self.root = Path(self.private.name).resolve()
        self.binary = self.root / "renderflow"
        self.bytes = bytearray(64)
        self.bytes[:6] = b"\x7fELF\x02\x01"
        self.bytes[18:20] = (62).to_bytes(2, "little")
        self.binary.write_bytes(self.bytes)
        self.output = self.root / "receipt.json"
        self.args = Namespace(binary=str(self.binary), version="0.3.0-rc.1", output=str(self.output))
        self.host = patch("native_cohort.host_target", return_value=release.TARGET)
        self.host.start()
        self.addCleanup(self.host.stop)

    def smoke(self, selected, version):
        self.assertNotEqual(selected, self.binary)
        self.assertEqual(selected.read_bytes(), self.bytes)
        return {"version": version, "outputs": []}

    def test_captured_bytes_receipt_never_grants_release_or_host_authority(self):
        with patch.object(release, "smoke", side_effect=self.smoke):
            release.local_smoke(self.args)
        receipt = json.loads(self.output.read_text())
        self.assertEqual(receipt["binary"]["sha256"], hashlib.sha256(self.bytes).hexdigest())
        self.assertEqual(receipt["binary"]["bytes"], len(self.bytes))
        for name in ("source_authenticated", "release_authenticated", "supported_host_qualified",
                     "native_matrix_qualified", "publication_performed"):
            self.assertIs(receipt[name], False)
        self.assertEqual(self.output.stat().st_mode & 0o777, 0o600)

    def test_changed_selected_source_refuses_without_receipt(self):
        def change(selected, version):
            result = self.smoke(selected, version)
            self.binary.write_bytes(bytes(self.bytes) + b"changed")
            return result
        with patch.object(release, "smoke", side_effect=change):
            with self.assertRaisesRegex(ValueError, "changed during smoke"):
                release.local_smoke(self.args)
        self.assertFalse(self.output.exists())

    def test_existing_receipt_is_preserved_before_execution(self):
        self.output.write_bytes(b"prior evidence")
        with patch.object(release, "smoke") as smoke:
            with self.assertRaisesRegex(ValueError, "never overwrite"):
                release.local_smoke(self.args)
        smoke.assert_not_called()
        self.assertEqual(self.output.read_bytes(), b"prior evidence")

    def test_linked_binary_and_wrong_cpu_refuse_before_execution(self):
        alias = self.root / "alias"
        alias.symlink_to(self.binary)
        with patch.object(release, "smoke") as smoke:
            self.args.binary = str(alias)
            with self.assertRaisesRegex(ValueError, "canonical"):
                release.local_smoke(self.args)
            self.args.binary = str(self.binary)
            self.bytes[18:20] = (183).to_bytes(2, "little")
            self.binary.write_bytes(self.bytes)
            with self.assertRaisesRegex(ValueError, "CPU/class"):
                release.local_smoke(self.args)
        smoke.assert_not_called()

    def test_workload_failure_has_no_success_receipt(self):
        with patch.object(release, "smoke", side_effect=ValueError("fixture failed")):
            with self.assertRaisesRegex(ValueError, "fixture failed"):
                release.local_smoke(self.args)
        self.assertFalse(self.output.exists())

    def test_complete_staged_verifier_works_without_checkout_imports(self):
        empty = self.root / "empty"
        empty.mkdir()
        environment = dict(os.environ)
        environment.pop("PYTHONPATH", None)
        # Reproduce the old single-file clean-room mount failure.
        bare = empty / "verify.py"
        shutil.copyfile(release.__file__, bare)
        old = subprocess.run([sys.executable, "-B", str(bare), "--help"],
                             cwd=empty, env=environment, capture_output=True, text=True)
        self.assertNotEqual(old.returncode, 0)
        self.assertIn("native_cohort", old.stderr)
        bundle = self.root / "verifier"
        release.stage_verifier(Namespace(output_directory=str(bundle)))
        self.assertEqual({p.name for p in bundle.iterdir()},
                         {"release_assets.py", "native_cohort.py", "cohort_assets.py"})
        for path in bundle.iterdir():
            self.assertEqual(path.read_bytes(), (Path(release.__file__).parent / path.name).read_bytes())
        result = subprocess.run([sys.executable, "-B", str(bundle / "release_assets.py"), "--help"],
                                cwd=empty, env=environment, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("local-smoke", result.stdout)
        self.assertIn("cohort-verify", result.stdout)


if __name__ == "__main__":
    unittest.main()
