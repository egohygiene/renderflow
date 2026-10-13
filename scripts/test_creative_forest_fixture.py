#!/usr/bin/env python3
"""Source-authored unit cases for the creative fixture evidence reader.

These cases have not been run as part of #430 authoring. A later testing session
can run ``python3 scripts/test_creative_forest_fixture.py``. They exercise only
synthetic temporary files and do not launch Renderflow or any native provider.
"""

import copy
from pathlib import Path
import tempfile
import unittest

import run_creative_forest_fixture as fixture


class PublicationEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bundle = self.root / "jobs" / "recovery" / "bundle"
        self.bundle.mkdir(parents=True)
        self.payload = self.bundle / "recovered.fountain"
        self.payload.write_bytes(b"INT. LAB - DAY\n\nA lamp flickers.\n")
        data = self.payload.read_bytes()
        self.artifact_id = "sha256:" + fixture.sha256(data)
        self.target_id = "synthetic-recovery-target"
        self.manifest = {
            "schema_version": "renderflow.run/v1",
            "run_id": "synthetic-run",
            "state": "complete",
            "artifact_manifest": {
                "run_id": "synthetic-run",
                "output_dir": str(self.bundle),
                "outputs": [str(self.payload)],
                "artifacts": [{
                    "artifact_id": self.artifact_id,
                    "lifecycle": "terminal",
                    "locator": "bundle:recovered.fountain",
                    "digest": {"algorithm": "sha256", "value": fixture.sha256(data)},
                    "size_bytes": len(data),
                    "validation": "valid",
                    "metadata": {
                        "renderflow.target.identity": self.target_id,
                        "renderflow.target.published": True,
                    },
                }],
            },
            "artifact_forest": {
                "schema": "renderflow/artifact-forest-v1",
                "branches": [{
                    "target_id": self.target_id,
                    "format": "fountain",
                    "state": "selected",
                    "route": {
                        "source_identity": False,
                        "steps": [{"from": "text", "to": "fountain"}],
                    },
                }],
                "produced_artifacts": [self.artifact_id],
                "execution": {
                    "schema": "renderflow/forest-execution-v1",
                    "targets": [{
                        "target_id": self.target_id,
                        "outcome": "published",
                        "source_identity": False,
                        "artifact_evidence_index": 0,
                        "artifact_id": self.artifact_id,
                        "locator": "bundle:recovered.fountain",
                    }],
                },
            },
        }

    def publications(self, manifest=None):
        return fixture.published_artifacts(
            self.manifest if manifest is None else manifest, self.bundle, self.root
        )

    def test_published_payload_is_linked_to_native_record_and_actual_bytes(self):
        published = self.publications()
        self.assertEqual(len(published), 1)
        self.assertEqual(published[0]["target_id"], self.target_id)
        self.assertEqual(published[0]["artifact_evidence_index"], 0)
        self.assertEqual(published[0]["file"], {
            "path": "jobs/recovery/bundle/recovered.fountain",
            "sha256": fixture.sha256(self.payload.read_bytes()),
            "size_bytes": self.payload.stat().st_size,
        })

    def test_duplicate_missing_and_unknown_execution_rows_are_refused(self):
        original = self.manifest["artifact_forest"]["execution"]["targets"][0]
        for rows in ([original, original], [], [{**original, "target_id": "unknown"}]):
            with self.subTest(rows=rows):
                mutated = copy.deepcopy(self.manifest)
                mutated["artifact_forest"]["execution"]["targets"] = rows
                with self.assertRaisesRegex(ValueError, "execution target identities"):
                    self.publications(mutated)

    def test_published_record_requires_matching_identity_and_publication_marker(self):
        for key, value in (
            ("renderflow.target.identity", "different-target"),
            ("renderflow.target.published", False),
            ("renderflow.target.published", 1),
        ):
            with self.subTest(key=key, value=value):
                mutated = copy.deepcopy(self.manifest)
                mutated["artifact_manifest"]["artifacts"][0]["metadata"][key] = value
                with self.assertRaisesRegex(ValueError, "matching native terminal evidence"):
                    self.publications(mutated)

    def test_published_record_must_be_selected_and_terminal(self):
        mutated = copy.deepcopy(self.manifest)
        mutated["artifact_forest"]["branches"][0]["state"] = "blocked"
        with self.assertRaisesRegex(ValueError, "selected target"):
            self.publications(mutated)
        mutated = copy.deepcopy(self.manifest)
        mutated["artifact_manifest"]["artifacts"][0]["lifecycle"] = "intermediate"
        with self.assertRaisesRegex(ValueError, "native terminal evidence"):
            self.publications(mutated)

    def test_existing_payload_without_native_output_membership_is_refused(self):
        self.manifest["artifact_manifest"]["outputs"] = []
        with self.assertRaisesRegex(ValueError, "native output membership"):
            self.publications()

    def test_native_run_identity_and_output_directory_must_match(self):
        for key, value, message in (
            ("run_id", "different-run", "identity mismatch"),
            ("output_dir", str(self.root), "different output directory"),
        ):
            with self.subTest(key=key):
                mutated = copy.deepcopy(self.manifest)
                mutated["artifact_manifest"][key] = value
                with self.assertRaisesRegex(ValueError, message):
                    self.publications(mutated)

    def test_artifact_reference_requires_an_integer_in_range(self):
        for index in (True, -1, 1, "0", None):
            with self.subTest(index=index):
                mutated = copy.deepcopy(self.manifest)
                mutated["artifact_forest"]["execution"]["targets"][0]["artifact_evidence_index"] = index
                with self.assertRaisesRegex(ValueError, "no native artifact reference"):
                    self.publications(mutated)

    def test_changed_bytes_are_refused_even_when_payload_still_exists(self):
        self.payload.write_bytes(b"Different bytes of an otherwise valid-looking screenplay.\n")
        with self.assertRaisesRegex(ValueError, "differs from native artifact evidence"):
            self.publications()

    def test_native_digest_and_size_claims_are_verified(self):
        for key, value in (
            ("digest", {"algorithm": "sha256", "value": "0" * 64}),
            ("size_bytes", 0),
        ):
            with self.subTest(key=key):
                mutated = copy.deepcopy(self.manifest)
                mutated["artifact_manifest"]["artifacts"][0][key] = value
                with self.assertRaisesRegex(ValueError, "differs from native artifact evidence"):
                    self.publications(mutated)

    def test_invalid_artifact_cannot_become_a_successful_publication(self):
        self.manifest["artifact_manifest"]["artifacts"][0]["validation"] = "invalid"
        with self.assertRaisesRegex(ValueError, "invalid artifact"):
            self.publications()

    def test_unpublished_branch_does_not_invent_a_payload(self):
        forest = self.manifest["artifact_forest"]
        forest["branches"].append({"target_id": "blocked-target", "format": "fdx", "state": "blocked"})
        forest["execution"]["targets"].append({
            "target_id": "blocked-target", "outcome": "not_published", "source_identity": False,
        })
        self.assertEqual([item["target_id"] for item in self.publications()], [self.target_id])

    def test_source_identity_is_published_without_becoming_a_derivative(self):
        forest = self.manifest["artifact_forest"]
        forest["branches"][0]["route"] = {"source_identity": True, "steps": []}
        forest["execution"]["targets"][0]["source_identity"] = True
        forest["produced_artifacts"] = []
        self.assertTrue(self.publications()[0]["source_identity"])
        forest["produced_artifacts"] = [self.artifact_id]
        with self.assertRaisesRegex(ValueError, "derivative membership"):
            self.publications()

    def test_derivative_claim_cannot_exist_without_publication(self):
        self.manifest["artifact_forest"]["execution"]["targets"][0]["outcome"] = "not_published"
        with self.assertRaisesRegex(ValueError, "derivative membership"):
            self.publications()

    def test_empty_forest_does_not_satisfy_an_expected_target(self):
        job = {"expected_targets": [{"target_id": self.target_id, "format": "fountain"}]}
        with self.assertRaisesRegex(ValueError, "selected target membership"):
            fixture.expected_membership(job, {"branches": []}, [])

    def test_selected_target_without_publication_does_not_satisfy_execution(self):
        job = {"expected_targets": [{"target_id": self.target_id, "format": "fountain"}]}
        with self.assertRaisesRegex(ValueError, "published target membership"):
            fixture.expected_membership(job, self.manifest["artifact_forest"], [])

    def test_source_identity_cannot_satisfy_a_derivative_only_job(self):
        job = {
            "expected_targets": [{"target_id": self.target_id, "format": "fountain"}],
            "forbid_source_identity": True,
        }
        self.manifest["artifact_forest"]["branches"][0]["route"]["source_identity"] = True
        with self.assertRaisesRegex(ValueError, "original as a derivative"):
            fixture.expected_membership(job, self.manifest["artifact_forest"])


class EvidenceInputTests(unittest.TestCase):
    def test_duplicate_json_fields_are_refused_including_nested_fields(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "evidence.json"
            for data in ('{"schema":1,"schema":2}', '{"nested":{"digest":1,"digest":2}}'):
                with self.subTest(data=data):
                    path.write_text(data, encoding="utf-8")
                    with self.assertRaisesRegex(ValueError, "duplicate JSON field"):
                        fixture.read_json(path)

    def test_evidence_paths_cannot_escape_the_bundle(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for relative in ("../outside", "nested/../../outside", str(root / "absolute")):
                with self.subTest(relative=relative):
                    with self.assertRaisesRegex(ValueError, "escapes"):
                        fixture.within(root, relative)

    def test_evidence_paths_cannot_traverse_a_symlink(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / "target"
            target.mkdir()
            link = root / "link"
            try:
                link.symlink_to(target, target_is_directory=True)
            except (NotImplementedError, OSError) as error:
                self.skipTest("this platform cannot create a test symlink: " + str(error))
            with self.assertRaisesRegex(ValueError, "symlink evidence paths"):
                fixture.within(root, "link/payload")

    def test_selected_routes_cannot_repeat_formats_or_disconnect(self):
        for steps, message in (
            ([{"from": "srt", "to": "timed_text_json"}, {"from": "timed_text_json", "to": "srt"}], "cycle"),
            ([{"from": "srt", "to": "timed_text_json"}, {"from": "fountain", "to": "pdf"}], "disconnected"),
        ):
            with self.subTest(steps=steps):
                forest = {
                    "schema": "renderflow/artifact-forest-v1",
                    "branches": [{"target_id": "synthetic-target", "route": {"steps": steps}}],
                }
                with self.assertRaisesRegex(ValueError, message):
                    fixture.forest_invariants(forest)

    def test_duplicate_effective_branch_identities_are_refused(self):
        forest = {
            "schema": "renderflow/artifact-forest-v1",
            "branches": [{"target_id": "repeated"}, {"target_id": "repeated"}],
        }
        with self.assertRaisesRegex(ValueError, "duplicate effective target identity"):
            fixture.forest_invariants(forest)


if __name__ == "__main__":
    unittest.main()
