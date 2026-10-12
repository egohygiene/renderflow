//! Read-only clock assessment for a verified, retained speech result.
//!
//! The preparation profile deliberately resets timestamps to decoded sample
//! zero. A container stream's reported start time cannot restore discarded
//! discontinuities or establish which output sample belongs to which media
//! timestamp. This module never infers an offset, changes cues, or launches a
//! provider. Its public entry points accept only the recovery verifier's opaque
//! result; passing an arbitrary deserialized speech envelope is insufficient.

use serde::Serialize;
use thiserror::Error;

use super::recovery::SavedSpeechResult;
use super::SpeechTranscription;

pub const SPEECH_TIMELINE_SCHEMA_V1: &str = "renderflow.speech-timeline/v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechTimelineStatus {
    /// Cues refer to sample zero of the same original input WAV.
    OriginalInputWavSampleClock,
    /// Prepared audio has no verified mapping back to the original media clock.
    OriginalMediaMappingRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechTimelineOrigin {
    InputWavSamplesFromZero,
    SelectedStreamDecodedSamplesFromZero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechTimelineVerificationScope {
    /// The original source is identified by the saved receipt, not read again.
    RetainedBundleAndRecordedSourceIdentity,
}

/// Recorded preparation facts, not an offset or a synchronization certificate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreparedAudioTimeline {
    pub prepared_wav_sha256: String,
    pub prepared_wav_duration_ms: u64,
    pub selected_stream_index: u32,
    /// Informational only, including when zero. Missing timing stays unknown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recorded_stream_start_time_ms: Option<i64>,
}

/// The clock that existing cues describe, bound to verified retained evidence.
///
/// Verification establishes retained-byte and normalization consistency, not
/// current original-source bytes, recognition accuracy, human review, or
/// suitability for a separate video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SpeechTimelineAssessment {
    pub schema_version: String,
    pub status: SpeechTimelineStatus,
    pub timestamp_origin: SpeechTimelineOrigin,
    pub verification_scope: SpeechTimelineVerificationScope,
    pub original_source_sha256: String,
    pub transcript_source_sha256: String,
    pub provider_output_sha256: String,
    pub speech_reuse_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepared_audio: Option<PreparedAudioTimeline>,
    pub limitations: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SpeechTimelineError {
    #[error("original-media timing requires a verified sample-to-media mapping; retained preparation evidence only establishes decoded samples from zero, and stream start time is not a mapping")]
    OriginalMediaMappingRequired,
}

impl SpeechTimelineError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::OriginalMediaMappingRequired => "speech.timeline.original_media_mapping_required",
        }
    }
}

/// Assess an already verified saved bundle without executing native tools or
/// reopening the original source. Its digest is the verified receipt's record.
pub fn assess(saved: &SavedSpeechResult) -> SpeechTimelineAssessment {
    assess_transcription(saved.transcription())
}

/// Refuse original-media use when the retained evidence cannot establish it.
///
/// Success applies only to sample-relative cues for the original input WAV.
/// It does not align a WAV to another file or establish a container/video clock.
/// Every prepared-media result is refused, even when its reported stream start
/// time is zero or its duration matches the prepared WAV.
pub fn require_original_media_timeline(
    saved: &SavedSpeechResult,
) -> Result<SpeechTimelineAssessment, SpeechTimelineError> {
    require_assessment(assess(saved))
}

fn require_assessment(
    assessment: SpeechTimelineAssessment,
) -> Result<SpeechTimelineAssessment, SpeechTimelineError> {
    match assessment.status {
        SpeechTimelineStatus::OriginalInputWavSampleClock => Ok(assessment),
        SpeechTimelineStatus::OriginalMediaMappingRequired => Err(SpeechTimelineError::OriginalMediaMappingRequired),
    }
}

// Only the public verifier-backed entry point calls this outside authored unit
// fixtures. Do not expose an unchecked envelope-only assessment API.
fn assess_transcription(transcription: &SpeechTranscription) -> SpeechTimelineAssessment {
    let evidence = &transcription.evidence;
    let (status, timestamp_origin, original_source_sha256, prepared_audio, clock_limit) =
        match &evidence.audio_preparation {
            None => (
                SpeechTimelineStatus::OriginalInputWavSampleClock,
                SpeechTimelineOrigin::InputWavSamplesFromZero,
                evidence.identity.source_sha256.clone(),
                None,
                "Clock identity covers sample zero of this original input WAV only; it does not align any separate video or media file.",
            ),
            Some(preparation) => (
                SpeechTimelineStatus::OriginalMediaMappingRequired,
                SpeechTimelineOrigin::SelectedStreamDecodedSamplesFromZero,
                preparation.source_sha256.clone(),
                Some(PreparedAudioTimeline {
                    prepared_wav_sha256: preparation.prepared_wav_sha256.clone(),
                    prepared_wav_duration_ms: preparation.prepared_wav_duration_ms,
                    selected_stream_index: preparation.selected_stream.index,
                    recorded_stream_start_time_ms: preparation.selected_stream.start_time_ms,
                }),
                "Preparation resets timestamps to decoded sample zero. Reported stream start time and matching durations do not establish a sample-to-original-media mapping; offsets and gaps are not restored.",
            ),
        };
    SpeechTimelineAssessment {
        schema_version: SPEECH_TIMELINE_SCHEMA_V1.to_owned(),
        status,
        timestamp_origin,
        verification_scope: SpeechTimelineVerificationScope::RetainedBundleAndRecordedSourceIdentity,
        original_source_sha256,
        transcript_source_sha256: transcription.transcript.source.sha256.clone(),
        provider_output_sha256: evidence.provider_output_sha256.clone(),
        speech_reuse_key: evidence.reuse_key.clone(),
        prepared_audio,
        limitations: vec![
            clock_limit.to_owned(),
            "The retained bundle and recorded source identity were verified; the original media file was not read again for this assessment.".to_owned(),
            "Recognition words and segment timing remain an unreviewed draft; this assessment does not alter cues or establish synchronization accuracy.".to_owned(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::{
        attach_audio_preparation, audio_preparation_configuration_digest,
        normalize_whisper_json, AudioPreparationEvidence, AudioPreparationLimits,
        AudioStream, AudioToolIdentity, SpeechIdentity, SpeechOptions,
        AUDIO_PREPARATION_PROFILE, AUDIO_TIMESTAMP_ORIGIN, WHISPER_PROVIDER_ID,
    };

    const PROVIDER_JSON: &[u8] = include_bytes!("../../../../tests/fixtures/speech/whisper-segments.json");

    // Decision fixtures only: no source bytes or native results are qualified by
    // these tests. Recovery tests cover construction of the opaque public input.
    fn transcription() -> SpeechTranscription {
        normalize_whisper_json(PROVIDER_JSON, &SpeechIdentity {
            provider_id: WHISPER_PROVIDER_ID.to_owned(),
            provider_version: "synthetic timeline fixture".to_owned(),
            provider_sha256: "1".repeat(64),
            model_id: "synthetic".to_owned(),
            model_sha256: "2".repeat(64),
            source_sha256: "3".repeat(64),
        }, &SpeechOptions::default()).unwrap()
    }

    fn prepared(start_time_ms: Option<i64>) -> SpeechTranscription {
        let mut result = transcription();
        let mut preparation = AudioPreparationEvidence {
            source_sha256: "4".repeat(64),
            source_size_bytes: 100_000,
            container_format: "matroska,webm".to_owned(),
            demuxer: "matroska".to_owned(),
            selected_stream: AudioStream {
                index: 2,
                codec_name: "opus".to_owned(),
                channels: 2,
                sample_rate: 48_000,
                duration_ms: Some(3000),
                start_time_ms,
                language: None,
                title: None,
            },
            ffprobe: AudioToolIdentity {
                tool_id: "tool.ffprobe".to_owned(),
                version: "synthetic ffprobe".to_owned(),
                sha256: "5".repeat(64),
            },
            ffmpeg: AudioToolIdentity {
                tool_id: "tool.ffmpeg".to_owned(),
                version: "synthetic ffmpeg".to_owned(),
                sha256: "6".repeat(64),
            },
            probe_output_sha256: "7".repeat(64),
            prepared_wav_sha256: result.transcript.source.sha256.clone(),
            prepared_wav_size_bytes: 96_044,
            prepared_wav_duration_ms: 3000,
            profile: AUDIO_PREPARATION_PROFILE.to_owned(),
            configuration_sha256: String::new(),
            timestamp_origin: AUDIO_TIMESTAMP_ORIGIN.to_owned(),
            timeout_seconds: 300,
            limits: AudioPreparationLimits::default(),
            limitations: vec!["Synthetic timing fixture; no native decoding.".to_owned()],
        };
        preparation.configuration_sha256 = audio_preparation_configuration_digest(&preparation).unwrap();
        attach_audio_preparation(&mut result, &preparation).unwrap();
        result
    }

    #[test]
    fn strict_wav_identity_is_limited_to_the_original_input_sample_clock() {
        let original = transcription();
        let assessment = assess_transcription(&original);
        assert_eq!(assessment.status, SpeechTimelineStatus::OriginalInputWavSampleClock);
        assert_eq!(assessment.timestamp_origin, SpeechTimelineOrigin::InputWavSamplesFromZero);
        assert_eq!(assessment.original_source_sha256, original.transcript.source.sha256);
        assert_eq!(assessment.original_source_sha256, assessment.transcript_source_sha256);
        assert!(assessment.prepared_audio.is_none());
        assert!(require_assessment(assessment).is_ok());
    }

    #[test]
    fn no_recorded_start_time_or_matching_duration_enables_prepared_mapping() {
        for start_time in [None, Some(0), Some(-21), Some(9000)] {
            let original = prepared(start_time);
            let assessment = assess_transcription(&original);
            assert_eq!(assessment.status, SpeechTimelineStatus::OriginalMediaMappingRequired);
            assert_eq!(assessment.timestamp_origin, SpeechTimelineOrigin::SelectedStreamDecodedSamplesFromZero);
            assert_eq!(assessment.original_source_sha256, "4".repeat(64));
            assert_eq!(assessment.transcript_source_sha256, "3".repeat(64));
            let preparation = assessment.prepared_audio.as_ref().unwrap();
            assert_eq!(preparation.prepared_wav_sha256, assessment.transcript_source_sha256);
            assert_eq!(preparation.prepared_wav_duration_ms, 3000);
            assert_eq!(preparation.selected_stream_index, 2);
            assert_eq!(preparation.recorded_stream_start_time_ms, start_time);
            let error = require_assessment(assessment).unwrap_err();
            assert_eq!(error.code(), "speech.timeline.original_media_mapping_required");
        }
    }

    #[test]
    fn assessment_preserves_cues_and_binds_the_retained_recognition_result() {
        let original = prepared(Some(9000));
        let before = original.clone();
        let assessment = assess_transcription(&original);
        assert_eq!(original, before);
        assert_eq!(assessment.provider_output_sha256, original.evidence.provider_output_sha256);
        assert_eq!(assessment.speech_reuse_key, original.evidence.reuse_key);
        assert_eq!(assessment, assess_transcription(&original));
    }

    #[test]
    fn serialized_assessment_keeps_missing_timing_unknown_and_has_no_offset() {
        let strict = serde_json::to_value(assess_transcription(&transcription())).unwrap();
        assert_eq!(strict["schema_version"], SPEECH_TIMELINE_SCHEMA_V1);
        assert_eq!(strict["status"], "original_input_wav_sample_clock");
        assert_eq!(strict["verification_scope"], "retained_bundle_and_recorded_source_identity");
        assert!(strict.get("prepared_audio").is_none());
        let prepared = serde_json::to_value(assess_transcription(&prepared(None))).unwrap();
        assert_eq!(prepared["status"], "original_media_mapping_required");
        assert!(prepared["prepared_audio"].get("recorded_stream_start_time_ms").is_none());
        assert!(prepared.get("offset_ms").is_none());
        assert!(prepared.get("confidence").is_none());
    }
}
