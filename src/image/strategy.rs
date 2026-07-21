#![allow(dead_code)]
use anyhow::{Context, Result};
use tracing::info;

use crate::adapters::command::run_command;
use crate::image::ffmpeg::FfmpegImageArgs;
use crate::image::format::ImageFormat;
use crate::strategies::{OutputStrategy, RenderContext};

/// Renders image output by delegating to FFmpeg.
///
/// Supports any [`ImageFormat`] that FFmpeg can encode (see
/// [`ImageFormat::supports_encoding`]).  When the target format does not
/// support encoding, `render` returns a descriptive error rather than
/// attempting the conversion.
///
/// The optional `profile` field maps to an [`ImageProfile`](crate::image::profile::ImageProfile)
/// name as defined in the renderflow YAML config.  When `None`, the strategy
/// uses format-appropriate defaults.
pub struct ImageStrategy {
    pub format: ImageFormat,
    pub profile: Option<String>,
}

impl ImageStrategy {
    /// Create a new `ImageStrategy` for the given format and optional quality profile.
    pub fn new(format: ImageFormat, profile: Option<String>) -> Self {
        Self { format, profile }
    }
}

impl OutputStrategy for ImageStrategy {
    fn render(&self, ctx: &RenderContext) -> Result<()> {
        if ctx.dry_run {
            info!(
                input  = %ctx.input_path,
                output = %ctx.output_path,
                format = %self.format,
                profile = ?self.profile,
                "[dry-run] Would convert image via ffmpeg"
            );
            return Ok(());
        }

        if !self.format.supports_encoding() {
            anyhow::bail!(
                "Image format '{}' ({}) does not support encoding. \
                 renderflow can decode this format but cannot write it. \
                 Use a supported output format such as: jpeg, png, webp, avif, gif, bmp, tiff.",
                self.format,
                self.format.description()
            );
        }

        info!(
            input  = %ctx.input_path,
            output = %ctx.output_path,
            format = %self.format,
            profile = ?self.profile,
            "Converting image via ffmpeg"
        );

        let args = FfmpegImageArgs::new(
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
                 input file is a valid image file.",
                ctx.input_path, self.format, ctx.output_path
            )
        })?;

        info!(output = %ctx.output_path, format = %self.format, "Image conversion completed");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input_format::InputFormat;
    use std::collections::HashMap;

    fn image_ctx<'a>(
        input: &'a str,
        output: &'a str,
        vars: &'a HashMap<String, String>,
        dry_run: bool,
    ) -> RenderContext<'a> {
        RenderContext {
            input_path: input,
            input_format: InputFormat::Markdown, // ignored for image
            output_path: output,
            variables: vars,
            dry_run,
        }
    }

    #[test]
    fn test_dry_run_succeeds_without_ffmpeg() {
        let vars = HashMap::new();
        let strategy = ImageStrategy::new(ImageFormat::Png, None);
        let ctx = image_ctx("/nonexistent/input.jpg", "/tmp/output.png", &vars, true);
        let result = strategy.render(&ctx);
        assert!(
            result.is_ok(),
            "dry-run must succeed even without ffmpeg installed: {:?}",
            result
        );
    }

    #[test]
    fn test_unsupported_encoding_svg_returns_error() {
        let vars = HashMap::new();
        let strategy = ImageStrategy::new(ImageFormat::Svg, None);
        let ctx = image_ctx("/input/test.png", "/output/test.svg", &vars, false);
        let result = strategy.render(&ctx);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("does not support encoding"),
            "error should mention unsupported encoding: {msg}"
        );
    }

    #[test]
    fn test_unsupported_encoding_psd_returns_error() {
        let vars = HashMap::new();
        let strategy = ImageStrategy::new(ImageFormat::Psd, None);
        let ctx = image_ctx("/input/test.jpg", "/output/test.psd", &vars, false);
        let result = strategy.render(&ctx);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("does not support encoding"), "{msg}");
    }

    #[test]
    fn test_unsupported_encoding_cr2_returns_error() {
        let vars = HashMap::new();
        let strategy = ImageStrategy::new(ImageFormat::Cr2, None);
        let ctx = image_ctx("/input/test.jpg", "/output/test.cr2", &vars, false);
        let result = strategy.render(&ctx);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("does not support encoding"), "{msg}");
    }

    #[test]
    fn test_missing_input_produces_error_message() {
        let vars = HashMap::new();
        let strategy = ImageStrategy::new(ImageFormat::Png, None);
        let ctx = image_ctx("/nonexistent/input.jpg", "/tmp/output.png", &vars, false);
        let result = strategy.render(&ctx);
        // Either ffmpeg is not installed or it fails on the missing input.
        assert!(result.is_err());
    }

    #[test]
    fn test_stores_format_and_profile() {
        let strategy = ImageStrategy::new(ImageFormat::Webp, Some("web".to_string()));
        assert_eq!(strategy.format, ImageFormat::Webp);
        assert_eq!(strategy.profile.as_deref(), Some("web"));
    }
}
