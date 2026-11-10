//! Synthetic native-format evidence only; authored/uncompiled/unrun.
use super::*;
fn stereo24(rate:u32,frames:u32)->Vec<u8> {
    let size=frames*6;let mut bytes=b"RIFF".to_vec();bytes.extend((36+size).to_le_bytes());bytes.extend(b"WAVEfmt ");bytes.extend(16u32.to_le_bytes());bytes.extend(1u16.to_le_bytes());bytes.extend(2u16.to_le_bytes());bytes.extend(rate.to_le_bytes());bytes.extend((rate*6).to_le_bytes());bytes.extend(6u16.to_le_bytes());bytes.extend(24u16.to_le_bytes());bytes.extend(b"data");bytes.extend(size.to_le_bytes());bytes.resize(44+size as usize,0);bytes
}
pub(super) fn native(root:&Path,input:&Path,recipe:&mut SonicRecipe) {
    let bytes=stereo24(44_100,4410);fs::write(root.join("source.wav"),&bytes).unwrap();recipe.audio.sha256=hash(&bytes);
    mutate_analysis(root,input,recipe,|v|{v["source"]["artifact"]["sha256"]=json!(hash(&bytes));v["source"]["artifact"]["byte_size"]=json!(bytes.len());v["source"]["sample_rate_hz"]=json!(44_100);v["source"]["channels"]=json!(2);v["source"]["frame_count"]=json!(4410);for observation in v["observations"].as_array_mut().unwrap(){observation["scope"]["channels"]=json!([0,1]);}});
}
#[test]
fn source_plan_and_dna_report_native_stereo_frames_without_conversion() {
    let root=tempfile::tempdir().unwrap();let (input,mut recipe)=fixture(root.path());native(root.path(),&input,&mut recipe);
    let plan=sonic::plan(&input,None,token()).unwrap();assert_eq!(plan.technical["sample_rate_hz"],44_100);assert_eq!(plan.technical["channels"],2);assert_eq!(plan.technical["bits_per_sample"],24);assert_eq!(plan.technical["frame_count"],4410);assert_eq!(plan.technical["duration_ms"],100);assert_eq!(plan.technical["sample_conversion"],"none");
    let before=fs::read(root.path().join("source.wav")).unwrap();let output=root.path().join("native");sonic::build(&request(&input,&output),None,token()).unwrap();let dna=read(&output.join("sonic-dna.json"));let rows=dna["observations"].as_array().unwrap();assert_eq!(rows.iter().find(|r|r["id"]=="pcm.sample-rate").unwrap()["value"],44_100);assert_eq!(fs::read(root.path().join("source.wav")).unwrap(),before);assert!(!output.join("source.wav").exists());
}
#[test]
fn native_analysis_clock_and_channel_scope_mismatches_refuse() {
    for change in 0..3 {let root=tempfile::tempdir().unwrap();let (input,mut recipe)=fixture(root.path());native(root.path(),&input,&mut recipe);mutate_analysis(root.path(),&input,&mut recipe,|v|match change {0=>v["source"]["sample_rate_hz"]=json!(16_000),1=>v["observations"][0]["scope"]["channels"]=json!([2]),_=>v["observations"][0]["scope"]["channels"]=json!([1,1])});assert!(sonic::plan(&input,None,token()).is_err());}
}
#[test]
fn selected_srt_context_uses_original_44100_clock_and_ceiling_sample_grid() {
    let root=tempfile::tempdir().unwrap();let (input,mut recipe)=fixture(root.path());native(root.path(),&input,&mut recipe);
    let bytes=b"1\n00:00:00,001 --> 00:00:00,099\nPrivate native clock words\n";fs::write(root.path().join("words.srt"),bytes).unwrap();
    recipe.companions.push(Companion {file:FileInput {id:"lyrics".into(),path:"words.srt".into(),sha256:hash(bytes)},source_sha256:recipe.audio.sha256.clone(),kind:CompanionKind::ReviewedLyrics,authority_reference:"synthetic only".into(),timed_text_context:Some(sonic::TimedTextContext {format:sonic::CompanionTextFormat::Srt,maximum_cues:8})});save(&input,&recipe);
    let plan=sonic::plan(&input,None,token()).unwrap();let context=&plan.declared_companions[0]["structural_context"];assert_eq!(context["source_frame_range"],json!({"start":45,"end":4366}));assert_eq!(context["sample_rate_hz"],44_100);assert!(!serde_json::to_string(context).unwrap().contains("Private native"));
}
