//! Finite captured publication admission; core owns native plans and reviews.
use super::*;
use std::collections::BTreeSet;
pub(super) const BRIEFING_OUTPUTS:[(&str,&str,&str,&str);6]=[
    ("briefing_script","application/json","json","script.json"),
    ("briefing_text","text/plain","txt","transcript.txt"),
    ("briefing_html","text/html","html","transcript.html"),
    ("briefing_chapters","application/json","json","chapters.json"),
    ("briefing_feed","application/rss+xml","rss","feed.rss"),
    ("briefing_audio","audio/wav","wav","narration.wav"),
];
fn reference(model:&str,value:&str,files:&BTreeMap<String,Arc<[u8]>>,required:&mut BTreeSet<String>)->ApiResult<()> {
    relative(value)?;
    if value.split('/').any(|part|part.is_empty()||part=="."||part=="..") {return Err(refused("publication_reference","Publication references require exact confined relative paths"));}
    let parent=std::path::Path::new(model).parent().unwrap_or_else(||std::path::Path::new(""));
    let path=parent.join(value);let name=path.to_str().ok_or_else(||refused("publication_reference","Publication reference must be UTF-8"))?;
    relative(name)?;
    if !files.contains_key(name){return Err(refused("publication_reference","A referenced publication input is absent from the captured package allowlist"));}
    required.insert(name.into());Ok(())
}
pub(super) fn admit_briefing(config:&PipelineConfig,spec:&mut SpecV2,files:&BTreeMap<String,Arc<[u8]>>)->ApiResult<()> {
    if config.allow_local_processes||config.allow_source_uploads||spec.sources[0].kind!=SourceKind::Briefing||!stable_id(&spec.sources[0].id)||!spec.validate().is_empty(){
        return Err(refused("briefing_profile","Briefing service admission requires the existing closed local core profile without processes, uploads or custom effects"));
    }
    let path=spec.sources[0].path.as_deref().ok_or_else(ApiError::internal)?;
    let model=renderflow::publication::briefing::parse(files.get(path).ok_or_else(ApiError::internal)?)
        .map_err(|_|refused("briefing_model","Core refused the captured briefing model"))?;
    let mut required=BTreeSet::from([config.config.clone(),path.into()]);
    for source in &model.sources {reference(path,&source.path,files,&mut required)?;}
    if let Some(narration)=&model.narration {reference(path,&narration.path,files,&mut required)?;}
    if files.keys().cloned().collect::<BTreeSet<_>>()!=required {return Err(refused("publication_allowlist","Publication package must contain exactly its config, model and explicitly referenced local inputs"));}
    // Retain exact source/script/narration reviews and allow_draft; never reset
    // them as if a client could approve new content by editing service policy.
    spec.execution.requirements.local_only=true;spec.execution.requirements.offline=true;Ok(())
}
pub(super) fn guard_briefing(plan:&ExecutionPlan)->ApiResult<()> {
    let forest=plan.artifact_forest.as_ref().ok_or_else(||refused("briefing_plan","Core briefing forest is missing"))?;
    if plan.source!="briefing_snapshot"||plan.targets.is_empty()||plan.targets.len()>6||forest.branches.len()!=plan.targets.len()
        ||forest.profiles!=vec![renderflow::publication::briefing::PROFILE.to_owned()]
        ||forest.branches.iter().any(|b|!BRIEFING_OUTPUTS.iter().any(|(id,_,_,file)|b.format==*id&&b.options.get("file").and_then(|v|v.as_str())==Some(*file)))
        ||plan.edges.iter().any(|e|e.provider_id.as_deref()!=Some("renderflow.briefing")||!e.required_provider_ids.is_empty()
            ||!BRIEFING_OUTPUTS.iter().any(|(id,_,_,_)|e.to==*id&&e.capability_id.as_deref()==Some(format!("briefing.{id}").as_str()))){
        return Err(refused("briefing_plan","SDK selected a route outside the finite local briefing profile"));
    }Ok(())
}

#[cfg(test)]
#[path="publication_profile_tests.rs"]
mod tests;
