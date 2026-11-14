"""Source metadata regressions for the deferred packaging qualification pass.

These checks never invoke Cargo, installers, Snap or release tools.
"""
import importlib.util
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("packaging_check", ROOT / "scripts/check-packaging.py")
packaging = importlib.util.module_from_spec(spec)
spec.loader.exec_module(packaging)


class PackagingDeclarations(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        manifests = ["Cargo.toml", "Cargo.lock", "snap/snapcraft.yaml", "pkg/aur/renderflow/PKGBUILD"]
        manifests += [str(path.relative_to(ROOT)) for path in (ROOT / "crates").glob("*/Cargo.toml")]
        for name in manifests:
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, destination)

    def rewrite(self, name, before, after):
        path = self.root / name
        source = path.read_text()
        self.assertIn(before, source)
        path.write_text(source.replace(before, after, 1))

    def test_current_declarations_do_not_claim_runtime_qualification(self):
        result = packaging.check(self.root)
        self.assertEqual(result["qualification"], "source_metadata_only")
        self.assertFalse(result["installer_observed"])
        self.assertFalse(result["native_build_observed"])
        self.assertFalse(result["snap_observed"])
        self.assertIn("renderflow-api", result["packages"])

    def test_virtual_workspace_is_not_the_binary_target(self):
        self.rewrite("snap/snapcraft.yaml", 'rust-path: ["crates/renderflow-cli"]', 'rust-path: ["."]')
        with self.assertRaisesRegex(ValueError, "CLI manifest"):
            packaging.check(self.root)

    def test_cli_target_identity_is_checked_independently_of_package_name(self):
        self.rewrite("crates/renderflow-cli/Cargo.toml", 'name = "renderflow"', 'name = "another-tool"')
        with self.assertRaisesRegex(ValueError, "binary target"):
            packaging.check(self.root)

    def test_optional_workspace_api_cannot_escape_the_cargo_version_check(self):
        self.rewrite("Cargo.lock", 'name = "renderflow-api"\nversion = "0.3.0-rc.1"',
                     'name = "renderflow-api"\nversion = "99.0.0"')
        with self.assertRaisesRegex(ValueError, "renderflow-api"):
            packaging.check(self.root)

    def test_snap_cannot_derive_a_separate_git_version(self):
        self.rewrite("snap/snapcraft.yaml", "adopt-info: renderflow", "version: git")
        with self.assertRaisesRegex(ValueError, "Cargo version"):
            packaging.check(self.root)

    def test_aur_normalized_package_version_is_not_the_source_archive_root(self):
        self.rewrite("pkg/aur/renderflow/PKGBUILD", 'cd "$pkgname-$_source_version"', 'cd "$pkgname-$pkgver"')
        with self.assertRaisesRegex(ValueError, "exact source tag"):
            packaging.check(self.root)


if __name__ == "__main__":
    unittest.main()
