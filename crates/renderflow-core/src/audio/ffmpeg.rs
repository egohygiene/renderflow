use super::format::AudioFormat;
use super::profile::AudioProfile;
use anyhow::Result;

pub(crate) fn configured_profile(profile: Option<&str>) -> Result<AudioProfile> {
    match profile {
        None => Ok(AudioProfile::Default),
        Some(name) => AudioProfile::from_config_str(name).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown audio quality profile; select a documented profile or omit it for format defaults"
            )
        }),
    }
}

/// Build the complete FFmpeg argument list for an audio conversion.
///
/// The resulting list is ready for use with
/// [`crate::adapters::command::run_command`]:
///
/// ```ignore
/// let args = FfmpegAudioArgs::new(
///     "/input/recording.wav",
///     "/output/track.mp3",
///     AudioFormat::Mp3,
///     Some("320k"),
/// )
/// .try_build()?;
/// let refs: Vec<&str> = args.iter().map(String::as_str).collect();
/// run_command("ffmpeg", &refs)?;
/// ```
pub struct FfmpegAudioArgs<'a> {
    input_path: &'a str,
    output_path: &'a str,
    format: AudioFormat,
    profile_str: Option<&'a str>,
}

impl<'a> FfmpegAudioArgs<'a> {
    /// Create a new argument builder.
    ///
    /// * `input_path`  – path to the source audio file.
    /// * `output_path` – desired destination file path.
    /// * `format`      – target [`AudioFormat`].
    /// * `profile_str` – optional profile name from YAML config (e.g. `"320k"`).
    ///   Parsed via [`AudioProfile::from_config_str`]. [`Self::try_build`]
    ///   uses defaults only when absent and rejects an unrecognised selection.
    ///   [`Self::build`] retains the historical fallback for SDK compatibility.
    pub fn new(
        input_path: &'a str,
        output_path: &'a str,
        format: AudioFormat,
        profile_str: Option<&'a str>,
    ) -> Self {
        Self {
            input_path,
            output_path,
            format,
            profile_str,
        }
    }

    /// Assemble the complete FFmpeg argument list.
    ///
    /// The typical invocation produced is:
    /// ```text
    /// ffmpeg -y -i <input> [codec/quality options] <output>
    /// ```
    ///
    /// `-y` is always included to overwrite the output file without
    /// confirmation, which is correct for a non-interactive pipeline.
    /// This compatibility entrypoint retains the historical default fallback
    /// for unknown names. Configuration-driven callers should use
    /// [`Self::try_build`] to reject explicit mistakes.
    pub fn build(self) -> Vec<String> {
        let profile = self
            .profile_str
            .and_then(AudioProfile::from_config_str)
            .unwrap_or(AudioProfile::Default);

        self.build_profile(profile)
    }

    /// Assemble arguments after validating an explicitly selected profile.
    ///
    /// An omitted profile retains the format defaults. An explicit unknown or
    /// empty name is an error before any provider is invoked. This does not
    /// change the existing profile aliases or format-specific option mappings.
    pub fn try_build(self) -> Result<Vec<String>> {
        configured_profile(self.profile_str)?;
        Ok(self.build())
    }

    fn build_profile(self, profile: AudioProfile) -> Vec<String> {
        let options = profile.resolve(self.format);

        let mut args: Vec<String> = vec![
            "-y".to_string(),
            "-i".to_string(),
            self.input_path.to_string(),
        ];

        // Append codec / quality arguments produced by the profile.
        args.extend(options.to_ffmpeg_args(self.format));

        // Append the output path last.
        args.push(self.output_path.to_string());

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_args(format: AudioFormat, profile: Option<&str>) -> Vec<String> {
        FfmpegAudioArgs::new("/input/test.wav", "/output/test.ext", format, profile).build()
    }

    #[test]
    fn test_args_always_start_with_y_flag() {
        let args = build_args(AudioFormat::Mp3, None);
        assert_eq!(args[0], "-y", "first arg must be -y");
    }

    #[test]
    fn test_args_include_input_flag() {
        let args = build_args(AudioFormat::Mp3, None);
        assert!(args.contains(&"-i".to_string()), "args must include -i");
        let i_pos = args.iter().position(|a| a == "-i").unwrap();
        assert_eq!(args[i_pos + 1], "/input/test.wav");
    }

    #[test]
    fn test_args_last_element_is_output_path() {
        let args = build_args(AudioFormat::Mp3, None);
        assert_eq!(args.last().unwrap(), "/output/test.ext");
    }

    #[test]
    fn test_mp3_default_has_codec() {
        let args = build_args(AudioFormat::Mp3, None);
        let joined = args.join(" ");
        assert!(
            joined.contains("libmp3lame"),
            "mp3 default args must include libmp3lame codec: {joined}"
        );
    }

    #[test]
    fn test_mp3_320k_profile_sets_bitrate() {
        let args = build_args(AudioFormat::Mp3, Some("320k"));
        let joined = args.join(" ");
        assert!(
            joined.contains("-b:a 320k"),
            "320k profile must set -b:a 320k: {joined}"
        );
    }

    #[test]
    fn test_mp3_vbr_v0_sets_q_flag() {
        let args = build_args(AudioFormat::Mp3, Some("vbr_v0"));
        let joined = args.join(" ");
        assert!(
            joined.contains("-q:a 0"),
            "vbr_v0 profile must set -q:a 0: {joined}"
        );
        assert!(
            !joined.contains("-b:a"),
            "vbr_v0 profile must not set a fixed bitrate: {joined}"
        );
    }

    #[test]
    fn test_flac_broadcast_sets_sample_rate_48k() {
        let args = build_args(AudioFormat::Flac, Some("broadcast"));
        let joined = args.join(" ");
        assert!(
            joined.contains("-ar 48000"),
            "flac broadcast must set -ar 48000: {joined}"
        );
    }

    #[test]
    fn test_wav_cd_quality_uses_pcm_s16le() {
        let args = build_args(AudioFormat::Wav, Some("cd_quality"));
        let joined = args.join(" ");
        assert!(
            joined.contains("pcm_s16le"),
            "wav cd_quality must use pcm_s16le codec: {joined}"
        );
        assert!(
            joined.contains("-ar 44100"),
            "wav cd_quality must set -ar 44100: {joined}"
        );
    }

    #[test]
    fn test_legacy_builder_unknown_profile_falls_back_to_default() {
        let args_default = build_args(AudioFormat::Mp3, None);
        let args_unknown = build_args(AudioFormat::Mp3, Some("nonexistent_profile_xyz"));
        // Both should produce the same output since unknown profile falls back to Default.
        assert_eq!(args_default, args_unknown);
    }

    #[test]
    fn test_checked_builder_rejects_explicit_unknown_and_empty_selections() {
        for profile in ["hq_320kk", "", " ", "default_typo"] {
            let error = FfmpegAudioArgs::new(
                "/input/test.wav",
                "/output/test.mp3",
                AudioFormat::Mp3,
                Some(profile),
            )
            .try_build()
            .unwrap_err();
            assert!(error.to_string().contains("unknown audio quality profile"));
        }
    }

    #[test]
    fn test_checked_builder_preserves_absent_defaults_and_known_alias_arguments() {
        for (format, profile) in [
            (AudioFormat::Mp3, None),
            (AudioFormat::Mp3, Some("HQ-320K")),
            (AudioFormat::Wav, Some("broadcast")),
            (AudioFormat::Flac, Some("flac_hires")),
            (AudioFormat::Opus, Some("opus_128k")),
        ] {
            let checked =
                FfmpegAudioArgs::new("/input/test.wav", "/output/test.ext", format, profile)
                    .try_build()
                    .unwrap();
            assert_eq!(checked, build_args(format, profile));
        }
    }

    #[test]
    fn test_flac_level8_sets_compression_level() {
        let args = build_args(AudioFormat::Flac, Some("flac_8"));
        let joined = args.join(" ");
        assert!(
            joined.contains("-compression_level 8"),
            "flac_8 must set -compression_level 8: {joined}"
        );
    }

    #[test]
    fn test_opus_128k_sets_bitrate() {
        let args = build_args(AudioFormat::Opus, Some("opus_128k"));
        let joined = args.join(" ");
        assert!(
            joined.contains("-b:a 128k"),
            "opus_128k profile must set -b:a 128k: {joined}"
        );
    }
}
