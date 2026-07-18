use super::format::ImageFormat;

/// Named quality profiles for image output.
///
/// Each profile maps to a specific set of FFmpeg quality/compression options
/// suited to a recognised use case.
///
/// Profiles are optional; when no profile is specified the strategy falls back
/// to [`ImageProfile::Default`], which selects sensible defaults for the target
/// format.
///
/// # YAML configuration example
///
/// ```yaml
/// outputs:
///   - type: jpeg
///     profile: web
///   - type: png
///     profile: png_max
///   - type: webp
///     profile: webp_lossless
///   - type: avif
///     profile: avif_high
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageProfile {
    /// Format-appropriate defaults: good quality within the codec's typical range.
    Default,

    // ── JPEG quality levels ────────────────────────────────────────────────────
    /// JPEG maximum quality (~q:v 1) — archival / near-lossless.
    JpegMaximum,
    /// JPEG high quality (~q:v 2) — excellent visual quality, larger file.
    JpegHigh,
    /// JPEG web quality (~q:v 5, ~85%) — standard web delivery quality.
    JpegWeb,
    /// JPEG medium quality (~q:v 10) — smaller file, noticeable compression.
    JpegMedium,
    /// JPEG low quality (~q:v 18) — smallest file, visible artefacts.
    JpegLow,

    // ── PNG compression levels ────────────────────────────────────────────────
    /// PNG fastest encode — compression level 1, largest file.
    PngFast,
    /// PNG default compression — compression level 6 (zlib default).
    PngDefault,
    /// PNG maximum compression — compression level 9, smallest file.
    PngMax,

    // ── WebP quality levels ───────────────────────────────────────────────────
    /// WebP lossless encoding.
    WebpLossless,
    /// WebP high quality (quality 90).
    WebpHigh,
    /// WebP standard web quality (quality 75).
    WebpWeb,
    /// WebP low quality / small file (quality 50).
    WebpLow,

    // ── AVIF / AV1 still image ────────────────────────────────────────────────
    /// AVIF high quality (crf 18) — visually lossless.
    AvifHigh,
    /// AVIF medium quality (crf 30) — good balance of quality and size.
    AvifMedium,
    /// AVIF low quality (crf 50) — small file, visible loss.
    AvifLow,
    /// AVIF lossless mode.
    AvifLossless,

    // ── GIF ───────────────────────────────────────────────────────────────────
    /// GIF with palette optimisation (two-pass palettegen/paletteuse).
    GifOptimized,

    // ── General lossless ──────────────────────────────────────────────────────
    /// Lossless encoding (applicable to WebP, AVIF, PNG, etc.).
    Lossless,
}

/// A resolved set of FFmpeg video options produced by a profile.
#[derive(Debug, Clone, Default)]
pub struct ImageProfileOptions {
    /// Video codec name passed to `-vcodec` (overrides the format default when set).
    pub codec: Option<String>,
    /// JPEG/image quality value for `-q:v` (lower = better quality; 1–31 for JPEG).
    pub quality: Option<u8>,
    /// CRF value for `-crf` (AV1/AVIF; lower = better quality).
    pub crf: Option<u8>,
    /// PNG compression level for `-compression_level` (0–9).
    pub compression_level: Option<u8>,
    /// Enable lossless encoding (`-lossless 1`).
    pub lossless: bool,
    /// Additional raw FFmpeg options appended verbatim as key=value pairs.
    pub extra_args: Vec<(String, String)>,
}

impl ImageProfileOptions {
    /// Convert the resolved options into FFmpeg command-line arguments.
    ///
    /// Returns a flat list of argument strings ready for `run_command`.
    pub fn to_ffmpeg_args(&self, format: ImageFormat) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();

        // Codec — either from the profile or from the format default.
        let codec = self.codec.as_deref().or_else(|| format.ffmpeg_codec());
        if let Some(c) = codec {
            args.extend(["-vcodec".to_string(), c.to_string()]);
        }

        // JPEG / image quality (-q:v).
        if let Some(q) = self.quality {
            args.extend(["-q:v".to_string(), q.to_string()]);
        }

        // CRF (AV1 / AVIF).
        if let Some(c) = self.crf {
            args.extend(["-crf".to_string(), c.to_string()]);
        }

        // PNG compression level.
        if let Some(lvl) = self.compression_level {
            args.extend(["-compression_level".to_string(), lvl.to_string()]);
        }

        // Lossless flag.
        if self.lossless {
            args.extend(["-lossless".to_string(), "1".to_string()]);
        }

        // Extra codec-specific options.
        for (k, v) in &self.extra_args {
            args.push(format!("-{}", k));
            args.push(v.clone());
        }

        args
    }
}

impl ImageProfile {
    /// Parse a profile identifier from the string used in YAML configuration.
    ///
    /// Returns `None` when the string does not match a known profile; the caller
    /// should treat `None` as [`ImageProfile::Default`].
    pub fn from_config_str(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('-', "_").as_str() {
            // JPEG
            "jpeg_maximum" | "maximum" => Some(ImageProfile::JpegMaximum),
            "jpeg_high" | "hq" | "high" => Some(ImageProfile::JpegHigh),
            "jpeg_web" | "web" | "jpeg_85" => Some(ImageProfile::JpegWeb),
            "jpeg_medium" | "medium" => Some(ImageProfile::JpegMedium),
            "jpeg_low" | "low" => Some(ImageProfile::JpegLow),
            // PNG
            "png_fast" | "fast" => Some(ImageProfile::PngFast),
            "png_default" => Some(ImageProfile::PngDefault),
            "png_max" | "png_maximum" => Some(ImageProfile::PngMax),
            // WebP
            "webp_lossless" => Some(ImageProfile::WebpLossless),
            "webp_high" | "webp_90" => Some(ImageProfile::WebpHigh),
            "webp_web" | "webp_75" => Some(ImageProfile::WebpWeb),
            "webp_low" | "webp_50" => Some(ImageProfile::WebpLow),
            // AVIF
            "avif_high" | "avif_lossless_approx" => Some(ImageProfile::AvifHigh),
            "avif_medium" => Some(ImageProfile::AvifMedium),
            "avif_low" => Some(ImageProfile::AvifLow),
            "avif_lossless" => Some(ImageProfile::AvifLossless),
            // GIF
            "gif_optimized" | "optimized" => Some(ImageProfile::GifOptimized),
            // General lossless
            "lossless" => Some(ImageProfile::Lossless),
            _ => None,
        }
    }

    /// Resolve this profile to a set of FFmpeg options for the given format.
    ///
    /// Returns [`ImageProfileOptions::default`] when the profile has no specific
    /// options for the target format (i.e. use codec defaults).
    pub fn resolve(&self, format: ImageFormat) -> ImageProfileOptions {
        match self {
            ImageProfile::Default => Self::default_options(format),

            // ── JPEG quality ──────────────────────────────────────────────────
            ImageProfile::JpegMaximum => ImageProfileOptions {
                quality: Some(1),
                ..Default::default()
            },
            ImageProfile::JpegHigh => ImageProfileOptions {
                quality: Some(2),
                ..Default::default()
            },
            ImageProfile::JpegWeb => ImageProfileOptions {
                quality: Some(5),
                ..Default::default()
            },
            ImageProfile::JpegMedium => ImageProfileOptions {
                quality: Some(10),
                ..Default::default()
            },
            ImageProfile::JpegLow => ImageProfileOptions {
                quality: Some(18),
                ..Default::default()
            },

            // ── PNG compression ────────────────────────────────────────────────
            ImageProfile::PngFast => ImageProfileOptions {
                compression_level: Some(1),
                ..Default::default()
            },
            ImageProfile::PngDefault => ImageProfileOptions {
                compression_level: Some(6),
                ..Default::default()
            },
            ImageProfile::PngMax => ImageProfileOptions {
                compression_level: Some(9),
                ..Default::default()
            },

            // ── WebP ──────────────────────────────────────────────────────────
            ImageProfile::WebpLossless => ImageProfileOptions {
                lossless: true,
                ..Default::default()
            },
            ImageProfile::WebpHigh => ImageProfileOptions {
                quality: Some(90),
                ..Default::default()
            },
            ImageProfile::WebpWeb => ImageProfileOptions {
                quality: Some(75),
                ..Default::default()
            },
            ImageProfile::WebpLow => ImageProfileOptions {
                quality: Some(50),
                ..Default::default()
            },

            // ── AVIF ──────────────────────────────────────────────────────────
            ImageProfile::AvifHigh => ImageProfileOptions {
                crf: Some(18),
                extra_args: vec![("still-picture".to_string(), "1".to_string())],
                ..Default::default()
            },
            ImageProfile::AvifMedium => ImageProfileOptions {
                crf: Some(30),
                extra_args: vec![("still-picture".to_string(), "1".to_string())],
                ..Default::default()
            },
            ImageProfile::AvifLow => ImageProfileOptions {
                crf: Some(50),
                extra_args: vec![("still-picture".to_string(), "1".to_string())],
                ..Default::default()
            },
            ImageProfile::AvifLossless => ImageProfileOptions {
                lossless: true,
                extra_args: vec![("still-picture".to_string(), "1".to_string())],
                ..Default::default()
            },

            // ── GIF optimised ─────────────────────────────────────────────────
            // Two-pass palette optimisation is handled in FfmpegImageArgs::build
            // via extra_args when GifOptimized is selected.
            ImageProfile::GifOptimized => ImageProfileOptions {
                extra_args: vec![
                    ("vf".to_string(), "split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse".to_string()),
                ],
                ..Default::default()
            },

            // ── General lossless ──────────────────────────────────────────────
            ImageProfile::Lossless => ImageProfileOptions {
                lossless: true,
                ..Default::default()
            },
        }
    }

    /// Return the default [`ImageProfileOptions`] for a given format when no
    /// explicit profile is specified.
    fn default_options(format: ImageFormat) -> ImageProfileOptions {
        match format {
            // JPEG default: web quality (~85%, q:v 5).
            ImageFormat::Jpeg | ImageFormat::Jpg => ImageProfileOptions {
                quality: Some(5),
                ..Default::default()
            },
            // PNG default: compression level 6 (zlib default).
            ImageFormat::Png => ImageProfileOptions {
                compression_level: Some(6),
                ..Default::default()
            },
            // WebP default: quality 80 — good balance of size and quality.
            ImageFormat::Webp => ImageProfileOptions {
                quality: Some(80),
                ..Default::default()
            },
            // AVIF default: CRF 30 — medium quality.
            ImageFormat::Avif => ImageProfileOptions {
                crf: Some(30),
                extra_args: vec![("still-picture".to_string(), "1".to_string())],
                ..Default::default()
            },
            // All other formats: rely on FFmpeg codec defaults.
            _ => ImageProfileOptions::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_config_str_web() {
        assert_eq!(
            ImageProfile::from_config_str("web"),
            Some(ImageProfile::JpegWeb)
        );
    }

    #[test]
    fn test_from_config_str_lossless() {
        assert_eq!(
            ImageProfile::from_config_str("lossless"),
            Some(ImageProfile::Lossless)
        );
    }

    #[test]
    fn test_from_config_str_png_max() {
        assert_eq!(
            ImageProfile::from_config_str("png_max"),
            Some(ImageProfile::PngMax)
        );
    }

    #[test]
    fn test_from_config_str_unknown_returns_none() {
        assert!(ImageProfile::from_config_str("nonexistent_profile_xyz").is_none());
    }

    #[test]
    fn test_resolve_default_jpeg_sets_quality() {
        let opts = ImageProfile::Default.resolve(ImageFormat::Jpeg);
        assert_eq!(opts.quality, Some(5), "JPEG default should set q:v 5");
    }

    #[test]
    fn test_resolve_default_png_sets_compression() {
        let opts = ImageProfile::Default.resolve(ImageFormat::Png);
        assert_eq!(
            opts.compression_level,
            Some(6),
            "PNG default should set compression_level 6"
        );
    }

    #[test]
    fn test_resolve_jpeg_high_sets_quality_2() {
        let opts = ImageProfile::JpegHigh.resolve(ImageFormat::Jpeg);
        assert_eq!(opts.quality, Some(2));
    }

    #[test]
    fn test_resolve_png_max_sets_compression_9() {
        let opts = ImageProfile::PngMax.resolve(ImageFormat::Png);
        assert_eq!(opts.compression_level, Some(9));
    }

    #[test]
    fn test_resolve_webp_lossless_sets_lossless_flag() {
        let opts = ImageProfile::WebpLossless.resolve(ImageFormat::Webp);
        assert!(opts.lossless, "WebpLossless should set lossless = true");
    }

    #[test]
    fn test_resolve_avif_high_sets_crf() {
        let opts = ImageProfile::AvifHigh.resolve(ImageFormat::Avif);
        assert_eq!(opts.crf, Some(18));
    }

    #[test]
    fn test_to_ffmpeg_args_jpeg_quality() {
        let opts = ImageProfileOptions {
            quality: Some(5),
            ..Default::default()
        };
        let args = opts.to_ffmpeg_args(ImageFormat::Jpeg);
        let joined = args.join(" ");
        assert!(
            joined.contains("-vcodec mjpeg"),
            "must include -vcodec mjpeg: {joined}"
        );
        assert!(
            joined.contains("-q:v 5"),
            "must include -q:v 5: {joined}"
        );
    }

    #[test]
    fn test_to_ffmpeg_args_png_compression() {
        let opts = ImageProfileOptions {
            compression_level: Some(9),
            ..Default::default()
        };
        let args = opts.to_ffmpeg_args(ImageFormat::Png);
        let joined = args.join(" ");
        assert!(
            joined.contains("-compression_level 9"),
            "must include -compression_level 9: {joined}"
        );
    }

    #[test]
    fn test_to_ffmpeg_args_lossless() {
        let opts = ImageProfileOptions {
            lossless: true,
            ..Default::default()
        };
        let args = opts.to_ffmpeg_args(ImageFormat::Webp);
        let joined = args.join(" ");
        assert!(
            joined.contains("-lossless 1"),
            "must include -lossless 1: {joined}"
        );
    }
}
