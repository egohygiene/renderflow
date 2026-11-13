//! Authored source cases only: uncompiled/unrun. No native media is needed.
use super::*;
use crate::service::upload_tests::Fixture;
use std::os::unix::fs::PermissionsExt;
fn fixture() -> Fixture {
    let mut fixture = Fixture::new();
    let state = fixture.root.path().join("state");
    fs::create_dir(&state).unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).unwrap();
    fixture.config.persistence_directory = Some(state);
    fixture.config.retain_completed_artifacts = true;
    fixture
}
fn complete(service: &Service, operation: Operation, dry_run: bool) -> JobView {
    let job = service
        .submit(
            operation,
            SubmitRequest {
                pipeline_id: "lyrics".into(),
                dry_run,
                upload_id: None,
            },
        )
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let view = service.job(&job.job_id).unwrap();
        if view.state.terminal() {
            return view;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
fn stop(service: Service) {
    let weak = Arc::downgrade(&service.book);
    drop(service);
    let deadline = Instant::now() + Duration::from_secs(5);
    while weak.upgrade().is_some() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
}
#[test]
fn opt_in_requires_explicit_private_persistence() {
    let mut fixture = Fixture::new();
    fixture.config.retain_completed_artifacts = true;
    assert_eq!(
        Service::new(fixture.config, fixture.root.path())
            .err()
            .unwrap()
            .code,
        "artifact_persistence_required"
    );
}
#[test]
fn restart_keeps_native_result_and_downloads_without_original_workspace() {
    let fixture = fixture();
    let service = fixture.service();
    let view = complete(&service, Operation::Execute, false);
    assert_eq!(view.state, JobState::Complete);
    assert!(view.artifacts_retained);
    let result = serde_json::to_value(&view.result).unwrap();
    let row = service.artifacts(&view.job_id).unwrap().remove(0);
    let original = service.download(&view.job_id, &row.id).unwrap().bytes;
    let old = service.book.directory.path().to_owned();
    stop(service);
    assert!(!old.exists());
    let service = fixture.service();
    let recovered = service.job(&view.job_id).unwrap();
    assert!(recovered.recovered_after_restart && recovered.artifacts_retained);
    assert!(!recovered.workspace_retained);
    assert_eq!(serde_json::to_value(&recovered.result).unwrap(), result);
    assert_eq!(
        service.download(&view.job_id, &row.id).unwrap().bytes,
        original
    );
    assert!(
        service.book.jobs.lock().unwrap()[&view.job_id]
            .workspace
            .as_ref()
            .unwrap()
            .artifacts_only
    );
}
#[test]
fn changed_cache_refuses_restart_and_preserves_evidence() {
    let fixture = fixture();
    let service = fixture.service();
    let view = complete(&service, Operation::Execute, false);
    let cache = fixture
        .root
        .path()
        .join("state/artifacts")
        .join(&view.job_id);
    let descriptor = read_marker(&cache.join("cache.json"), &service.book.limits).unwrap();
    stop(service);
    let path = cache.join("bundle").join(&descriptor.files[0].path);
    fs::write(&path, b"changed").unwrap();
    assert!(Service::new(fixture.config.clone(), fixture.root.path()).is_err());
    assert_eq!(fs::read(path).unwrap(), b"changed");
}
#[test]
fn restarted_download_lease_survives_durable_forget() {
    let fixture = fixture();
    let service = fixture.service();
    let view = complete(&service, Operation::Execute, false);
    stop(service);
    let service = fixture.service();
    let row = service.artifacts(&view.job_id).unwrap().remove(0);
    let mut download = service.prepare_download(&view.job_id, &row.id).unwrap();
    service.forget(&view.job_id).unwrap();
    assert!(!fixture
        .root
        .path()
        .join("state/artifacts")
        .join(&view.job_id)
        .exists());
    let mut bytes = Vec::new();
    download.copy_to(&mut bytes).unwrap();
    assert_eq!(package::digest(&bytes), row.sha256);
}
#[test]
fn planning_and_dry_run_have_no_cache_or_replay() {
    let fixture = fixture();
    let service = fixture.service();
    let plan = complete(&service, Operation::Plan, false);
    let dry = complete(&service, Operation::Execute, true);
    assert_eq!(dry.state, JobState::Planned);
    assert!(!plan.artifacts_retained && !dry.artifacts_retained);
    stop(service);
    let service = fixture.service();
    for id in [&plan.job_id, &dry.job_id] {
        let view = service.job(id).unwrap();
        assert!(view.recovered_after_restart);
        assert!(!view.workspace_retained && !view.artifacts_retained);
    }
    assert_eq!(
        fs::read_dir(fixture.root.path().join("state/artifacts"))
            .unwrap()
            .count(),
        0
    );
}
#[test]
fn partial_orphan_is_preserved_and_never_adopted() {
    let fixture = fixture();
    let service = fixture.service();
    stop(service);
    let orphan = fixture
        .root
        .path()
        .join("state/artifacts")
        .join(format!("job-{}", "f".repeat(64)));
    mkdir(&orphan).unwrap();
    let mut file = new_file(&orphan.join("partial")).unwrap();
    file.write_all(b"unfinished").unwrap();
    drop(file);
    let service = fixture.service();
    assert!(service.book.jobs.lock().unwrap().is_empty());
    assert_eq!(fs::read(orphan.join("partial")).unwrap(), b"unfinished");
}
