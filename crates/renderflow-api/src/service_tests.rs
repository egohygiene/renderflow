//! Meaningful #420 adapter cases. Authored, UNRUN; no listener/provider fixture.

use super::*;
use std::fs;
use std::path::PathBuf;
use serde_json::json;

pub(crate) struct Fixture { _directory:tempfile::TempDir,root:PathBuf,config:ServiceConfig }
impl Fixture {
    pub(crate) fn new(formats:&[&str])->Self {
        let directory=tempfile::tempdir().unwrap();let root=directory.path().canonicalize().unwrap();
        let source=root.join("source");fs::create_dir(&source).unwrap();fs::create_dir(root.join("work")).unwrap();
        let original=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/page-composition/raster-composition");
        let files=["page.composition.json","content/en.json","content/de.json","fonts/registry.json","fonts/DejaVuSans.ttf",
            "fonts/DEJAVU-LICENSE.txt","assets/two-panels.svg","assets/alpha-tile.png","assets/red-tile.jpg"];
        for name in &files {
            let target=source.join(name);fs::create_dir_all(target.parent().unwrap()).unwrap();fs::copy(original.join(name),target).unwrap();
        }
        let spec=json!({"schema":"renderflow/v2","sources":[{"id":"page","kind":"composition","path":"page.composition.json"}],
            "targets":{"exact":formats.iter().map(|format|json!({"format":format})).collect::<Vec<_>>()},"output":{"bundle_root":"operator-original-output"}});
        fs::write(source.join("renderflow.json"),serde_json::to_vec(&spec).unwrap()).unwrap();
        let config=ServiceConfig {schema_version:SERVICE_SCHEMA.into(),persistence_directory:None,workspace_parent:"work".into(),
            pipelines:vec![PipelineConfig{id:"page".into(),root:"source".into(),config:"renderflow.json".into(),
                files:files.into_iter().chain(std::iter::once("renderflow.json")).map(str::to_string).collect(),
                profile:PipelineProfile::Composition,allow_local_processes:false,allow_source_uploads:false}],limits:Limits::default()};
        Self {_directory:directory,root,config}
    }
    pub(crate) fn service(&self)->Service {Service::new(self.config.clone(),&self.root).unwrap()}
    fn submit(&self,service:&Service,operation:Operation,dry_run:bool)->JobView {
        service.submit(operation,SubmitRequest{pipeline_id:"page".into(),dry_run,upload_id:None}).unwrap()
    }
}
fn wait(service:&Service,id:&str)->JobView {
    let deadline=Instant::now()+Duration::from_secs(15);
    loop {let job=service.job(id).unwrap();if job.state.terminal(){return job;}
        assert!(Instant::now()<deadline,"bounded fixture job did not finish");std::thread::sleep(Duration::from_millis(10));}
}

#[test]
fn plans_delegate_to_the_sdk_over_the_same_captured_package() {
    let fixture=Fixture::new(&["transcript_json"]);let service=fixture.service();
    let job=fixture.submit(&service,Operation::Plan,false);let view=wait(&service,&job.job_id);
    assert_eq!(view.state,JobState::Complete);
    let workspace=service.book.jobs.lock().unwrap()[&job.job_id].workspace.clone().unwrap();
    let direct=EngineBuilder::new().build().unwrap().provider_plan(PlanRequest::from_path(workspace.root().join("renderflow.json"))).unwrap();
    let JobOutput::Plan(observed)=view.result.unwrap() else {panic!("wrong native result");};
    assert_eq!(serde_json::to_value(observed).unwrap(),serde_json::to_value(direct).unwrap());
    assert!(!workspace.bundle().exists());assert!(service.artifacts(&job.job_id).is_err());
}

#[test]
fn immutable_capture_execution_and_binary_delivery_use_native_publications() {
    let fixture=Fixture::new(&["transcript_json"]);let service=fixture.service();
    let before=fs::read(fixture.root.join("source/page.composition.json")).unwrap();
    fs::write(fixture.root.join("source/content/en.json"),b"changed after startup").unwrap();
    let job=fixture.submit(&service,Operation::Execute,false);let view=wait(&service,&job.job_id);
    assert_eq!(view.state,JobState::Complete);let artifacts=service.artifacts(&job.job_id).unwrap();assert_eq!(artifacts.len(),2);
    let download=service.download(&job.job_id,&artifacts[0].id).unwrap();
    let value:serde_json::Value=serde_json::from_slice(&download.bytes).unwrap();assert_eq!(value["schema"],"renderflow.page-transcript/v1");
    assert_eq!(download.sha256,artifacts[0].sha256);assert!(value["requires_human_review"].as_bool().unwrap());
    assert_eq!(fs::read(fixture.root.join("source/page.composition.json")).unwrap(),before);
    assert!(!fixture.root.join("source/operator-original-output").exists());
    let events=service.events(&job.job_id,0).unwrap();assert!(events.last_event_id>0);
    assert!(events.events.iter().all(|event|event.event.schema_version==renderflow::sdk::PROGRESS_EVENT_V1));
}

#[test]
fn dry_runs_stay_planned_without_publication_and_partial_runs_stay_partial() {
    let fixture=Fixture::new(&["transcript_json","ora"]);let service=fixture.service();
    let dry=fixture.submit(&service,Operation::Execute,true);let view=wait(&service,&dry.job_id);
    assert_eq!(view.state,JobState::Planned);assert!(service.artifacts(&dry.job_id).unwrap().is_empty());
    let job=fixture.submit(&service,Operation::Execute,false);let view=wait(&service,&job.job_id);
    assert_eq!(view.state,JobState::Partial);assert_eq!(service.artifacts(&job.job_id).unwrap().len(),2);
    let JobOutput::Execute(native)=view.result.unwrap() else {panic!("wrong native result");};
    assert_eq!(native.run_manifest.state,renderflow::evidence::RunState::Partial);
}

#[test]
fn cancellation_retains_work_until_native_terminal_state_and_forget_is_explicit() {
    let fixture=Fixture::new(&["transcript_json"]);let service=fixture.service();let job=fixture.submit(&service,Operation::Execute,false);
    let requested=service.cancel(&job.job_id).unwrap();
    if !requested.state.terminal() {
        assert_eq!(requested.state,JobState::Cancelling);
    }
    let done=wait(&service,&job.job_id);assert!(done.state.terminal());
    if let Some(JobOutput::Execute(native))=done.result {
        assert_eq!(done.state,match native.run_manifest.state {renderflow::evidence::RunState::Complete=>JobState::Complete,
            renderflow::evidence::RunState::Cancelled=>JobState::Cancelled,other=>panic!("unexpected native state {other:?}")});
    }
    service.forget(&job.job_id).unwrap();assert_eq!(service.job(&job.job_id).unwrap_err().code,"job_not_found");
}

#[test]
fn retained_capacity_unknown_pipeline_limits_and_shutdown_are_structured() {
    let mut fixture=Fixture::new(&["transcript_json"]);fixture.config.limits.workers=1;fixture.config.limits.queued_jobs=1;fixture.config.limits.retained_jobs=2;
    let service=fixture.service();for _ in 0..2 {let job=fixture.submit(&service,Operation::Inspect,false);wait(&service,&job.job_id);}
    assert_eq!(service.submit(Operation::Plan,SubmitRequest{pipeline_id:"page".into(),dry_run:false,upload_id:None}).unwrap_err().code,"job_capacity");
    assert_eq!(service.submit(Operation::Plan,SubmitRequest{pipeline_id:"unknown".into(),dry_run:false,upload_id:None}).unwrap_err().code,"pipeline_not_found");
    service.shutdown();assert!(!service.ready());assert_eq!(service.submit(Operation::Inspect,SubmitRequest{pipeline_id:"page".into(),dry_run:false,upload_id:None}).unwrap_err().code,"shutting_down");
    fixture.config.limits.workers=0;assert!(Service::new(fixture.config,&fixture.root).is_err());
}

#[test]
fn unsafe_packages_and_external_profiles_are_refused_before_jobs() {
    let mut fixture=Fixture::new(&["transcript_json"]);fixture.config.pipelines[0].files.push("../outside".into());
    assert!(Service::new(fixture.config.clone(),&fixture.root).is_err());
    fixture.config.pipelines[0].files.pop();fixture.config.pipelines[0].files.push("bundle/old.json".into());
    assert!(Service::new(fixture.config.clone(),&fixture.root).is_err());
    fixture.config.pipelines[0].files.pop();
    fs::write(fixture.root.join("source/renderflow.json"),br#"{"schema":"renderflow/v2","sources":[{"id":"text","path":"content/en.json"}],"targets":{"exact":[{"format":"text"}]}}"#).unwrap();
    assert!(Service::new(fixture.config,&fixture.root).is_err());
}

#[test]
fn changed_publication_cannot_be_downloaded_from_a_successful_job() {
    let fixture=Fixture::new(&["transcript_json"]);let service=fixture.service();let job=fixture.submit(&service,Operation::Execute,false);wait(&service,&job.job_id);
    let artifact=service.artifacts(&job.job_id).unwrap().remove(0);
    let workspace=service.book.jobs.lock().unwrap()[&job.job_id].workspace.clone().unwrap();
    let entries=fs::read_dir(workspace.bundle().join("outputs")).unwrap();
    for locale in entries {fs::write(locale.unwrap().path().join("page.transcript.json"),b"corrupt").unwrap();}
    assert_eq!(service.download(&job.job_id,&artifact.id).unwrap_err().code,"artifact_integrity");
    assert!(service.download(&job.job_id,"../../outside").is_err());
}

#[test]
fn progress_eviction_and_replay_cursor_are_explicit() {
    let mut fixture=Fixture::new(&["transcript_json"]);fixture.config.limits.progress_events=1;
    let service=fixture.service();let job=fixture.submit(&service,Operation::Execute,false);wait(&service,&job.job_id);
    let events=service.events(&job.job_id,0).unwrap();assert_eq!(events.events.len(),1);
    assert!(events.dropped_events>0 && events.cursor_gap);
    assert!(service.events(&job.job_id,events.last_event_id).unwrap().events.is_empty());
    assert_eq!(service.events(&job.job_id,events.last_event_id+1).unwrap_err().code,"invalid_cursor");
}

#[test]
fn terminal_retention_expires_without_deleting_active_work() {
    let mut fixture=Fixture::new(&["transcript_json"]);fixture.config.limits.retention_seconds=1;
    let service=fixture.service();let job=fixture.submit(&service,Operation::Inspect,false);wait(&service,&job.job_id);
    let deadline=Instant::now()+Duration::from_secs(3);
    while service.job(&job.job_id).is_ok() {
        assert!(Instant::now()<deadline,"terminal job did not expire");std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(service.active_jobs().unwrap(),0);
}

#[test]
fn a_dropped_native_event_at_the_tail_reports_a_replay_gap() {
    let fixture=Fixture::new(&["transcript_json"]);let service=fixture.service();let job=fixture.submit(&service,Operation::Inspect,false);wait(&service,&job.job_id);
    let after=service.events(&job.job_id,0).unwrap().last_event_id;
    let reporter=Reporter{book:Arc::downgrade(&service.book),id:job.job_id.clone()};
    reporter.on_event(&ProgressEvent{schema_version:renderflow::sdk::PROGRESS_EVENT_V1.into(),stage:renderflow::ProgressStage::Executing,
        message:"x".repeat(service.limits().progress_bytes+1),run_id:None,step_id:None,artifact_ids:Vec::new(),state:None,diagnostics:Vec::new()});
    let replay=service.events(&job.job_id,after).unwrap();assert!(replay.events.is_empty() && replay.cursor_gap && replay.dropped_events>0);
}
