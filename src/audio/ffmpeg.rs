use super::format::AudioFormat;
use super::profile::AudioProfile;

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
/// .build();
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
    ///   Parsed via [`AudioProfile::from_config_str`]; falls back to
    ///   [`AudioProfile::Default`] when `None` or unrecognised.
    pub fn new(
        input_path: &'a str,
        output_path: &'a str,
        format: AudioFormat,
        profile_str: Option<&'a str>,
    ) -> Self {
        Self { input_path, output_path, format, profile_str }
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
    pub fn build(self) -> Vec<String> {
        let profile = self
            .profile_str
            .and_then(AudioProfile::from_config_str)
            .unwrap_or(AudioProfile::Default);

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
    fn test_unknown_profile_falls_back_to_default() {
        let args_default = build_args(AudioFormat::Mp3, None);
        let args_unknown = build_args(AudioFormat::Mp3, Some("nonexistent_profile_xyz"));
        // Both should produce the same output since unknown profile falls back to Default.
        assert_eq!(args_default, args_unknown);
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
