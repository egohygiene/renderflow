#!/usr/bin/env python3
"""Observe the existing canonical manual-lyrics profile on authored SRT only.

Requires Python3 + PyYAML for an independent YAML readback. No provider/model,
audio recognition, alignment, player qualification or human approval is selected.
"""
from __future__ import annotations
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import selectors
import re
import signal
from pathlib import Path
import stat
import subprocess
import time
import xml.etree.ElementTree as ET
import yaml

SRT = ("7\r\n00:00:00,125 --> 00:00:00,875\r\nBonjour, lune 🌙\r\nПривет, мир\r\n\r\n"
       "9\r\n00:00:00,650 --> 00:00:01,250\r\nEncore la lune.\r\n\r\n"
       "13\r\n00:00:01,500 --> 00:00:02,375\r\nEncore la lune.\r\n").encode()
CUES = [("7",125,875,"Bonjour, lune 🌙\nПривет, мир"),
        ("9",650,1250,"Encore la lune."),("13",1500,2375,"Encore la lune.")]
SPECIAL = ("Error: lyrics.input.capture\n\nCaused by:\n"
           "    subtitle input must be a regular file, not a symlink or special file\n").encode()
OVERSIZE = ("Error: lyrics.input.capture\n\nCaused by:\n"
            "    subtitle input exceeds the 16 MiB limit\n").encode()


def require(value, message):
    if not value: raise ValueError(message)


def digest(data): return hashlib.sha256(data).hexdigest()


def file_hash(path):
    value = hashlib.sha256()
    with path.open("rb") as reader:
        for block in iter(lambda:reader.read(1024*1024),b""): value.update(block)
    return value.hexdigest()


def inventory(directory):
    observed={}
    for path in sorted(directory.rglob("*")):
        mode=path.lstat().st_mode
        if stat.S_ISDIR(mode): continue
        require(stat.S_ISREG(mode), "unexpected special file in accepted tree")
        observed[str(path.relative_to(directory))]={"bytes":path.stat().st_size,"sha256":file_hash(path)}
    return observed


def capture(argv, cwd, maximum=2*1024*1024, timeout=30):
    """Bound both pipe captures during execution; kill/reap the process group."""
    process=subprocess.Popen(argv,cwd=cwd,env=dict(os.environ,NO_COLOR="1",TERM="dumb"),
        stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,start_new_session=True)
    streams=selectors.DefaultSelector(); buffers={"stdout":bytearray(),"stderr":bytearray()}
    completed=False
    try:
        for name in buffers:
            stream=getattr(process,name);os.set_blocking(stream.fileno(),False)
            streams.register(stream,selectors.EVENT_READ,name)
        deadline=time.monotonic()+timeout; total=0
        while streams.get_map():
            remaining=deadline-time.monotonic()
            require(remaining>0,"selected CLI observation timed out")
            for key,_ in streams.select(min(remaining,0.1)):
                data=os.read(key.fileobj.fileno(),65536)
                if not data: streams.unregister(key.fileobj);continue
                total+=len(data); require(total<=maximum,"selected CLI output exceeded capture limit")
                buffers[key.data].extend(data)
        remaining=deadline-time.monotonic();require(remaining>0,"selected CLI exit timed out")
        code=process.wait(timeout=remaining)
        completed=True
        return subprocess.CompletedProcess(argv,code,bytes(buffers["stdout"]),bytes(buffers["stderr"]))
    finally:
        try:os.killpg(process.pid,signal.SIGKILL)
        except ProcessLookupError:pass
        if process.poll() is None:process.wait()
        streams.close()
        for name in buffers:getattr(process,name).close()


def put_json(path, data): path.write_text(json.dumps(data,indent=2,sort_keys=True)+"\n",encoding="utf-8")


def clock(ms): return f"{ms//3600000:02}:{ms//60000%60:02}:{ms//1000%60:02}.{ms%1000:03}"


def pilot(binary, work):
    observer_hash=file_hash(Path(__file__))
    binary=binary.resolve(strict=True)
    require(binary.is_file(),"regular selected binary required")
    binary_hash=file_hash(binary)
    require(work.parent.resolve()==work.parent and work.parent.is_dir(),"canonical existing work parent required")
    work.mkdir(mode=0o700)
    inputs=work/"inputs";inputs.mkdir(mode=0o700)
    source=inputs/"manual 歌詞.srt";source.write_bytes(SRT)
    config={"schema":"renderflow/v2","sources":[{"id":"manual-lyrics","path":source.name,"format":"srt"}],
        "targets":{"profiles":["lyrics"]},"execution":{"lyrics":{"source_sha256":digest(SRT),
        "review_state":"unreviewed","language":"fr","revision":"synthetic-akas-r1",
        "sections":{"7":"verse","9":"refrain","13":"refrain"}},
        "derivative_planning":{"max_hops":4,"max_candidates":128}},"output":{"bundle_root":"../published"}}
    config_path=inputs/"renderflow.json";put_json(config_path,config)
    source_before=inventory(inputs)
    events=[]
    def run(name,args,code=0,stderr=None):
        started=time.monotonic()
        result=capture([str(binary),*map(str,args)],inputs)
        (work/(name+".stdout")).write_bytes(result.stdout)
        (work/(name+".stderr")).write_bytes(result.stderr)
        require(result.returncode==code,"different native exit for "+name)
        require(len(result.stdout)+len(result.stderr)<=2*1024*1024,"CLI capture exceeded selected scope")
        if stderr is not None:
            require(result.stdout==b"" and result.stderr.endswith(stderr),"different owning native refusal for "+name)
            prefix=result.stderr[:-len(stderr)]
            # Build intentionally emits one informational resolved-plan line.
            # Admit that exact owner/shape only; no arbitrary prefix/panic/error.
            allowed=rb"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{6}Z  INFO renderflow::commands::build: Resolved canonical execution plan source=srt targets=(?:text|lrc, lyrics_json, markdown, srt, text, ttml, vtt, yaml) depth=2 waves=2\n"
            require(prefix==b"" or re.fullmatch(allowed,prefix) is not None,"unexpected native diagnostic prefix for "+name)
        events.append({"name":name,"argv":[str(v).replace(str(work),"{work}") for v in args],
            "exit_code":result.returncode,"elapsed_ms":round((time.monotonic()-started)*1000),
            "stdout_sha256":digest(result.stdout),"stderr_sha256":digest(result.stderr)})
        return result
    inspected=run("inspect",["lyrics","inspect","--input",source,"--language","fr","--revision","synthetic-akas-r1"])
    inspection=json.loads(inspected.stdout)
    require(inspection["cues"]==3 and inspection["policy"]["source_sha256"]==digest(SRT)
        and inspection["policy"]["review_state"]=="unreviewed"
        and inspection["eligibility"]["recognition_or_alignment"] is False
        and inspection["runtime_qualified"] is False,"inspection changed timing/review authority")
    run("build",["build","--config",config_path])
    output=work/"published"
    manifest_path=output/"renderflow-run.json"
    manifest=json.loads(manifest_path.read_bytes())
    require(manifest["state"]=="complete","canonical lyrics run is not complete")
    records=[row for row in manifest["artifact_manifest"]["artifacts"] if row["lifecycle"]=="terminal"]
    require(len(records)==8 and len(manifest["artifact_manifest"]["outputs"])==8,"native profile did not publish exactly8 targets")
    formats={"lyrics_json","yaml","srt","vtt","lrc","ttml","text","markdown"}
    require({r["format"] for r in records}==formats and len({r["artifact_id"] for r in records})==8
        and len({r["locator"] for r in records})==8,"terminal formats, IDs or locators are not exact and unique")
    original=[r for r in manifest["artifact_manifest"]["artifacts"] if r["lifecycle"]=="source"]
    require(len(original)==1 and original[0]["format"]=="srt" and original[0]["sources"]==[]
        and original[0]["digest"]["value"]==digest(SRT),"original lineage is not the authored source")
    canonical_row=next(r for r in records if r["format"]=="lyrics_json")
    require(canonical_row["sources"]==[original[0]["artifact_id"]],"canonical is detached from original")
    expected_outputs={(output/row["locator"][7:]).resolve() for row in records}
    require({(inputs/Path(path)).resolve() for path in manifest["artifact_manifest"]["outputs"]}==expected_outputs,"output list differs from exact selected locators")
    paths={}
    losses={}
    for row in records:
        require(row["locator"].startswith("bundle:") and row["metadata"]["renderflow.target.published"] is True
                and row["validation"]=="valid","unpublished or unvalidated native target")
        path=output/row["locator"][7:]
        require(path.resolve().is_relative_to(output) and path.is_file(),"target outside selected bundle")
        require(file_hash(path)==row["digest"]["value"] and path.stat().st_size==row["size_bytes"],"publication differs from its record")
        paths[row["format"]]=path
        if row["format"]=="srt":
            require(row["artifact_id"]==original[0]["artifact_id"] and row["sources"]==[],"source-equivalent SRT is not the original artifact")
        else:
            if row["format"]!="lyrics_json":require(row["sources"]==[canonical_row["artifact_id"]],"derivative is detached from canonical")
            binding=[v for v in row["validation_evidence"] if v["validator_id"]=="validator.lyrics.source_binding"]
            require(len(binding)==1 and binding[0]["state"]=="valid" and binding[0]["provider"]=="renderflow.core","mandatory exact source/rerender validation missing")
            metadata=row["metadata"]["renderflow.lyrics"]
            require(metadata["source_sha256"]==digest(SRT) and metadata["review_state"]=="unreviewed"
                and metadata["review_is_declared"] is True and metadata["runtime_qualified"] is False,
                "native output lost source or promoted review")
            losses[row["format"]]=metadata["losses"]
    canonical=json.loads(paths["lyrics_json"].read_bytes())
    actual=[(v["id"],v["start_ms"],v["end_ms"],v["text"]) for v in canonical["transcript"]["cues"]]
    require(actual==CUES and canonical["transcript"]["overlap_policy"]=="preserve", "cue IDs/text/clocks/overlap changed")
    require(canonical["policy"]==config["execution"]["lyrics"] and canonical["transcript"]["language"]=="fr",
            "explicit policy changed")
    require(yaml.safe_load(paths["yaml"].read_bytes())==canonical,"independent YAML readback differs")
    require(paths["srt"].read_bytes()==SRT,"source-equivalent SRT changed original bytes")
    plain="\n\n".join(c[3] for c in CUES)+"\n"
    require(paths["text"].read_text()==plain,"plain text lost repeated/multiline words")
    require(paths["markdown"].read_text()=="".join("    "+line+"\n" for line in plain.splitlines()),"Markdown is not literal indented authored text")
    vtt="WEBVTT\n\n"+"".join(f"{i}\n{clock(begin)} --> {clock(end)}\n{words}\n\n" for i,begin,end,words in CUES)
    require(paths["vtt"].read_text()==vtt,"VTT changed IDs/clocks/text")
    lrc="".join(f"[{begin//60000:02}:{begin//1000%60:02}.{begin%1000:03}]{line}\n"
        for _,begin,_,words in CUES for line in words.split("\n"))
    require(paths["lrc"].read_text()==lrc,"LRC changed line starts or words")
    xml=ET.fromstring(paths["ttml"].read_bytes());ns={"t":"http://www.w3.org/ns/ttml"}
    require(xml.tag=="{http://www.w3.org/ns/ttml}tt" and xml.attrib["{http://www.w3.org/XML/1998/namespace}lang"]=="fr","TTML namespace/language changed")
    paragraphs=xml.findall("./t:body/t:div/t:p",ns)
    require(len(paragraphs)==len(CUES),"TTML cue count differs")
    for index,(node,(_,begin,end,words)) in enumerate(zip(paragraphs,CUES),1):
        text=node.text or ""
        for child in node:
            require(child.tag=="{http://www.w3.org/ns/ttml}br","TTML added nonnative content")
            text+="\n"+(child.tail or "")
        require(node.attrib["{http://www.w3.org/XML/1998/namespace}id"]==f"cue-{index}"
            and node.attrib["begin"]==clock(begin) and node.attrib["end"]==clock(end) and text==words,"TTML timing/words differ")
    for kind in ("text","markdown"):
        require("timed_text.Timing" in losses[kind] and "timed_text.CueIds" in losses[kind],"plain export hid timing/ID losses")
    require("cue_end_times_not_represented" in losses["lrc"]
        and "multiline_cue_grouping_not_represented" in losses["lrc"],"LRC hid unrepresented end times/grouping")
    for row in records:
        if row["format"]!="srt":require(row["metadata"]["renderflow.lyrics"]["canonical_sha256"]==file_hash(paths["lyrics_json"]),"canonical evidence detached")
    stores={json.dumps(r["metadata"]["renderflow.lyrics.artifact_store"],sort_keys=True) for r in manifest["artifact_manifest"]["artifacts"]}
    require(len(stores)==1,"manual artifacts disagree on retained store")
    store_info=json.loads(next(iter(stores)))
    require(store_info["relative_to"]=="bundle_parent" and store_info["path"].startswith(".renderflow/manual-lyrics-"),"retained store binding is not explicit and isolated")
    store=output.parent/store_info["path"]
    require(store.resolve().is_relative_to(work/".renderflow"),"retained store escaped selected state")
    for row in manifest["artifact_manifest"]["artifacts"]:
        if row["locator"].startswith("artifact-store:"):
            payload=store/row["locator"][15:]
            require(payload.is_file() and file_hash(payload)==row["digest"]["value"] and payload.stat().st_size==row["size_bytes"],"retained original readback failed")
    before_output=inventory(output);before_state=inventory(work/".renderflow")
    exists=b"Error: lyrics.output.preflight\n\nCaused by:\n    publication.output.exists: refusing to replace an existing destination\n"
    symlink=b"Error: lyrics.output.preflight\n\nCaused by:\n    publication.output.symlink: output paths must not traverse existing symlinks\n"
    alias=b"Error: lyrics.output.preflight\n\nCaused by:\n    publication.output.original_alias: source and configuration must remain outside output and state roots\n"
    duplicate=b"Error: lyrics.output.preflight\n\nCaused by:\n    publication.output.path_alias: selected payload and evidence paths must be distinct inside the bundle\n"
    run("rerun-no-clobber",["build","--config",config_path],1,exists)
    require(inventory(output)==before_output and inventory(work/".renderflow")==before_state,"refused rerun changed accepted bundle/state")
    cases=work/"cases";cases.mkdir()
    def selected(name, destination, naming="selected.txt", chosen_source=source, publication=False):
        case=cases/name;case.mkdir()
        spec=json.loads(json.dumps(config));spec["sources"][0]["path"]=str(chosen_source)
        spec["targets"]={"exact":[{"id":"plain","role":"lyrics/plain-text","format":"text"}]}
        spec["output"]={"bundle_root":str(destination),"naming_template":naming}
        if publication:spec["publication"]={"publication":"Synthetic authored fixture","issue_id":"fixture-1","title":"Manual lyrics fixture","publication_date":"2026-11-14","language":"fr","geometry":{"width":100,"height":100,"unit":"mm"}}
        path=case/"renderflow.json";put_json(path,spec);return path
    occupied=cases/"occupied-output";occupied.mkdir();sentinel=occupied/"selected.txt";sentinel.write_bytes(b"original sentinel\x00bytes")
    sentinel_hash=file_hash(sentinel);occupied_config=selected("occupied",occupied)
    run("occupied-no-clobber",["build","--config",occupied_config],1,exists)
    require(file_hash(sentinel)==sentinel_hash and inventory(occupied)=={"selected.txt":{"bytes":len(b"original sentinel\x00bytes"),"sha256":sentinel_hash}},"occupied output changed or acquired evidence")
    alias_root=cases/"alias-output";alias_root.mkdir();alias_leaf=alias_root/"selected.txt";alias_leaf.symlink_to(source)
    alias_config=selected("source-leaf-alias",alias_root)
    run("source-leaf-alias-refused",["build","--config",alias_config],1,exists)
    require(alias_leaf.is_symlink() and alias_leaf.readlink()==source and file_hash(source)==digest(SRT)
        and sorted(p.name for p in alias_root.iterdir())==["selected.txt"],"source alias changed or acquired evidence")
    external=cases/"external";external.mkdir();(external/"private-sentinel").write_bytes(b"external original")
    external_before=inventory(external);parent_link=cases/"linked-parent";parent_link.symlink_to(external,target_is_directory=True)
    parent_config=selected("parent-alias",parent_link/"new-bundle")
    run("parent-alias-refused",["build","--config",parent_config],1,symlink)
    require(inventory(external)==external_before and sorted(p.name for p in external.iterdir())==["private-sentinel"],"parent alias added or changed external files")
    manifest_root=cases/"manifest-output";manifest_root.mkdir();manifest_sentinel=manifest_root/"renderflow-run.json";manifest_sentinel.write_bytes(b"original manifest sentinel")
    manifest_before=inventory(manifest_root);manifest_config=selected("manifest-occupied",manifest_root)
    run("manifest-no-clobber",["build","--config",manifest_config],1,exists)
    require(inventory(manifest_root)==manifest_before,"manifest original changed")
    collision_config=selected("manifest-collision",cases/"collision-output",naming="renderflow-run.json")
    run("manifest-path-collision",["build","--config",collision_config],1,duplicate)
    require(not (cases/"collision-output").exists(),"reserved manifest collision created output")
    original_root=cases/"original-output";original_root.mkdir();nested_source=original_root/"original.srt";nested_source.write_bytes(SRT)
    original_config=selected("original-nested",original_root,chosen_source=nested_source)
    run("original-root-alias",["build","--config",original_config],1,alias)
    require(inventory(original_root)=={"original.srt":{"bytes":len(SRT),"sha256":digest(SRT)}},"original nested under output changed")
    for sidecar in ("publication.json","manifest.json","provenance.json","preflight.json","checksums.sha256"):
        side_root=cases/("sidecar-"+sidecar);(side_root/"metadata").mkdir(parents=True)
        protected=side_root/"metadata"/sidecar;protected.write_bytes(b"original metadata sentinel")
        side_before=inventory(side_root);side_config=selected("case-"+sidecar,side_root,publication=True)
        run("sidecar-"+sidecar+"-refused",["build","--config",side_config],1,exists)
        require(inventory(side_root)==side_before,"metadata original changed")
    stale_config=selected("stale",cases/"stale-output")
    stale=json.loads(stale_config.read_bytes());stale["execution"]["lyrics"]["source_sha256"]="0"*64;put_json(stale_config,stale)
    run("stale-source-refused",["build","--config",stale_config],1,b"Error: lyrics.source.changed: intake differs from explicit manual-source binding\n")
    require(not (cases/"stale-output").exists(),"stale source created output")
    require(inventory(work/".renderflow")==before_state,"refused controls altered accepted retained state")
    special=work/"special";special.mkdir(mode=0o700)
    linked=special/"link.srt";linked.symlink_to(source)
    fifo=special/"pipe.srt";os.mkfifo(fifo)
    directory=special/"directory.srt";directory.mkdir()
    oversized=special/"oversize.srt"
    with oversized.open("wb") as writer:writer.truncate(16*1024*1024+1)
    for name,path in (("symlink",linked),("fifo",fifo),("directory",directory)):
        run(name+"-refused",["lyrics","inspect","--input",path],1,SPECIAL)
    run("oversize-refused",["lyrics","inspect","--input",oversized],1,OVERSIZE)
    portable=work/"portable"
    exported=json.loads(run("bundle-export",["bundle","export","--manifest",manifest_path,"--manifest-sha256",file_hash(manifest_path),
        "--bundle-root",output,"--artifact-store-root",store,"--output-dir",portable,"--include-sources"]).stdout)
    require(exported["run_state"]=="complete" and exported["record_count"]==9 and exported["runtime_qualified"] is False,
        "native portable export changed source lineage/state/authority")
    verified=json.loads(run("bundle-verify-portable",["bundle","verify-portable","--bundle-root",portable,"--index-sha256",exported["index_sha256"]]).stdout)
    require(verified["observed_records"]==9 and verified["external_source_records"]==0 and len(verified["publications"])==8
        and verified["runtime_qualified"] is False and verified["publisher_authenticated"] is False
        and verified["creative_approval_inferred"] is False,"portable readback detached records or promoted authority")
    require(inventory(output)==before_output and inventory(work/".renderflow")==before_state,"export changed native originals/state")
    require(inventory(inputs)==source_before,"manual source or configuration changed")
    require(file_hash(binary)==binary_hash,"selected executable changed")
    require(file_hash(Path(__file__))==observer_hash,"selected observer changed")
    report={"schema_version":"renderflow.manual-lyrics-candidate-proof/v1",
        "observed_at_utc":datetime.now(timezone.utc).isoformat(),"observer_sha256":observer_hash,
        "binary_sha256":binary_hash,"source_sha256":digest(SRT),"canonical_sha256":file_hash(paths["lyrics_json"]),
        "yaml_observer_version":yaml.__version__,"cue_count":len(CUES),"cue_timing_ms":[[c[1],c[2]] for c in CUES],
        "published_formats":sorted(paths),"declared_losses":losses,"events":events,
        "output_inventory":inventory(output),"input_inventory":source_before,
        "portable_inventory":inventory(portable),"portable_export":exported,"portable_verification":verified,
        "retained_store":store_info,"retained_store_inventory":inventory(store),
        "protected_sentinel_sha256":sentinel_hash,"no_clobber_and_alias_controls_passed":True,
        "review_state":"unreviewed","recognition_or_alignment":False,"human_approval_granted":False,
        "external_provider_or_model_executed":False,"full_player_or_schema_qualification":False,"remote_publication_performed":False}
    put_json(work/"report.json",report)
    return report


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary",type=Path,required=True)
    parser.add_argument("--work-directory",type=Path,required=True)
    args=parser.parse_args()
    report=pilot(args.binary,args.work_directory.absolute())
    print(json.dumps({"state":"selected_native_manual_lyrics_checks_passed","binary_sha256":report["binary_sha256"],
                      "formats":report["published_formats"],"human_approval_granted":False},sort_keys=True))
