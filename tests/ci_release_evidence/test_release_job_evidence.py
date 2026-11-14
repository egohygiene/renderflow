"""Mocked read-only policy checks; never dispatch workflows or run workloads."""

from copy import deepcopy
import base64
from datetime import datetime, timedelta, timezone
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from urllib.parse import parse_qs, urlparse
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("release_workload_guard", ROOT / "scripts/ci_release_evidence.py")
guard = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(guard)
REVISION = "a" * 40
NOW = datetime(2026, 11, 13, 12, tzinfo=timezone.utc)


def run_record(identity=10, workflow=".github/workflows/ci.yml", event="push", revision=REVISION, repository="egohygiene/renderflow"):
    return {"id": identity, "workflow_id": 3, "run_attempt": 1, "head_sha": revision,
            "head_branch": "main", "event": event, "status": "completed", "conclusion": "success",
            "path": workflow, "repository": {"full_name": repository}, "head_repository": {"full_name": repository},
            "created_at": (NOW - timedelta(days=1)).isoformat(), "updated_at": NOW.isoformat()}


def job_records(run, specification):
    jobs = []
    for number, entry in enumerate(specification["jobs"]):
        jobs.append({"id": 100 + number, "run_id": run["id"], "run_attempt": run["run_attempt"], "head_sha": run["head_sha"],
                     "name": entry["names"][0], "status": "completed", "conclusion": "success",
                     "steps": [{"name": name, "number": index + 1, "status": "completed", "conclusion": "success"}
                               for index, name in enumerate(entry["steps"])]})
    for name in specification.get("allowed_skipped", []):
        jobs.append({"id": 100 + len(jobs), "run_id": run["id"], "run_attempt": run["run_attempt"], "head_sha": run["head_sha"],
                     "name": name, "status": "completed", "conclusion": "skipped", "steps": []})
    return jobs


class GitHub:
    def __init__(self):
        self.runs, self.jobs, self.calls = {}, {}, []

    def add(self, run, specification):
        self.runs[run["id"]] = run
        self.jobs[run["id"]] = job_records(run, specification)

    def __call__(self, path):
        self.calls.append(path)
        url, query = urlparse(path), parse_qs(urlparse(path).query)
        pieces = url.path.split("/")
        if "/contents/" in url.path:
            assert query["ref"][0] in [run["head_sha"] for run in self.runs.values()]
            name = url.path.split("/contents/", 1)[1]
            content = (ROOT / name).read_bytes()
            return {"path": name, "type": "file", "encoding": "base64", "size": len(content),
                    "content": base64.b64encode(content).decode("ascii")}
        elif "/actions/workflows/" in url.path:
            records = [run for run in self.runs.values() if run["path"].endswith("/" + pieces[-2])]
            records = [run for run in records if run["event"] == query["event"][0] and
                       ("head_sha" not in query or run["head_sha"] == query["head_sha"][0])]
            field = "workflow_runs"
        elif url.path.endswith("/jobs"):
            records, field = self.jobs[int(pieces[-2])], "jobs"
            assert query["filter"] == ["latest"]
        else:
            return deepcopy(self.runs[int(pieces[-1])])
        page = int(query.get("page", [1])[0])
        return {"total_count": len(records), field: deepcopy(records[(page - 1) * 100:page * 100])}


class WorkloadTests(unittest.TestCase):
    def setUp(self):
        self.policy = guard.source_policy(ROOT)
        self.repository = self.policy["repository"]
        self.spec = next(iter(self.policy["workflows"].values()))
        self.run = run_record(workflow=self.spec["path"], event=self.spec["event"], repository=self.repository)
        self.api = GitHub()
        self.api.add(self.run, self.spec)

    def check(self):
        return guard.workload(self.api, self.repository, self.run, self.spec)

    def test_success_requires_every_named_job_and_critical_workload_step(self):
        report = self.check()
        self.assertEqual(len(report["jobs"]), len(self.spec["jobs"]))
        self.assertEqual(report["source_revision"], REVISION)
        self.assertTrue(all("per_page=100&page=1" in path for path in self.api.calls if "/jobs?" in path))

    def test_empty_all_skipped_and_policy_only_success_refuse(self):
        originals = deepcopy(self.api.jobs[self.run["id"]])
        variants = [[], [{"id": 99, "run_id": 10, "head_sha": REVISION, "run_attempt": 1,
                         "name": "Budget policy", "status": "completed", "conclusion": "success", "steps": []}]]
        skipped = deepcopy(originals)
        for job in skipped:
            job["conclusion"] = "skipped"
        variants.append(skipped)
        for jobs in variants:
            with self.subTest(jobs=bool(jobs)):
                self.api.jobs[self.run["id"]] = jobs
                with self.assertRaises(ValueError):
                    self.check()

    def test_missing_duplicate_and_extra_job_identities_refuse(self):
        originals = deepcopy(self.api.jobs[self.run["id"]])
        duplicate = deepcopy(originals[0])
        duplicate["id"] += 10000
        unknown = deepcopy(duplicate)
        unknown["name"] = "Source-only formatter"
        for jobs in (originals[1:], originals + [duplicate], originals + [unknown], originals + [originals[0]]):
            self.api.jobs[self.run["id"]] = jobs
            with self.assertRaises(ValueError):
                self.check()

    def test_missing_skipped_failed_or_duplicate_critical_steps_refuse(self):
        original = deepcopy(self.api.jobs[self.run["id"]])
        for mutation in ("missing", "skipped", "failure", "duplicate"):
            jobs = deepcopy(original)
            step = jobs[0]["steps"][0]
            if mutation == "missing":
                jobs[0]["steps"] = jobs[0]["steps"][1:]
            elif mutation == "duplicate":
                jobs[0]["steps"].append(deepcopy(step))
            else:
                step["conclusion"] = mutation
            self.api.jobs[self.run["id"]] = jobs
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                self.check()

    def test_job_run_sha_and_attempt_must_match(self):
        for key, value in (("run_id", 11), ("head_sha", "b" * 40), ("run_attempt", 2)):
            original = deepcopy(self.api.jobs[self.run["id"]][0])
            self.api.jobs[self.run["id"]][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.check()
            self.api.jobs[self.run["id"]][0] = original

    def test_changed_run_during_collection_refuses(self):
        stale = deepcopy(self.run)
        self.api.runs[self.run["id"]]["run_attempt"] += 1
        with self.assertRaises(ValueError):
            guard.workload(self.api, self.repository, stale, self.spec)

    def test_source_workflow_and_target_drift_refuse(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            (root / "scripts/ci-release-evidence-policy.json").write_text(json.dumps(self.policy))
            for name in self.policy["source_sha256"]:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((ROOT / name).read_bytes())
            self.assertEqual(guard.source_policy(root), self.policy)
            for name in self.policy["source_sha256"]:
                target = root / name
                original = target.read_bytes()
                target.write_bytes(original + b"\n")
                with self.subTest(source=name), self.assertRaisesRegex(ValueError, "source drift"):
                    guard.source_policy(root)
                target.write_bytes(original)

    def test_run_identity_and_full_source_validation_refuse(self):
        for key, value in (("status", "in_progress"), ("conclusion", "skipped"), ("head_sha", "b" * 40),
                           ("head_branch", "other"), ("path", ".github/workflows/workflow-lint.yml"),
                           ("event", "pull_request"), ("repository", {"full_name": "other/repo"}),
                           ("head_repository", {"full_name": "other/repo"}), ("run_attempt", 0)):
            run = deepcopy(self.run)
            run[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                guard.validate_run(run, self.repository, self.spec["path"], self.spec["event"], REVISION)

    def test_complete_pagination_and_negative_truncated_counts_duplicates(self):
        records = [{"id": i + 1} for i in range(101)]
        calls = []
        def full(path):
            calls.append(path)
            page = int(parse_qs(urlparse(path).query)["page"][0])
            return {"total_count": 101, "jobs": records[(page - 1) * 100:page * 100]}
        self.assertEqual(guard.paginated(full, "jobs?filter=latest", "jobs"), records)
        self.assertEqual(len(calls), 2)
        variants = [lambda _: {"total_count": 2, "jobs": [{"id": 1}]},
                    lambda _: {"total_count": 1001, "jobs": []},
                    lambda _: {"total_count": True, "jobs": []},
                    lambda _: {"total_count": 2, "jobs": [{"id": 1}, {"id": 1}]},
                    lambda p: {"total_count": 101 if "page=1" in p else 102,
                               "jobs": records[:100] if "page=1" in p else [{"id": 101}]}]
        for api in variants:
            with self.assertRaises(ValueError):
                guard.paginated(api, "jobs", "jobs")

    def test_latest_failed_run_cannot_fall_back_to_older_green(self):
        newer = deepcopy(self.run)
        newer.update(id=11, created_at=NOW.isoformat(), conclusion="failure")
        self.api.add(newer, self.spec)
        with self.assertRaisesRegex(ValueError, "did not complete"):
            guard.newest_run(self.api, self.repository, self.spec["path"], self.spec["event"], REVISION)

    def test_oversized_api_capture_refuses(self):
        with patch.object(guard, "MAX_BYTES", 64):
            with self.assertRaisesRegex(ValueError, "byte budget"):
                guard.paginated(self.api, "repos/" + self.repository + "/actions/runs/10/jobs?filter=latest", "jobs")

    def test_candidate_matrix_complete_and_each_declared_target_required(self):
        if self.repository != "egohygiene/aniflow":
            self.skipTest("candidate consumer exists in Aniflow")
        self.assertEqual(len([entry for entry in self.spec["jobs"] if entry["identity"].startswith("native:")]), 3)
        guard.aniflow_candidate(self.api, self.run, REVISION, ROOT)
        original = deepcopy(self.api.jobs[self.run["id"]])
        for index in (1, 2, 3):
            self.api.jobs[self.run["id"]] = original[:index] + original[index + 1:]
            with self.assertRaisesRegex(ValueError, "incomplete"):
                guard.aniflow_candidate(self.api, self.run, REVISION, ROOT)

    def test_render_consumer_requires_all_four_workload_runs_and_schedule_age(self):
        if self.repository != "egohygiene/renderflow":
            self.skipTest("CI/doc/conformance consumer exists in Renderflow")
        api = GitHub()
        for number, specification in enumerate(self.policy["workflows"].values()):
            run = run_record(20 + number, specification["path"], specification["event"],
                             "b" * 40 if specification["event"] == "schedule" else REVISION)
            api.add(run, specification)
        report = guard.renderflow_release(api, REVISION, ROOT, NOW)
        self.assertEqual(len(report["runs"]), 4)
        self.assertEqual(report["publication"], "not-performed")
        scheduled = next(run for run in api.runs.values() if run["event"] == "schedule")
        for delta in (timedelta(days=9), timedelta(days=-1)):
            scheduled["created_at"] = (NOW - delta).isoformat()
            with self.assertRaisesRegex(ValueError, "stale or future"):
                guard.renderflow_release(api, REVISION, ROOT, NOW)

    def test_historical_schedule_requires_identical_reviewed_workflow_bytes(self):
        if self.repository != "egohygiene/renderflow":
            self.skipTest("scheduled consumer exists in Renderflow")
        specification = self.policy["workflows"]["scheduled_conformance"]
        run = run_record(workflow=specification["path"], event="schedule", revision="b" * 40)
        self.api.add(run, specification)
        expected = self.policy["source_sha256"][specification["path"]]
        guard.historical_workflow(self.api, self.repository, run, expected)
        self.assertTrue(any("?ref=" + "b" * 40 in path for path in self.api.calls))
        original = self.api
        def changed_api(path):
            data = original(path)
            data["content"] = base64.b64encode(b"altered workload with matching display names").decode("ascii")
            data["size"] = len(b"altered workload with matching display names")
            return data
        with self.assertRaisesRegex(ValueError, "differs from reviewed"):
            guard.historical_workflow(changed_api, self.repository, run, expected)


if __name__ == "__main__":
    unittest.main()
