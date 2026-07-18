use super::format::ImageFormat;
use super::profile::ImageProfile;

/// Build the complete FFmpeg argument list for an image conversion.
///
/// The resulting list is ready for use with
/// [`crate::adapters::command::run_command`]:
///
/// ```ignore
/// let args = FfmpegImageArgs::new(
///     "/input/photo.jpg",
///     "/output/photo.png",
///     ImageFormat::Png,
///     None,
/// )
/// .build();
/// let refs: Vec<&str> = args.iter().map(String::as_str).collect();
/// run_command("ffmpeg", &refs)?;
/// ```
pub struct FfmpegImageArgs<'a> {
    input_path: &'a str,
    output_path: &'a str,
    format: ImageFormat,
    profile_str: Option<&'a str>,
}

impl<'a> FfmpegImageArgs<'a> {
    /// Create a new argument builder.
    ///
    /// * `input_path`  – path to the source image file.
    /// * `output_path` – desired destination file path.
    /// * `format`      – target [`ImageFormat`].
    /// * `profile_str` – optional profile name from YAML config (e.g. `"web"`, `"lossless"`).
    ///   Parsed via [`ImageProfile::from_config_str`]; falls back to
    ///   [`ImageProfile::Default`] when `None` or unrecognised.
    pub fn new(
        input_path: &'a str,
        output_path: &'a str,
        format: ImageFormat,
        profile_str: Option<&'a str>,
    ) -> Self {
        Self { input_path, output_path, format, profile_str }
    }

    /// Assemble the complete FFmpeg argument list.
    ///
    /// The typical invocation produced is:
    /// ```text
    /// ffmpeg -y -i <input> [-vframes 1] [codec/quality options] <output>
    /// ```
    ///
    /// `-y` is always included to overwrite the output file without
    /// confirmation, which is correct for a non-interactive pipeline.
    ///
    /// `-vframes 1` is added for non-animated formats so that FFmpeg outputs
    /// exactly one frame even when the input contains multiple frames.
    pub fn build(self) -> Vec<String> {
        let profile = self
            .profile_str
            .and_then(ImageProfile::from_config_str)
            .unwrap_or(ImageProfile::Default);

        let options = profile.resolve(self.format);

        let mut args: Vec<String> = vec![
            "-y".to_string(),
            "-i".to_string(),
            self.input_path.to_string(),
        ];

        // Limit to a single output frame for still-image formats to prevent
        // FFmpeg from treating image sequences as multi-frame video.
        if !self.format.is_animated() {
            args.extend(["-vframes".to_string(), "1".to_string()]);
        }

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

    fn build_args(format: ImageFormat, profile: Option<&str>) -> Vec<String> {
        FfmpegImageArgs::new("/input/test.jpg", "/output/test.ext", format, profile).build()
    }

    #[test]
    fn test_args_always_start_with_y_flag() {
        let args = build_args(ImageFormat::Png, None);
        assert_eq!(args[0], "-y", "first arg must be -y");
    }

    #[test]
    fn test_args_include_input_flag() {
        let args = build_args(ImageFormat::Png, None);
        assert!(args.contains(&"-i".to_string()), "args must include -i");
        let i_pos = args.iter().position(|a| a == "-i").unwrap();
        assert_eq!(args[i_pos + 1], "/input/test.jpg");
    }

    #[test]
    fn test_args_last_element_is_output_path() {
        let args = build_args(ImageFormat::Png, None);
        assert_eq!(args.last().unwrap(), "/output/test.ext");
    }

    #[test]
    fn test_still_image_includes_vframes_1() {
        let args = build_args(ImageFormat::Jpeg, None);
        let joined = args.join(" ");
        assert!(
            joined.contains("-vframes 1"),
            "still image args must include -vframes 1: {joined}"
        );
    }

    #[test]
    fn test_animated_gif_omits_vframes_1() {
        let args = build_args(ImageFormat::Gif, None);
        let joined = args.join(" ");
        assert!(
            !joined.contains("-vframes 1"),
            "animated GIF must not include -vframes 1: {joined}"
        );
    }

    #[test]
    fn test_jpeg_default_has_mjpeg_codec() {
        let args = build_args(ImageFormat::Jpeg, None);
        let joined = args.join(" ");
        assert!(
            joined.contains("mjpeg"),
            "jpeg args must include mjpeg codec: {joined}"
        );
    }

    #[test]
    fn test_jpeg_web_profile_sets_quality() {
        let args = build_args(ImageFormat::Jpeg, Some("web"));
        let joined = args.join(" ");
        assert!(
            joined.contains("-q:v 5"),
            "web profile must set -q:v 5: {joined}"
        );
    }

    #[test]
    fn test_png_max_profile_sets_compression() {
        let args = build_args(ImageFormat::Png, Some("png_max"));
        let joined = args.join(" ");
        assert!(
            joined.contains("-compression_level 9"),
            "png_max profile must set -compression_level 9: {joined}"
        );
    }

    #[test]
    fn test_webp_lossless_profile_sets_lossless() {
        let args = build_args(ImageFormat::Webp, Some("webp_lossless"));
        let joined = args.join(" ");
        assert!(
            joined.contains("-lossless 1"),
            "webp_lossless profile must set -lossless 1: {joined}"
        );
    }

    #[test]
    fn test_unknown_profile_falls_back_to_default() {
        let args_default = build_args(ImageFormat::Png, None);
        let args_unknown = build_args(ImageFormat::Png, Some("nonexistent_profile_xyz"));
        assert_eq!(
            args_default, args_unknown,
            "unknown profile must fall back to default"
        );
    }

    #[test]
    fn test_avif_high_profile_sets_crf() {
        let args = build_args(ImageFormat::Avif, Some("avif_high"));
        let joined = args.join(" ");
        assert!(
            joined.contains("-crf 18"),
            "avif_high profile must set -crf 18: {joined}"
        );
    }
}
