//! Authored/unrun immutable-upload and admission cases. No paid/native provider.
use super::*;
use std::fs;
use serde_json::{json,Value};

pub(crate) const NEW:&[u8]=b"1\n00:00:00,123 --> 00:00:02,345\nNew manual words\n";
pub(crate) struct Fixture {pub root:tempfile::TempDir,pub config:ServiceConfig}
impl Fixture {
    pub fn new()->Self {
        let root=tempfile::tempdir().unwrap();let base=root.path().canonicalize().unwrap();
        fs::create_dir(base.join("source")).unwrap();fs::create_dir(base.join("work")).unwrap();
        let original=b"1\n00:00:01,000 --> 00:00:02,000\nOriginal words\n";
        fs::write(base.join("source/manual.srt"),original).unwrap();
        let spec=json!({"schema":"renderflow/v2","sources":[{"id":"source","path":"manual.srt","format":"srt","sha256":package::digest(original)}],
            "targets":{"all_reachable":true},"execution":{"network":"deny","ai":"deny","lyrics":{
                "source_sha256":package::digest(original),"review_state":"reviewed","language":"en","revision":"old-review","sections":{"cue-000001":"old section"}}},
            "output":{"bundle_root":"operator-original-output"}});
        fs::write(base.join("source/renderflow.json"),serde_json::to_vec_pretty(&spec).unwrap()).unwrap();
        let config=ServiceConfig {schema_version:SERVICE_SCHEMA.into(),persistence_directory:None,retain_completed_artifacts:false,workspace_parent:base.join("work"),
            pipelines:vec![PipelineConfig {id:"lyrics".into(),root:base.join("source"),config:"renderflow.json".into(),files:vec!["renderflow.json".into(),"manual.srt".into()],
                profile:PipelineProfile::ManualLyrics,allow_local_processes:false,allow_source_uploads:true}],limits:Limits::default()};
        Self {root,config}
    }
    pub fn service(&self)->Service {Service::new(self.config.clone(),self.root.path()).unwrap()}
}
fn wait(service:&Service,id:&str)->JobView {
    let deadline=Instant::now()+Duration::from_secs(15);
    loop {let view=service.job(id).unwrap();if view.state.terminal(){return view;}
        assert!(Instant::now()<deadline);std::thread::sleep(Duration::from_millis(10));}
}
fn uploaded_job(service:&Service,upload:&UploadView,operation:Operation)->JobView {
    service.submit(operation,SubmitRequest {pipeline_id:"lyrics".into(),dry_run:false,upload_id:Some(upload.upload_id.clone())}).unwrap()
}

#[test]
fn uploaded_lyrics_keep_exact_new_words_timing_and_no_inherited_review() {
    let fixture=Fixture::new();let original=fs::read(fixture.config.pipelines[0].root.join("manual.srt")).unwrap();let service=fixture.service();
    let upload=service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).unwrap();
    assert!(!upload.review_inherited);assert_eq!(service.active_jobs().unwrap(),0);
    let submitted=uploaded_job(&service,&upload,Operation::Execute);let view=wait(&service,&submitted.job_id);
    assert_eq!(view.state,JobState::Complete);assert_eq!(view.source_upload.unwrap().sha256,package::digest(NEW));
    let row=service.artifacts(&submitted.job_id).unwrap().into_iter().find(|row|row.format=="lyrics_json").unwrap();
    let result=service.download(&submitted.job_id,&row.id).unwrap();let doc=renderflow::lyrics::parse_document(&result.bytes).unwrap();
    assert_eq!(doc.policy.source_sha256,package::digest(NEW));assert_eq!(doc.policy.review_state,renderflow::lyrics::ReviewState::Unreviewed);
    assert!(doc.policy.language.is_none()&&doc.policy.revision.is_none()&&doc.policy.sections.is_empty());
    assert_eq!(doc.transcript.cues[0].text,"New manual words");assert_eq!(doc.transcript.cues[0].start_ms,123);assert_eq!(doc.transcript.cues[0].end_ms,2345);
    assert_eq!(fs::read(fixture.config.pipelines[0].root.join("manual.srt")).unwrap(),original);
    assert!(!fixture.config.pipelines[0].root.join("operator-original-output").exists());
}

#[test]
fn uploaded_package_is_frozen_and_base_catalog_stays_unchanged() {
    let fixture=Fixture::new();let service=fixture.service();let base=service.pipelines()[0].package_sha256.clone();
    let upload=service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).unwrap();
    fs::write(fixture.config.pipelines[0].root.join("manual.srt"),b"changed outside capture").unwrap();
    let submitted=uploaded_job(&service,&upload,Operation::Plan);let view=wait(&service,&submitted.job_id);
    assert_eq!(view.state,JobState::Complete);assert_ne!(view.package_sha256,base);assert_eq!(service.pipelines()[0].package_sha256,base);
    let workspace=service.book.jobs.lock().unwrap()[&submitted.job_id].workspace.clone().unwrap();
    assert_eq!(fs::read(workspace.root().join("manual.srt")).unwrap(),NEW);
    let spec:Value=serde_json::from_slice(&fs::read(workspace.root().join("renderflow.json")).unwrap()).unwrap();
    assert_eq!(spec["sources"][0]["sha256"],package::digest(NEW));
    assert_eq!(spec["execution"]["network"],"deny");assert_eq!(spec["execution"]["ai"],"deny");
}

#[test]
fn upload_requires_operator_opt_in_and_cannot_cross_registered_pipelines() {
    let mut fixture=Fixture::new();fixture.config.pipelines[0].allow_source_uploads=false;let service=fixture.service();
    assert_eq!(service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).unwrap_err().code,"uploads_disabled");drop(service);
    fixture.config.pipelines[0].allow_source_uploads=true;let mut second=fixture.config.pipelines[0].clone();second.id="other".into();fixture.config.pipelines.push(second);
    let service=fixture.service();let upload=service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).unwrap();
    assert_eq!(service.submit(Operation::Plan,SubmitRequest {pipeline_id:"other".into(),dry_run:false,upload_id:Some(upload.upload_id)}).unwrap_err().code,"upload_pipeline");
    assert_eq!(service.active_jobs().unwrap(),0);
}

#[test]
fn upload_rejects_mismatched_digest_size_and_finite_pool_exhaustion() {
    let mut fixture=Fixture::new();fixture.config.limits.upload_count=1;fixture.config.limits.upload_bytes=64;fixture.config.limits.all_upload_bytes=64;let service=fixture.service();
    assert_eq!(service.upload_source("lyrics",NEW.to_vec(),&"0".repeat(64)).unwrap_err().code,"upload_digest");
    assert_eq!(service.upload_source("lyrics",vec![b'x';65],&package::digest(&[b'x';65])).unwrap_err().code,"upload_limit");
    let upload=service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).unwrap();
    assert_eq!(service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).unwrap_err().code,"upload_capacity");
    service.forget_upload(&upload.upload_id).unwrap();assert!(service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).is_ok());
}

#[test]
fn expiry_refuses_new_submission_without_replacing_source_bytes() {
    let fixture=Fixture::new();let service=fixture.service();let upload=service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).unwrap();
    {let mut uploads=service.book.uploads.lock().unwrap();Arc::get_mut(uploads.get_mut(&upload.upload_id).unwrap()).unwrap().expires=Instant::now()-Duration::from_secs(1);}
    assert_eq!(service.upload(&upload.upload_id).unwrap_err().code,"upload_not_found");
    assert_eq!(service.submit(Operation::Plan,SubmitRequest {pipeline_id:"lyrics".into(),dry_run:false,upload_id:Some(upload.upload_id)}).unwrap_err().code,"upload_not_found");
    assert_eq!(service.active_jobs().unwrap(),0);
}

#[test]
fn retained_job_keeps_expired_upload_charged_until_explicit_forget() {
    let fixture=Fixture::new();let service=fixture.service();let upload=service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).unwrap();
    let job=uploaded_job(&service,&upload,Operation::Plan);wait(&service,&job.job_id);
    assert_eq!(service.forget_upload(&upload.upload_id).unwrap_err().code,"upload_in_use");
    let future=Instant::now()+Duration::from_secs(service.limits().upload_retention_seconds+1);
    service.book.reap_uploads(future);assert!(service.book.uploads.lock().unwrap().contains_key(&upload.upload_id));
    service.forget(&job.job_id).unwrap();service.book.reap_uploads(future);
    assert!(!service.book.uploads.lock().unwrap().contains_key(&upload.upload_id));
}

#[test]
fn source_upload_cannot_supply_a_config_path_format_or_process_permission() {
    for value in [json!({"pipeline_id":"lyrics","path":"/host/private"}),json!({"pipeline_id":"lyrics","format":"markdown"}),json!({"pipeline_id":"lyrics","allow_local_processes":true})] {
        assert!(renderflow::evidence::json::object::<SubmitRequest>(&serde_json::to_vec(&value).unwrap(),16384).is_err());
    }
    let fixture=Fixture::new();let service=fixture.service();let upload=service.upload_source("lyrics",b"{}".to_vec(),&package::digest(b"{}")).unwrap();
    let job=uploaded_job(&service,&upload,Operation::Execute);let view=wait(&service,&job.job_id);
    assert_eq!(view.state,JobState::Failed);assert_eq!(service.pipelines()[0].profile,PipelineProfile::ManualLyrics.name());
    assert!(service.artifacts(&job.job_id).map(|rows|rows.is_empty()).unwrap_or(true));
}

#[test]
fn disabled_source_uploads_preserve_legacy_package_identity_and_unknown_options_fail() {
    let mut fixture=Fixture::new();fixture.config.pipelines[0].allow_source_uploads=false;
    let package=Package::capture(&fixture.config.pipelines[0],fixture.root.path(),&fixture.config.limits).unwrap();
    let mut config=serde_json::to_value(&fixture.config.pipelines[0]).unwrap();config.as_object_mut().unwrap().remove("allow_source_uploads");
    let original:PipelineConfig=serde_json::from_value(config.clone()).unwrap();
    assert_eq!(package.sha256,Package::capture(&original,fixture.root.path(),&fixture.config.limits).unwrap().sha256);
    config["upload_any_path"]=json!(true);assert!(serde_json::from_value::<PipelineConfig>(config).is_err());
    let service=fixture.service();service.shutdown();assert_eq!(service.upload_source("lyrics",NEW.to_vec(),&package::digest(NEW)).unwrap_err().code,"shutting_down");
}
