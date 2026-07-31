use anyhow::{Context, Result};
use tracing::info;

use crate::adapters::command::run_command;
use crate::audio::ffmpeg::FfmpegAudioArgs;
use crate::audio::format::AudioFormat;
use crate::strategies::{OutputStrategy, RenderContext};

/// Renders audio output by delegating to FFmpeg.
///
/// Supports any [`AudioFormat`] that FFmpeg can encode (see
/// [`AudioFormat::supports_encoding`]).  When the target format does not
/// support encoding, `render` returns a descriptive error rather than
/// attempting the conversion.
///
/// The optional `profile` field maps to an [`AudioProfile`](crate::audio::profile::AudioProfile)
/// name as defined in the renderflow YAML config.  When `None`, the strategy
/// uses format-appropriate defaults.
pub struct AudioStrategy {
    pub format: AudioFormat,
    pub profile: Option<String>,
}

impl AudioStrategy {
    /// Create a new `AudioStrategy` for the given format and optional quality profile.
    pub fn new(format: AudioFormat, profile: Option<String>) -> Self {
        Self { format, profile }
    }
}

impl OutputStrategy for AudioStrategy {
    fn render(&self, ctx: &RenderContext) -> Result<()> {
        if ctx.dry_run {
            info!(
                input  = %ctx.input_path,
                output = %ctx.output_path,
                format = %self.format,
                profile = ?self.profile,
                "[dry-run] Would convert audio via ffmpeg"
            );
            return Ok(());
        }

        if !self.format.supports_encoding() {
            anyhow::bail!(
                "Audio format '{}' ({}) does not support encoding. \
                 renderflow can decode this format but cannot write it. \
                 Use a supported output format such as: wav, flac, mp3, aac, ogg, opus.",
                self.format,
                self.format.description()
            );
        }

        info!(
            input  = %ctx.input_path,
            output = %ctx.output_path,
            format = %self.format,
            profile = ?self.profile,
            "Converting audio via ffmpeg"
        );

        let args = FfmpegAudioArgs::new(
            ctx.input_path,
            ctx.output_path,
            self.format,
            self.profile.as_deref(),
        )
        .build();

        let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();

        run_command("ffmpeg", &args_refs).with_context(|| {
            format!(
                "Failed to convert '{}' to {} format at '{}'. \
                 Ensure ffmpeg is installed (`ffmpeg -version`) and that the \
                 input file is a valid audio file.",
                ctx.input_path, self.format, ctx.output_path
            )
        })?;

        info!(output = %ctx.output_path, format = %self.format, "Audio conversion completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_format::InputFormat;
    use std::collections::HashMap;

    fn audio_ctx<'a>(
        input: &'a str,
        output: &'a str,
        vars: &'a HashMap<String, String>,
        dry_run: bool,
    ) -> RenderContext<'a> {
        RenderContext {
            input_path: input,
            input_format: InputFormat::Markdown, // ignored for audio
            output_path: output,
            variables: vars,
            dry_run,
        }
    }

    #[test]
    fn test_dry_run_succeeds_without_ffmpeg() {
        let vars = HashMap::new();
        let strategy = AudioStrategy::new(AudioFormat::Mp3, None);
        let ctx = audio_ctx("/nonexistent/input.wav", "/tmp/output.mp3", &vars, true);
        let result = strategy.render(&ctx);
        assert!(
            result.is_ok(),
            "dry-run must succeed even without ffmpeg installed: {:?}",
            result
        );
    }

    #[test]
    fn test_unsupported_encoding_format_returns_error() {
        let vars = HashMap::new();
        let strategy = AudioStrategy::new(AudioFormat::Ra, None); // RealAudio – encode not supported
        let ctx = audio_ctx("/input/test.wav", "/output/test.ra", &vars, false);
        let result = strategy.render(&ctx);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("does not support encoding"),
            "error should mention unsupported encoding: {msg}"
        );
    }

    #[test]
    fn test_unsupported_encoding_dsf_returns_error() {
        let vars = HashMap::new();
        let strategy = AudioStrategy::new(AudioFormat::Dsf, None);
        let ctx = audio_ctx("/input/test.wav", "/output/test.dsf", &vars, false);
        let result = strategy.render(&ctx);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("does not support encoding"), "{msg}");
    }

    #[test]
    fn test_missing_input_produces_error_message() {
        let vars = HashMap::new();
        let strategy = AudioStrategy::new(AudioFormat::Mp3, None);
        let ctx = audio_ctx("/nonexistent/input.wav", "/tmp/output.mp3", &vars, false);
        let result = strategy.render(&ctx);
        // Either ffmpeg is not installed (which gives a "not found" error) or
        // it fails on the missing input file.  Either way the result is Err.
        assert!(result.is_err());
    }

    #[test]
    fn test_stores_format_and_profile() {
        let strategy = AudioStrategy::new(AudioFormat::Flac, Some("broadcast".to_string()));
        assert_eq!(strategy.format, AudioFormat::Flac);
        assert_eq!(strategy.profile.as_deref(), Some("broadcast"));
    }
}
