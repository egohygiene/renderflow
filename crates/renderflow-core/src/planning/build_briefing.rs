//! Frozen local briefing publication through the ordinary build/forest facade.
use std::{collections::BTreeMap,fs,path::{Path,PathBuf},sync::{Arc,atomic::AtomicBool},time::Instant};
use anyhow::{Result,Context,ensure};
use serde_json::{json,Value};
use crate::artifact::{ArtifactDigest,ArtifactId,CanonicalFormat};
use crate::evidence::*;
use crate::evidence::files::{self,BundleRoot,ExpectedFile};
use crate::evidence::verification::VerificationLimits;
use crate::graph::execution_plan::{EdgeType,ExecutionMetadata,ExecutionWave,PlanEdge,PlanNode,NodeType,ForestExecutionEvidence,ForestTargetExecution,ForestTargetOutcome,FOREST_EXECUTION_SCHEMA_V1};
use crate::graph::{ArtifactForest,ExecutionPlan,ForestBranch,ForestBranchState,ForestRouteEvidence};
use crate::publication::{briefing::{self,BriefingPlan,BriefingRequest,Captured},local};
use crate::process::ProcessCancellationToken;
use crate::spec::{LoadedSpec,SourceSpecVersion,SpecV2,TargetSpec};
use super::{CanonicalExecutionResult,PlanningRequest};

pub(super) struct ResolvedBriefingBuild {
    request:BriefingRequest,captured:Captured,spec:SpecV2,plan:ExecutionPlan,cancellation:ProcessCancellationToken,
}
impl ResolvedBriefingBuild {
    pub(super) fn plan(&self)->&ExecutionPlan {&self.plan}
    pub(super) fn spec(&self)->&SpecV2 {&self.spec}
    pub(super) fn source_version(&self)->SourceSpecVersion {SourceSpecVersion::V2}
    pub(super) fn source_path(&self)->&Path {&self.request.input}
    pub(super) fn predicted_output_paths(&self)->Result<Vec<PathBuf>> {
        Ok(self.plan.artifact_forest.as_ref().context("briefing.forest")?.branches.iter().map(|b|self.request.output_dir.join(b.options["file"].as_str().expect("closed briefing target"))).collect())
    }
    pub(super) fn with_cancellation_flag(mut self,flag:Arc<AtomicBool>)->Self {self.cancellation=ProcessCancellationToken::from_shared(flag);self}
}
pub(super) fn resolve(loaded:LoadedSpec,selection:&PlanningRequest)->Result<ResolvedBriefingBuild> {
    ensure!(loaded.source_version==SourceSpecVersion::V2&&!selection.all_reachable&&selection.profile.is_none()
        &&selection.exclude.is_empty()&&selection.optimization.is_none(),"briefing.build.selection: use v2 and exact targets");
    let mut spec=loaded.spec;
    if let Some(target)=&selection.target {spec.targets.exact=vec![TargetSpec {format:Some(target.clone()),..Default::default()}];}
    ensure!(spec.validate().is_empty(),"briefing.build.spec: unsupported specification");
    let config=fs::canonicalize(&selection.config_path)?;let parent=config.parent().context("briefing.config.parent")?;
    let relative=|value:&str|{let p=Path::new(value);if p.is_absolute(){p.to_path_buf()}else{parent.join(p)}};
    let request=BriefingRequest {input:relative(spec.sources[0].path.as_deref().context("briefing.source.path")?),output_dir:relative(&spec.output.bundle_root),allow_draft:spec.execution.briefing.clone().unwrap_or_default().allow_draft};
    let cancellation=ProcessCancellationToken::new();let captured=briefing::capture(&request.input,cancellation.clone())?;
    spec.sources[0].path=Some(request.input.to_str().context("briefing.input.utf8")?.into());spec.output.bundle_root=request.output_dir.to_str().context("briefing.output.utf8")?.into();
    let targets=crate::spec::briefing::targets(&spec.targets.exact)?;
    let plan=project(&spec,&request,&captured.plan,&targets)?;
    Ok(ResolvedBriefingBuild {request,captured,spec,plan,cancellation})
}
fn availability(request:&BriefingRequest,native:&BriefingPlan,path:&str)->Option<&'static str> {
    if native.freshness=="stale" {Some("briefing.source.stale")}
    else if native.narration_script_current==Some(false) {Some("briefing.narration.script.changed")}
    else if !native.reviews_declared&&!request.allow_draft {Some("briefing.review.required")}
    else if !native.expected_files.iter().any(|f|f.path==path){Some("briefing.optional_input.unavailable")}
    else {None}
}
fn project(spec:&SpecV2,request:&BriefingRequest,native:&BriefingPlan,targets:&[String])->Result<ExecutionPlan> {
    let mut edges=Vec::new();let mut branches=Vec::new();
    for target in targets {
        let path=crate::spec::briefing::filename(target).context("briefing.target")?;let refusal=availability(request,native,path);
        let edge=PlanEdge {from:"briefing_snapshot".into(),to:target.clone(),cost:1.0,quality:0.0,input_kind:"single".into(),edge_type:EdgeType::Lossy,
            provider_id:Some("renderflow.briefing".into()),capability_id:Some(format!("briefing.{target}")),required_provider_ids:Vec::new(),variant_id:None,
            evidence:BTreeMap::from([("profile".into(),briefing::PROFILE.into()),("quality_meaning".into(),"unqualified fixed-profile placeholder".into())])};
        let route=refusal.is_none().then(||ForestRouteEvidence {steps:vec![edge.clone()],source_identity:false,cumulative_quality:0.0,cumulative_loss:1.0,hop_count:1,weighted_cost:1.0,
            ranking:"fixed local briefing publication, not a general conversion or fidelity search".into(),explored_candidates:0});
        if refusal.is_none(){edges.push(edge);}
        branches.push(ForestBranch {target_id:format!("briefing:{}",sha256_serialized(&json!({"native":native,"target":target,"policy":spec.execution.briefing}))?.value),
            format:target.clone(),role:Some("briefing".into()),preset:None,template:None,variant:None,requirement:"required".into(),
            options:BTreeMap::from([("file".into(),json!(path))]),state:if refusal.is_none(){ForestBranchState::Selected}else{ForestBranchState::Unavailable},
            reason_code:refusal.unwrap_or("briefing.selected").into(),reason:if refusal.is_none(){"Frozen supplied content; declared review and publication approval remain separate"}else{"Source/review binding or explicitly supplied optional feed/audio is unavailable; no inference or approval is invented"}.into(),route});
    }
    let identity=format!("sha256:{}",sha256_serialized(&json!({"spec":spec,"native":native,"branches":branches}))?.value);let count=edges.len();
    let mut nodes=vec![PlanNode {format:"briefing_snapshot".into(),node_type:NodeType::Source}];nodes.extend(targets.iter().map(|t|PlanNode {format:t.clone(),node_type:NodeType::Output}));
    Ok(ExecutionPlan {source:"briefing_snapshot".into(),targets:targets.to_vec(),optimization:"fixed_briefing_dependencies".into(),nodes,edges:edges.clone(),
        waves:if edges.is_empty(){Vec::new()}else{vec![ExecutionWave {index:1,edges}]},
        metadata:ExecutionMetadata {total_nodes:1+targets.len(),total_edges:count,execution_depth:usize::from(count>0),execution_waves:usize::from(count>0),estimated_cost:count as f32,estimated_quality:0.0,reused_intermediates:0,output_count:count},
        diagnostics:Vec::new(),source_artifact:None,source_collection:None,toolchain:None,
        artifact_forest:Some(ArtifactForest {identity,profiles:vec![briefing::PROFILE.into()],intermediates:"cache_only".into(),branches,
            policy:json!({"adapter":"frozen_local_briefing","native_plan":native,"runtime_qualified":false,"publication_approval":"not_granted",
                "auxiliary_files":"The existing publisher retains the complete captured local bundle, including source snapshots and model. Primary branches are selected representations, not a file-redaction policy.",
                "audio":"Only explicitly supplied pinned narration; no voice provider, synthesis or measured alignment"}),..Default::default()})})
}
pub(super) fn execute(resolved:ResolvedBriefingBuild,dry_run:bool)->Result<CanonicalExecutionResult> {
    let started=unix_time_ms();if resolved.cancellation.is_cancelled(){return cancelled(resolved);}
    if dry_run {return finish(&resolved,RunState::Planned,None,started);}
    ensure!(resolved.plan.artifact_forest.as_ref().context("briefing.forest")?.branches.iter().all(|b|b.state==ForestBranchState::Selected),"briefing.build.unavailable: a required source/review/optional-input gate is unsatisfied");
    let receipt=briefing::build_captured(&resolved.request,&resolved.captured,resolved.cancellation.clone())?;
    finish(&resolved,RunState::Complete,Some(receipt),started)
}
pub(super) fn cancelled(resolved:ResolvedBriefingBuild)->Result<CanonicalExecutionResult> {finish(&resolved,RunState::Cancelled,None,unix_time_ms())}
fn record(format:&str,path:&str,subject:&local::FileSubject,source:&[ArtifactId],lifecycle:ArtifactRole)->Result<ArtifactEvidence> {
    let digest=ArtifactDigest::sha256(subject.sha256.clone());let id=ArtifactId::from_record(&digest,&CanonicalFormat::new(format)?,source);
    Ok(ArtifactEvidence {artifact_id:id.to_string(),role:"briefing".into(),lifecycle,locator:path.into(),format:format.into(),media_type:match format {"briefing_text"|"text"=>"text/plain","briefing_html"=>"text/html","briefing_audio"=>"audio/wav","briefing_feed"=>"application/rss+xml",_=>"application/json"}.into(),
        digest:DigestEvidence {algorithm:"sha256".into(),value:subject.sha256.clone()},size_bytes:subject.size_bytes,
        producer:ProducerEvidence {system:"renderflow".into(),transform:Some("briefing".into()),capability:Some("briefing.local".into()),provider:Some("renderflow.briefing".into()),version:Some(env!("CARGO_PKG_VERSION").into())},
        sources:source.iter().map(ToString::to_string).collect(),cache:CacheDisposition::NotApplicable,validation:ValidationState::ValidWithWarnings,validation_evidence:Vec::new(),fidelity:FidelityDeclaration::Unknown,hygiene:None,
        warnings:vec!["briefing.runtime_quality_review_and_publication_unobserved".into()],metadata:BTreeMap::new()})
}
fn finish(resolved:&ResolvedBriefingBuild,state:RunState,receipt:Option<Value>,started:u64)->Result<CanonicalExecutionResult> {
    let plan_digest=sha256_serialized(&resolved.plan)?;let run=run_id(&plan_digest,started);let mut forest=resolved.plan.artifact_forest.clone().context("briefing.forest")?;
    let mut records=Vec::new();let mut outputs=Vec::new();let mut observed=BTreeMap::<String,(usize,String,String)>::new();
    if let Some(receipt)=receipt {
        ensure!(receipt["state"]=="complete"&&receipt["plan"]==serde_json::to_value(&resolved.captured.plan)?&&receipt["publication_approval"]=="not_granted","briefing.receipt: native result differs from frozen plan");
        let mut subjects:Vec<local::FileSubject>=serde_json::from_value(receipt["files"].clone())?;
        ensure!(subjects==resolved.captured.plan.expected_files,"briefing.receipt.files: native inventory differs from frozen expected membership");
        let source=local::FileSubject {path:"memory:briefing-input".into(),sha256:resolved.captured.plan.input_sha256.clone(),size_bytes:resolved.captured.files["briefing.json"].len() as u64};
        let evidence=record("briefing",&source.path,&source,&[],ArtifactRole::Source)?;let source_id:ArtifactId=serde_json::from_value(json!(evidence.artifact_id))?;records.push(evidence);
        let receipt_bytes=serde_json::to_vec_pretty(&receipt)?;ensure!(receipt_bytes.len()<=local::MAX_JSON,"briefing.receipt.limit");
        subjects.push(local::FileSubject {path:"briefing-run.json".into(),sha256:local::sha256(&receipt_bytes),size_bytes:receipt_bytes.len() as u64});
        let root=BundleRoot::new(&resolved.request.output_dir)?;let limits=VerificationLimits {max_file_bytes:local::MAX_TOTAL,max_total_bytes:local::MAX_TOTAL+local::MAX_JSON as u64,max_publications:257,..Default::default()};let observation_started=Instant::now();
        for subject in subjects {
            files::observe(&root,&subject.path,ExpectedFile {size_bytes:subject.size_bytes,sha256:&subject.sha256},&limits,observation_started,&resolved.cancellation,&mut std::io::sink())?;
            let branch=forest.branches.iter().find(|b|b.options["file"]==subject.path);
            let format=branch.map_or(if subject.path.ends_with(".txt"){"text"}else if subject.path.ends_with(".wav"){"briefing_audio"}else if subject.path.ends_with(".html"){"briefing_html"}else if subject.path.ends_with(".rss"){"briefing_feed"}else{"json"},|b|b.format.as_str());
            let mut evidence=record(format,&format!("bundle:{}",subject.path),&subject,std::slice::from_ref(&source_id),if branch.is_some(){ArtifactRole::Terminal}else{ArtifactRole::Intermediate})?;
            if let Some(branch)=branch {
                evidence.metadata.insert("renderflow.target.identity".into(),json!(branch.target_id));evidence.metadata.insert("renderflow.target.published".into(),json!(true));
                evidence.metadata.insert("renderflow.briefing.native_receipt".into(),json!("bundle:briefing-run.json"));
                forest.produced_artifacts.push(evidence.artifact_id.clone());observed.insert(branch.target_id.clone(),(records.len(),evidence.artifact_id.clone(),evidence.locator.clone()));
            }
            outputs.push(subject.path);records.push(evidence);
        }root.recheck()?;
    }
    forest.execution=Some(ForestExecutionEvidence {schema:FOREST_EXECUTION_SCHEMA_V1.into(),targets:forest.branches.iter().map(|b|{
        let item=observed.get(&b.target_id);ForestTargetExecution {target_id:b.target_id.clone(),outcome:if b.state!=ForestBranchState::Selected{ForestTargetOutcome::NotSelected}else if item.is_some(){ForestTargetOutcome::Published}else if state==RunState::Planned{ForestTargetOutcome::Planned}else{ForestTargetOutcome::NotPublished},source_identity:false,
            artifact_evidence_index:item.map(|v|v.0),artifact_id:item.map(|v|v.1.clone()),locator:item.map(|v|v.2.clone()),producing_step_id:None}
    }).collect()});
    let manifest=RunManifest {schema_version:RUN_MANIFEST_SCHEMA_V1.into(),run_id:run.clone(),execution_plan_digest:plan_digest,source_spec_digest:sha256_serialized(&resolved.spec)?,engine_version:env!("CARGO_PKG_VERSION").into(),started_at_unix_ms:started,completed_at_unix_ms:unix_time_ms().max(started),state,
        artifact_manifest:ArtifactManifest {schema_version:ARTIFACT_MANIFEST_SCHEMA_V1.into(),run_id:run,output_dir:resolved.spec.output.bundle_root.clone(),outputs:outputs.clone(),artifacts:records},steps:Vec::new(),
        diagnostics:vec![ExecutionDiagnostic {severity:DiagnosticSeverity::Warning,code:"briefing.build.scope".into(),step_id:None,message:"Frozen local briefing bundle; exact bytes and publication joins are observed separately from declared review, timing, native/semantic/accessibility qualification and publication approval. Copied source snapshots remain in the local bundle.".into()}],toolchain:None,artifact_forest:Some(forest)};
    let manifest_path=if state==RunState::Complete {
        write_new(&resolved.request.output_dir.join("build-plan.json"),&serde_json::to_vec(&resolved.plan)?)?;
        let path=resolved.request.output_dir.join("renderflow-run.json");write_new(&path,&serde_json::to_vec_pretty(&manifest)?)?;Some(path.display().to_string())
    }else{None};
    let paths=if state==RunState::Planned{resolved.predicted_output_paths()?}else{outputs.iter().map(|p|resolved.request.output_dir.join(p)).collect()};
    Ok(CanonicalExecutionResult {plan:resolved.plan.clone(),output_dir:resolved.spec.output.bundle_root.clone(),outputs:paths.iter().map(|p|p.display().to_string()).collect(),diagnostics:manifest.diagnostics.iter().map(|d|d.message.clone()).collect(),toolchain:None,run_manifest:manifest,manifest_path})
}
fn write_new(path:&Path,bytes:&[u8])->Result<()> {
    use std::io::Write;let mut file=fs::OpenOptions::new().write(true).create_new(true).open(path)?;file.write_all(bytes)?;file.sync_all()?;Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(targets:&[&str],allow_draft:bool)->(tempfile::TempDir,PlanningRequest) {
        let root=tempfile::tempdir().unwrap();
        fs::write(root.path().join("briefing.json"),include_bytes!("../../../../examples/briefing/briefing.json")).unwrap();
        fs::write(root.path().join("canonical.txt"),include_bytes!("../../../../examples/briefing/canonical.txt")).unwrap();
        let value=json!({"schema":"renderflow/v2","sources":[{"id":"briefing","kind":"briefing","path":"briefing.json"}],"targets":{"exact":targets.iter().map(|t|json!({"format":t})).collect::<Vec<_>>()},"execution":{"briefing":{"allow_draft":allow_draft}},"output":{"bundle_root":"out"}});
        fs::write(root.path().join("build.json"),serde_json::to_vec(&value).unwrap()).unwrap();let request=PlanningRequest::from_path(root.path().join("build.json"));(root,request)
    }
    #[test]
    fn common_build_uses_one_frozen_snapshot_and_exact_target_publication_joins() {
        let (root,request)=fixture(&["briefing_script","briefing_text","briefing_html"],true);
        let resolved=super::super::build::resolve(request).unwrap();assert!(resolved.graph().is_err());
        let identity=resolved.plan().artifact_forest.as_ref().unwrap().identity.clone();
        fs::write(root.path().join("canonical.txt"),b"changed after resolution").unwrap();fs::write(root.path().join("briefing.json"),b"changed after resolution").unwrap();
        let result=super::super::build::execute(resolved,false).unwrap();
        assert_eq!(fs::read(root.path().join("out/sources/overview.txt")).unwrap(),include_bytes!("../../../../examples/briefing/canonical.txt"));
        let manifest=result.run_manifest;let forest=manifest.artifact_forest.unwrap();assert_eq!(forest.identity,identity);assert_eq!(forest.produced_artifacts.len(),3);
        for target in forest.execution.unwrap().targets {
            assert_eq!(target.outcome,ForestTargetOutcome::Published);
            let artifact=&manifest.artifact_manifest.artifacts[target.artifact_evidence_index.unwrap()];assert_eq!(Some(&artifact.artifact_id),target.artifact_id.as_ref());assert_eq!(Some(&artifact.locator),target.locator.as_ref());
        }
        assert!(root.path().join("out/briefing-run.json").is_file());assert!(root.path().join("out/renderflow-run.json").is_file());
    }
    #[test]
    fn absent_optional_audio_or_feed_is_unavailable_and_never_a_publication() {
        for target in ["briefing_audio","briefing_feed"] {
            let (root,request)=fixture(&[target],true);let resolved=super::super::build::resolve(request.clone()).unwrap();
            assert_eq!(resolved.plan().artifact_forest.as_ref().unwrap().branches[0].state,ForestBranchState::Unavailable);
            let dry=super::super::build::execute(resolved,true).unwrap();assert!(dry.run_manifest.artifact_manifest.artifacts.is_empty());assert!(!root.path().join("out").exists());
            assert!(super::super::build::execute(super::super::build::resolve(request).unwrap(),false).is_err());assert!(!root.path().join("out").exists());
        }
    }
    #[test]
    fn review_is_required_and_allow_draft_does_not_override_stale_source() {
        let (root,request)=fixture(&["briefing_text"],false);let resolved=super::super::build::resolve(request).unwrap();assert!(super::super::build::execute(resolved,false).is_err());assert!(!root.path().join("out").exists());
        let (root,request)=fixture(&["briefing_text"],true);fs::write(root.path().join("canonical.txt"),"one\ntwo\nthree\nfour\nfive\n").unwrap();
        let resolved=super::super::build::resolve(request).unwrap();assert_eq!(resolved.plan().artifact_forest.as_ref().unwrap().branches[0].reason_code,"briefing.source.stale");assert!(super::super::build::execute(resolved,false).is_err());assert!(!root.path().join("out").exists());
    }
    #[test]
    fn closed_admission_refuses_ignored_options_duplicate_targets_and_wrong_source() {
        let (_root,request)=fixture(&["briefing_text"],true);let mut spec=crate::spec::load_spec(request.config_path.to_str().unwrap()).unwrap().spec;
        assert!(spec.validate().is_empty());spec.execution.max_parallel=2;assert!(!spec.validate().is_empty());spec.execution.max_parallel=1;
        spec.targets.exact.push(spec.targets.exact[0].clone());assert!(!spec.validate().is_empty());spec.targets.exact.pop();
        spec.sources[0].kind=crate::spec::SourceKind::Artifact;assert!(spec.validate().iter().any(|d|d.code=="briefing.source.required"));
    }
}
