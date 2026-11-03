use std::collections::BTreeMap;
use std::io::{Read,Write};
use std::path::Path;

use anyhow::{ensure,Result};
use crate::graph::Format;
use crate::lyrics::{self,LyricsPolicy,ReviewState};

pub fn inspect(input:&str,input_format:Option<&str>,language:Option<String>,revision:Option<String>,reviewed:bool) -> Result<()> {
    let format = match input_format {
        Some(format) => format.parse()?,
        None if input.to_ascii_lowercase().ends_with(".srt") => Format::Srt,
        None if input.to_ascii_lowercase().ends_with(".vtt") => Format::WebVtt,
        None if input.to_ascii_lowercase().ends_with(".timed-transcript.json") => Format::TimedTranscriptJson,
        _ => anyhow::bail!("lyrics.input_format: specify srt, vtt or timed_transcript_json for an unrecognized suffix"),
    };
    lyrics::input_format(format)?;
    let mut bytes = Vec::new();
    std::fs::File::open(Path::new(input))?.take(crate::timed_text::MAX_INPUT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= crate::timed_text::MAX_INPUT_BYTES,"lyrics.input.limit");
    let policy = LyricsPolicy {source_sha256:lyrics::sha256(&bytes),review_state:if reviewed {ReviewState::Reviewed} else {ReviewState::Unreviewed},
        language,revision,sections:BTreeMap::new()};
    let document = lyrics::normalize(&bytes,format,&policy)?;
    let mut output = serde_json::to_vec_pretty(&serde_json::json!({"schema_version":"renderflow.lyrics-inspection/v1",
        "policy":policy,"cues":document.transcript.cues.len(),"eligibility":lyrics::eligibility(&document),
        "provider_executed":false,"runtime_qualified":false}))?;
    output.push(b'\n');
    std::io::stdout().write_all(&output)?;
    Ok(())
}
