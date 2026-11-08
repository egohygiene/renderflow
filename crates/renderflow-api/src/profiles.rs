//! Closed service admission policies over core-owned SDK/config contracts.
//! These ceilings select existing routes; they do not plan or render artifacts.

use std::collections::BTreeMap;
use std::sync::Arc;
use renderflow::graph::{ExecutionPlan,Format};
use renderflow::spec::{CollisionPolicy,ExecutionPolicy,SourceKind,SpecV2,TargetSpec};
use crate::model::*;
use crate::package::{digest,relative};
#[path="publication_profile.rs"]
mod publication;

fn refused(code:&str,message:&str)->ApiError {ApiError::new(code,message,422)}

pub(crate) fn outputs(profile:PipelineProfile)->Vec<Format> {
    match profile {
        PipelineProfile::Composition|PipelineProfile::Briefing|PipelineProfile::Presentation=>Vec::new(),
        PipelineProfile::ManualLyrics=>renderflow::lyrics::OUTPUTS.to_vec(),
        PipelineProfile::TimedText=>vec![Format::Srt,Format::WebVtt,Format::TimedTranscriptJson,Format::Text],
        PipelineProfile::FountainBody=>vec![Format::ScreenplayJson,Format::Fdx,Format::FadeIn,Format::Osf,Format::Trelby],
        PipelineProfile::Editorial=>vec![Format::Markdown,Format::Html,Format::Pdf,Format::Docx,Format::Epub],
    }
}
pub(crate) fn providers(profile:PipelineProfile)->Vec<String> {
    let ids:&[&str]=match profile {
        PipelineProfile::Composition=>&[],
        PipelineProfile::ManualLyrics=>&["tool.renderflow-lyrics"],
        PipelineProfile::TimedText=>&["tool.renderflow-timed-text"],
        PipelineProfile::FountainBody=>&["tool.renderflow-screenplay-body"],
        PipelineProfile::Editorial=>&["tool.pandoc","tool.tectonic"],
        PipelineProfile::Briefing=>&["renderflow.briefing"],
        PipelineProfile::Presentation=>&["renderflow.presentation","tool.slidev"],
    };ids.iter().map(|value|value.to_string()).collect()
}
fn transforms(profile:PipelineProfile,source:Format)->Vec<String> {
    match profile {
        PipelineProfile::Composition|PipelineProfile::Briefing|PipelineProfile::Presentation=>Vec::new(),
        PipelineProfile::ManualLyrics=>std::iter::once(format!("builtin.lyrics.{source}.lyrics_json"))
            .chain(outputs(profile).iter().filter(|format|**format!=Format::LyricsJson)
                .map(|format|format!("builtin.lyrics.lyrics_json.{format}"))).collect(),
        PipelineProfile::TimedText=>vec![("srt","timed_transcript_json"),("vtt","timed_transcript_json"),
            ("timed_transcript_json","srt"),("timed_transcript_json","vtt"),("timed_transcript_json","text")]
            .iter().map(|(from,to)|format!("builtin.subtitle.{from}.{to}")).collect(),
        PipelineProfile::FountainBody=>outputs(profile).iter().map(|format|format!("builtin.screenplay.body.fountain.{format}")).collect(),
        PipelineProfile::Editorial=>["html","pdf","docx","epub"].iter().map(|format|format!("builtin.markdown.{format}")).collect(),
    }
}

fn target(profile:PipelineProfile,target:&TargetSpec,transforms:&[String],providers:&[String])->ApiResult<()> {
    if target.id.as_ref().is_some_and(|id|!stable_id(id)) {
        return Err(refused("profile_target","Target IDs must be stable service identifiers"));
    }
    if let Some(role)=&target.role {relative(role)?;}
    let format=target.format.as_deref().and_then(|value|value.parse::<Format>().ok());
    if !format.is_some_and(|format|outputs(profile).contains(&format)) || target.family.is_some()
        || target.capability.is_some() || target.variant.is_some() || target.template.is_some() || !target.options.is_empty()
        || target.provider.as_ref().is_some_and(|value|!providers.contains(value))
        || target.transform.as_ref().is_some_and(|value|!transforms.contains(value))
        || (if profile==PipelineProfile::Editorial && format!=Some(Format::Markdown) {
            target.preset.as_deref()!=Some("editorial-v1")
        } else {target.preset.is_some()}) {
        return Err(refused("profile_target","Target does not fit the registered finite service profile"));
    }
    Ok(())
}

/// Called before a package is admitted or an SDK planner can discover tools.
/// Only the effective private output/policy ceilings are rewritten. Original
/// source identity and the effective config identity remain separate.
pub(crate) fn admit(config:&PipelineConfig,spec:&mut SpecV2,files:&BTreeMap<String,Arc<[u8]>>)->ApiResult<()> {
    if spec.schema!="renderflow/v2" || spec.sources.len()!=1 {
        return Err(refused("unsupported_profile","Service profiles require one explicit v2 source"));
    }
    let source=&spec.sources[0];
    let path=source.path.as_deref().ok_or_else(||refused("source_path","A captured source-relative path is required"))?;
    relative(path)?;
    if source.uri.is_some() || !files.contains_key(path) {
        return Err(refused("source_path","The source must be present in the package allowlist without a URI"));
    }
    if config.profile==PipelineProfile::Briefing {return publication::admit_briefing(config,spec,files);}
    if config.profile==PipelineProfile::Presentation {return publication::admit_presentation(config,spec,files);}
    if config.profile==PipelineProfile::Composition {
        if config.allow_local_processes || source.kind!=SourceKind::Composition {
            return Err(refused("unsupported_profile","Composition does not admit external process effects"));
        }
        if spec.execution.composition.as_ref().is_some_and(|policy|policy.cache_directory.is_some()) {
            return Err(refused("shared_cache_refused","Service jobs require private bundle-local caches"));
        }
        return Ok(()); // Core's composition admission validates all native policy.
    }
    if source.kind!=SourceKind::Artifact || spec.transforms.is_some() || !spec.variables.is_empty()
        || !spec.hygiene.is_empty() || spec.publication.is_some() || spec.output.collision!=CollisionPolicy::Error {
        return Err(refused("profile_package","Graph profiles require one captured artifact and no custom registry, variables, hygiene, publication or overwrite policy"));
    }
    if !stable_id(&source.id) {return Err(refused("source_identity","Source ID must be a stable service identifier"));}
    relative(&spec.output.naming_template)?;
    if config.allow_local_processes!=config.profile.local_processes() {
        return Err(refused("process_policy","Editorial requires explicit local-process permission; built-in profiles refuse that permission"));
    }
    let format:Format=source.format.as_deref().ok_or_else(||refused("source_format","Graph service sources need an explicit format"))?
        .parse().map_err(|_|refused("source_format","Unknown source format"))?;
    let format_allowed=match config.profile {
        PipelineProfile::ManualLyrics|PipelineProfile::TimedText=>matches!(format,Format::Srt|Format::WebVtt|Format::TimedTranscriptJson),
        PipelineProfile::FountainBody=>format==Format::Fountain,
        PipelineProfile::Editorial=>format==Format::Markdown,
        PipelineProfile::Composition|PipelineProfile::Briefing|PipelineProfile::Presentation=>false,
    };
    if !format_allowed {return Err(refused("source_format","Source format is outside the registered profile"));}
    let providers=providers(config.profile);let transforms=transforms(config.profile,format);
    let mut allowed=ExecutionPolicy::default();
    allowed.optimization=spec.execution.optimization;
    allowed.derivative_planning=spec.execution.derivative_planning.clone();
    allowed.max_parallel=spec.execution.max_parallel;
    allowed.budgets=spec.execution.budgets.clone();
    allowed.tools=spec.execution.tools.clone();allowed.transforms=spec.execution.transforms.clone();
    allowed.requirements=spec.execution.requirements.clone();allowed.validation=spec.execution.validation.clone();
    allowed.minimum_fidelity=spec.execution.minimum_fidelity;allowed.reject_loss_classes=spec.execution.reject_loss_classes.clone();
    match config.profile {
        PipelineProfile::ManualLyrics=> {
            let policy=spec.execution.lyrics.as_ref().ok_or_else(||refused("profile_policy","Manual lyrics requires its existing explicit source policy"))?;
            if policy.source_sha256!=digest(files.get(path).ok_or_else(ApiError::internal)?) {
                return Err(refused("source_identity","Manual lyric source bytes differ from the explicit policy digest"));
            }
            allowed.lyrics=Some(policy.clone());
        },
        PipelineProfile::TimedText=> {
            let policy=spec.execution.subtitles.as_ref().ok_or_else(||refused("profile_policy","Timed text requires an explicit standalone subtitle policy"))?;
            if policy.stream_index.is_some(){return Err(refused("profile_policy","Embedded media extraction is not admitted"));}
            allowed.subtitles=Some(policy.clone());
        },
        PipelineProfile::FountainBody=> {
            let policy=spec.execution.screenplay_body.as_ref().ok_or_else(||refused("profile_policy","Fountain exports require explicit unqualified body policy"))?;
            if !policy.allow_unqualified{return Err(refused("profile_policy","Fountain body exports require the core's explicit unqualified opt-in"));}
            allowed.screenplay_body=Some(policy.clone());
        },
        _=>{},
    }
    if allowed!=spec.execution || allowed.max_parallel>2 || allowed.derivative_planning.max_hops>8
        || allowed.derivative_planning.max_candidates>10000 || !allowed.validation.validators.is_empty()
        || allowed.tools.allow.iter().any(|id|!providers.contains(id))
        || allowed.transforms.allow.iter().any(|id|!transforms.contains(id)) {
        return Err(refused("profile_policy","Execution requests an unadmitted effect, custom policy/provider, validator or planning bound"));
    }
    let bundled_name=match config.profile {PipelineProfile::ManualLyrics=>Some("lyrics"),PipelineProfile::Editorial=>Some("editorial"),_=>None};
    for (name,profile) in &spec.profiles {
        if Some(name.as_str())!=bundled_name || renderflow::spec::bundled_profile(name).map_err(|_|ApiError::internal())?.as_ref()!=Some(profile) {
            return Err(refused("custom_profile","Only the unchanged selected core bundled profile may be expanded"));
        }
    }
    if spec.targets.profiles.iter().any(|name|Some(name.as_str())!=bundled_name) || spec.targets.exact.len()>32 {
        return Err(refused("profile_target","Unknown profile or too many exact targets"));
    }
    for selected in spec.targets.exact.iter().chain(spec.profiles.values().flat_map(|profile|&profile.targets)) {
        target(config.profile,selected,&transforms,&providers)?;
    }
    // Clamp optional graph expansion to the same existing closed routes. Denies
    // stay intact. No planner, native executor or provider registry is replaced.
    spec.execution.tools.allow=if allowed.tools.allow.is_empty(){providers}else{allowed.tools.allow};
    spec.execution.transforms.allow=if allowed.transforms.allow.is_empty(){transforms}else{allowed.transforms.allow};
    spec.execution.requirements.local_only=true;spec.execution.requirements.offline=true;
    Ok(())
}

pub(crate) fn guard_plan(profile:PipelineProfile,plan:&ExecutionPlan)->ApiResult<()> {
    if profile==PipelineProfile::Briefing {return publication::guard_briefing(plan);}
    if profile==PipelineProfile::Presentation {return publication::guard_presentation(plan);}
    if profile==PipelineProfile::Composition{return Ok(());}
    let providers=providers(profile);let source:Format=plan.source.parse().map_err(|_|ApiError::internal())?;
    let transforms=transforms(profile,source);
    for edge in &plan.edges {
        if edge.provider_id.as_ref().is_none_or(|id|!providers.contains(id))
            || edge.required_provider_ids.iter().any(|id|!providers.contains(id))
            || edge.evidence.get("transform_id").is_none_or(|id|!transforms.contains(id)) {
            return Err(refused("profile_plan","SDK selected a route outside the registered service ceilings"));
        }
    }
    Ok(())
}

pub(crate) fn graph_download_extension(format:&str)->ApiResult<String> {
    if let Some((_,_,extension,_))=publication::BRIEFING_OUTPUTS.iter().find(|row|row.0==format){return Ok((*extension).into());}
    if let Some((_,_,extension,_))=publication::PRESENTATION_OUTPUTS.iter().find(|row|row.0==format){return Ok((*extension).into());}
    let format:Format=format.parse().map_err(|_|ApiError::internal())?;
    let registry=renderflow::graph::capability::FormatCapabilityRegistry::global();
    let descriptor=registry.get(format).ok_or_else(ApiError::internal)?;
    let extension=descriptor.extensions.first().ok_or_else(ApiError::internal)?;
    if extension.is_empty() || !extension.bytes().all(|byte|byte.is_ascii_alphanumeric()||matches!(byte,b'.'|b'-'|b'_')) {
        return Err(ApiError::internal());
    }
    Ok(extension.to_string())
}

pub(crate) fn inventory()->serde_json::Value {
    let registry=renderflow::graph::capability::FormatCapabilityRegistry::global();
    serde_json::json!(PipelineProfile::ALL.iter().map(|profile| {
        let formats=if *profile==PipelineProfile::Composition {
            use renderflow::composition::CompositionOutput::*;
            [Svg,Html,TranscriptJson,TranscriptText,Png,Pdf,Ora,Psd,Idml].iter().map(|format|
                serde_json::json!({"id":format.name(),"media_type":format.media_type(),"implemented":format.supported()})).collect::<Vec<_>>()
        } else if *profile==PipelineProfile::Briefing {publication::BRIEFING_OUTPUTS.iter().map(|(id,media,_,_)|serde_json::json!({"id":id,"media_type":media,"implemented":true})).collect()}
        else if *profile==PipelineProfile::Presentation {publication::PRESENTATION_OUTPUTS.iter().map(|(id,media,_,_)|serde_json::json!({"id":id,"media_type":media,"implemented":true,"native_provider_required":*id!="presentation_source"})).collect()}
        else {outputs(*profile).iter().filter_map(|format|registry.get(*format)).map(|descriptor|
            serde_json::json!({"id":descriptor.id,"media_type":descriptor.media_types.first(),"implemented":true})).collect()};
        serde_json::json!({"selection":profile,"profile":profile.name(),"local_processes":profile.local_processes(),
            "providers":providers(*profile),"outputs":formats,"runtime_qualified":false})
    }).collect::<Vec<_>>())
}

#[cfg(test)]
#[path="profile_tests.rs"]
mod tests;
