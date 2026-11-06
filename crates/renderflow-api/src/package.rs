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
        if spec.schema != "renderflow/v2" || spec.sources.len() != 1 || spec.sources[0].kind != renderflow::spec::SourceKind::Composition {
            return Err(ApiError::new("unsupported_profile","This service checkpoint admits native page composition only",422));
        }
        relative(spec.sources[0].path.as_deref().ok_or_else(ApiError::internal)?)?;
        if spec.execution.composition.as_ref().is_some_and(|policy|policy.cache_directory.is_some()) {
            return Err(ApiError::new("shared_cache_refused","Service jobs require private bundle-local caches",422));
        }
        // Only output placement changes. Core still validates and owns every
        // target, locale, provider, geometry and native execution policy.
        spec.output.bundle_root = "bundle".into();
        let effective = serde_json::to_vec_pretty(&spec).map_err(|_|ApiError::internal())?;
        let identity = files.iter().map(|(name,bytes)|(name,digest(bytes),bytes.len())).collect::<Vec<_>>();
        let sha256 = digest(&serde_json::to_vec(&(config.id.as_str(),&config.config,identity)).map_err(|_|ApiError::internal())?);
        total = total.saturating_sub(input.len() as u64).saturating_add(effective.len() as u64);
        if total > limits.package_bytes { return Err(ApiError::new("package_limit","Normalized package exceeds source byte limit",413)); }
        files.insert(config.config.clone(),Arc::from(effective));
        Ok(Self { id:config.id.clone(),files,config:config.config.clone(),sha256,size_bytes:total })
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
            profile:renderflow::composition::COMPOSITION_PROFILE }
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
