#!/usr/bin/env python3
"""Run isolated public Renderflow CLI jobs; never synthesize conversion evidence.

Authored for #430. No tests or executions qualified this runner at authoring time.
Describe is static. Plan and execute launch a supplied binary and may probe tools.
"""

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import signal
import stat
import subprocess
import sys
import threading
import time

REQUEST_SCHEMA = "renderflow.creative-fixture/v1"
RESULT_SCHEMA = "renderflow.creative-fixture-result/v1"
ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "examples" / "creative-forest"
MAX_JSON = 64 * 1024 * 1024
MAX_FILE = 512 * 1024 * 1024
CAPTURE_BYTES = 8 * 1024 * 1024


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON field: " + key)
        result[key] = value
    return result


def read_file(path, maximum=MAX_FILE):
    path = Path(path)
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_size > maximum:
        raise ValueError("bounded regular file required: " + str(path))
    with path.open("rb") as stream:
        data = stream.read(maximum + 1)
        opened = os.fstat(stream.fileno())
    after = path.lstat()
    identity = lambda value: (value.st_dev, value.st_ino, value.st_size, value.st_mtime_ns)
    if len(data) > maximum or identity(before) != identity(opened) or identity(before) != identity(after):
        raise ValueError("file changed or exceeded bound: " + str(path))
    return data


def read_json(path):
    return json.loads(read_file(path, MAX_JSON), object_pairs_hook=unique_object)


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    data = (json.dumps(value, indent=2, ensure_ascii=False, allow_nan=False) + "\n").encode()
    with Path(path).open("xb") as stream:
        stream.write(data)


def within(root, relative):
    candidate = Path(relative)
    if candidate.is_absolute() or ".." in candidate.parts:
        raise ValueError("relative path escapes its evidence root")
    path = root / candidate
    cursor = root
    for part in candidate.parts:
        cursor = cursor / part
        if cursor.is_symlink():
            raise ValueError("symlink evidence paths are refused: " + str(cursor))
    if not path.resolve().is_relative_to(root.resolve()):
        raise ValueError("path escapes its evidence root")
    return path


def file_reference(path, root, maximum=MAX_FILE):
    data = read_file(path, maximum)
    relative = Path(path).relative_to(root).as_posix()
    within(root, relative)
    return {"path": relative, "sha256": sha256(data), "size_bytes": len(data)}


def stop_child(process):
    try:
        if os.name == "posix":
            # The leader may have exited while descendants still own its pipes.
            os.killpg(process.pid, signal.SIGKILL)
        elif process.poll() is None:
            process.kill()
    except ProcessLookupError:
        pass


def run_child(argv, directory, timeout):
    """Bound wall time and both captures without buffering arbitrary child output."""
    result = {"argv": argv, "exit_code": None, "termination": "spawn_failed"}
    exceeded = threading.Event()
    errors = []
    paths = {name: directory / (name + ".log") for name in ("stdout", "stderr")}

    def drain(pipe, path):
        try:
            total = 0
            with path.open("xb") as output:
                while True:
                    block = pipe.read(65536)
                    if not block:
                        break
                    remaining = CAPTURE_BYTES - total
                    output.write(block[:max(0, remaining)])
                    total += len(block)
                    if total > CAPTURE_BYTES:
                        exceeded.set()
                        break
        except (OSError, ValueError) as error:
            errors.append(str(error))
            exceeded.set()
        finally:
            pipe.close()

    process = None
    workers = []
    try:
        process = subprocess.Popen(argv, cwd=directory, stdin=subprocess.DEVNULL,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                   shell=False, start_new_session=os.name == "posix")
        for name, pipe in (("stdout", process.stdout), ("stderr", process.stderr)):
            worker = threading.Thread(target=drain, args=(pipe, paths[name]), daemon=True)
            worker.start()
            workers.append(worker)
        deadline = time.monotonic() + timeout
        result["termination"] = "exited"
        while process.poll() is None:
            if exceeded.is_set() or time.monotonic() >= deadline:
                result["termination"] = "capture_limit" if exceeded.is_set() else "timeout"
                stop_child(process)
                break
            try:
                process.wait(timeout=0.1)
            except subprocess.TimeoutExpired:
                pass
        result["exit_code"] = process.wait(timeout=5)
        for worker in workers:
            worker.join(timeout=2)
        if any(worker.is_alive() for worker in workers):
            result["termination"] = "capture_incomplete"
            stop_child(process)
            for worker in workers:
                worker.join(timeout=2)
        elif exceeded.is_set():
            result["termination"] = "capture_limit"
    except (OSError, subprocess.SubprocessError) as error:
        result["diagnostic"] = str(error)
        if process is not None:
            stop_child(process)
    except BaseException:
        if process is not None:
            stop_child(process)
        raise
    if errors:
        result["capture_errors"] = errors
    result["captures_closed"] = not any(worker.is_alive() for worker in workers)
    return result


def forest_invariants(forest):
    if forest.get("schema") != "renderflow/artifact-forest-v1":
        raise ValueError("expected public artifact-forest contract")
    ids = set()
    for branch in forest["branches"]:
        if branch["target_id"] in ids:
            raise ValueError("duplicate effective target identity in forest")
        ids.add(branch["target_id"])
        route = branch.get("route")
        if not route:
            continue
        visited = set()
        previous = None
        for edge in route["steps"]:
            if previous is None:
                visited.add(edge["from"])
            elif edge["from"] != previous:
                raise ValueError("disconnected selected route")
            if edge["to"] in visited:
                raise ValueError("cycle in selected route")
            visited.add(edge["to"])
            previous = edge["to"]


def published_artifacts(manifest, bundle, report_root):
    """Use target publication rows, never manufacture outputs from requested names."""
    if manifest.get("schema_version") != "renderflow.run/v1":
        raise ValueError("expected native renderflow.run/v1 evidence")
    forest = manifest.get("artifact_forest", {})
    forest_invariants(forest)
    execution = forest.get("execution", {})
    if execution.get("schema") != "renderflow/forest-execution-v1":
        raise ValueError("native manifest lacks explicit target publication evidence")
    artifact_manifest = manifest["artifact_manifest"]
    if manifest.get("run_id") != artifact_manifest.get("run_id") or not manifest.get("run_id"):
        raise ValueError("native run/artifact manifest identity mismatch")
    if Path(artifact_manifest["output_dir"]).resolve() != bundle.resolve():
        raise ValueError("native artifact manifest belongs to a different output directory")
    artifacts = artifact_manifest["artifacts"]
    branches = {branch["target_id"]: branch for branch in forest["branches"]}
    target_ids = [target["target_id"] for target in execution["targets"]]
    if len(set(target_ids)) != len(target_ids) or set(target_ids) != set(branches):
        raise ValueError("execution target identities do not exactly match planning branches")
    results = []
    for target in execution["targets"]:
        if target["outcome"] != "published":
            continue
        index = target.get("artifact_evidence_index")
        if type(index) is not int or not 0 <= index < len(artifacts):
            raise ValueError("published target has no native artifact reference")
        record = artifacts[index]
        branch = branches[target["target_id"]]
        metadata = record.get("metadata", {})
        if branch["state"] != "selected" or record["lifecycle"] != "terminal" or metadata.get("renderflow.target.identity") != target["target_id"] or metadata.get("renderflow.target.published") is not True:
            raise ValueError("publication lacks a selected target and matching native terminal evidence")
        if target["source_identity"] != branch["route"]["source_identity"]:
            raise ValueError("publication source/derivative identity mismatch")
        locator = target.get("locator", "")
        if not locator.startswith("bundle:") or locator != record["locator"]:
            raise ValueError("published target has no matching bundle locator")
        if target.get("artifact_id") != record["artifact_id"]:
            raise ValueError("target/native artifact identity mismatch")
        path = within(bundle, locator[len("bundle:"):])
        if path.resolve() not in {Path(value).resolve() for value in artifact_manifest["outputs"]}:
            raise ValueError("published target is absent from native output membership")
        reference = file_reference(path, report_root)
        if record["digest"] != {"algorithm": "sha256", "value": reference["sha256"]} or record["size_bytes"] != reference["size_bytes"]:
            raise ValueError("published payload differs from native artifact evidence")
        if record["validation"] == "invalid":
            raise ValueError("invalid artifact was marked published")
        results.append({"target_id": target["target_id"], "artifact_id": record["artifact_id"],
                        "artifact_evidence_index": index, "source_identity": target["source_identity"],
                        "format": branch["format"], "file": reference})
    produced = sorted({item["artifact_id"] for item in results if not item["source_identity"]})
    if produced != sorted(forest.get("produced_artifacts", [])):
        raise ValueError("produced derivative membership differs from publication rows")
    return results


def expected_membership(job, forest, published=None):
    expected = {(item["target_id"], item["format"]) for item in job["expected_targets"]}
    selected = {(item["target_id"], item["format"]) for item in forest["branches"] if item["state"] == "selected"}
    if selected != expected:
        raise ValueError("selected target membership differs from the authored fixture expectation")
    if "expected_blocked_targets" in job:
        blocked = {(item["target_id"], item["format"]) for item in forest["branches"] if item["state"] != "selected"}
        if blocked != {(item["target_id"], item["format"]) for item in job["expected_blocked_targets"]}:
            raise ValueError("blocked target membership differs from the authored fixture expectation")
    if published is not None and {(item["target_id"], item["format"]) for item in published} != expected:
        raise ValueError("published target membership differs from the authored fixture expectation")
    if job.get("forbid_source_identity") and any(branch.get("route", {}).get("source_identity") for branch in forest["branches"] if branch["state"] == "selected"):
        raise ValueError("automatic reachable targets must not publish the original as a derivative")


def configure(job, native, directory):
    entry = native.get(job.get("configuration", job["id"]))
    if job.get("native") and entry is None:
        return None
    entry = entry or {}
    if not isinstance(entry, dict) or set(entry) - {"policy", "source", "format"}:
        raise ValueError("native entry accepts policy/source/format only")
    source_name = job.get("source") if job.get("fixed_source") else entry.get("source", job.get("source"))
    if source_name is None:
        raise ValueError("this media job requires an explicitly supplied local source")
    if "source" in entry and not job.get("fixed_source"):
        source = Path(source_name)
        if not source.is_absolute():
            raise ValueError("native source override must be absolute")
    else:
        source = within(FIXTURE, source_name)
    source_bytes = read_file(source, 64 * 1024 * 1024)
    source_record = {"path": str(source), "sha256": sha256(source_bytes), "size_bytes": len(source_bytes)}
    spec = None
    if job["kind"] == "build":
        spec = copy.deepcopy(read_json(within(FIXTURE, job["spec"])))
        spec["sources"][0]["path"] = str(source)
        if "format" in entry and not job.get("fixed_source"):
            spec["sources"][0]["format"] = entry["format"]
        if job.get("native"):
            if not isinstance(entry.get("policy"), dict):
                raise ValueError("native execution requires a full explicit policy object")
            spec["execution"][job["policy_field"]] = entry["policy"]
            if job.get("requires_ocr") and not isinstance(entry["policy"].get("ocr"), dict):
                raise ValueError("pdf-ocr requires an explicitly pinned OCR policy")
        if job.get("missing_speech_provider"):
            spec["execution"]["speech"] = copy.deepcopy(spec["execution"]["speech"])
            spec["execution"]["speech"]["provider"]["path"] = str(directory / "deliberately-absent-whisper-cli")
            spec["execution"]["speech"]["source_sha256"] = source_record["sha256"]
            spec["execution"]["speech"].pop("audio", None)
            spec["execution"]["speech"].pop("recovery", None)
            spec["execution"]["speech"]["saved_results"] = "disabled"
        spec["output"]["bundle_root"] = str(directory / "bundle")
        write_json(directory / "config.json", spec)
    return source, source_record, spec


def run_job(job, native, binary, mode, root, timeout):
    result = {"id": job["id"], "status": "failed", "expected": job.get("expected", "success"),
              "evidence": [], "published_artifacts": []}
    if job.get("native") and job.get("configuration", job["id"]) not in native:
        result.update(status="skipped_unconfigured", reason="No explicit native policy supplied; availability was not observed.")
        return result
    if mode == "plan" and job["kind"] != "build":
        result.update(status="not_run", reason="Standalone command has no public dry-run contract; no fabricated plan.")
        return result
    directory = root / "jobs" / job["id"]
    directory.mkdir(parents=True)
    try:
        source, source_record, spec = configure(job, native, directory)
        result["source"] = source_record
        if spec is not None:
            result["evidence"].append({"kind": "request", **file_reference(directory / "config.json", root)})
            argv = [str(binary), "build", "--config", str(directory / "config.json")]
            if mode == "plan":
                argv.append("--dry-run")
        elif job["kind"] == "analysis":
            argv = [str(binary), "screenplay", "analyze", "--input", str(source)]
        elif job["kind"] == "transcript_text":
            argv = [str(binary), "subtitles", "convert", "--input", str(source), "--input-format", "srt",
                    "--output", str(directory / "transcript.txt"), "--output-format", "text"]
        else:
            raise ValueError("unsupported fixture command kind")
        result["process"] = run_child(argv, directory, timeout)
        if not result["process"]["captures_closed"]:
            raise ValueError("child capture streams remain open; mutable logs are retained without digest claims")
        for name in ("stdout", "stderr"):
            path = directory / (name + ".log")
            if path.exists():
                result["evidence"].append({"kind": name, **file_reference(path, root, CAPTURE_BYTES)})
        source_after = read_file(source, 64 * 1024 * 1024)
        if sha256(source_after) != source_record["sha256"]:
            raise ValueError("source changed during child operation")
        result["source_unchanged"] = True
        manifest_path = directory / "bundle" / "renderflow-run.json"
        manifest = None
        if manifest_path.exists():
            result["evidence"].append({"kind": "run", **file_reference(manifest_path, root, MAX_JSON)})
            manifest = read_json(manifest_path)
            result["native_run_state"] = manifest.get("state")
            result["published_artifacts"] = published_artifacts(manifest, directory / "bundle", root)
        process = result["process"]
        if process["termination"] != "exited":
            raise ValueError("child did not finish with complete bounded captures")
        stdout = read_file(directory / "stdout.log", CAPTURE_BYTES)
        stderr = read_file(directory / "stderr.log", CAPTURE_BYTES)
        if process["exit_code"] != 0:
            marker = job.get("failure_marker")
            diagnostic = (stdout + stderr).decode("utf-8", errors="replace")
            if manifest is not None:
                diagnostic += json.dumps(manifest.get("diagnostics", []))
            if mode in job.get("refusal_modes", ["execute"]) and job.get("expected") == "refusal" and marker and marker in diagnostic and not result["published_artifacts"]:
                result["status"] = "expected_refusal"
                return result
            raise ValueError("child failed; retained logs/native manifest remain authoritative")
        if mode == "plan":
            plan = json.loads(stdout, object_pairs_hook=unique_object)
            source_evidence = plan.get("source_artifact", {})
            if source_evidence.get("digest") != "sha256:" + source_record["sha256"] or source_evidence.get("size_bytes") != source_record["size_bytes"]:
                raise ValueError("planned source identity differs from the actual fixture input")
            forest_invariants(plan.get("artifact_forest", {}))
            expected_membership(job, plan["artifact_forest"])
            result["evidence"].append({"kind": "plan", **file_reference(directory / "stdout.log", root, CAPTURE_BYTES)})
            result["status"] = "planned"
            return result
        if job.get("expected") == "refusal":
            raise ValueError("negative fixture unexpectedly succeeded")
        if spec is not None:
            if manifest is None or manifest.get("state") != "complete":
                raise ValueError("successful child lacks a complete native run manifest")
            source_records = [record for record in manifest["artifact_manifest"]["artifacts"] if record.get("lifecycle") == "source"]
            if len(source_records) != 1 or source_records[0].get("digest") != {"algorithm": "sha256", "value": source_record["sha256"]} or source_records[0].get("size_bytes") != source_record["size_bytes"]:
                raise ValueError("native run source identity differs from the actual fixture input")
            expected_membership(job, manifest["artifact_forest"], result["published_artifacts"])
            if job.get("expected") == "review_required":
                records = manifest["artifact_manifest"]["artifacts"]
                for item in result["published_artifacts"]:
                    metadata = records[item["artifact_evidence_index"]].get("metadata", {})
                    evidence = metadata.get("renderflow.screenplay_recovery", {})
                    if evidence.get("review_required") is not True or evidence.get("lossy") is not True:
                        raise ValueError("ambiguous recovery lost mandatory loss/review declarations")
                    if evidence.get("candidate", {}).get("source_sha256") != source_record["sha256"]:
                        raise ValueError("recovery candidate has a different original source")
                    if job.get("requires_ocr") and not any(page.get("method") == "ocr" for page in evidence.get("extraction_evidence", {}).get("pages", [])):
                        raise ValueError("declared OCR scenario did not retain an actually OCR-extracted page")
                    reason = job.get("required_ambiguity_reason")
                    if reason and not any(element.get("confidence") == "low" and reason in element.get("reason_codes", []) for element in evidence["candidate"]["elements"]):
                        raise ValueError("declared ambiguity scenario lacks its low-confidence reason evidence")
        else:
            value = json.loads(stdout, object_pairs_hook=unique_object)
            if job["kind"] == "analysis":
                if value.get("schema_version") != "renderflow.screenplay-analysis/v1" or value["source"]["sha256"] != source_record["sha256"]:
                    raise ValueError("standalone analysis source/contract mismatch")
            else:
                reference = file_reference(directory / "transcript.txt", root)
                if value.get("schema_version") != "renderflow.subtitle-export/v1" or value.get("input_sha256") != source_record["sha256"] or value.get("output_sha256") != reference["sha256"] or value.get("output_size_bytes") != reference["size_bytes"]:
                    raise ValueError("standalone text export evidence mismatch")
                result["evidence"].append({"kind": "standalone_payload", **reference})
            result["evidence"].append({"kind": "standalone_report", **file_reference(directory / "stdout.log", root, CAPTURE_BYTES)})
        result["status"] = "completed"
    except (OSError, ValueError, KeyError, TypeError, IndexError, AttributeError) as error:
        result["diagnostic"] = str(error)
    return result


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=("describe", "plan", "execute"), default="describe")
    parser.add_argument("--renderflow", help="Absolute path to an already built regular executable")
    parser.add_argument("--output", help="New retained evidence directory; parent must exist")
    parser.add_argument("--native-config", help="Explicit per-job policies and optional absolute media source paths")
    parser.add_argument("--include-negatives", action="store_true")
    parser.add_argument("--timeout-seconds", type=int, default=900)
    arguments = parser.parse_args(argv)
    request_path = FIXTURE / "fixture.json"
    request = read_json(request_path)
    if request.get("schema") != REQUEST_SCHEMA:
        parser.error("unsupported fixture request contract")
    if arguments.mode == "describe":
        print(json.dumps(request, indent=2, ensure_ascii=False))
        return 0
    if not arguments.renderflow or not arguments.output or not 1 <= arguments.timeout_seconds <= 7200:
        parser.error("plan/execute require --renderflow, --output and timeout in 1..7200")
    binary = Path(arguments.renderflow)
    if not binary.is_absolute():
        parser.error("--renderflow must be an absolute regular executable path")
    binary_bytes = read_file(binary)
    native = read_json(arguments.native_config) if arguments.native_config else {}
    if not isinstance(native, dict):
        parser.error("native configuration must be a JSON object keyed by job ID")
    permitted = {job.get("configuration", job["id"]) for job in request["jobs"] if job.get("native")}
    if set(native) - permitted:
        parser.error("native configuration contains unknown job IDs")
    root = Path(arguments.output).absolute()
    if root.exists() or root.is_symlink() or not root.parent.is_dir():
        parser.error("--output must be new and its parent must exist")
    root.mkdir()
    root = root.resolve()
    report = {"schema": RESULT_SCHEMA, "mode": arguments.mode, "state": "running",
              "qualification": "unqualified", "coverage_complete": False,
              "request": {"sha256": sha256(read_file(request_path, MAX_JSON)), "schema": REQUEST_SCHEMA},
              "runner_sha256": sha256(read_file(Path(__file__))),
              "binary": {"path": str(binary), "sha256": sha256(binary_bytes), "size_bytes": len(binary_bytes)},
              "gaps": request["gaps"], "jobs": []}
    write_json(root / "request.json", request)
    report["request"].update(file_reference(root / "request.json", root, MAX_JSON))
    # A separate incomplete checkpoint is preserved if interrupted before result.json.
    write_json(root / "started.json", report)
    try:
        for job in request["jobs"]:
            if job.get("negative") and not arguments.include_negatives:
                continue
            result = run_job(job, native, binary, arguments.mode, root, arguments.timeout_seconds)
            report["jobs"].append(result)
            write_json(root / (job["id"] + ".result.json"), result)
        if sha256(read_file(binary)) != report["binary"]["sha256"]:
            raise ValueError("Renderflow executable changed during suite execution")
        failures = [job for job in report["jobs"] if job["status"] == "failed"]
        successes = [job for job in report["jobs"] if job["status"] in ("planned", "completed", "expected_refusal")]
        report["state"] = ("partial" if successes else "failed") if failures else ("planned" if arguments.mode == "plan" else "complete")
    except KeyboardInterrupt:
        report["state"] = "cancelled"
    except (OSError, ValueError) as error:
        report.update(state="failed", diagnostic=str(error))
    write_json(root / "result.json", report)
    print(str(root / "result.json"))
    return 0 if report["state"] in ("planned", "complete") else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError) as error:
        print("creative fixture: " + str(error), file=sys.stderr)
        sys.exit(2)
