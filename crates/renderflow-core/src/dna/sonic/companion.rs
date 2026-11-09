//! Explicit source-clock-bound structural context; no lyric text leaves here.
use super::*;
pub(super) fn project(companion:&Companion,bytes:&[u8],index:usize,pcm:PcmWavInspection)->Result<Option<SonicFact>> {
    let Some(context)=&companion.timed_text_context else {return Ok(None);};
    let format=match context.format {recipe::CompanionTextFormat::Srt=>crate::timed_text::TimedTextFormat::Srt,recipe::CompanionTextFormat::Webvtt=>crate::timed_text::TimedTextFormat::WebVtt};
    // Use the existing plain-cue parser; markup/settings are never stripped.
    let transcript=crate::timed_text::parse(bytes,format).context("sonic.companion.timed_text")?;
    ensure!(!transcript.cues.is_empty()&&transcript.cues.len()<=context.maximum_cues,"sonic.companion.cue_bound");
    let first=transcript.cues.iter().map(|cue|cue.start_ms).min().context("sonic.companion.empty")?;
    let last=transcript.cues.iter().map(|cue|cue.end_ms).max().context("sonic.companion.empty")?;
    ensure!(last.checked_mul(16).is_some_and(|end|end<=pcm.frame_count),"sonic.companion.outside_audio_clock");
    let mut furthest=0;let mut overlap=false;
    for cue in &transcript.cues {overlap|=cue.start_ms<furthest;furthest=furthest.max(cue.end_ms);}
    Ok(Some(SonicFact {id:format!("companion.{index}.timed-text-structure"),dimension:"sonic.lyrics.declared_timed_text_structure".into(),
        value:json!({"declared_role":companion.kind,"cue_count":transcript.cues.len(),"first_start_ms":first,"last_end_ms":last,
            "overlap_present":overlap,"sample_rate_hz":16000,"source_frame_range":{"start":first*16,"end":last*16},"words_exposed":false}),
        origin:DnaEvidenceOrigin::SourceReported,class:DnaDeterminism::Deterministic,confidence:Confidence::NotApplicable,
        evidence:json!({"companion_sha256":companion.file.sha256,"source_sha256":companion.source_sha256,"request_pointer":format!("/companions/{index}"),
            "authority_reference":companion.authority_reference,"authority_is_declared":true,"validation":"syntax_and_source_clock_only; not text_accuracy_or_alignment",
            "review_authentication":"not_established","payload_exposed_to_model":false}),correction_of:None}))
}
