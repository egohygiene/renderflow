use std::collections::{BTreeMap, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, Weak, atomic::{AtomicBool, AtomicU64, Ordering}, mpsc::{self, SyncSender, TrySendError}};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use renderflow::{CancellationToken, EngineBuilder, ExecutionRequest, InspectionRequest, PlanRequest, ProgressEvent, ProgressReporter};
use crate::model::*;
use crate::package::{self, Package, Workspace};

struct Entry {
    pipeline_id: String, package_sha256: String, operation: Operation, state: JobState,
    profile: PipelineProfile,
    cancellation: CancellationToken, cancel_reason: Option<CancelReason>,
    submitted_at: u64, started_at: Option<u64>, completed_at: Option<u64>,
    deadline: Instant, completed: Option<Instant>, workspace: Option<Arc<Workspace>>,
    result: Option<Vec<u8>>, error: Option<ApiError>, events: VecDeque<RecordedEvent>,
    event_bytes: usize, sequence: u64, dropped: u64,
}

impl Entry {
    fn view(&self,id:&str) -> ApiResult<JobView> {
        Ok(JobView { schema_version:API_SCHEMA,job_id:id.into(),pipeline_id:self.pipeline_id.clone(),package_sha256:self.package_sha256.clone(),
            profile:self.profile.name(),local_processes:self.profile.local_processes(),
            effective_config_sha256:self.workspace.as_ref().map(|workspace|workspace.effective_config_sha256.clone()),
            operation:self.operation,state:self.state,cancel_requested:self.cancel_reason,submitted_at_unix_ms:self.submitted_at,
            started_at_unix_ms:self.started_at,completed_at_unix_ms:self.completed_at,last_event_id:self.sequence,dropped_events:self.dropped,
            result:self.result.as_ref().map(|bytes|serde_json::from_slice(bytes)).transpose().map_err(|_|ApiError::internal())?,error:self.error.clone() })
    }
    fn cancel(&mut self,reason:CancelReason) {
        if !self.state.terminal() {
            self.cancel_reason.get_or_insert(reason); self.cancellation.cancel(); self.state = JobState::Cancelling;
        }
    }
    fn finish(&mut self,state:JobState,error:Option<ApiError>,result:Option<Vec<u8>>) {
        self.state=state; self.error=error; self.result=result; self.completed=Some(Instant::now()); self.completed_at=Some(now());
    }
}

struct Book {
    limits: Limits, packages: BTreeMap<String,Arc<Package>>, jobs: Mutex<BTreeMap<String,Entry>>,
    directory: tempfile::TempDir, shutdown: AtomicBool, next: AtomicU64,
}

impl Book {
    fn maintain(&self) {
        let Ok(mut jobs) = self.jobs.lock() else { return; };
        jobs.retain(|_,entry|!entry.completed.is_some_and(|time|time.elapsed() >= Duration::from_secs(self.limits.retention_seconds)));
        let mut storage = Vec::new();
        for (id,entry) in jobs.iter_mut() {
            if !entry.state.terminal() {
                if Instant::now() >= entry.deadline { entry.cancel(CancelReason::Deadline); }
                if let Some(workspace) = &entry.workspace { storage.push((id.clone(),workspace.clone())); }
            }
        }
        drop(jobs);
        if package::storage_size(self.directory.path(),65536,self.limits.workspace_storage_bytes).is_err() {
            if let Ok(mut jobs)=self.jobs.lock() { for entry in jobs.values_mut() { entry.cancel(CancelReason::Storage); } }
        }
        for (id,workspace) in storage {
            if package::storage_size(workspace.root(),4096,self.limits.job_storage_bytes).is_err() {
                if let Ok(mut jobs) = self.jobs.lock() {
                    if let Some(entry) = jobs.get_mut(&id) { entry.cancel(CancelReason::Storage); }
                }
            }
        }
    }
}

struct Work { id:String, package:Arc<Package>, operation:Operation, dry_run:bool, cancellation:CancellationToken }

struct Reporter { book:Weak<Book>, id:String }
impl ProgressReporter for Reporter {
    fn on_event(&self,event:&ProgressEvent) {
        let Some(book) = self.book.upgrade() else { return; };
        let Ok(mut jobs) = book.jobs.lock() else { return; };
        let Some(entry) = jobs.get_mut(&self.id) else { return; };
        entry.sequence = entry.sequence.saturating_add(1);
        let Ok(bytes) = serde_json::to_vec(event) else { entry.dropped+=1; return; };
        if bytes.len() > book.limits.progress_bytes { entry.dropped+=1; return; }
        while entry.events.len() >= book.limits.progress_events || entry.event_bytes + bytes.len() > book.limits.progress_bytes {
            let Some(old) = entry.events.pop_front() else { break; };
            entry.event_bytes = entry.event_bytes.saturating_sub(serde_json::to_vec(&old.event).map(|bytes|bytes.len()).unwrap_or(0));
            entry.dropped+=1;
        }
        entry.event_bytes += bytes.len();
        entry.events.push_back(RecordedEvent { id:entry.sequence,event:event.clone() });
    }
}

/// Per-process transport jobs only. SDK plans, rendering, caches, evidence and
/// cancellation semantics remain in core. No durable orchestration or resume.
pub struct Service { book:Arc<Book>, sender:SyncSender<Work> }

impl Service {
    pub fn new(config:ServiceConfig,base:&Path) -> ApiResult<Self> {
        config.limits.validate()?;
        if config.schema_version != SERVICE_SCHEMA || config.pipelines.is_empty() || config.pipelines.len() > 16 {
            return Err(ApiError::new("invalid_service","A service needs a supported schema and 1–16 registered pipelines",400));
        }
        let base = if base.is_absolute() { base.to_path_buf() } else {
            std::env::current_dir().map_err(|_|ApiError::io())?.join(base)
        };
        let parent = package::operator_path(&base,&config.workspace_parent)?;
        let mut packages = BTreeMap::new(); let mut bytes=0_u64;
        for pipeline in &config.pipelines {
            let package = Arc::new(Package::capture(pipeline,&base,&config.limits)?);
            bytes = bytes.saturating_add(package.size_bytes);
            if bytes > config.limits.all_package_bytes || packages.insert(pipeline.id.clone(),package).is_some() {
                return Err(ApiError::new("catalog_limit","Duplicate pipeline or total source-byte limit exceeded",413));
            }
        }
        let directory = tempfile::Builder::new().prefix("renderflow-api-").tempdir_in(parent).map_err(|_|ApiError::io())?;
        let book = Arc::new(Book { limits:config.limits,packages,jobs:Mutex::new(BTreeMap::new()),directory,shutdown:AtomicBool::new(false),next:AtomicU64::new(1) });
        let (sender,receiver) = mpsc::sync_channel::<Work>(book.limits.queued_jobs);
        let receiver = Arc::new(Mutex::new(receiver));
        for _ in 0..book.limits.workers {
            let book = book.clone(); let receiver = receiver.clone();
            std::thread::spawn(move || loop {
                if book.shutdown.load(Ordering::Acquire) { break; }
                let work = match receiver.lock() {
                    Ok(receiver)=>receiver.recv_timeout(Duration::from_millis(100)),
                    Err(_)=>break,
                };
                match work {
                    Ok(work)=> {
                        let id=work.id.clone();
                        // Release-profile panic=abort remains a process failure;
                        // this catches only unwind panics in compatible builds.
                        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||execute_work(&book,work)));
                        if outcome.is_err() {
                            if let Ok(mut jobs)=book.jobs.lock() { if let Some(entry)=jobs.get_mut(&id) { entry.finish(JobState::Failed,Some(ApiError::internal()),None); } }
                        }
                    },
                    Err(mpsc::RecvTimeoutError::Timeout)=>{},
                    Err(mpsc::RecvTimeoutError::Disconnected)=>break,
                }
            });
        }
        let weak = Arc::downgrade(&book);
        std::thread::spawn(move || loop {
            let Some(book)=weak.upgrade() else { break; };
            if book.shutdown.load(Ordering::Acquire) { break; }
            book.maintain(); drop(book); std::thread::sleep(Duration::from_millis(100));
        });
        Ok(Self { book,sender })
    }

    pub fn limits(&self) -> &Limits { &self.book.limits }
    pub fn pipelines(&self) -> Vec<PipelineView> { self.book.packages.values().map(|package|package.view()).collect() }
    pub fn ready(&self) -> bool { !self.book.shutdown.load(Ordering::Acquire) }
    pub fn shutdown(&self) {
        self.book.shutdown.store(true,Ordering::Release);
        if let Ok(mut jobs)=self.book.jobs.lock() { for entry in jobs.values_mut() {
            let never_started=!entry.state.terminal() && entry.started_at.is_none();
            entry.cancel(CancelReason::Shutdown);
            if never_started { entry.finish(JobState::Cancelled,None,None); }
        } }
    }
    pub fn active_jobs(&self) -> ApiResult<usize> {
        Ok(self.book.jobs.lock().map_err(|_|ApiError::internal())?.values().filter(|entry|!entry.state.terminal()).count())
    }
    pub fn submit(&self,operation:Operation,request:SubmitRequest) -> ApiResult<JobView> {
        self.book.maintain();
        if !self.ready() { return Err(ApiError::new("shutting_down","The service is shutting down",503)); }
        if package::storage_size(self.book.directory.path(),65536,self.book.limits.workspace_storage_bytes).is_err() {
            return Err(ApiError::new("storage_capacity","Service workspace budget reached; expire or forget terminal jobs",503));
        }
        if !stable_id(&request.pipeline_id) || (operation != Operation::Execute && request.dry_run) {
            return Err(ApiError::new("invalid_request","Invalid pipeline ID or operation options",400));
        }
        let package=self.book.packages.get(&request.pipeline_id).cloned().ok_or_else(||ApiError::new("pipeline_not_found","No registered pipeline has that ID",404))?;
        let mut jobs=self.book.jobs.lock().map_err(|_|ApiError::internal())?;
        if !self.ready() { return Err(ApiError::new("shutting_down","The service is shutting down",503)); }
        if jobs.len() >= self.book.limits.retained_jobs { return Err(ApiError::new("job_capacity","Retained job capacity reached; expire or forget a terminal job",503)); }
        let id=format!("job-{}",package::digest(format!("{}:{}:{}",self.book.directory.path().display(),self.book.next.fetch_add(1,Ordering::Relaxed),now()).as_bytes()));
        let cancellation=CancellationToken::new();
        let entry=Entry { pipeline_id:package.id.clone(),package_sha256:package.sha256.clone(),profile:package.profile,operation,state:JobState::Queued,
            cancellation:cancellation.clone(),cancel_reason:None,submitted_at:now(),started_at:None,completed_at:None,
            deadline:Instant::now()+Duration::from_secs(self.book.limits.job_timeout_seconds),completed:None,workspace:None,
            result:None,error:None,events:VecDeque::new(),event_bytes:0,sequence:0,dropped:0 };
        let view=entry.view(&id)?; jobs.insert(id.clone(),entry);
        let work=Work { id:id.clone(),package,operation,dry_run:request.dry_run,cancellation };
        match self.sender.try_send(work) {
            Ok(())=>Ok(view),
            Err(error)=> {
                jobs.remove(&id);
                Err(match error {
                    TrySendError::Full(_)=>ApiError::new("queue_full","Job queue capacity reached",503),
                    TrySendError::Disconnected(_)=>ApiError::new("shutting_down","Job workers are unavailable",503),
                })
            },
        }
    }
    pub fn job(&self,id:&str) -> ApiResult<JobView> {
        self.book.maintain();
        self.book.jobs.lock().map_err(|_|ApiError::internal())?.get(id).ok_or_else(not_found)?.view(id)
    }
    pub fn cancel(&self,id:&str) -> ApiResult<JobView> {
        let mut jobs=self.book.jobs.lock().map_err(|_|ApiError::internal())?;
        let entry=jobs.get_mut(id).ok_or_else(not_found)?; entry.cancel(CancelReason::Client); entry.view(id)
    }
    /// Terminal deletion is separate from cancellation; active work retains its
    /// workspace and concurrency slot until the SDK actually returns.
    pub fn forget(&self,id:&str) -> ApiResult<()> {
        let mut jobs=self.book.jobs.lock().map_err(|_|ApiError::internal())?;
        let entry=jobs.get(id).ok_or_else(not_found)?;
        if !entry.state.terminal() { return Err(ApiError::new("job_active","Cancel and wait for terminal state before forgetting a job",409)); }
        jobs.remove(id); Ok(())
    }
    pub fn events(&self,id:&str,after:u64) -> ApiResult<EventsView> {
        let jobs=self.book.jobs.lock().map_err(|_|ApiError::internal())?; let entry=jobs.get(id).ok_or_else(not_found)?;
        if after > entry.sequence { return Err(ApiError::new("invalid_cursor","Event cursor is ahead of the retained sequence",400)); }
        let events=entry.events.iter().filter(|event|event.id>after).cloned().collect::<Vec<_>>();
        let cursor_gap=entry.sequence-after>events.len() as u64;
        Ok(EventsView { schema_version:API_SCHEMA,job_id:id.into(),state:entry.state,
            events,last_event_id:entry.sequence,dropped_events:entry.dropped,cursor_gap })
    }
    fn execution(&self,id:&str) -> ApiResult<(renderflow::ExecutionResult,Arc<Workspace>)> {
        let jobs=self.book.jobs.lock().map_err(|_|ApiError::internal())?; let entry=jobs.get(id).ok_or_else(not_found)?;
        if !entry.state.terminal() { return Err(ApiError::new("job_active","Artifacts are available after terminal execution",409)); }
        let output:JobOutput=serde_json::from_slice(entry.result.as_deref().ok_or_else(||ApiError::new("no_execution_result","This job has no native execution result",409))?).map_err(|_|ApiError::internal())?;
        let JobOutput::Execute(result)=output else { return Err(ApiError::new("no_execution_result","Inspection and planning do not publish artifacts",409)); };
        Ok((result,entry.workspace.clone().ok_or_else(ApiError::internal)?))
    }
    pub fn artifacts(&self,id:&str) -> ApiResult<Vec<ArtifactView>> {
        let (result,_workspace)=self.execution(id)?;
        let publications=result.run_manifest.published_targets().map_err(|_|ApiError::new("invalid_native_evidence","Native publication joins could not be verified",422))?;
        Ok(publications.iter().enumerate().map(|(index,publication)| {
            let artifact=&publication.artifact; let object_id=format!("artifact-{index}");
            ArtifactView { id:object_id.clone(),target_id:publication.branch.target_id.clone(),artifact_id:artifact.artifact_id.clone(),
                format:artifact.format.clone(),media_type:artifact.media_type.clone(),size_bytes:artifact.size_bytes,sha256:artifact.digest.value.clone(),
                validation:artifact.validation,fidelity:artifact.fidelity,evidence:"native_publication_join; file bytes observed only on download",
                download_path:format!("/v1/jobs/{id}/artifacts/{object_id}") }
        }).collect())
    }
    pub fn download(&self,id:&str,artifact_id:&str) -> ApiResult<Download> {
        let index=artifact_id.strip_prefix("artifact-").filter(|value|!value.is_empty() && value.len()<=6 && value.bytes().all(|byte|byte.is_ascii_digit()))
            .and_then(|value|value.parse::<usize>().ok()).ok_or_else(||ApiError::new("artifact_not_found","Unknown job artifact ID",404))?;
        let (result,workspace)=self.execution(id)?;
        let publications=result.run_manifest.published_targets().map_err(|_|ApiError::internal())?;
        let publication=publications.get(index).ok_or_else(||ApiError::new("artifact_not_found","Unknown job artifact ID",404))?;
        if publication.artifact.size_bytes > self.book.limits.download_bytes { return Err(ApiError::new("download_limit","Artifact exceeds the bounded download profile",413)); }
        let limits=renderflow::evidence::verification::VerificationLimits {
            max_file_bytes:self.book.limits.download_bytes,max_total_bytes:self.book.limits.download_bytes,..Default::default()
        };
        let observed=renderflow::evidence::verification::read_publication(&result.run_manifest,&publication.branch.target_id,&workspace.bundle(),&limits,renderflow::process::ProcessCancellationToken::new())
            .map_err(|_|ApiError::new("artifact_integrity","Artifact bytes or confined locator differ from native evidence",422))?;
        let extension=match publication.artifact.format.parse::<renderflow::composition::CompositionOutput>() {
            Ok(format)=>format.extension().to_string(),
            Err(_)=>crate::profiles::graph_download_extension(&publication.artifact.format)?,
        };
        Ok(Download { sha256:observed.observation.sha256,bytes:observed.bytes,media_type:publication.artifact.media_type.clone(),filename:format!("artifact-{index}.{extension}") })
    }
}

impl Drop for Service { fn drop(&mut self) { self.shutdown(); } }

fn execute_work(book:&Arc<Book>,work:Work) {
    {
        let Ok(mut jobs)=book.jobs.lock() else { return; }; let Some(entry)=jobs.get_mut(&work.id) else { return; };
        if work.cancellation.is_cancelled() { entry.finish(JobState::Cancelled,None,None); return; }
        entry.state=JobState::Running; entry.started_at=Some(now());
    }
    let output=(||->ApiResult<JobOutput> {
        let workspace=Arc::new(work.package.materialize(book.directory.path())?);
        {
            let mut jobs=book.jobs.lock().map_err(|_|ApiError::internal())?;
            jobs.get_mut(&work.id).ok_or_else(not_found)?.workspace=Some(workspace.clone());
        }
        let reporter=Arc::new(Reporter {book:Arc::downgrade(book),id:work.id.clone()});
        let engine=EngineBuilder::new().with_cancellation_token(work.cancellation.clone()).with_progress_reporter(reporter).build().map_err(sdk_error)?;
        let config=workspace.root().join(&work.package.config);
        let output=match work.operation {
            Operation::Inspect=>JobOutput::Inspect(engine.inspect(InspectionRequest::from_path(&config)).map_err(sdk_error)?),
            Operation::Plan=> {
                let plan=engine.provider_plan(PlanRequest::from_path(&config)).map_err(sdk_error)?;
                crate::profiles::guard_plan(work.package.profile,&plan.plan)?;JobOutput::Plan(plan)
            },
            Operation::Execute=> {
                let resolved=engine.resolve_execution(ExecutionRequest::from_path(&config).dry_run(work.dry_run)).map_err(sdk_error)?;
                crate::profiles::guard_plan(work.package.profile,resolved.plan())?;
                JobOutput::Execute(engine.execute_resolved(resolved,work.dry_run).map_err(sdk_error)?)
            },
        };
        package::storage_size(workspace.root(),4096,book.limits.job_storage_bytes)?;
        Ok(output)
    })();
    let outcome=output.and_then(|output| {
        let state=match &output {
            JobOutput::Execute(result)=>match result.run_manifest.state {
                renderflow::evidence::RunState::Planned=>JobState::Planned,
                renderflow::evidence::RunState::Complete=>JobState::Complete,
                renderflow::evidence::RunState::Partial=>JobState::Partial,
                renderflow::evidence::RunState::Failed=>JobState::Failed,
                renderflow::evidence::RunState::Cancelled=>JobState::Cancelled,
            },
            _=>JobState::Complete,
        };
        let bytes=serde_json::to_vec(&output).map_err(|_|ApiError::internal())?;
        if bytes.len()>book.limits.result_bytes { return Err(ApiError::new("result_limit","Native result exceeds retained API JSON limit",413)); }
        Ok((state,bytes))
    });
    if let Ok(mut jobs)=book.jobs.lock() {
        if let Some(entry)=jobs.get_mut(&work.id) {
            match outcome {
                Ok((state,bytes))=>entry.finish(state,None,Some(bytes)),
                Err(error)=>entry.finish(if error.code=="cancelled" {JobState::Cancelled} else {JobState::Failed},Some(error),None),
            }
        }
    }
}

fn sdk_error(error:renderflow::RenderflowError) -> ApiError {
    match error {
        renderflow::RenderflowError::Cancelled=>ApiError::new("cancelled","SDK operation cancelled",409),
        renderflow::RenderflowError::Configuration(_)=>ApiError::new("sdk_configuration","Core rejected the captured package configuration",422),
        renderflow::RenderflowError::Planning(_)=>ApiError::new("sdk_planning","Core could not plan the captured package",422),
        renderflow::RenderflowError::Execution(_)=>ApiError::new("sdk_execution","Core could not complete the operation; no native success is inferred",422),
    }
}
fn not_found() -> ApiError { ApiError::new("job_not_found","Unknown or expired job ID",404) }
fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().try_into().unwrap_or(u64::MAX) }

#[cfg(test)]
#[path="service_tests.rs"]
pub(crate) mod tests;
