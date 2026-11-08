//! Authored/uncompiled/unrun finite publication admission and SDK cases.
use super::*;
use crate::Service;
use serde_json::{json,Value};
use std::{fs,time::{Instant,Duration}};
struct Fixture {root:tempfile::TempDir,config:ServiceConfig,spec:Value}
impl Fixture {
    fn new()->Self {
        let root=tempfile::tempdir().unwrap();let base=root.path().canonicalize().unwrap();
        fs::create_dir_all(base.join("source/episode")).unwrap();fs::create_dir(base.join("work")).unwrap();
        fs::write(base.join("source/episode/briefing.json"),include_bytes!("../../../examples/briefing/briefing.json")).unwrap();
        fs::write(base.join("source/episode/canonical.txt"),include_bytes!("../../../examples/briefing/canonical.txt")).unwrap();
        let spec=json!({"schema":"renderflow/v2","sources":[{"id":"briefing","kind":"briefing","path":"episode/briefing.json"}],
            "targets":{"exact":[{"format":"briefing_text"},{"format":"briefing_script"}]},"execution":{"briefing":{"allow_draft":true}},"output":{"bundle_root":"operator-original-output"}});
        let config=ServiceConfig {schema_version:SERVICE_SCHEMA.into(),workspace_parent:base.join("work"),persistence_directory:None,retain_completed_artifacts:false,
            pipelines:vec![PipelineConfig {id:"briefing".into(),root:base.join("source"),config:"renderflow.json".into(),files:vec!["renderflow.json".into(),"episode/briefing.json".into(),"episode/canonical.txt".into()],profile:PipelineProfile::Briefing,allow_local_processes:false,allow_source_uploads:false}],limits:Limits::default()};
        let fixture=Self {root,config,spec};fixture.write();fixture
    }
    fn write(&self) {fs::write(self.config.pipelines[0].root.join("renderflow.json"),serde_json::to_vec(&self.spec).unwrap()).unwrap();}
    fn service(&self)->ApiResult<Service>{Service::new(self.config.clone(),self.root.path())}
}
fn finish(service:&Service,operation:Operation)->JobView {
    let job=service.submit(operation,SubmitRequest {pipeline_id:"briefing".into(),dry_run:false,upload_id:None}).unwrap();
    let deadline=Instant::now()+Duration::from_secs(15);loop {let view=service.job(&job.job_id).unwrap();if view.state.terminal(){return view;}
        assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));}
}
#[test]
fn captured_briefing_downloads_exact_native_publications_after_operator_source_edits() {
    let fixture=Fixture::new();let service=fixture.service().unwrap();
    fs::write(fixture.config.pipelines[0].root.join("episode/canonical.txt"),b"changed outside capture").unwrap();
    let view=finish(&service,Operation::Execute);assert_eq!(view.state,JobState::Complete);assert_eq!(view.profile,PipelineProfile::Briefing.name());
    let rows=service.artifacts(&view.job_id).unwrap();assert_eq!(rows.len(),2);let row=rows.iter().find(|r|r.format=="briefing_text").unwrap();
    let download=service.download(&view.job_id,&row.id).unwrap();assert_eq!(digest(&download.bytes),row.sha256);assert!(download.filename.ends_with(".txt"));
    assert!(String::from_utf8(download.bytes).unwrap().contains("Harbor Notes"));assert!(!fixture.config.pipelines[0].root.join("operator-original-output").exists());
}
#[test]
fn briefing_reference_closure_and_escape_are_refused_before_sdk_admission() {
    let mut fixture=Fixture::new();fixture.config.pipelines[0].files.pop();assert!(fixture.service().is_err());
    let fixture=Fixture::new();let path=fixture.config.pipelines[0].root.join("episode/briefing.json");let mut model:Value=serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    model["sources"][0]["path"]=json!("../../outside.txt");fs::write(&path,serde_json::to_vec(&model).unwrap()).unwrap();assert!(fixture.service().is_err());
}
#[test]
fn missing_review_remains_unavailable_in_plan_and_refuses_execution() {
    let mut fixture=Fixture::new();fixture.spec["execution"]["briefing"]["allow_draft"]=json!(false);fixture.write();let service=fixture.service().unwrap();
    let planned=finish(&service,Operation::Plan);let JobOutput::Plan(plan)=planned.result.unwrap() else{panic!("plan result required");};
    assert!(plan.plan.artifact_forest.unwrap().branches.iter().all(|b|b.state==renderflow::graph::ForestBranchState::Unavailable));
    let executed=finish(&service,Operation::Execute);assert_eq!(executed.state,JobState::Failed);assert!(!fixture.config.pipelines[0].root.join("operator-original-output").exists());
}
#[test]
fn briefing_service_does_not_admit_uploads_processes_or_unreferenced_private_files() {
    let mut fixture=Fixture::new();fixture.config.pipelines[0].allow_source_uploads=true;assert!(fixture.service().is_err());
    fixture.config.pipelines[0].allow_source_uploads=false;fixture.config.pipelines[0].allow_local_processes=true;assert!(fixture.service().is_err());
    fixture.config.pipelines[0].allow_local_processes=false;fixture.config.pipelines[0].files.push("private.txt".into());fs::write(fixture.config.pipelines[0].root.join("private.txt"),b"unreferenced private content").unwrap();assert!(fixture.service().is_err());
}
