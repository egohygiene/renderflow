"""Focused Draft 2020-12 schema checks; samples are authored, not Rust outputs."""
import argparse
from copy import deepcopy
import hashlib
from importlib.metadata import version
import json
from pathlib import Path

from jsonschema import Draft202012Validator
from referencing import Registry, Resource

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--repo", type=Path, required=True)
parser.add_argument("--evidence-json", type=Path, required=True)
parser.add_argument("--native-dir", type=Path)
args = parser.parse_args()
names = {
    "transcript": "renderflow-timed-transcript-v1.schema.json",
    "export": "renderflow-subtitle-export-v1.schema.json",
    "inspection": "renderflow-subtitle-inspection-v1.schema.json",
    "qualification": "renderflow-subtitle-qualification-v1.schema.json",
    "spec": "renderflow-v2.schema.json",
}
schemas = {name: json.loads((args.repo / "schemas" / filename).read_text()) for name, filename in names.items()}
for schema in schemas.values():
    Draft202012Validator.check_schema(schema)
registry = Registry().with_resources((schema["$id"], Resource.from_contents(schema)) for schema in schemas.values())
validators = {name: Draft202012Validator(schema, registry=registry) for name, schema in schemas.items()}
validators["subtitles_policy"] = Draft202012Validator(schemas["spec"]["$defs"]["subtitles"])
results = []

def check(name, schema, value, valid=True):
    errors = list(validators[schema].iter_errors(value))
    observed = not errors
    results.append({"name": name, "schema": schema, "expected_valid": valid, "observed_valid": observed, "passed": observed == valid})

def mutate(name, schema, source, path, value, valid=False):
    sample = deepcopy(source)
    target = sample
    for key in path[:-1]:
        target = target[key]
    target[path[-1]] = value
    check(name, schema, sample, valid)

source_bytes = (args.repo / "tests/fixtures/timed-text/plain-unicode.srt").read_bytes()
transcript = {
    "schema_version": "renderflow.timed-transcript/v1",
    "source": {"sha256": hashlib.sha256(source_bytes).hexdigest(), "format": "srt"},
    "overlap_policy": "preserve",
    "cues": [
        {"id": "7", "start_ms": 1250, "end_ms": 3750, "text": "Hello, orbit.\nПривет, мир 🌙"},
        {"id": "9", "start_ms": 5000, "end_ms": 6125, "text": "The stars reply."},
    ],
    "losses": [],
}
check("authored-transcript-matching-unicode-fixture", "transcript", transcript)
for path, value in [
    (["schema_version"], "renderflow.timed-transcript/v999"),
    (["source", "sha256"], "A" * 64),
    (["source", "sha256"], "a" * 63),
    (["source", "format"], "\t"),
    (["source", "format"], "x" * 65),
    (["source", "stream"], []),
    (["cues"], []),
    (["cues", 0], []),
    (["cues", 0, "id"], "NOTE content"),
    (["cues", 0, "id"], "WEBVTT"),
    (["cues", 0, "id"], "x --> y"),
    (["cues", 0, "text"], ""),
    (["cues", 0, "text"], "hello\n\nworld"),
    (["cues", 0, "text"], "hello\n \t\nworld"),
    (["cues", 0, "text"], "hello\n"),
    (["cues", 0, "text"], "hello\rworld"),
    (["cues", 0, "text"], "<b>text</b>"),
    (["cues", 0, "text"], "literal & ampersand"),
    (["cues", 0, "text"], "{\\i1}style"),
    (["cues", 0, "text"], "x" * 65537),
    (["cues", 0, "start_ms"], -1),
    (["cues", 0, "end_ms"], 360000000),
    (["cues", 0, "style"], "unknown"),
    (["losses"], ["language", "language"]),
    (["losses"], ["unrecognized"]),
    (["overlap_policy"], "repair"),
    (["language"], ""),
    (["unexpected"], True),
]:
    mutate("transcript-refuses-" + "/".join(map(str, path)) + "-" + repr(value)[:24], "transcript", transcript, path, value)
for path, value in [(["language"], None), (["source", "stream"], None), (["cues", 0, "speaker"], None), (["cues", 0, "text"], "hello\tworld"), (["cues", 1, "end_ms"], 359999999)]:
    mutate("transcript-accepts-" + "/".join(map(str, path)), "transcript", transcript, path, value, True)
extracted = deepcopy(transcript)
extracted["source"]["format"] = "matroska"
extracted["source"]["stream"] = {"index": 1, "codec": "subrip", "language": "eng"}
extracted["source"]["extraction"] = {"provider": "adapter.subtitles.ffmpeg", "ffprobe_version": "ffprobe version synthetic", "ffmpeg_version": "ffmpeg version synthetic", "subtitle_sha256": "b" * 64}
check("extraction-provenance", "transcript", extracted)
mutate("extraction-requires-stream", "transcript", extracted, ["source", "stream"], None)
mutate("stream-index-bound", "transcript", extracted, ["source", "stream", "index"], 4294967296)
inspection = {"schema": "renderflow.subtitle-inspection/v1", "source_path": "/synthetic/supported.mkv", "source_sha256": "a" * 64, "ffprobe_version": "ffprobe version synthetic", "streams": [{"index": 1, "codec": "subrip", "language": "eng", "supported": True, "unsupported_reason": None}]}
check("inspection-sample", "inspection", inspection)
mutate("inspection-empty-inventory", "inspection", inspection, ["streams"], [], True)
mutate("inspection-invalid-language", "inspection", inspection, ["streams", 0, "language"], "en\ninjected")
mutate("inspection-invalid-codec", "inspection", inspection, ["streams", 0, "codec"], "invalid-codec")
export = {"schema_version": "renderflow.subtitle-export/v1", "input_sha256": "a" * 64, "output": "/synthetic/captions.vtt", "output_format": "vtt", "output_sha256": "b" * 64, "output_size_bytes": 123, "export_losses": ["source_provenance", "overlap_policy"], "transcript": transcript}
check("export-sample-with-resolved-transcript", "export", export)
mutate("export-format-is-cli-vtt", "export", export, ["output_format"], "webvtt")
mutate("export-output-byte-limit", "export", export, ["output_size_bytes"], 16777217)
mutate("export-nested-transcript-enforced", "export", export, ["transcript", "cues", 0, "id"], "NOTE")
case_ids = ["fixture_generation", "srt_stream", "webvtt_stream", "sdk_lineage", "no_subtitles", "unsupported_ass", "unsupported_mov_text", "pre_cancelled", "malformed_container", "provider_versions"]
qualification = {"schema_version": "renderflow.subtitle-qualification/v1", "status": "not_run", "os": "linux", "arch": "x86_64", "implementation": None, "providers": {tool: {"expected_version_line": "", "observed_version_line": None, "diagnostic": None} for tool in ["ffmpeg", "ffprobe"]}, "cases": [{"id": name, "description": "Authored schema sample; never executed", "required": True, "status": "not_run", "detail": {}} for name in case_ids]}
check("qualification-initial-not-run", "qualification", qualification)
complete = deepcopy(qualification)
complete["status"] = "passed"
complete["implementation"] = {"crate_version": "synthetic", "harness_sha256": "a" * 64, "source_revision": None}
for tool, record in complete["providers"].items():
    record["expected_version_line"] = record["observed_version_line"] = tool + " version synthetic"
for case in complete["cases"]:
    case["status"] = "passed"
check("qualification-authored-complete-shape-not-actual-evidence", "qualification", complete)
for path, value in [(["implementation"], None), (["providers", "ffmpeg", "expected_version_line"], ""), (["providers", "ffmpeg", "observed_version_line"], None), (["providers", "ffmpeg", "diagnostic"], "warning"), (["cases", 0, "status"], "not_run"), (["cases", 0, "status"], "unavailable"), (["cases", 0, "status"], "failed"), (["cases", 0, "required"], False), (["cases", 0, "id"], "substitute-case"), (["cases", 0, "detail"], None), (["cases"], complete["cases"][:-1])]:
    mutate("qualification-refuses-" + "/".join(map(str, path)) + "-" + repr(value)[:24], "qualification", complete, path, value)
failed = deepcopy(complete)
failed["cases"][0]["status"] = "failed"
failed["status"] = "failed"
check("qualification-failed-shape", "qualification", failed)
for name, value, valid in [("empty-opt-in", {}, True), ("explicit-index", {"stream_index": 1}, True), ("null-index", {"stream_index": None}, True), ("negative-index", {"stream_index": -1}, False), ("overflow-index", {"stream_index": 4294967296}, False), ("positional-sequence", [1], False), ("unknown-option", {"repair": True}, False)]:
    check("policy-" + name, "subtitles_policy", value, valid)
if args.native_dir:
    for filename, schema in [("qualification.json", "qualification"), ("extracted-srt.timed-transcript.json", "transcript"), ("extracted-vtt.timed-transcript.json", "transcript"), ("sdk-output/transcript.timed-transcript.json", "transcript")]:
        check("native-artifact-" + filename, schema, json.loads((args.native_dir / filename).read_text()))
evidence = {"check": "Standard Draft 2020-12 schema self-validation and authored positive/negative samples", "validator": "jsonschema " + version("jsonschema"), "schemas_checked": list(names.values()), "schema_hashes": {name: hashlib.sha256((args.repo / "schemas" / filename).read_bytes()).hexdigest() for name, filename in names.items()}, "assertions": len(results), "passed": sum(item["passed"] for item in results), "failed": [item for item in results if not item["passed"]], "native_artifacts": str(args.native_dir) if args.native_dir else "not supplied; Rust output/schema agreement unverified", "limitations": ["Examples are authored representations of fixture/model fields, not evidence of Rust execution.", "Schema cannot prove cross-field timing/order, exact version-string equality, UTF-8 byte budgets, or raw duplicate-key rejection; application checks remain necessary."], "results": results}
args.evidence_json.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n")
print(json.dumps({key: evidence[key] for key in ["validator", "assertions", "passed", "failed", "native_artifacts"]}, indent=2))
raise SystemExit(bool(evidence["failed"]))
