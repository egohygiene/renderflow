//! Optional single-writer local status/upload journal. No work is replayed.
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use super::*;

const SCHEMA: &str = "renderflow.api-local-state/v1";
pub(super) struct Store { root:PathBuf, binding:String, retain_artifacts:bool, _lock:File }

#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    schema:String, catalog_sha256:String, jobs:Vec<StoredJob>, uploads:Vec<StoredUpload>,
    #[serde(default,skip_serializing_if="Vec::is_empty")]
    garbage:Vec<retained::Retained>,
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredUpload { id:String,pipeline:String,sha256:String,size:u64,expires:u64 }
impl StoredUpload {
    fn capture(view:&UploadView)->Self {Self {id:view.upload_id.clone(),pipeline:view.pipeline_id.clone(),sha256:view.sha256.clone(),size:view.size_bytes,expires:view.expires_at_unix_ms}}
    fn view(&self)->UploadView {UploadView {schema_version:"renderflow.api-source-upload/v1",upload_id:self.id.clone(),pipeline_id:self.pipeline.clone(),sha256:self.sha256.clone(),size_bytes:self.size,expires_at_unix_ms:self.expires,review_inherited:false}}
}

#[cfg(all(test,unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    fn fixture()->tempfile::TempDir {
        let root=tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("state")).unwrap();fs::set_permissions(root.path().join("state"),fs::Permissions::from_mode(0o700)).unwrap();
        fs::create_dir(root.path().join("source")).unwrap();root
    }
    fn book(root:&Path)->Book {
        let package=Package {id:"manual".into(),files:BTreeMap::new(),config:"renderflow.json".into(),sha256:"a".repeat(64),size_bytes:0,
            profile:PipelineProfile::ManualLyrics,admission:PipelineConfig {id:"manual".into(),root:root.join("source"),config:"renderflow.json".into(),files:vec!["renderflow.json".into()],profile:PipelineProfile::ManualLyrics,allow_local_processes:false,allow_source_uploads:true},source_upload_path:Some("manual.srt".into())};
        let packages=BTreeMap::from([("manual".into(),Arc::new(package))]);let limits=Limits::default();
        let persistence=Some(Store::open(root,Path::new("state"),&packages,&limits).unwrap());
        Book {limits,packages,jobs:Mutex::new(BTreeMap::new()),directory:tempfile::tempdir_in(root).unwrap(),shutdown:AtomicBool::new(false),next:AtomicU64::new(1),
            uploads:Mutex::new(BTreeMap::new()),downloads:Mutex::new(download::Pool::default()),download_cancellation:renderflow::process::ProcessCancellationToken::new(),persistence,persistence_failed:AtomicBool::new(false)}
    }
    fn pending()->Entry {Entry {pipeline_id:"manual".into(),package_sha256:"a".repeat(64),operation:Operation::Execute,state:JobState::Running,profile:PipelineProfile::ManualLyrics,
        source_upload:None,cancellation:CancellationToken::new(),cancel_reason:None,submitted_at:now(),started_at:None,completed_at:None,deadline:Instant::now()+Duration::from_secs(5),completed:None,workspace:None,
        result:None,error:None,events:VecDeque::new(),event_bytes:0,sequence:7,dropped:0,recovered:false,historical_upload:None,historical_config_sha256:None,recovered_expiry:None}}
    fn upload()->SourceUpload {
        let bytes=b"1\n00:00:00,000 --> 00:00:01,000\nManual words\n";
        SourceUpload {view:UploadView {schema_version:"renderflow.api-source-upload/v1",upload_id:format!("upload-{}","b".repeat(64)),pipeline_id:"manual".into(),sha256:package::digest(bytes),size_bytes:bytes.len() as u64,expires_at_unix_ms:now()+60_000,review_inherited:false},bytes:Arc::from(bytes.as_slice()),expires:Instant::now()+Duration::from_secs(60)}
    }
    #[test]
    fn restart_marks_inflight_interrupted_without_a_worker_or_completion_claim() {
        let root=fixture();let first=book(root.path());let id=format!("job-{}","c".repeat(64));
        first.jobs.lock().unwrap().insert(id.clone(),pending());first.persist().unwrap();drop(first);
        let next=book(root.path());next.persistence.as_ref().unwrap().restore(&next).unwrap();
        let jobs=next.jobs.lock().unwrap();let view=jobs[&id].view(&id).unwrap();
        assert_eq!(view.state,JobState::Failed);assert_eq!(view.error.unwrap().code,"service_interrupted");
        assert!(view.recovered_after_restart);assert!(!view.workspace_retained);assert!(view.result.is_none());
        assert_eq!(jobs[&id].sequence,jobs[&id].dropped);assert!(jobs[&id].events.is_empty());
    }
    #[test]
    fn unexpired_upload_keeps_exact_identity_and_source_bytes_after_restart() {
        let root=fixture();let first=book(root.path());let upload=upload();let id=upload.view.upload_id.clone();let digest=upload.view.sha256.clone();
        first.uploads.lock().unwrap().insert(id.clone(),Arc::new(upload));first.persist().unwrap();drop(first);
        let next=book(root.path());next.persistence.as_ref().unwrap().restore(&next).unwrap();
        let uploads=next.uploads.lock().unwrap();assert_eq!(uploads[&id].view.sha256,digest);assert_eq!(package::digest(&uploads[&id].bytes),digest);
        assert!(!uploads[&id].view.review_inherited);
    }
    #[test]
    fn changed_upload_and_corrupt_journal_are_refused_and_preserved() {
        let root=fixture();let first=book(root.path());let upload=upload();let id=upload.view.upload_id.clone();
        first.uploads.lock().unwrap().insert(id.clone(),Arc::new(upload));first.persist().unwrap();drop(first);
        fs::write(root.path().join(format!("state/{id}.bin")),b"tampered").unwrap();
        let next=book(root.path());assert!(next.persistence.as_ref().unwrap().restore(&next).is_err());drop(next);
        fs::write(root.path().join("state/state.json"),b"{truncated").unwrap();
        let next=book(root.path());assert!(next.persistence.as_ref().unwrap().restore(&next).is_err());
        assert_eq!(fs::read(root.path().join("state/state.json")).unwrap(),b"{truncated");
    }
    #[test]
    fn exclusive_writer_and_changed_catalog_limits_fail_closed() {
        let root=fixture();let first=book(root.path());first.persist().unwrap();
        assert!(Store::open(root.path(),Path::new("state"),&first.packages,&first.limits).is_err());
        drop(first);let mut next=book(root.path());next.limits.retention_seconds+=1;
        let packages=std::mem::take(&mut next.packages);drop(next);
        let limits=Limits {retention_seconds:901,..Limits::default()};
        let store=Store::open(root.path(),Path::new("state"),&packages,&limits).unwrap();
        let bytes=read(&store.root.join("state.json"),limits.workspace_storage_bytes).unwrap();
        let old:Snapshot=serde_json::from_slice(&bytes).unwrap();assert_ne!(old.catalog_sha256,store.binding);
    }
    #[test]
    fn forgetting_upload_is_committed_before_orphan_cleanup_and_expiry_is_finite() {
        let root=fixture();let first=book(root.path());let mut upload=upload();upload.view.expires_at_unix_ms=now()-1;
        let id=upload.view.upload_id.clone();first.uploads.lock().unwrap().insert(id.clone(),Arc::new(upload));first.persist().unwrap();drop(first);
        let next=book(root.path());next.persistence.as_ref().unwrap().restore(&next).unwrap();assert!(next.uploads.lock().unwrap().is_empty());
        next.persist().unwrap();assert!(!root.path().join(format!("state/{id}.bin")).exists());
    }
    #[test]
    fn failed_snapshot_commit_stops_admission_without_a_durable_acknowledgement() {
        let root=fixture();let book=book(root.path());book.persist().unwrap();
        fs::remove_file(root.path().join("state/state.json")).unwrap();fs::create_dir(root.path().join("state/state.json")).unwrap();
        assert!(book.persist().is_err());assert!(book.shutdown.load(Ordering::Acquire));assert!(book.persistence_failed.load(Ordering::Acquire));
    }
}
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredError { code:String,message:String,status:u16 }
#[derive(Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredJob {
    id:String,pipeline:String,package_sha256:String,profile:PipelineProfile,operation:Operation,state:JobState,
    source_upload:Option<StoredUpload>,effective_config_sha256:Option<String>,cancel_reason:Option<CancelReason>,
    submitted:u64,started:Option<u64>,completed:Option<u64>,sequence:u64,
    result:Option<serde_json::Value>,error:Option<StoredError>,
    #[serde(default,skip_serializing_if="Option::is_none")]
    retained:Option<retained::Retained>,
}

fn corrupt()->ApiError {ApiError::new("journal_invalid","Local state is unsupported, changed, corrupt or exceeds the current admission limits; preserve it for manual review",422)}
fn storage()->ApiError {ApiError::new("journal_storage","Local state could not be durably committed; admission stopped and existing work was cooperatively cancelled",503)}
pub(super) fn read(path:&Path,limit:u64)->ApiResult<Vec<u8>> {
    package::no_links(path)?;
    let meta=fs::symlink_metadata(path).map_err(|_|storage())?;
    if !meta.is_file()||meta.len()>limit{return Err(corrupt());}
    #[cfg(unix)] {use std::os::unix::fs::MetadataExt;if meta.nlink()!=1||meta.mode()&0o077!=0{return Err(corrupt());}}
    let mut bytes=Vec::new();File::open(path).map_err(|_|storage())?.take(limit+1).read_to_end(&mut bytes).map_err(|_|storage())?;
    if bytes.len() as u64!=meta.len(){return Err(corrupt());}Ok(bytes)
}

impl Store {
    #[cfg(test)]
    pub(super) fn open(base:&Path,path:&Path,packages:&BTreeMap<String,Arc<Package>>,limits:&Limits)->ApiResult<Self> {
        Self::open_with_artifacts(base,path,packages,limits,false)
    }
    pub(super) fn open_with_artifacts(base:&Path,path:&Path,packages:&BTreeMap<String,Arc<Package>>,limits:&Limits,retain_artifacts:bool)->ApiResult<Self> {
        #[cfg(not(unix))] {let _=(base,path,packages,limits,retain_artifacts);return Err(ApiError::new("journal_platform","Durable local state currently requires a qualified Unix-style private filesystem",422));}
        #[cfg(unix)] {
            use std::os::unix::fs::{MetadataExt,OpenOptionsExt};
            let root=package::operator_path(base,path)?;
            if fs::metadata(&root).map_err(|_|storage())?.mode()&0o077!=0{return Err(ApiError::new("journal_private","Persistence directory must already exist with owner-only permissions",422));}
            for package in packages.values(){
                let source=package::operator_path(base,&package.admission.root)?;
                if root.starts_with(&source)||source.starts_with(&root){return Err(ApiError::new("journal_overlap","Persistence must be separate from registered source packages",422));}
            }
            let lock=root.join("writer.lock");
            match fs::symlink_metadata(&lock){Ok(_)=>{let _=read(&lock,0)?;},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>{},Err(_)=>return Err(storage())}
            let lock=OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).open(&lock).map_err(|_|storage())?;
            let meta=lock.metadata().map_err(|_|storage())?;
            if !meta.is_file()||meta.nlink()!=1||meta.mode()&0o077!=0{return Err(corrupt());}
            lock.try_lock().map_err(|_|ApiError::new("journal_locked","Another service still owns the persistence writer lock",409))?;
            let catalog=packages.iter().map(|(id,p)|(id,&p.sha256,p.profile)).collect::<Vec<_>>();
            let binding=if retain_artifacts {package::digest(&serde_json::to_vec(&(SCHEMA,catalog,limits,"published-artifact-cache/v1")).map_err(|_|corrupt())?)}
                else {package::digest(&serde_json::to_vec(&(SCHEMA,catalog,limits)).map_err(|_|corrupt())?)};
            if retain_artifacts {retained::prepare_root(&root)?;}
            let store=Self {root,binding,retain_artifacts,_lock:lock};store.inventory(limits)?;Ok(store)
        }
    }
    fn inventory(&self,limits:&Limits)->ApiResult<Vec<PathBuf>> {
        let mut files=Vec::new();
        for entry in fs::read_dir(&self.root).map_err(|_|storage())? {
            if files.len()>=limits.upload_count*2+8{return Err(corrupt());}
            let path=entry.map_err(|_|storage())?.path();let meta=fs::symlink_metadata(&path).map_err(|_|storage())?;
            let name=path.file_name().and_then(|s|s.to_str()).ok_or_else(corrupt)?;
            let known=name=="writer.lock"||name=="state.json"||name.starts_with(".journal-")||name.starts_with(".upload-")
                ||name.strip_suffix(".bin").is_some_and(crate::uploads::upload_id);
            if name=="artifacts"&&self.retain_artifacts {retained::inventory_root(&path,limits)?;continue;}
            if !known||!meta.is_file()||meta.file_type().is_symlink(){return Err(corrupt());}files.push(path);
        }
        package::storage_size(&self.root,65536,limits.workspace_storage_bytes)?;Ok(files)
    }
    pub(super) fn bytes(&self,limits:&Limits)->ApiResult<u64> {self.inventory(limits)?;package::storage_size(&self.root,65536,limits.workspace_storage_bytes)}
    fn budget(&self,new_bytes:u64,limits:&Limits,workspace_bytes:u64)->ApiResult<()> {
        let size=self.bytes(limits)?;
        if size.saturating_add(new_bytes as u64).saturating_add(workspace_bytes)>limits.workspace_storage_bytes{return Err(storage());}Ok(())
    }
    pub(super) fn restore(&self,book:&Book)->ApiResult<()> {
        let path=self.root.join("state.json");
        match fs::symlink_metadata(&path){Ok(_)=>{},Err(e) if e.kind()==std::io::ErrorKind::NotFound=>return Ok(()),Err(_)=>return Err(storage())}
        let bytes=read(&path,book.limits.workspace_storage_bytes/2)?;
        let snapshot:Snapshot=renderflow::evidence::json::object(&bytes,book.limits.workspace_storage_bytes as usize).map_err(|_|corrupt())?;
        if snapshot.schema!=SCHEMA||snapshot.catalog_sha256!=self.binding||snapshot.jobs.len()>book.limits.retained_jobs||snapshot.uploads.len()>book.limits.upload_count||snapshot.garbage.len()>book.limits.retained_jobs*2{return Err(corrupt());}
        for garbage in &snapshot.garbage {
            if !self.retain_artifacts||snapshot.jobs.iter().any(|j|j.id==garbage.job_id){return Err(corrupt());}
            retained::remove(&self.root,garbage,&book.limits)?;
        }
        let at=now();let instant=Instant::now();let mut uploads=BTreeMap::new();let mut total=0_u64;
        for stored in snapshot.uploads {
            if !crate::uploads::upload_id(&stored.id)||!crate::uploads::sha256(&stored.sha256)||stored.size==0||stored.size>book.limits.upload_bytes
                ||!book.packages.get(&stored.pipeline).is_some_and(|p|p.source_upload_path.is_some())||uploads.contains_key(&stored.id){return Err(corrupt());}
            if stored.expires<=at {continue;}
            let remaining=stored.expires-at;
            if remaining>book.limits.upload_retention_seconds*1000{return Err(corrupt());}
            total=total.checked_add(stored.size).ok_or_else(corrupt)?;
            if total>book.limits.all_upload_bytes{return Err(corrupt());}
            let bytes=read(&self.root.join(format!("{}.bin",stored.id)),stored.size)?;
            if bytes.len() as u64!=stored.size||package::digest(&bytes)!=stored.sha256{return Err(corrupt());}
            uploads.insert(stored.id.clone(),Arc::new(SourceUpload {view:stored.view(),bytes:Arc::from(bytes),expires:instant+Duration::from_millis(remaining)}));
        }
        let mut jobs=BTreeMap::new();
        for mut stored in snapshot.jobs {
            let package=book.packages.get(&stored.pipeline).ok_or_else(corrupt)?;
            if !stored.id.strip_prefix("job-").is_some_and(crate::uploads::sha256)||!crate::uploads::sha256(&stored.package_sha256)
                ||stored.profile!=package.profile||stored.submitted>at||stored.started.is_some_and(|t|t<stored.submitted||t>at)
                ||stored.completed.is_some_and(|t|t<stored.submitted||t>at)||jobs.contains_key(&stored.id){return Err(corrupt());}
            if stored.completed.is_some_and(|t|at-t>=book.limits.retention_seconds*1000){continue;}
            if !stored.state.terminal(){stored.state=JobState::Failed;stored.completed=Some(at);stored.result=None;
                stored.error=Some(StoredError {code:"service_interrupted".into(),message:"Service stopped before a terminal SDK result was durably recorded; no work was rerun and no native outcome is inferred".into(),status:409});}
            if stored.completed.is_none(){return Err(corrupt());}
            let result=stored.result.map(|value|serde_json::to_vec(&value).map_err(|_|corrupt())).transpose()?;
            if let Some(bytes)=&result {if bytes.len()>book.limits.result_bytes||serde_json::from_slice::<JobOutput>(bytes).is_err(){return Err(corrupt());}}
            let error=stored.error.map(|e|{
                if !stable_id(&e.code)||e.message.len()>4096||!(400..=599).contains(&e.status){return Err(corrupt());}
                Ok(ApiError::new(&e.code,&e.message,e.status))
            }).transpose()?;
            let historical_upload=stored.source_upload.map(|upload|{
                if !crate::uploads::upload_id(&upload.id)||upload.pipeline!=stored.pipeline||!crate::uploads::sha256(&upload.sha256)||upload.size>book.limits.upload_bytes{return Err(corrupt());}Ok(upload.view())
            }).transpose()?;
            if stored.effective_config_sha256.as_deref().is_some_and(|s|!crate::uploads::sha256(s)){return Err(corrupt());}
            let age=Duration::from_millis(at-stored.completed.ok_or_else(corrupt)?);
            let remaining=Duration::from_secs(book.limits.retention_seconds).checked_sub(age).ok_or_else(corrupt)?;
            let workspace=if let Some(cache)=stored.retained {
                if !self.retain_artifacts||stored.state!=JobState::Complete||stored.operation!=Operation::Execute||cache.job_id!=stored.id{return Err(corrupt());}
                let temporary=package::storage_size(book.directory.path(),65536,book.limits.workspace_storage_bytes)?;
                self.budget(retained::estimate(result.as_deref().ok_or_else(corrupt)?,&book.limits)?,&book.limits,temporary)?;
                Some(Arc::new(retained::hydrate(&self.root,&cache,result.as_deref().ok_or_else(corrupt)?,
                    stored.effective_config_sha256.as_deref().ok_or_else(corrupt)?,book.directory.path(),&book.limits)?))
            }else{None};
            jobs.insert(stored.id,Entry {pipeline_id:stored.pipeline,package_sha256:stored.package_sha256,profile:stored.profile,source_upload:None,
                operation:stored.operation,state:stored.state,cancellation:CancellationToken::new(),cancel_reason:stored.cancel_reason,
                submitted_at:stored.submitted,started_at:stored.started,completed_at:stored.completed,deadline:instant,
                completed:Some(instant),recovered_expiry:Some(instant+remaining),workspace,result,error,events:VecDeque::new(),event_bytes:0,
                sequence:stored.sequence,dropped:stored.sequence,recovered:true,historical_upload,historical_config_sha256:stored.effective_config_sha256});
        }
        *book.jobs.lock().map_err(|_|ApiError::internal())?=jobs;
        *book.uploads.lock().map_err(|_|ApiError::internal())?=uploads;Ok(())
    }
    pub(super) fn save(&self,jobs:&BTreeMap<String,Entry>,uploads:&BTreeMap<String,Arc<SourceUpload>>,limits:&Limits,workspace_bytes:u64)->ApiResult<()> {
        let prior=match fs::symlink_metadata(self.root.join("state.json")) {
            Ok(_)=>Some(renderflow::evidence::json::object::<Snapshot>(&read(&self.root.join("state.json"),limits.workspace_storage_bytes/2)?,limits.workspace_storage_bytes as usize).map_err(|_|corrupt())?),
            Err(e) if e.kind()==std::io::ErrorKind::NotFound=>None,Err(_)=>return Err(storage()),
        };
        if prior.as_ref().is_some_and(|p|p.schema!=SCHEMA||p.catalog_sha256!=self.binding){return Err(corrupt());}
        let mut stored_jobs=Vec::new();
        for (id,entry) in jobs {
            let view=entry.view(id)?;
            let cache=if self.retain_artifacts&&entry.operation==Operation::Execute&&entry.state==JobState::Complete {
                if let (Some(result),Some(workspace))=(&entry.result,&entry.workspace) {
                    let additional=if self.root.join("artifacts").join(id).try_exists().map_err(|_|storage())? {0}else{retained::estimate(result,limits)?};
                    self.budget(additional,limits,workspace_bytes)?;
                    let cached=retained::capture(&self.root,id,result,workspace,limits)?;
                    workspace.cache_ready.store(cached.is_some(),Ordering::Release);cached
                }else{None}
            }else{None};
            stored_jobs.push(StoredJob {id:id.clone(),pipeline:entry.pipeline_id.clone(),package_sha256:entry.package_sha256.clone(),profile:entry.profile,
                operation:entry.operation,state:entry.state,source_upload:view.source_upload.as_ref().map(StoredUpload::capture),
                effective_config_sha256:view.effective_config_sha256,cancel_reason:entry.cancel_reason,submitted:entry.submitted_at,started:entry.started_at,completed:entry.completed_at,
                sequence:entry.sequence,result:entry.result.as_ref().map(|b|serde_json::from_slice(b).map_err(|_|corrupt())).transpose()?,
                error:entry.error.as_ref().map(|e|StoredError {code:e.code.clone(),message:e.message.clone(),status:e.status}),retained:cache});
        }
        let mut stored_uploads=Vec::new();
        for upload in uploads.values() {
            let name=format!("{}.bin",upload.view.upload_id);let path=self.root.join(&name);
            if path.try_exists().map_err(|_|storage())? {
                if package::digest(&read(&path,upload.view.size_bytes)?)!=upload.view.sha256{return Err(corrupt());}
            } else {
                self.budget(upload.bytes.len() as u64,limits,workspace_bytes)?;
                let mut file=tempfile::Builder::new().prefix(".upload-").tempfile_in(&self.root).map_err(|_|storage())?;
                file.write_all(&upload.bytes).map_err(|_|storage())?;file.as_file().sync_all().map_err(|_|storage())?;
                file.persist_noclobber(&path).map_err(|_|storage())?;
            }
            stored_uploads.push(StoredUpload::capture(&upload.view));
        }
        let mut garbage=BTreeMap::new();
        if let Some(prior)=prior {
            for cache in prior.garbage.into_iter().chain(prior.jobs.into_iter().filter_map(|j|j.retained)) {
                if !jobs.contains_key(&cache.job_id)&&self.root.join("artifacts").join(&cache.job_id).try_exists().map_err(|_|storage())? {garbage.insert(cache.job_id.clone(),cache);}
            }
        }
        if garbage.len()>limits.retained_jobs*2{return Err(corrupt());}
        let snapshot=Snapshot {schema:SCHEMA.into(),catalog_sha256:self.binding.clone(),jobs:stored_jobs,uploads:stored_uploads,garbage:garbage.into_values().collect()};
        let bytes=serde_json::to_vec(&snapshot).map_err(|_|corrupt())?;
        if bytes.len() as u64>limits.workspace_storage_bytes/2{return Err(storage());}self.budget(bytes.len() as u64,limits,workspace_bytes)?;
        let mut file=tempfile::Builder::new().prefix(".journal-").tempfile_in(&self.root).map_err(|_|storage())?;
        file.write_all(&bytes).map_err(|_|storage())?;file.as_file().sync_all().map_err(|_|storage())?;
        file.persist(self.root.join("state.json")).map_err(|_|storage())?;
        File::open(&self.root).and_then(|f|f.sync_all()).map_err(|_|storage())?;
        for garbage in &snapshot.garbage {retained::remove(&self.root,garbage,limits)?;}
        for path in self.inventory(limits)? {
            let name=path.file_name().and_then(|s|s.to_str()).ok_or_else(corrupt)?;
            if name.starts_with(".journal-")||name.starts_with(".upload-")||name.strip_suffix(".bin").is_some_and(|id|!uploads.contains_key(id)) {
                fs::remove_file(path).map_err(|_|storage())?;
            }
        }
        File::open(&self.root).and_then(|f|f.sync_all()).map_err(|_|storage())?;Ok(())
    }
}
