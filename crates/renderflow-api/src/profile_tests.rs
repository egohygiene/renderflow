//! Authored/unrun profile-boundary and real SDK publication regressions.
//! No editorial job or external provider is invoked by these cases.

use super::*;
use std::fs;
use std::time::{Duration,Instant};
use serde_json::{json,Value};
use crate::Service;

struct Fixture {root:tempfile::TempDir,config:ServiceConfig,spec:Value}
impl Fixture {
    fn new(profile:PipelineProfile)->Self {
        let root=tempfile::tempdir().unwrap();let base=root.path().canonicalize().unwrap();
        fs::create_dir(base.join("source")).unwrap();fs::create_dir(base.join("work")).unwrap();
        let (bytes,format):(&[u8],&str)=match profile {
            PipelineProfile::FountainBody=>(include_bytes!("../../../examples/creative-forest/sources/original.fountain"),"fountain"),
            PipelineProfile::Editorial=>(include_bytes!("../../../examples/editorial/article.md"),"markdown"),
            _=>(include_bytes!("../../../examples/lyrics/manual.srt"),"srt"),
        };
        fs::write(base.join("source/input"),bytes).unwrap();
        let mut spec=json!({"schema":"renderflow/v2","sources":[{"id":"source","path":"input","format":format}],
            "targets":{"all_reachable":true},"execution":{"network":"deny","ai":"deny"},"output":{"bundle_root":"operator-original-output"}});
        match profile {
            PipelineProfile::ManualLyrics=>spec["execution"]["lyrics"]=json!({"source_sha256":digest(bytes),"review_state":"unreviewed","language":"fr"}),
            PipelineProfile::TimedText=>spec["execution"]["subtitles"]=json!({}),
            PipelineProfile::FountainBody=>spec["execution"]["screenplay_body"]=json!({"allow_unqualified":true}),
            PipelineProfile::Editorial=>spec["targets"]=json!({"profiles":["editorial"]}),
            _=>{},
        }
        fs::write(base.join("source/renderflow.json"),serde_json::to_vec(&spec).unwrap()).unwrap();
        let config=ServiceConfig {schema_version:SERVICE_SCHEMA.into(),workspace_parent:base.join("work"),
            pipelines:vec![PipelineConfig{id:"source".into(),root:base.join("source"),config:"renderflow.json".into(),
                files:vec!["renderflow.json".into(),"input".into()],profile,allow_local_processes:profile.local_processes()}],limits:Limits::default()};
        Self {root,config,spec}
    }
    fn rewrite(&mut self,change:impl FnOnce(&mut Value)) {
        change(&mut self.spec);fs::write(self.config.pipelines[0].root.join("renderflow.json"),serde_json::to_vec(&self.spec).unwrap()).unwrap();
    }
    fn service(&self)->ApiResult<Service> {Service::new(self.config.clone(),self.root.path())}
}
fn wait(service:&Service,id:&str)->JobView {
    let deadline=Instant::now()+Duration::from_secs(20);
    loop {let view=service.job(id).unwrap();if view.state.terminal(){return view;}
        assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));}
}
fn execute(service:&Service)->JobView {
    let job=service.submit(Operation::Execute,SubmitRequest{pipeline_id:"source".into(),dry_run:false}).unwrap();wait(service,&job.job_id)
}
fn downloaded(service:&Service,id:&str,format:&str)->Vec<u8> {
    let rows=service.artifacts(id).unwrap();let row=rows.iter().find(|row|row.format==format).unwrap();
    let result=service.download(id,&row.id).unwrap();assert_eq!(result.sha256,row.sha256);result.bytes
}

#[test]
fn manual_srt_lyrics_forest_retains_exact_supplied_words_and_millisecond_timing() {
    let fixture=Fixture::new(PipelineProfile::ManualLyrics);let original=fs::read(fixture.config.pipelines[0].root.join("input")).unwrap();
    let service=fixture.service().unwrap();let job=execute(&service);assert_eq!(job.state,JobState::Complete);
    assert_eq!(job.profile,PipelineProfile::ManualLyrics.name());assert!(!job.local_processes);
    let doc=renderflow::lyrics::parse_document(&downloaded(&service,&job.job_id,"lyrics_json")).unwrap();
    assert_eq!(doc.transcript.cues.len(),3);assert_eq!(doc.transcript.cues[0].start_ms,1123);assert_eq!(doc.transcript.cues[0].end_ms,3456);
    assert_eq!(doc.transcript.cues[0].text,"Sous la lune\nEncore ici");assert_eq!(doc.transcript.cues[1].text,"Sous la lune");
    assert_eq!(doc.policy.review_state,renderflow::lyrics::ReviewState::Unreviewed);
    let rows=service.artifacts(&job.job_id).unwrap();assert!(!rows.iter().any(|row|row.format=="srt"));
    assert!(rows.iter().any(|row|row.format=="vtt") && rows.iter().any(|row|row.format=="ttml"));
    assert_eq!(fs::read(fixture.config.pipelines[0].root.join("input")).unwrap(),original);
    assert!(!fixture.config.pipelines[0].root.join("operator-original-output").exists());
}

#[test]
fn standalone_timed_text_forest_keeps_native_evidence_and_uses_binary_downloads() {
    let fixture=Fixture::new(PipelineProfile::TimedText);let service=fixture.service().unwrap();let job=execute(&service);
    assert_eq!(job.state,JobState::Complete);let bytes=downloaded(&service,&job.job_id,"timed_transcript_json");
    let doc:renderflow::timed_text::TimedTranscript=serde_json::from_slice(&bytes).unwrap();
    assert_eq!(doc.cues[2].end_ms,7890);assert_eq!(doc.cues[2].text,"Nous gardons la lumière");
    assert_eq!(doc.source.sha256,digest(include_bytes!("../../../examples/lyrics/manual.srt")));
    let rows=service.artifacts(&job.job_id).unwrap();assert!(!rows.iter().any(|row|row.format=="srt"));
    assert!(String::from_utf8(downloaded(&service,&job.job_id,"vtt")).unwrap().starts_with("WEBVTT"));
}

#[test]
fn original_fountain_exports_retain_finite_native_body_routes_and_file_identity() {
    let fixture=Fixture::new(PipelineProfile::FountainBody);let service=fixture.service().unwrap();let job=execute(&service);
    assert_eq!(job.state,JobState::Complete);let rows=service.artifacts(&job.job_id).unwrap();assert_eq!(rows.len(),5);
    for row in rows {let download=service.download(&job.job_id,&row.id).unwrap();assert!(!download.bytes.is_empty());assert_eq!(download.sha256,row.sha256);}
    let text=String::from_utf8(downloaded(&service,&job.job_id,"fdx")).unwrap();assert!(text.contains("Keep the light on."));
    let JobOutput::Execute(result)=job.result.unwrap() else {panic!("native execution missing")};
    assert!(result.run_manifest.artifact_manifest.artifacts.iter().any(|artifact|artifact.fidelity!=renderflow::evidence::FidelityDeclaration::Lossless));
}

#[test]
fn extra_process_network_ai_and_custom_registry_effects_are_refused_before_planning() {
    for change in ["remote","ai","provider","registry","media","validator","font"] {
        let mut fixture=Fixture::new(PipelineProfile::TimedText);
        fixture.rewrite(|spec|match change {
            "remote"=>spec["execution"]["network"]=json!("allow"),
            "ai"=>spec["execution"]["ai"]=json!("allow"),
            "provider"=>spec["execution"]["tools"]=json!({"allow":["tool.ffmpeg"]}),
            "registry"=>spec["transforms"]=json!("outside.yaml"),
            "media"=>spec["execution"]["subtitles"]=json!({"stream_index":0}),
            "validator"=>spec["execution"]["validation"]=json!({"validators":["external.custom"]}),
            _=>spec["variables"]=json!({"renderflow-font-registry":"/outside/fonts.json"}),
        });
        assert!(fixture.service().is_err(),"unadmitted effect {change}");
    }
}

#[test]
fn graph_source_paths_custom_profiles_and_templates_cannot_escape_capture() {
    for change in ["path","uri","profile","template","output"] {
        let mut fixture=Fixture::new(PipelineProfile::TimedText);
        fixture.rewrite(|spec|match change {
            "path"=>spec["sources"][0]["path"]=json!("../outside.srt"),
            "uri"=>spec["sources"][0]["uri"]=json!("file:///outside.srt"),
            "profile"=>spec["profiles"]=json!({"custom":{"targets":[{"format":"pdf"}]}}),
            "template"=>spec["targets"]=json!({"exact":[{"format":"text","template":"/outside/template"}]}),
            _=>spec["output"]["naming_template"]=json!("../outside.{ext}"),
        });assert!(fixture.service().is_err(),"capture escape {change}");
    }
}

#[test]
fn editorial_process_permission_is_explicit_and_capture_does_not_probe_a_provider() {
    let mut fixture=Fixture::new(PipelineProfile::Editorial);fixture.config.pipelines[0].allow_local_processes=false;
    assert_eq!(fixture.service().err().unwrap().code,"process_policy");
    fixture.config.pipelines[0].allow_local_processes=true;let service=fixture.service().unwrap();
    assert!(service.ready());assert_eq!(service.active_jobs().unwrap(),0);assert!(service.pipelines()[0].local_processes);
    let mut builtin=Fixture::new(PipelineProfile::TimedText);builtin.config.pipelines[0].allow_local_processes=true;assert!(builtin.service().is_err());
}

#[test]
fn profile_defaults_and_effect_permission_are_bound_into_package_identity() {
    let legacy:PipelineConfig=serde_json::from_value(json!({"id":"old","root":"source","config":"renderflow.json","files":["renderflow.json"]})).unwrap();
    assert_eq!(legacy.profile,PipelineProfile::Composition);assert!(!legacy.allow_local_processes);
    let fixture=Fixture::new(PipelineProfile::ManualLyrics);let service=fixture.service().unwrap();
    let package=service.pipelines()[0].package_sha256.clone();drop(service);
    let mut changed=fixture.config.clone();changed.pipelines[0].profile=PipelineProfile::TimedText;
    assert!(Service::new(changed,fixture.root.path()).is_err());
    let second=fixture.service().unwrap();assert_eq!(second.pipelines()[0].package_sha256,package);
}

#[test]
fn graph_dry_run_never_publishes_and_retains_bounded_plan_semantics() {
    let fixture=Fixture::new(PipelineProfile::ManualLyrics);let service=fixture.service().unwrap();
    let job=service.submit(Operation::Execute,SubmitRequest{pipeline_id:"source".into(),dry_run:true}).unwrap();let job=wait(&service,&job.job_id);
    assert_eq!(job.state,JobState::Planned);assert!(service.artifacts(&job.job_id).unwrap().is_empty());
    let JobOutput::Execute(result)=job.result.unwrap() else {panic!("native result missing")};
    assert!(result.run_manifest.published_targets().unwrap().is_empty());
    assert!(result.diagnostics.fatal_failures.is_empty());
}
