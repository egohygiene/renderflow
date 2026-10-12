//! Original synthetic subtitle fixtures exercise the public SDK and CLI boundary.
//! No external provider, network access, or user media is required.
//! Authored for #427; execution remains deferred for the implementation sprint.

use std::fs;
use std::path::Path;

use clap::Parser;
use renderflow::app;
use renderflow::cli::Cli;
use renderflow::timed_text::{self, OverlapPolicy, TimedTextFormat, TimedTextLoss};
use sha2::{Digest, Sha256};

const SRT: &str = include_str!("../../../tests/fixtures/timed-text/plain-unicode.srt");
const WEBVTT: &str = include_str!("../../../tests/fixtures/timed-text/identified.vtt");

fn convert(input: &Path, source_format: &str, output: &Path, target_format: &str) -> anyhow::Result<()> {
    let cli = Cli::try_parse_from([
        "renderflow",
        "subtitles",
        "convert",
        "--input",
        input.to_str().unwrap(),
        "--input-format",
        source_format,
        "--output",
        output.to_str().unwrap(),
        "--output-format",
        target_format,
    ])?;
    app::run_cli(cli)
}

#[test]
fn cli_converts_original_unicode_srt_without_mutating_or_replacing_files() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("captions.srt");
    let output = directory.path().join("transcript.json");
    let subtitles = directory.path().join("captions.vtt");
    let original = SRT.replace('\n', "\r\n");
    fs::write(&input, original.as_bytes()).unwrap();

    convert(&input, "srt", &output, "json").unwrap();
    let written = fs::read(&output).unwrap();
    let transcript = timed_text::parse(&written, TimedTextFormat::Json).unwrap();
    assert_eq!(transcript.schema_version, "renderflow.timed-transcript/v1");
    assert_eq!(transcript.source.sha256, format!("{:x}", Sha256::digest(original.as_bytes())));
    assert_eq!(transcript.source.format, "srt");
    assert_eq!(transcript.overlap_policy, OverlapPolicy::Preserve);
    assert_eq!(transcript.cues.len(), 2);
    assert_eq!(transcript.cues[0].id, "7");
    assert_eq!(transcript.cues[0].start_ms, 1250);
    assert_eq!(transcript.cues[0].end_ms, 3750);
    assert_eq!(transcript.cues[1].id, "9");
    assert_eq!(transcript.cues[1].start_ms, 5000);
    assert_eq!(transcript.cues[1].end_ms, 6125);
    let plain_text = timed_text::render(&transcript, TimedTextFormat::Text).unwrap();
    assert_eq!(
        String::from_utf8(plain_text).unwrap(),
        "Hello, orbit.\nПривет, мир 🌙\n\nThe stars reply.\n"
    );
    convert(&output, "json", &subtitles, "vtt").unwrap();
    let webvtt = fs::read(&subtitles).unwrap();
    let converted = timed_text::parse(&webvtt, TimedTextFormat::WebVtt).unwrap();
    assert_eq!(converted.cues, transcript.cues);
    assert_eq!(converted.source.sha256, format!("{:x}", Sha256::digest(&webvtt)));
    assert_eq!(fs::read(&output).unwrap(), written);
    assert_eq!(fs::read(&input).unwrap(), original.as_bytes());

    assert!(convert(&input, "srt", &output, "json").is_err());
    assert_eq!(fs::read(&output).unwrap(), written);
    assert!(convert(&input, "srt", &input, "vtt").is_err());
    assert_eq!(fs::read(&input).unwrap(), original.as_bytes());
}

#[test]
fn unsupported_or_malformed_cli_input_does_not_publish_a_destination() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("broken.srt");
    let output = directory.path().join("captions.vtt");
    for (format, original) in [
        ("srt", b"1\n00:00:03,000 --> 00:00:01,000\nReversed timing.\n".as_slice()),
        ("srt", b"1\n00:00:01,000 --> 00:00:02,000\n\xff\n".as_slice()),
        ("srt", b"1\n00:00:01,000 --> 00:00:02,000\n<i>Styled text.</i>\n".as_slice()),
        ("vtt", b"WEBVTT\n\nSTYLE\n::cue { color: lime; }\n\n00:00:01.000 --> 00:00:02.000\nHello.\n".as_slice()),
        ("vtt", b"WEBVTT\n\n00:00:01.000 --> 00:00:02.000 align:start\nHello.\n".as_slice()),
    ] {
        fs::write(&input, original).unwrap();
        assert!(convert(&input, format, &output, "vtt").is_err());
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), original);
    }
}

#[test]
fn json_retains_editorial_metadata_and_plain_text_discloses_losses() {
    let mut transcript = timed_text::parse(WEBVTT.as_bytes(), TimedTextFormat::WebVtt).unwrap();
    transcript.language = Some("mul".to_owned());
    transcript.cues[0].speaker = Some("Synthetic narrator".to_owned());
    transcript.losses = vec![TimedTextLoss::Speakers];
    let original = transcript.clone();
    let json = timed_text::render(&transcript, TimedTextFormat::Json).unwrap();
    assert_eq!(timed_text::parse(&json, TimedTextFormat::Json).unwrap(), original);
    assert!(timed_text::export_losses(&transcript, TimedTextFormat::Json).is_empty());

    let loss = timed_text::export_losses(&transcript, TimedTextFormat::Text);
    for expected in [
        TimedTextLoss::Timing,
        TimedTextLoss::CueIds,
        TimedTextLoss::Speakers,
        TimedTextLoss::Language,
        TimedTextLoss::SourceProvenance,
        TimedTextLoss::OverlapPolicy,
        TimedTextLoss::PriorLossEvidence,
    ] {
        assert!(loss.contains(&expected), "missing export loss: {expected:?}");
    }
    assert_eq!(transcript, original, "loss disclosure must not alter the input");
    assert!(timed_text::export_losses(&transcript, TimedTextFormat::Srt)
        .contains(&TimedTextLoss::CueIds));
    assert!(!timed_text::export_losses(&transcript, TimedTextFormat::WebVtt)
        .contains(&TimedTextLoss::CueIds));
}

#[test]
fn cli_honors_the_json_overlap_policy_without_silently_retiming() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("overlapping.json");
    let accepted = directory.path().join("preserved.vtt");
    let refused = directory.path().join("refused.vtt");
    let mut transcript = timed_text::parse(WEBVTT.as_bytes(), TimedTextFormat::WebVtt).unwrap();
    transcript.cues[1].start_ms = 3000;
    fs::write(&input, timed_text::render(&transcript, TimedTextFormat::Json).unwrap()).unwrap();
    convert(&input, "json", &accepted, "vtt").unwrap();
    assert_eq!(
        timed_text::parse(&fs::read(&accepted).unwrap(), TimedTextFormat::WebVtt).unwrap().cues,
        transcript.cues
    );

    transcript.overlap_policy = OverlapPolicy::Reject;
    let rejected_bytes = serde_json::to_vec(&transcript).unwrap();
    fs::write(&input, &rejected_bytes).unwrap();
    assert!(convert(&input, "json", &refused, "vtt").is_err());
    assert!(!refused.exists());
    assert_eq!(fs::read(&input).unwrap(), rejected_bytes);
}

#[test]
fn cli_refuses_unknown_json_schema_or_fields_before_creating_output() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("transcript.json");
    let output = directory.path().join("captions.srt");
    let transcript = timed_text::parse(SRT.as_bytes(), TimedTextFormat::Srt).unwrap();
    let valid = serde_json::to_value(transcript).unwrap();
    let mut unknown_schema = valid.clone();
    unknown_schema["schema_version"] = "renderflow.timed-transcript/v999".into();
    let mut unknown_field = valid;
    unknown_field["cues"][0]["style"] = "unrecognized".into();

    for invalid in [unknown_schema, unknown_field] {
        let original = serde_json::to_vec(&invalid).unwrap();
        fs::write(&input, &original).unwrap();
        assert!(convert(&input, "json", &output, "srt").is_err());
        assert!(!output.exists());
        assert_eq!(fs::read(&input).unwrap(), original);
    }
}
