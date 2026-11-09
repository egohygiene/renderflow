//! Selected candidate notes from a hash-bound public Aniflow report. This is
//! bounded projection, not Aniflow's native validator or an authored score.
use super::*;
use crate::dna::sonic::recipe::MidiContext;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Semantic {artifact_id:String,capability_id:String,kind:String,provenance:Provenance,authority:Option<Value>}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Report {schema:String,source:Source,scope:Scope,technical_artifact:ArtifactRef,upstream_analysis_artifact:ArtifactRef,raw_observation:Option<ArtifactRef>,provider:ProviderRef,implementation_sha256:String,configuration_sha256:String,provider_lock_sha256:String,settings:Value,probe:Value,commands:Vec<Value>,method:Value,provenance:DnaDeterminism,confidence:Confidence,result:MidiResult}
#[derive(Deserialize)]
#[serde(tag="status",rename_all="snake_case",deny_unknown_fields)]
enum MidiResult {Candidate {observation:Observation}}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Observation {native:Native,notes:Vec<Note>}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Native {schema:String,sample_rate_hz:u32,channels:u32,sample_frames:u64,duration_microseconds:u64,notes:Vec<NativeNote>}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeNote {start_microseconds:u64,end_microseconds:u64,pitch:u8,velocity:u8,activation:f64}
#[derive(Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Seconds {numerator:i64,denominator:u32}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Note {id:String,start:Seconds,end:Seconds,start_tick:u32,end_tick:u32,pitch:u8,velocity:u8,activation:f64}
fn seconds(us:u64)->Seconds {let(mut a,mut b)=(us,1_000_000);while b!=0 {let next=a%b;a=b;b=next;}Seconds {numerator:(us/a) as i64,denominator:(1_000_000/a) as u32}}
fn tick(us:u64)->u32 {((us*1920+500_000)/1_000_000) as u32}
pub(super) fn project(value:&Analysis,selection:&MidiContext,payloads:&BTreeMap<&str,&[u8]>,artifacts:&BTreeMap<&str,(&str,u64)>,providers:&BTreeMap<&str,Value>,manifest:&str,pcm:WavInspection,seen:&mut BTreeSet<String>)->Result<Vec<SonicFact>> {
    let matches=value.semantic_artifacts.iter().enumerate().filter(|(_,v)|v.get("artifact_id").and_then(Value::as_str)==Some(selection.artifact_id.as_str())).collect::<Vec<_>>();
    ensure!(matches.len()==1,"sonic.midi.semantic_selection");let (semantic_index,raw)=matches[0];let semantic:Semantic=serde_json::from_value(raw.clone())?;
    ensure!(semantic.kind=="candidate_midi"&&semantic.capability_id=="aniflow/audio-midi-extraction"&&semantic.authority.is_none(),"sonic.midi.candidate_kind");
    let cap=value.capabilities.iter().find(|c|c.capability.id==semantic.capability_id).context("sonic.midi.capability")?;
    ensure!(matches!(cap.status.as_str(),"complete"|"partial")&&cap.capability.version=="1.0.0"&&cap.provider_evidence_ids.contains(&semantic.provenance.provider_evidence_id),"sonic.midi.capability_authority");
    ensure!(!semantic.provenance.evidence_artifact_ids.is_empty(),"sonic.midi.evidence.empty");refs(&semantic.provenance.evidence_artifact_ids,|id|artifacts.contains_key(id)&&cap.evidence_artifact_ids.iter().any(|v|v==id))?;
    ensure!(semantic.provenance.evidence_artifact_ids.contains(&semantic.artifact_id),"sonic.midi.report_membership");
    let report_bytes=*payloads.get(selection.artifact_id.as_str()).context("sonic.midi.report_missing")?;let report:Report=local::parse(report_bytes)?;
    let provider=providers.get(semantic.provenance.provider_evidence_id.as_str()).context("sonic.midi.provider")?;
    ensure!(report.schema=="aniflow.audio-midi/v1"&&report.source==value.source&&report.source.artifact.id=="source_audio"&&report.scope.channels==(0..u32::from(pcm.channels)).collect::<Vec<_>>()&&report.scope.stem_id.as_deref()==value.source.stem.as_ref().map(|s|s.id.as_str()),"sonic.midi.source_or_scope");
    ensure!(report.provider.id=="org.egohygiene.aniflow.audio-midi"&&report.provider.version=="1.0.0"&&provider["id"]==report.provider.id&&provider["version"]==report.provider.version&&provider["implementation_sha256"]==report.implementation_sha256&&provider["configuration_sha256"]==report.configuration_sha256,"sonic.midi.provider_binding");
    files::digest(&report.provider_lock_sha256)?;
    for reference in [&report.technical_artifact,&report.upstream_analysis_artifact] {ensure!(artifacts.get(reference.id.as_str())==Some(&(reference.sha256.as_str(),reference.byte_size)),"sonic.midi.upstream_binding");}
    ensure!(report.technical_artifact.id=="technical"&&matches!(report.upstream_analysis_artifact.id.as_str(),"analysis"|"stem_analysis"),"sonic.midi.upstream_roles");
    ensure!([&report.source.artifact.id,&report.technical_artifact.id,&report.upstream_analysis_artifact.id,&semantic.artifact_id].into_iter().all(|id|semantic.provenance.evidence_artifact_ids.contains(id)),"sonic.midi.required_lineage");
    let raw=report.raw_observation.as_ref().context("sonic.midi.raw_reference")?;files::digest(&raw.sha256)?;ensure!(raw.id=="midi_observation"&&raw.byte_size>0&&raw.byte_size<=2*1024*1024,"sonic.midi.raw_reference");
    ensure!(report.settings.is_object()&&report.probe.is_object()&&report.commands.len()<=8,"sonic.midi.opaque_declarations");
    ensure!(report.method==json!({"sample_rate_hz":22050,"downmix":"none","backend":"onnxruntime_cpu","gpu":false,"pitch_bends":false,"instrument":"unavailable","native_time_quantization":"nearest_microsecond_ties_up","maximum_native_time_error_nanoseconds":500,"velocity_mapping":"round_ties_even_127_times_activation","minimum_pitch":21,"maximum_pitch":108}),"sonic.midi.method");
    ensure!(report.provenance==DnaDeterminism::Probabilistic&&semantic.provenance.class==report.provenance&&semantic.provenance.confidence==report.confidence&&matches!(&report.confidence,Confidence::Unavailable {reason} if reason=="Note activations and MIDI velocities are not calibrated confidence; inferred notes are unreviewed candidates."),"sonic.midi.uncertainty");
    report.confidence.validate(report.provenance)?;
    let MidiResult::Candidate {observation}=report.result;let native=&observation.native;
    ensure!(pcm.sample_rate_hz==22050&&pcm.channels==1&&pcm.bits_per_sample==16&&pcm.encoding=="pcm_s16le"&&pcm.frame_count<=2_646_000&&value.source.artifact.byte_size<=8*1024*1024&&native.schema=="aniflow.audio-midi-observation/v1"&&native.sample_rate_hz==pcm.sample_rate_hz&&native.channels==u32::from(pcm.channels)&&native.sample_frames==pcm.frame_count&&native.duration_microseconds==pcm.frame_count*1_000_000/u64::from(pcm.sample_rate_hz),"sonic.midi.native_clock");
    ensure!(!observation.notes.is_empty()&&observation.notes.len()<=selection.maximum_notes&&observation.notes.len()==native.notes.len(),"sonic.midi.note_bound");
    let report_sha=local::sha256(report_bytes);let mut output=Vec::new();let mut previous=None;let mut ends=[0u64;128];let mut ticks=[0u32;128];
    for (index,(note,raw)) in observation.notes.iter().zip(&native.notes).enumerate() {
        let key=(raw.start_microseconds,raw.pitch,raw.end_microseconds);
        ensure!(raw.start_microseconds<raw.end_microseconds&&raw.end_microseconds<=native.duration_microseconds&&(21..=108).contains(&raw.pitch)&&raw.activation.is_finite()&&(0.0..=1.0).contains(&raw.activation)&&(1..=127).contains(&raw.velocity)&&f64::from(raw.velocity)==(127.0*raw.activation).round_ties_even()&&previous.is_none_or(|p|p<key),"sonic.midi.native_note");
        ensure!(note.id==format!("note_{index:06}")&&seen.insert(note.id.clone())&&note.start==seconds(raw.start_microseconds)&&note.end==seconds(raw.end_microseconds)&&note.start_tick==tick(raw.start_microseconds)&&note.end_tick==tick(raw.end_microseconds)&&note.start_tick<note.end_tick&&note.pitch==raw.pitch&&note.velocity==raw.velocity&&note.activation==raw.activation,"sonic.midi.note_mapping");
        let pitch=usize::from(note.pitch);ensure!(raw.start_microseconds>=ends[pitch]&&note.start_tick>=ticks[pitch],"sonic.midi.same_pitch_overlap");ends[pitch]=raw.end_microseconds;ticks[pitch]=note.end_tick;previous=Some(key);
        output.push(SonicFact {id:format!("analysis.{manifest}.midi.{index}"),dimension:"sonic.score.inferred_note".into(),value:json!({"pitch_midi":note.pitch,"velocity":note.velocity,"activation":note.activation,"activation_is_confidence":false,"start_seconds":note.start,"end_seconds":note.end,"start_tick":note.start_tick,"end_tick":note.end_tick,"tick_clock":"960_PPQ_at_500000_us_per_quarter; serialization_clock_not_detected_tempo","sample_rate_hz":pcm.sample_rate_hz,"channel_scope":report.scope.channels,"stem_id":report.scope.stem_id,"review_state":"unreviewed_candidate","alignment_to_original_mix":"not_established"}),origin:DnaEvidenceOrigin::SourceReported,class:report.provenance,confidence:report.confidence.clone(),evidence:json!({"manifest_sha256":manifest,"pointer":format!("/semantic_artifacts/{semantic_index}"),"report_sha256":report_sha,"report_pointer":format!("/result/observation/notes/{index}"),"native_note_id":note.id,"provider_declaration":provider,"provider_authentication":"not_established","native_validation":"bounded_projection_only","native_raw_bytes_observed":false,"raw_observation_reference":raw_reference(&report.raw_observation),"instrument_semantics":"not_inferred"}),correction_of:None});
    }
    Ok(output)
}
fn raw_reference(reference:&Option<ArtifactRef>)->Value {serde_json::to_value(reference).unwrap_or(Value::Null)}
