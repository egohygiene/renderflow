#!/usr/bin/env python3
"""Source-authored unit cases for the creative fixture evidence reader.

These cases have not been run as part of #430 authoring. A later testing session
can run ``python3 scripts/test_creative_forest_fixture.py``. They exercise only
synthetic temporary files and do not launch Renderflow or any native provider.
"""

import copy
from contextlib import redirect_stderr, redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

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


class OriginalBodyEvidenceTests(unittest.TestCase):
    """Synthetic declarations check envelope refusal, never native format fidelity."""

    def setUp(self):
        self.source = {"sha256": "1" * 64, "size_bytes": 32}
        self.policy = {"allow_unqualified": True}
        original_id = "artifact:sha256:" + "a" * 64
        canonical_id = "artifact:sha256:" + "b" * 64
        self.canonical_digest = "2" * 64
        self.original = {"id": original_id, "format": "fountain",
                         "digest": {"algorithm": "sha256", "value": self.source["sha256"]},
                         "size_bytes": self.source["size_bytes"], "sources": []}
        canonical = {"id": canonical_id, "format": "screenplay_json",
                     "digest": {"algorithm": "sha256", "value": self.canonical_digest},
                     "sources": [original_id]}
        source_record = {"artifact_id": original_id, "lifecycle": "source",
                         "format": "fountain", "digest": self.original["digest"],
                         "size_bytes": self.source["size_bytes"], "sources": []}
        self.manifest = {"artifact_manifest": {"artifacts": [source_record]},
                         "artifact_forest": {"branches": []}}
        self.published = []
        for output_format, artifact_id, digest in (
            ("screenplay_json", canonical_id, self.canonical_digest),
            ("trelby", "artifact:sha256:" + "c" * 64, "3" * 64),
        ):
            evidence = {
                "schema_version": "renderflow.screenplay-body-artifact/v1",
                "profile": "strict-six-kind-body-v1", "qualification": "authored_unqualified",
                "runtime_qualified": False, "review_required": True,
                "original_source": copy.deepcopy(self.original),
                "input_artifact_id": original_id, "input_sha256": self.source["sha256"],
                "canonical_artifact": copy.deepcopy(canonical), "canonical_sha256": self.canonical_digest,
                "output_format": output_format, "output_sha256": digest,
                "policy_sha256": fixture.sha256(json.dumps(self.policy, separators=(",", ":")).encode()),
            }
            records = self.manifest["artifact_manifest"]["artifacts"]
            index = len(records)
            records.append({"artifact_id": artifact_id, "format": output_format,
                            "sources": [original_id], "metadata": {"renderflow.screenplay_body": evidence}})
            self.published.append({"target_id": "synthetic-" + output_format,
                "artifact_id": artifact_id, "artifact_evidence_index": index,
                "source_identity": False, "format": output_format,
                "file": {"sha256": digest}})
            self.manifest["artifact_forest"]["branches"].append({
                "target_id": "synthetic-" + output_format,
                "route": {"steps": [{"from": "fountain", "to": output_format}]}})

    def review(self):
        fixture.original_body_evidence(self.manifest, self.published, self.source, self.policy)

    def evidence(self):
        return self.manifest["artifact_manifest"]["artifacts"][2]["metadata"]["renderflow.screenplay_body"]

    def test_direct_original_declarations_remain_unqualified(self):
        self.review()
        self.assertIs(self.evidence()["runtime_qualified"], False)

    def test_runtime_or_review_promotion_is_refused(self):
        for key, value in (("runtime_qualified", True), ("runtime_qualified", 0),
                           ("review_required", False), ("qualification", "qualified")):
            with self.subTest(key=key, value=value):
                original = copy.deepcopy(self.evidence())
                self.evidence()[key] = value
                with self.assertRaisesRegex(ValueError, "unqualified review declarations"):
                    self.review()
                self.evidence().update(original)

    def test_changed_original_and_output_evidence_are_refused(self):
        self.evidence()["original_source"]["digest"]["value"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "different original source"):
            self.review()
        self.evidence()["original_source"] = copy.deepcopy(self.original)
        self.evidence()["output_sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "output or policy identity"):
            self.review()

    def test_native_reference_must_match_the_actual_canonical_publication(self):
        self.evidence()["canonical_sha256"] = "0" * 64
        self.evidence()["canonical_artifact"]["digest"]["value"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "retained canonical reference"):
            self.review()

    def test_recovered_or_indirect_route_cannot_satisfy_original_body_job(self):
        self.manifest["artifact_forest"]["branches"][1]["route"]["steps"] = [
            {"from": "fountain", "to": "screenplay_json"},
            {"from": "screenplay_json", "to": "trelby"},
        ]
        with self.assertRaisesRegex(ValueError, "one direct Fountain edge"):
            self.review()

    def test_original_body_metadata_namespace_must_be_an_object(self):
        self.manifest["artifact_manifest"]["artifacts"][2]["metadata"]["renderflow.screenplay_body"] = []
        with self.assertRaisesRegex(ValueError, "object metadata namespace"):
            self.review()


class TranscriptTextEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.source = {"sha256": "1" * 64}
        self.canonical_id = "artifact:sha256:" + "a" * 64
        self.canonical_digest = "2" * 64
        source = {"sha256": self.source["sha256"], "format": "srt"}
        self.evidence = {
            "schema_version": "renderflow.subtitle-artifact/v1", "operation": "export",
            "input_artifact_id": self.canonical_id, "input_sha256": self.canonical_digest,
            "canonical_transcript": {"artifact_id": self.canonical_id, "sha256": self.canonical_digest},
            "source": copy.deepcopy(source), "language": None, "prior_losses": [],
            "export_losses": ["source_provenance", "overlap_policy", "timing", "cue_ids"],
        }
        self.parent_evidence = {"source": source, "language": None, "prior_losses": []}
        self.manifest = {"artifact_manifest": {"artifacts": [
            {"artifact_id": self.canonical_id, "format": "timed_transcript_json",
             "digest": {"algorithm": "sha256", "value": self.canonical_digest},
             "metadata": {"renderflow.subtitle": self.parent_evidence}},
            {"artifact_id": "synthetic-text", "format": "text", "sources": [self.canonical_id],
             "fidelity": "partial", "metadata": {"renderflow.subtitle": self.evidence}},
        ]}}
        self.published = [{"format": "text", "artifact_evidence_index": 1}]

    def review(self):
        fixture.transcript_text_evidence(self.manifest, self.published, self.source)

    def test_text_retains_canonical_parent_and_timing_loss_declarations(self):
        self.review()

    def test_missing_canonical_parent_or_changed_source_is_refused(self):
        self.manifest["artifact_manifest"]["artifacts"][0]["digest"]["value"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "retained canonical parent"):
            self.review()
        self.manifest["artifact_manifest"]["artifacts"][0]["digest"]["value"] = self.canonical_digest
        self.evidence["source"]["sha256"] = "0" * 64
        with self.assertRaisesRegex(ValueError, "original provenance"):
            self.review()

    def test_timing_loss_and_partial_fidelity_cannot_be_silenced(self):
        self.evidence["export_losses"].remove("timing")
        with self.assertRaisesRegex(ValueError, "timing or provenance losses"):
            self.review()
        self.evidence["export_losses"].append("timing")
        self.manifest["artifact_manifest"]["artifacts"][1]["fidelity"] = "lossless"
        with self.assertRaisesRegex(ValueError, "partial-fidelity"):
            self.review()

    def test_language_and_prior_loss_omissions_require_disclosure(self):
        self.evidence["language"] = "en"
        self.parent_evidence["language"] = "en"
        with self.assertRaisesRegex(ValueError, "language-loss"):
            self.review()
        self.evidence["export_losses"].append("language")
        self.evidence["prior_losses"] = ["speakers"]
        self.parent_evidence["prior_losses"] = ["speakers"]
        with self.assertRaisesRegex(ValueError, "prior-loss disclosure"):
            self.review()
        self.evidence["export_losses"].append("prior_loss_evidence")
        self.review()

    def test_optional_stream_and_metadata_namespace_are_typed(self):
        self.evidence["source"]["stream"] = None
        self.parent_evidence["source"]["stream"] = None
        self.review()
        self.evidence["source"]["stream"] = []
        with self.assertRaisesRegex(ValueError, "optional stream must be an object"):
            self.review()
        self.evidence["source"].pop("stream")
        self.manifest["artifact_manifest"]["artifacts"][1]["metadata"]["renderflow.subtitle"] = []
        with self.assertRaisesRegex(ValueError, "object metadata namespace"):
            self.review()


class FixtureDeclarationTests(unittest.TestCase):
    def test_original_job_uses_preserved_fountain_without_native_configuration(self):
        request = fixture.read_json(fixture.FIXTURE / "fixture.json")
        job = next(job for job in request["jobs"] if job["id"] == "original-body")
        self.assertFalse(job.get("native", False))
        self.assertEqual(job["source"], "sources/original.fountain")
        spec = fixture.read_json(fixture.FIXTURE / job["spec"])
        self.assertEqual(spec["sources"][0]["format"], "fountain")
        self.assertEqual(spec["execution"]["screenplay_body"], {"allow_unqualified": True})
        self.assertEqual({target["format"] for target in spec["targets"]["exact"]},
                         {"screenplay_json", "fdx", "fadein", "osf", "trelby"})

    def test_timed_graph_and_exhaustive_cycle_expect_terminal_text(self):
        request = fixture.read_json(fixture.FIXTURE / "fixture.json")
        for job_id in ("timed-text", "cycle-suppression"):
            job = next(job for job in request["jobs"] if job["id"] == job_id)
            self.assertIn("text", {target["format"] for target in job["expected_targets"]})


class FixtureSelectionTests(unittest.TestCase):
    def setUp(self):
        self.request = fixture.read_json(fixture.FIXTURE / "fixture.json")
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name).resolve()
        # Never executable: main only hashes this marker; dispatch is mocked.
        self.binary = self.root / "binary-marker"
        self.binary.write_bytes(b"synthetic binary identity; not a program\n")

    def test_default_and_negative_selection_preserve_catalog_and_do_not_expand_explicit_jobs(self):
        original = copy.deepcopy(self.request)
        ordinary = fixture.select_request(self.request, None, False)
        self.assertTrue(ordinary["jobs"])
        self.assertFalse(any(job.get("negative") for job in ordinary["jobs"]))
        self.assertIn("speech", [job["id"] for job in ordinary["jobs"]])
        self.assertEqual(fixture.select_request(self.request, None, True), self.request)
        selected = fixture.select_request(self.request, ["original-body"], True)
        self.assertEqual([job["id"] for job in selected["jobs"]], ["original-body"])
        self.assertEqual(self.request, original)

    def test_describe_is_static_and_uses_catalog_order(self):
        output = self.root / "must-not-exist"
        stdout = io.StringIO()
        with mock.patch.object(fixture, "run_child", side_effect=AssertionError("unexpected child")), \
                mock.patch.object(fixture, "run_job") as dispatch, redirect_stdout(stdout):
            code = fixture.main(["--mode", "describe", "--job", "timed-text",
                                 "--job", "original-body", "--output", str(output),
                                 "--renderflow", str(self.root / "missing-binary")])
        self.assertEqual(code, 0)
        self.assertEqual([job["id"] for job in json.loads(stdout.getvalue())["jobs"]],
                         ["original-body", "timed-text"])
        dispatch.assert_not_called()
        self.assertFalse(output.exists())

    def test_invalid_selection_refuses_before_output_or_dispatch(self):
        for selection, diagnostic in (
            (["--job", "does-not-exist"], "unknown --job IDs"),
            (["--job", "original-body", "--job", "original-body"], "must not be repeated"),
            (["--job", "malformed-recovery"], "requires --include-negatives"),
        ):
            with self.subTest(selection=selection):
                output = self.root / "refused"
                stderr = io.StringIO()
                with mock.patch.object(fixture, "run_job") as dispatch, redirect_stderr(stderr):
                    with self.assertRaises(SystemExit) as refusal:
                        fixture.main(["--mode", "execute", "--output", str(output),
                                      "--renderflow", str(self.root / "missing-binary"), *selection])
                self.assertEqual(refusal.exception.code, 2)
                self.assertIn(diagnostic, stderr.getvalue())
                dispatch.assert_not_called()
                self.assertFalse(output.exists())

    def test_selected_plan_and_execution_retain_the_same_request_as_describe(self):
        selection = ["--job", "timed-text", "--job", "original-body"]
        described = io.StringIO()
        with redirect_stdout(described):
            self.assertEqual(fixture.main(["--mode", "describe", *selection]), 0)
        for mode in ("plan", "execute"):
            with self.subTest(mode=mode):
                output = self.root / mode
                dispatched = []

                def observe_dispatch(job, native, binary, actual_mode, root, timeout):
                    dispatched.append(job["id"])
                    self.assertEqual(native, {})
                    self.assertEqual(actual_mode, mode)
                    return {"id": job["id"], "status": "planned" if mode == "plan" else "completed",
                            "expected": job.get("expected", "success"),
                            "evidence": [], "published_artifacts": []}

                with mock.patch.object(fixture, "run_job", side_effect=observe_dispatch), \
                        mock.patch.object(fixture, "run_child", side_effect=AssertionError("unexpected child")), \
                        redirect_stdout(io.StringIO()):
                    code = fixture.main(["--mode", mode, "--renderflow", str(self.binary),
                                         "--output", str(output), *selection])
                self.assertEqual(code, 0)
                self.assertEqual(dispatched, ["original-body", "timed-text"])
                retained = fixture.read_json(output / "request.json")
                self.assertEqual(retained, json.loads(described.getvalue()))
                report = fixture.read_json(output / "result.json")
                self.assertEqual([job["id"] for job in report["jobs"]], dispatched)
                self.assertEqual(report["request"]["sha256"],
                                 fixture.sha256((output / "request.json").read_bytes()))
                self.assertEqual(fixture.read_json(output / "started.json")["request"], report["request"])
                self.assertEqual(report["qualification"], "unqualified")
                self.assertFalse(report["coverage_complete"])
                self.assertFalse((output / "speech.result.json").exists())

    def test_native_configuration_for_unselected_jobs_is_refused(self):
        native = self.root / "native.json"
        native.write_text(json.dumps({"speech": {"policy": {}}}), encoding="utf-8")
        output = self.root / "refused-native"
        stderr = io.StringIO()
        with mock.patch.object(fixture, "run_job") as dispatch, redirect_stderr(stderr):
            with self.assertRaises(SystemExit) as refusal:
                fixture.main(["--mode", "execute", "--job", "original-body",
                              "--renderflow", str(self.root / "missing-binary"),
                              "--native-config", str(native), "--output", str(output)])
        self.assertEqual(refusal.exception.code, 2)
        self.assertIn("keys not used by selected jobs: speech", stderr.getvalue())
        dispatch.assert_not_called()
        self.assertFalse(output.exists())

    def test_selected_negative_accepts_its_shared_native_configuration_only(self):
        native = self.root / "native.json"
        configuration = {"speech": {"policy": {}}}
        native.write_text(json.dumps(configuration), encoding="utf-8")
        output = self.root / "negative"
        result = {"id": "transcription-unavailable", "status": "expected_refusal",
                  "expected": "refusal", "evidence": [], "published_artifacts": []}
        with mock.patch.object(fixture, "run_job", return_value=result) as dispatch, \
                mock.patch.object(fixture, "run_child", side_effect=AssertionError("unexpected child")), \
                redirect_stdout(io.StringIO()):
            code = fixture.main(["--mode", "execute", "--job", "transcription-unavailable",
                                 "--include-negatives", "--renderflow", str(self.binary),
                                 "--native-config", str(native), "--output", str(output)])
        self.assertEqual(code, 0)
        dispatch.assert_called_once()
        self.assertEqual(dispatch.call_args.args[0]["id"], "transcription-unavailable")
        self.assertEqual(dispatch.call_args.args[1], configuration)
        self.assertEqual([job["id"] for job in fixture.read_json(output / "request.json")["jobs"]],
                         ["transcription-unavailable"])
        self.assertFalse((output / "speech.result.json").exists())

    def test_selected_unconfigured_native_job_is_distinct_from_unselected_jobs(self):
        output = self.root / "unconfigured"
        with mock.patch.object(fixture, "run_child", side_effect=AssertionError("unexpected child")), \
                redirect_stdout(io.StringIO()):
            code = fixture.main(["--mode", "execute", "--job", "speech",
                                 "--renderflow", str(self.binary), "--output", str(output)])
        self.assertEqual(code, 0)
        report = fixture.read_json(output / "result.json")
        self.assertEqual(len(report["jobs"]), 1)
        self.assertEqual(report["jobs"][0]["id"], "speech")
        self.assertEqual(report["jobs"][0]["status"], "skipped_unconfigured")
        self.assertNotIn("process", report["jobs"][0])
        self.assertEqual(report["jobs"][0]["published_artifacts"], [])
        self.assertFalse(report["coverage_complete"])
        self.assertFalse((output / "original-body.result.json").exists())


if __name__ == "__main__":
    unittest.main()
