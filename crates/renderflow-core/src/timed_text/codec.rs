use sha2::{Digest, Sha256};

use super::{invalid, validate, TimedCue, TimedTextError, TimedTextFormat, TimedTranscript, TranscriptSource, MAX_CUES, MAX_INPUT_BYTES};

/// Parse a supported subtitle format or a full provenance-preserving JSON model.
pub fn parse(bytes: &[u8], format: TimedTextFormat) -> Result<TimedTranscript, TimedTextError> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(TimedTextError::InputTooLarge);
    }
    if format == TimedTextFormat::Text {
        return Err(TimedTextError::UnsupportedInput);
    }
    let input = std::str::from_utf8(bytes)?;
    if format == TimedTextFormat::Json {
        // Serde derives can otherwise accept positional sequences for structs.
        // Inspect object shapes, then deserialize the ORIGINAL bytes so that
        // duplicate fields are still rejected by serde's struct visitors.
        let shape: serde_json::Value = serde_json::from_str(input)?;
        require_object_shape(&shape)?;
        let transcript: TimedTranscript = serde_json::from_str(input)?;
        validate(&transcript)?;
        return Ok(transcript);
    }
    let normalized = input.strip_prefix('\u{feff}').unwrap_or(input).replace("\r\n", "\n");
    if normalized.contains('\r') {
        return Err(invalid("bare carriage returns are unsupported; use LF or CRLF"));
    }
    let mut lines = normalized.lines();
    if format == TimedTextFormat::WebVtt {
        if lines.next() != Some("WEBVTT") {
            return Err(invalid("WebVTT requires an exact WEBVTT header"));
        }
        if lines.next().is_none_or(|line| !line.trim().is_empty()) {
            return Err(TimedTextError::UnsupportedFeature {
                feature: "WebVTT header metadata or missing header separator".to_owned(),
            });
        }
    }
    let mut cues = Vec::new();
    let mut block = Vec::new();
    for line in lines {
        if line.trim().is_empty() {
            if !block.is_empty() {
                parse_block(&block, format, &mut cues)?;
                block.clear();
            }
        } else {
            block.push(line);
        }
    }
    if !block.is_empty() {
        parse_block(&block, format, &mut cues)?;
    }
    let transcript = TimedTranscript::new(TranscriptSource {
        sha256: format!("{:x}", Sha256::digest(bytes)),
        format: format.as_str().to_owned(),
        stream: None,
        extraction: None,
    }, cues);
    validate(&transcript)?;
    Ok(transcript)
}

fn require_object_shape(value: &serde_json::Value) -> Result<(), TimedTextError> {
    let root = value.as_object().ok_or_else(|| invalid("JSON transcript must be an object"))?;
    let source = root.get("source").and_then(serde_json::Value::as_object).ok_or_else(|| invalid("JSON source must be an object"))?;
    for key in ["stream", "extraction"] {
        if source.get(key).is_some_and(|value| !value.is_null() && !value.is_object()) {
            return Err(invalid(format!("JSON source.{key} must be an object or null")));
        }
    }
    let cues = root.get("cues").and_then(serde_json::Value::as_array).ok_or_else(|| invalid("JSON cues must be an array"))?;
    if cues.iter().any(|cue| !cue.is_object()) {
        return Err(invalid("each JSON cue must be an object"));
    }
    Ok(())
}

fn parse_block(lines: &[&str], format: TimedTextFormat, cues: &mut Vec<TimedCue>) -> Result<(), TimedTextError> {
    if cues.len() >= MAX_CUES {
        return Err(invalid("transcript exceeds 100000 cues"));
    }
    let first = lines[0];
    let (id, timing_index) = match format {
        TimedTextFormat::Srt => {
            if first.is_empty() || !first.bytes().all(|byte| byte.is_ascii_digit()) || first.bytes().all(|byte| byte == b'0') {
                return Err(invalid("SRT cue identifiers must be positive decimal integers"));
            }
            (first.to_owned(), 1)
        }
        TimedTextFormat::WebVtt => {
            if matches!(first, "STYLE" | "REGION" | "NOTE") || first.starts_with("NOTE ") || first.starts_with("NOTE\t") {
                return Err(TimedTextError::UnsupportedFeature { feature: "WebVTT STYLE, REGION and NOTE blocks".to_owned() });
            }
            if first.contains("-->") {
                (format!("cue-{:06}", cues.len() + 1), 0)
            } else {
                (first.to_owned(), 1)
            }
        }
        _ => return Err(TimedTextError::UnsupportedInput),
    };
    let timing = lines.get(timing_index).ok_or_else(|| invalid("cue is missing its timing line"))?;
    let (start, end) = timing.split_once(" --> ").ok_or_else(|| invalid("timing line must use a single space around -->"))?;
    if end.chars().any(char::is_whitespace) {
        return Err(TimedTextError::UnsupportedFeature { feature: "cue settings or trailing timing metadata".to_owned() });
    }
    let start_ms = timestamp(start, format)?;
    let end_ms = timestamp(end, format)?;
    let text = lines.get(timing_index + 1..).unwrap_or_default().join("\n");
    cues.push(TimedCue { id, start_ms, end_ms, text, speaker: None });
    Ok(())
}

fn timestamp(value: &str, format: TimedTextFormat) -> Result<u64, TimedTextError> {
    let separator = if format == TimedTextFormat::Srt { ',' } else { '.' };
    let (clock, fraction) = value.split_once(separator).ok_or_else(|| invalid("timestamp has an incorrect millisecond separator"))?;
    if fraction.len() != 3 || !fraction.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(invalid("timestamps require exactly three millisecond digits"));
    }
    let parts: Vec<&str> = clock.split(':').collect();
    if !(parts.len() == 3 || format == TimedTextFormat::WebVtt && parts.len() == 2) || parts.iter().any(|part| part.len() != 2 || !part.bytes().all(|byte| byte.is_ascii_digit())) {
        return Err(invalid("timestamp requires HH:MM:SS (or WebVTT MM:SS), with two digits per field"));
    }
    let mut values = Vec::new();
    for part in parts {
        values.push(part.parse::<u64>().map_err(|_| invalid("invalid timestamp digits"))?);
    }
    let (hours, minutes, seconds) = if values.len() == 3 { (values[0], values[1], values[2]) } else { (0, values[0], values[1]) };
    if minutes >= 60 || seconds >= 60 {
        return Err(invalid("timestamp minutes and seconds must be below 60"));
    }
    let milliseconds = fraction.parse::<u64>().map_err(|_| invalid("invalid timestamp milliseconds"))?;
    Ok(((hours * 60 + minutes) * 60 + seconds) * 1000 + milliseconds)
}

fn write_timestamp(milliseconds: u64, separator: char) -> String {
    format!("{:02}:{:02}:{:02}{}{:03}", milliseconds / 3_600_000, milliseconds / 60_000 % 60, milliseconds / 1000 % 60, separator, milliseconds % 1000)
}

/// Deterministic LF output. Call `export_losses` before presenting non-JSON bytes.
pub fn render(transcript: &TimedTranscript, format: TimedTextFormat) -> Result<Vec<u8>, TimedTextError> {
    validate(transcript)?;
    if format == TimedTextFormat::Json {
        let mut output = serde_json::to_vec_pretty(transcript)?;
        output.push(b'\n');
        if output.len() > MAX_INPUT_BYTES {
            return Err(TimedTextError::OutputTooLarge);
        }
        return Ok(output);
    }
    let mut output = String::new();
    if format == TimedTextFormat::WebVtt {
        output.push_str("WEBVTT\n\n");
    }
    for (index, cue) in transcript.cues.iter().enumerate() {
        match format {
            TimedTextFormat::Srt | TimedTextFormat::WebVtt => {
                let separator = if format == TimedTextFormat::Srt { ',' } else { '.' };
                output.push_str(&if format == TimedTextFormat::Srt { (index + 1).to_string() } else { cue.id.clone() });
                output.push('\n');
                output.push_str(&write_timestamp(cue.start_ms, separator));
                output.push_str(" --> ");
                output.push_str(&write_timestamp(cue.end_ms, separator));
                output.push('\n');
                output.push_str(&cue.text);
                output.push_str("\n\n");
            }
            TimedTextFormat::Text => {
                if index > 0 {
                    output.push('\n');
                }
                output.push_str(&cue.text);
                output.push('\n');
            }
            TimedTextFormat::Json => unreachable!("JSON was handled before rendering cue text"),
        }
        if output.len() > MAX_INPUT_BYTES {
            return Err(TimedTextError::OutputTooLarge);
        }
    }
    Ok(output.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timed_text::{export_losses, OverlapPolicy, TimedTextLoss};

    const SRT: &[u8] = b"1\n00:00:01,250 --> 00:00:03,500\nHello\nworld\n\n2\n00:00:03,000 --> 00:00:04,000\nOverlapping cue\n";

    #[test]
    fn supported_formats_keep_cues_and_json_keeps_source() {
        let transcript = parse(SRT, TimedTextFormat::Srt).unwrap();
        assert_eq!(transcript.cues[0].start_ms, 1250);
        assert_eq!(transcript.cues[0].text, "Hello\nworld");
        let json = render(&transcript, TimedTextFormat::Json).unwrap();
        assert_eq!(parse(&json, TimedTextFormat::Json).unwrap(), transcript);
        let vtt = render(&transcript, TimedTextFormat::WebVtt).unwrap();
        assert_eq!(parse(&vtt, TimedTextFormat::WebVtt).unwrap().cues, transcript.cues);
    }

    #[test]
    fn bom_crlf_and_short_webvtt_timestamps_are_supported() {
        let source = "\u{feff}WEBVTT\r\n\r\n00:01.000 --> 00:02.010\r\nUnicode: привет\r\n";
        let transcript = parse(source.as_bytes(), TimedTextFormat::WebVtt).unwrap();
        assert_eq!(transcript.cues[0].id, "cue-000001");
        assert_eq!(transcript.cues[0].end_ms, 2010);
        assert_eq!(transcript.source.sha256, format!("{:x}", Sha256::digest(source.as_bytes())));
    }

    #[test]
    fn explicit_overlap_policy_controls_overlapping_cues() {
        let mut transcript = parse(SRT, TimedTextFormat::Srt).unwrap();
        assert_eq!(transcript.overlap_policy, OverlapPolicy::Preserve);
        transcript.overlap_policy = OverlapPolicy::Reject;
        assert!(validate(&transcript).is_err());
    }

    #[test]
    fn backwards_zero_duration_and_out_of_range_timing_are_refused() {
        for input in [
            "1\n00:00:02,000 --> 00:00:02,000\nA\n",
            "1\n00:00:03,000 --> 00:00:02,000\nA\n",
            "1\n00:60:00,000 --> 01:01:00,000\nA\n",
            "1\n00:00:03,000 --> 00:00:04,000\nA\n\n2\n00:00:02,000 --> 00:00:03,000\nB\n",
        ] {
            assert!(parse(input.as_bytes(), TimedTextFormat::Srt).is_err());
        }
    }

    #[test]
    fn malformed_and_styled_cues_are_refused() {
        for input in [
            "1\n00:00:01,000 --> 00:00:02,000\n<b>Hello</b>\n",
            "1\n00:00:01,000 --> 00:00:02,000\n&amp;\n",
            "1\n00:00:01,000 --> 00:00:02,000\n{\\an8}Hello\n",
            "1\n00:00:01,000 --> 00:00:02,000\n\n",
            "1\n00:00:01.000 --> 00:00:02.000\nHello\n",
            "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n1\n00:00:03,000 --> 00:00:04,000\nDuplicate\n",
        ] {
            assert!(parse(input.as_bytes(), TimedTextFormat::Srt).is_err());
        }
    }

    #[test]
    fn webvtt_metadata_settings_and_voice_tags_are_refused() {
        for input in [
            "WEBVTT\nX-TIMESTAMP-MAP=LOCAL:00:00:00.000,MPEGTS:0\n\n",
            "WEBVTT\n\nSTYLE\n::cue { color: red }\n",
            "WEBVTT\n\nNOTE hidden text\n",
            "WEBVTT\n\nREGION\nid:one\n",
            "WEBVTT\n\n00:01.000 --> 00:02.000 align:start\nHello\n",
            "WEBVTT\n\n00:01.000 --> 00:02.000\n<v Speaker>Hello\n",
        ] {
            assert!(parse(input.as_bytes(), TimedTextFormat::WebVtt).is_err());
        }
    }

    #[test]
    fn json_rejects_unknown_duplicate_and_positional_struct_fields() {
        let transcript = parse(SRT, TimedTextFormat::Srt).unwrap();
        let json = String::from_utf8(render(&transcript, TimedTextFormat::Json).unwrap()).unwrap();
        let unknown = json.replacen('{', "{\"unknown\":true,", 1);
        assert!(parse(unknown.as_bytes(), TimedTextFormat::Json).is_err());
        let duplicate = json.replacen('{', "{\"schema_version\":\"renderflow.timed-transcript/v1\",", 1);
        assert!(parse(duplicate.as_bytes(), TimedTextFormat::Json).is_err());
        // Derive tracks presence with Option<field_type>, so optional fields
        // have an outer Option<Option<T>>: even a null first value is present.
        let null_duplicate = json.replacen('{', "{\"language\":null,\"language\":\"en\",", 1);
        assert!(parse(null_duplicate.as_bytes(), TimedTextFormat::Json).is_err());
        let nested_null_duplicate = json.replacen("\"source\": {", "\"source\": {\"stream\":null,\"stream\":null,", 1);
        assert!(parse(nested_null_duplicate.as_bytes(), TimedTextFormat::Json).is_err());
        let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value["source"] = serde_json::json!(["a".repeat(64), "srt", null, null]);
        assert!(parse(&serde_json::to_vec(&value).unwrap(), TimedTextFormat::Json).is_err());
        assert!(parse(b"[]", TimedTextFormat::Json).is_err());
    }

    #[test]
    fn empty_non_utf8_and_plain_text_inputs_are_refused() {
        assert!(parse(b"", TimedTextFormat::Srt).is_err());
        assert!(parse(&[0xff], TimedTextFormat::Srt).is_err());
        assert!(matches!(parse(b"Hello", TimedTextFormat::Text), Err(TimedTextError::UnsupportedInput)));
        assert!(matches!(parse(&vec![b' '; MAX_INPUT_BYTES + 1], TimedTextFormat::Srt), Err(TimedTextError::InputTooLarge)));
    }

    #[test]
    fn losses_are_explicit_for_text_and_renumbered_srt() {
        let mut transcript = parse(SRT, TimedTextFormat::Srt).unwrap();
        transcript.cues[0].id = "intro".to_owned();
        transcript.cues[0].speaker = Some("Narrator".to_owned());
        transcript.language = Some("en".to_owned());
        assert!(export_losses(&transcript, TimedTextFormat::Srt).contains(&TimedTextLoss::CueIds));
        let losses = export_losses(&transcript, TimedTextFormat::Text);
        assert!(losses.contains(&TimedTextLoss::Timing));
        assert!(losses.contains(&TimedTextLoss::Speakers));
        assert!(losses.contains(&TimedTextLoss::Language));
        assert!(export_losses(&transcript, TimedTextFormat::Json).is_empty());
    }
}
