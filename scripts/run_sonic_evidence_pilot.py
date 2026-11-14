#!/usr/bin/env python3
"""Exercise the native Sonic CLI with generated PCM24 and private cue canaries.

No analyzer, model, network, human approval or publication is selected. Supplied
tempo/key values below are explicit synthetic assertions, not measured music.
"""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
CANARY = "PRIVATE_LYRIC_CANARY_月_Привет"


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def inventory(root):
    return {str(path.relative_to(root)): sha(path.read_bytes())
            for path in sorted(root.rglob("*")) if path.is_file()}


def require_native_outcome(result, success, expected_error=None):
    require(result.returncode == (0 if success else 1), "unexpected native CLI exit status")
    if not success:
        require(expected_error is not None, "negative trial requires an exact native diagnostic")
        require(result.stdout == b"" and result.stderr == f"Error: {expected_error}\n".encode(),
                "different CLI refusal than the selected negative trial")


def run(binary, args, events, success=True, expected_error=None):
    started = time.monotonic()
    result = subprocess.run([str(binary), *args], capture_output=True, timeout=30)
    require(len(result.stdout) + len(result.stderr) <= 2 * 1024 * 1024, "bounded CLI capture exceeded")
    events.append({"command": args[:3], "exit_code": result.returncode,
                   "elapsed_ms": round((time.monotonic() - started) * 1000)})
    require_native_outcome(result, success, expected_error)
    if expected_error is not None:
        events[-1]["expected_error"] = expected_error
    return json.loads(result.stdout) if success else None


def pilot(binary, work):
    binary = binary.resolve(strict=True)
    require(binary.is_file(), "selected binary required")
    binary_sha = sha(binary.read_bytes())
    require(work.parent.resolve() == work.parent and work.parent.is_dir(), "canonical existing work parent required")
    work.mkdir(mode=0o700)
    inputs = work / "inputs"
    inputs.mkdir(mode=0o700)
    frames, rate, channels, bits = 55_125, 44_100, 2, 24
    # Finite signed24 stereo values; no input file, audio provider or model.
    pcm = b"".join((index % 8192 - 4096).to_bytes(3, "little", signed=True)
                   + (4095 - index % 8192).to_bytes(3, "little", signed=True)
                   for index in range(frames))
    wav = (b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVEfmt "
           + struct.pack("<IHHIIHH", 16, 1, channels, rate, rate * 6, 6, bits)
           + b"data" + struct.pack("<I", len(pcm)) + pcm)
    (inputs / "source.wav").write_bytes(wav)
    cues = f"1\n00:00:00,125 --> 00:00:00,625\n{CANARY}\n\n2\n00:00:00,650 --> 00:00:01,250\nSynthetic second cue 🌙\n"
    (inputs / "manual lyrics.srt").write_text(cues, encoding="utf-8")
    evidence = b"Original synthetic assertions only; no analyzer/provider/model executed.\n"
    (inputs / "fixture-evidence.txt").write_bytes(evidence)
    analysis = json.loads((ROOT / "examples/sonic-dna/analysis.json").read_text())
    analysis["source"].update({"sample_rate_hz": rate, "channels": channels, "frame_count": frames})
    analysis["source"]["artifact"].update({"sha256": sha(wav), "byte_size": len(wav)})
    analysis["artifacts"][0].update({"sha256": sha(evidence), "byte_size": len(evidence)})
    analysis["extensions"]["org.egohygiene.renderflow.fixture"]["claim"] = "Authored assertions on generated PCM24; no musical analysis executed."
    write_json(inputs / "analysis.json", analysis)
    recipe = json.loads((ROOT / "examples/sonic-dna/sonic.json").read_text())
    require(all(recipe["policy"][key] is False for key in
                ("allow_ai", "allow_network", "allow_remote"))
            and recipe["semantic"] == {"mode": "disabled", "source_approved_for_ai": False,
                                      "preference": "local_only"},
            "public example no longer selects the provider-free offline pilot")
    recipe["id"] = "synthetic-native24-companion-pilot"
    recipe["audio"]["sha256"] = sha(wav)
    recipe["analyses"][0]["file"]["sha256"] = sha((inputs / "analysis.json").read_bytes())
    recipe["analyses"][0]["evidence_files"][0]["sha256"] = sha(evidence)
    recipe["companions"] = [{"file": {"id": "manual_lyrics", "path": "manual lyrics.srt",
        "sha256": sha(cues.encode())}, "source_sha256": sha(wav), "kind": "transcript_candidate",
        "authority_reference": "synthetic supplied words; no authenticated human review",
        "timed_text_context": {"format": "srt", "maximum_cues": 2}}]
    request = inputs / "sonic.json"
    write_json(request, recipe)
    source_before = inventory(inputs)
    events = []
    plan = run(binary, ["dna", "sonic", "plan", "--input", str(request)], events)
    expected = {"sample_rate_hz": rate, "channels": channels, "bits_per_sample": bits,
                "frame_count": frames, "duration_ms": 1250, "sample_encoding": "pcm_s24le",
                "sample_conversion": "none"}
    require(all(plan["technical"][key] == value for key, value in expected.items()), "native source clock/conversion differs")
    output = work / "candidate"
    produced = run(binary, ["dna", "sonic", "build", "--input", str(request), "--output-dir", str(output)], events)
    require(produced["state"] == "complete" and produced["runtime_qualified"] is False
            and produced["review_state"] == "candidate" and produced["publication_approval"] == "not_granted",
            "Sonic candidate acquired unexpected authority")
    receipt_sha = sha((output / "sonic-run.json").read_bytes())
    status_args = ["dna", "sonic", "status", "--directory", str(output),
                   "--receipt-sha256", receipt_sha, "--input", str(request)]
    status = run(binary, status_args, events)
    require(status["source_state"] == "current" and status["runtime_qualified"] is False,
            "retained candidate is not source-current or promoted authority")
    description = json.loads((output / "audio-description.json").read_text())
    require(description["mode"] == "deterministic_evidence_render" and description["human_review_required"] is True,
            "model/human state differs")
    dna = json.loads((output / "sonic-dna.json").read_text())
    facts = {fact["dimension"]: fact for fact in dna["provider_extensions"]["renderflow.sonic"]["facts"]}
    require(facts["sonic.rhythm.tempo_bpm"]["class"] == "heuristic"
            and facts["sonic.rhythm.tempo_bpm"]["confidence"] == {"kind": "uncalibrated", "score": 0.75}
            and facts["sonic.harmony.key"]["class"] == "probabilistic"
            and facts["sonic.harmony.key"]["confidence"]["kind"] == "unavailable",
            "supplied musical confidence/class was promoted or lost")
    companion = facts["sonic.lyrics.declared_timed_text_structure"]["value"]
    require(companion["cue_count"] == 2 and companion["words_exposed"] is False
            and companion["source_frame_range"] == {"start": 5513, "end": frames},
            "manual cue structure differs from the original sample clock")
    require(dna["approval"] == {"state": "candidate", "human_review_required": True},
            "Sonic DNA acquired human approval")
    original = inventory(output)
    require(all(not any(marker in path.read_bytes() for marker in
                        (b"PRIVATE_LYRIC_CANARY", b"Synthetic second cue"))
                for path in output.rglob("*") if path.is_file()),
            "private cue words leaked into retained candidate")
    run(binary, ["dna", "sonic", "build", "--input", str(request), "--output-dir", str(output)],
        events, success=False, expected_error="sonic.output.exists")
    tampered = work / "tampered"
    shutil.copytree(output, tampered)
    with (tampered / "audio-description.md").open("ab") as writer:
        writer.write(b"\nsubstituted output\n")
    run(binary, ["dna", "sonic", "status", "--directory", str(tampered), "--receipt-sha256", receipt_sha],
        events, success=False, expected_error="sonic.readback.file.changed")
    forged = work / "forged-authority"
    shutil.copytree(output, forged)
    forged_run = json.loads((forged / "sonic-run.json").read_text())
    forged_run["runtime_qualified"] = True
    write_json(forged / "sonic-run.json", forged_run)
    run(binary, ["dna", "sonic", "status", "--directory", str(forged),
                 "--receipt-sha256", sha((forged / "sonic-run.json").read_bytes())],
        events, success=False, expected_error="sonic.readback.run.contract")
    stale_recipe = dict(recipe)
    stale_recipe["id"] = "different-explicit-synthetic-intent"
    # Keep every relative source reference valid: this trial changes only the
    # explicit request identity, rather than moving the recipe away from inputs.
    stale_path = inputs / "stale-request.json"
    write_json(stale_path, stale_recipe)
    stale = run(binary, ["dna", "sonic", "status", "--directory", str(output),
                        "--receipt-sha256", receipt_sha, "--input", str(stale_path)], events)
    require(stale["source_state"] == "stale" and stale["publication_approval"] == "not_granted",
            "changed explicit intent was treated as current")
    stale_path.unlink()
    require(inventory(inputs) == source_before and inventory(output) == original, "source or original retained candidate changed")
    require(sha(binary.read_bytes()) == binary_sha, "selected executable changed")
    report = {"schema_version": "renderflow.sonic-local-pilot/v1", "binary_sha256": binary_sha,
        "executed_at_utc": datetime.now(timezone.utc).isoformat(),
        "pilot_script_sha256": sha(Path(__file__).read_bytes()),
        "recipe_template_sha256": sha((ROOT / "examples/sonic-dna/sonic.json").read_bytes()),
        "analysis_template_sha256": sha((ROOT / "examples/sonic-dna/analysis.json").read_bytes()),
        "technical": expected, "source_inventory": source_before, "candidate_inventory": original,
        "receipt_sha256": receipt_sha, "retained_status": status,
        "private_companion_words_retained_in_inputs_only": True,
        "external_provider_or_model_invoked": False, "analyzer_values": "supplied_synthetic_assertions_only",
        "human_approval_granted": False, "publication_performed": False,
        "tampered_output_refused": True, "forged_authority_refused": True,
        "existing_output_preserved": True, "stale_request_reported": True,
        "musical_confidence_preserved": True, "manual_cue_source_frame_range": companion["source_frame_range"],
        "events": events}
    write_json(work / "pilot-report.json", report)
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--work-directory", type=Path, required=True)
    args = parser.parse_args()
    result = pilot(args.binary, args.work_directory.absolute())
    print(json.dumps({"state": "selected_synthetic_cli_checks_passed", "binary_sha256": result["binary_sha256"],
                      "publication_performed": False}, sort_keys=True))
