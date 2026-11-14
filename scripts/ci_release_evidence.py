#!/usr/bin/env python3
"""Read-only admission: successful workflow conclusions alone never qualify work.

The checked-in policy binds exact workload names/steps to local source bytes.
No dispatch, rerun, issue mutation, artifact mutation or publication is performed.
"""

import argparse
import base64
import binascii
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys


PAGE_SIZE = 100
MAX_PAGES = 10
MAX_BYTES = 2 * 1024 * 1024
SHA = re.compile(r"[0-9a-f]{40}")
ROOT = Path(__file__).resolve().parents[1]


def need(condition, message):
    if not condition:
        raise ValueError("release workload evidence refused: " + message)


def positive(value):
    return type(value) is int and 0 < value <= 2**63 - 1


def gh_api(path):
    """Bound each read; API errors stay errors and no shell is involved."""
    result = subprocess.run(["gh", "api", path], check=False, capture_output=True, timeout=30)
    need(result.returncode == 0, "GitHub API read failed")
    need(len(result.stdout) <= MAX_BYTES, "GitHub API response exceeds byte budget")
    return json.loads(result.stdout)


def read(api, path):
    data = api(path)
    need(len(json.dumps(data).encode("utf-8")) <= MAX_BYTES, "GitHub API response exceeds byte budget")
    return data


def paginated(api, path, field):
    """Require the entire counted collection; truncated/overlapping pages refuse."""
    records, seen, total = [], set(), None
    for page in range(1, MAX_PAGES + 1):
        data = read(api, path + ("&" if "?" in path else "?") + "per_page=100&page=" + str(page))
        need(isinstance(data, dict), "API collection is not an object")
        count, batch = data.get("total_count"), data.get(field)
        need(type(count) is int and 0 <= count <= PAGE_SIZE * MAX_PAGES, "invalid or unbounded total_count")
        need(total is None or total == count, "collection changed during pagination")
        total = count
        need(isinstance(batch, list) and len(batch) <= PAGE_SIZE, "invalid page")
        for item in batch:
            need(isinstance(item, dict) and positive(item.get("id")), "invalid record identity")
            need(item["id"] not in seen, "duplicate record across pages")
            seen.add(item["id"])
            records.append(item)
        need(len(records) <= total, "page exceeds declared count")
        if len(records) == total:
            return records
        need(len(batch) == PAGE_SIZE, "truncated collection")
    raise ValueError("release workload evidence refused: pagination exhausted")


def timestamp(value):
    need(isinstance(value, str), "missing run timestamp")
    try:
        result = datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError as error:
        raise ValueError("release workload evidence refused: invalid timestamp") from error
    need(result.tzinfo is not None, "timestamp lacks timezone")
    return result


def source_policy(root=ROOT):
    path = Path(root) / "scripts/ci-release-evidence-policy.json"
    need(path.is_file() and not path.is_symlink(), "missing authored policy")
    need(path.stat().st_size <= 64 * 1024, "policy exceeds byte budget")
    policy = json.loads(path.read_text(encoding="utf-8"))
    need(policy.get("schema") == "flow.release-workload-evidence/v1", "unknown policy schema")
    need(policy.get("repository") in ("egohygiene/aniflow", "egohygiene/renderflow"), "unknown policy repository")
    for name, expected in policy["source_sha256"].items():
        need(name.startswith((".github/workflows/", ".egohygiene/")) and ".." not in Path(name).parts,
             "invalid source binding")
        source = Path(root) / name
        need(source.is_file() and not source.is_symlink(), "source input missing or linked")
        need(hashlib.sha256(source.read_bytes()).hexdigest() == expected,
             "job policy source drift: " + name + "; review and re-author policy")
    return policy


def validate_run(run, repository, workflow, event, revision=None):
    need(isinstance(run, dict) and positive(run.get("id")) and positive(run.get("run_attempt")) and
         positive(run.get("workflow_id")), "invalid run identity/attempt")
    need(run.get("repository", {}).get("full_name") == repository and
         run.get("head_repository", {}).get("full_name") == repository, "foreign repository run")
    need(run.get("path", "").split("@", 1)[0] == workflow and run.get("event") == event and
         run.get("head_branch") == "main", "workflow/event/branch mismatch")
    need(isinstance(run.get("head_sha"), str) and SHA.fullmatch(run["head_sha"]) is not None and
         (revision is None or run["head_sha"] == revision), "run source mismatch")
    need(run.get("status") == "completed" and run.get("conclusion") == "success", "workflow did not complete successfully")
    timestamp(run.get("created_at"))
    return run


def workload(api, repository, run, specification):
    """Every exact logical job is present once, and critical steps actually ran."""
    required = specification["jobs"]
    need(isinstance(required, list) and required, "empty required workload policy")
    aliases, expected = {}, set()
    for entry in required:
        identity = entry["identity"]
        need(identity not in expected and entry["names"] and entry.get("steps"), "invalid workload policy")
        expected.add(identity)
        for name in entry["names"]:
            need(name not in aliases, "ambiguous job policy")
            aliases[name] = entry
    skipped = set(specification.get("allowed_skipped", []))
    need(not skipped.intersection(aliases), "ambiguous skipped job policy")
    jobs = paginated(api, "repos/" + repository + "/actions/runs/" + str(run["id"]) + "/jobs?filter=latest", "jobs")
    need(bool(jobs), "empty job collection")
    observed, reports = set(), []
    for job in jobs:
        need(job.get("run_id") == run["id"] and job.get("head_sha") == run["head_sha"] and
             job.get("run_attempt") == run["run_attempt"], "job is not bound to selected source run/attempt")
        name = job.get("name")
        need(isinstance(name, str), "missing full job name")
        if name in skipped:
            need(name not in observed and job.get("status") == "completed" and job.get("conclusion") == "skipped",
                 "optional job has unexpected outcome")
            observed.add(name)
            continue
        need(name in aliases, "unknown job identity: " + name)
        entry = aliases[name]
        identity = entry["identity"]
        need(identity not in observed, "duplicate logical job/matrix identity")
        observed.add(identity)
        need(job.get("status") == "completed" and job.get("conclusion") == "success", "required job did not run successfully: " + name)
        steps = job.get("steps")
        need(isinstance(steps, list) and steps, "missing workload steps: " + name)
        for required_step in entry["steps"]:
            matches = [step for step in steps if isinstance(step, dict) and step.get("name") == required_step]
            need(len(matches) == 1 and matches[0].get("status") == "completed" and matches[0].get("conclusion") == "success",
                 "missing, skipped or failed workload step: " + name + "/" + required_step)
        reports.append({"identity": identity, "name": name, "job_id": job["id"], "critical_steps": len(entry["steps"])})
    need(observed == expected | skipped, "incomplete required job/matrix inventory")
    # Re-read after paginated jobs: reruns/cancellations during admission refuse.
    fresh = read(api, "repos/" + repository + "/actions/runs/" + str(run["id"]))
    for key in ("id", "workflow_id", "run_attempt", "head_sha", "head_branch", "event", "status", "conclusion", "path", "updated_at"):
        need(fresh.get(key) == run.get(key), "run changed during evidence collection")
    return {"workflow": run["path"].split("@", 1)[0], "run_id": run["id"], "run_attempt": run["run_attempt"],
            "source_revision": run["head_sha"], "jobs": reports}


def newest_run(api, repository, workflow, event, revision=None):
    query = "repos/" + repository + "/actions/workflows/" + workflow.rsplit("/", 1)[1] + "/runs?event=" + event
    if revision is not None:
        query += "&head_sha=" + revision
    runs = paginated(api, query, "workflow_runs")
    # Do not select an older green run when the newest matching attempt failed.
    candidates = [run for run in runs if run.get("event") == event and
                  run.get("path", "").split("@", 1)[0] == workflow and
                  (revision is None or run.get("head_sha") == revision)]
    need(candidates, "no matching workflow run")
    selected = max(candidates, key=lambda run: (timestamp(run.get("created_at")), run["id"]))
    full = read(api, "repos/" + repository + "/actions/runs/" + str(selected["id"]))
    need(full.get("id") == selected["id"] and full.get("run_attempt") == selected.get("run_attempt") and
         full.get("head_sha") == selected.get("head_sha"), "listed run changed before inspection")
    return validate_run(full, repository, workflow, event, revision)


def aniflow_candidate(api, run, revision, root=ROOT):
    policy = source_policy(root)
    need(policy["repository"] == "egohygiene/aniflow", "candidate policy belongs to another repository")
    spec = policy["workflows"]["candidate"]
    need(len(json.dumps(run).encode("utf-8")) <= MAX_BYTES, "candidate run exceeds byte budget")
    validate_run(run, policy["repository"], spec["path"], "workflow_dispatch", revision)
    return workload(api, policy["repository"], run, spec)


def historical_workflow(api, repository, run, expected_digest):
    """Older scheduled source is allowed only with identical reviewed workflow bytes."""
    workflow = run["path"].split("@", 1)[0]
    data = read(api, "repos/" + repository + "/contents/" + workflow + "?ref=" + run["head_sha"])
    need(isinstance(data, dict) and data.get("path") == workflow and data.get("type") == "file" and
         data.get("encoding") == "base64" and isinstance(data.get("content"), str) and
         type(data.get("size")) is int and 0 < data["size"] <= 128 * 1024,
         "historical workflow content missing or unbounded")
    try:
        content = base64.b64decode("".join(data["content"].splitlines()), validate=True)
    except (ValueError, binascii.Error) as error:
        raise ValueError("release workload evidence refused: invalid historical workflow bytes") from error
    need(len(content) == data["size"] and hashlib.sha256(content).hexdigest() == expected_digest,
         "scheduled workflow differs from reviewed source policy; obtain fresh qualification")


def renderflow_release(api, revision, root=ROOT, now=None):
    need(SHA.fullmatch(revision or "") is not None, "full release source SHA required")
    policy = source_policy(root)
    need(policy["repository"] == "egohygiene/renderflow", "release policy belongs to another repository")
    repository, reports = policy["repository"], []
    now = now or datetime.now(timezone.utc)
    for identity in ("ci", "docs", "manual_conformance", "scheduled_conformance"):
        spec = policy["workflows"][identity]
        scheduled = identity == "scheduled_conformance"
        run = newest_run(api, repository, spec["path"], spec["event"], None if scheduled else revision)
        if scheduled:
            age = (now - timestamp(run["created_at"])).total_seconds()
            need(0 <= age <= 8 * 86400, "scheduled workload evidence is stale or future-dated")
            historical_workflow(api, repository, run, policy["source_sha256"][spec["path"]])
        reports.append(workload(api, repository, run, spec))
    return {"schema": "flow.release-workload-admission/v1", "repository": repository,
            "source_revision": revision, "evidence_admitted": True, "publication": "not-performed", "runs": reports}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-commit", required=True)
    args = parser.parse_args()
    try:
        report = renderflow_release(gh_api, args.expected_commit)
        output = json.dumps(report, sort_keys=True)
        need(len(output.encode()) <= 32768, "report exceeds byte budget")
        print(output)
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
