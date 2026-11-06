use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use sha2::{Digest, Sha256};
use crate::model::*;

pub(crate) struct Package {
    pub id: String, pub files: BTreeMap<String,Arc<[u8]>>, pub config: String,
    pub sha256: String, pub size_bytes: u64,
    pub profile: PipelineProfile,
    pub admission:PipelineConfig,
    pub source_upload_path:Option<String>,
}

pub(crate) struct Workspace { pub directory: tempfile::TempDir, pub effective_config_sha256: String }
impl Workspace {
    pub fn root(&self) -> &Path { self.directory.path() }
    pub fn bundle(&self) -> PathBuf { self.root().join("bundle") }
}

pub(crate) fn digest(bytes: &[u8]) -> String { format!("{:x}",Sha256::digest(bytes)) }

pub(crate) fn relative(value: &str) -> ApiResult<&Path> {
    let path = Path::new(value);
    if value.is_empty() || value.len() > 1024 || value.contains(['\\',':','\0'])
        || !path.components().all(|part|matches!(part,Component::Normal(_))) {
        return Err(ApiError::new("invalid_path","Package paths must be confined relative paths",400));
    }
    Ok(path)
}

pub(crate) fn no_links(path: &Path) -> ApiResult<()> {
    let mut current = PathBuf::new();
    for part in path.components() {
        current.push(part.as_os_str());
        let meta = fs::symlink_metadata(&current).map_err(|_|ApiError::io())?;
        if meta.file_type().is_symlink() { return Err(ApiError::new("symlink_refused","Service storage cannot traverse symlinks",400)); }
    }
    Ok(())
}

pub(crate) fn operator_path(base: &Path, value: &Path) -> ApiResult<PathBuf> {
    let full = if value.is_absolute() { value.to_path_buf() } else { base.join(value) };
    let mut normalized = PathBuf::new();
    for part in full.components() {
        match part {
            Component::CurDir => {},
            Component::ParentDir => { if !normalized.pop() { return Err(ApiError::new("invalid_root","Invalid operator storage root",400)); } },
            _ => normalized.push(part.as_os_str()),
        }
    }
    no_links(&normalized)?;
    if !normalized.is_dir() { return Err(ApiError::new("invalid_root","Operator storage root must already be a directory",400)); }
    Ok(normalized)
}

impl Package {
    pub fn capture(config: &PipelineConfig, base: &Path, limits: &Limits) -> ApiResult<Self> {
        if !stable_id(&config.id) || config.files.is_empty() || config.files.len() > limits.package_files
            || relative(&config.config)?.components().count() != 1 {
            return Err(ApiError::new("invalid_pipeline","A pipeline needs a stable ID, flat config and bounded file allowlist",400));
        }
        let root = operator_path(base,&config.root)?;
        let mut files = BTreeMap::new(); let mut folded = BTreeSet::new(); let mut total = 0_u64;
        for name in &config.files {
            let path = root.join(relative(name)?);
            if !folded.insert(name.to_ascii_lowercase()) || name.split('/').any(|part|part.eq_ignore_ascii_case("bundle")) {
                return Err(ApiError::new("invalid_package","Package names collide or use the reserved bundle directory",400));
            }
            no_links(&path)?;
            let meta = fs::symlink_metadata(&path).map_err(|_|ApiError::io())?;
            total = total.checked_add(meta.len()).ok_or_else(ApiError::io)?;
            if !meta.is_file() || total > limits.package_bytes { return Err(ApiError::new("package_limit","Package source size limit exceeded",413)); }
            let mut bytes = Vec::new();
            fs::File::open(&path).map_err(|_|ApiError::io())?.take(limits.package_bytes+1).read_to_end(&mut bytes).map_err(|_|ApiError::io())?;
            if bytes.len() as u64 != meta.len() { return Err(ApiError::new("package_changed","Package source changed during capture",409)); }
            no_links(&path)?;
            files.insert(name.clone(),Arc::<[u8]>::from(bytes));
        }
        let input = files.get(&config.config).ok_or_else(||ApiError::new("config_missing","Pipeline config is absent from its allowlist",400))?;
        let text = std::str::from_utf8(input).map_err(|_|ApiError::new("invalid_config","Pipeline config must be UTF-8",400))?;
        let mut spec = renderflow::spec::load_spec_str(text).map_err(|_|ApiError::new("invalid_config","Core rejected the pipeline configuration",422))?.spec;
        crate::profiles::admit(config,&mut spec,&files)?;
        if config.allow_source_uploads && !matches!(config.profile,PipelineProfile::ManualLyrics|PipelineProfile::TimedText|PipelineProfile::FountainBody) {
            return Err(ApiError::new("uploads_profile","Source uploads require an explicitly enabled built-in lyrics, timed-text or Fountain profile",422));
        }
        let source_upload_path=if config.allow_source_uploads {
            Some(spec.sources[0].path.clone().ok_or_else(ApiError::internal)?)
        }else{None};
        if source_upload_path.as_ref()==Some(&config.config) {
            return Err(ApiError::new("uploads_profile","The uploaded source cannot be the captured configuration",422));
        }
        // Output is always private. Graph profiles also apply explicit service
        // ceilings over existing SDK providers/transforms; core owns semantics.
        spec.output.bundle_root = "bundle".into();
        let effective = serde_json::to_vec_pretty(&spec).map_err(|_|ApiError::internal())?;
        let identity = files.iter().map(|(name,bytes)|(name,digest(bytes),bytes.len())).collect::<Vec<_>>();
        let base_sha256 = digest(&serde_json::to_vec(&(config.id.as_str(),&config.config,config.profile,config.allow_local_processes,identity)).map_err(|_|ApiError::internal())?);
        let sha256=if config.allow_source_uploads {
            digest(&serde_json::to_vec(&("renderflow.api-source-upload-capability/v1",&base_sha256,&source_upload_path)).map_err(|_|ApiError::internal())?)
        }else{base_sha256};
        total = total.saturating_sub(input.len() as u64).saturating_add(effective.len() as u64);
        if total > limits.package_bytes { return Err(ApiError::new("package_limit","Normalized package exceeds source byte limit",413)); }
        files.insert(config.config.clone(),Arc::from(effective));
        Ok(Self { id:config.id.clone(),files,config:config.config.clone(),sha256,size_bytes:total,profile:config.profile,
            admission:config.clone(),source_upload_path })
    }

    /// Build an immutable per-job package. Only the registered source bytes and
    /// their digest-bound classification may change; no client config is parsed.
    pub fn with_upload(&self,bytes:Arc<[u8]>,sha256:&str,limits:&Limits)->ApiResult<Self> {
        let path=self.source_upload_path.as_ref().ok_or_else(||ApiError::new("uploads_disabled","This pipeline does not admit source uploads",403))?;
        if path==&self.config || bytes.is_empty() || bytes.len() as u64>limits.upload_bytes || digest(&bytes)!=sha256 {
            return Err(ApiError::new("upload_identity","Upload bytes or registered source identity are invalid",422));
        }
        let mut files=self.files.clone();files.insert(path.clone(),bytes);
        let mut spec:renderflow::spec::SpecV2=serde_json::from_slice(files.get(&self.config).ok_or_else(ApiError::internal)?)
            .map_err(|_|ApiError::internal())?;
        spec.sources[0].sha256=Some(sha256.into());
        if let Some(policy)=spec.execution.lyrics.as_mut() {
            policy.source_sha256=sha256.into();policy.review_state=renderflow::lyrics::ReviewState::Unreviewed;
            policy.language=None;policy.revision=None;policy.sections.clear();
        }
        crate::profiles::admit(&self.admission,&mut spec,&files)?;
        let config=serde_json::to_vec_pretty(&spec).map_err(|_|ApiError::internal())?;
        files.insert(self.config.clone(),Arc::from(config));
        let size_bytes=files.values().map(|bytes|bytes.len() as u64).sum::<u64>();
        if size_bytes>limits.package_bytes {return Err(ApiError::new("package_limit","Uploaded package exceeds source byte limit",413));}
        let identity=files.iter().map(|(name,bytes)|(name,digest(bytes),bytes.len())).collect::<Vec<_>>();
        let package_sha256=digest(&serde_json::to_vec(&("renderflow.api-upload-package/v1",&self.sha256,identity)).map_err(|_|ApiError::internal())?);
        Ok(Self {id:self.id.clone(),files,config:self.config.clone(),sha256:package_sha256,size_bytes,profile:self.profile,
            admission:self.admission.clone(),source_upload_path:self.source_upload_path.clone()})
    }

    pub fn materialize(&self, parent: &Path) -> ApiResult<Workspace> {
        let directory = tempfile::Builder::new().prefix("job-").tempdir_in(parent).map_err(|_|ApiError::io())?;
        for (name,bytes) in &self.files {
            let path = directory.path().join(relative(name)?);
            fs::create_dir_all(path.parent().ok_or_else(ApiError::io)?).map_err(|_|ApiError::io())?;
            let mut file = OpenOptions::new().write(true).create_new(true).open(path).map_err(|_|ApiError::io())?;
            file.write_all(bytes).map_err(|_|ApiError::io())?;
        }
        let effective_config_sha256 = digest(self.files.get(&self.config).ok_or_else(ApiError::internal)?);
        Ok(Workspace { directory,effective_config_sha256 })
    }

    pub fn view(&self) -> PipelineView {
        PipelineView { id:self.id.clone(),package_sha256:self.sha256.clone(),file_count:self.files.len(),size_bytes:self.size_bytes,
            profile:self.profile.name(),local_processes:self.profile.local_processes(),source_uploads:self.source_upload_path.is_some() }
    }
}

pub(crate) fn storage_size(root: &Path, max_files: usize, limit: u64) -> ApiResult<u64> {
    fn visit(path:&Path, count:&mut usize, total:&mut u64, max_files:usize, limit:u64) -> ApiResult<()> {
        for entry in fs::read_dir(path).map_err(|_|ApiError::io())? {
            let path = entry.map_err(|_|ApiError::io())?.path();
            let meta = match fs::symlink_metadata(&path) {
                Ok(meta)=>meta,
                Err(error) if error.kind()==std::io::ErrorKind::NotFound=>continue,
                Err(_)=>return Err(ApiError::io()),
            };
            if meta.file_type().is_symlink() { return Err(ApiError::new("storage_limit","Job storage contains a refused link",413)); }
            *count += 1; *total = total.saturating_add(meta.len());
            if *count > max_files || *total > limit { return Err(ApiError::new("storage_limit","Job storage budget exceeded",413)); }
            if meta.is_dir() { visit(&path,count,total,max_files,limit)?; }
        }
        Ok(())
    }
    let mut total = 0; visit(root,&mut 0,&mut total,max_files,limit)?; Ok(total)
}
