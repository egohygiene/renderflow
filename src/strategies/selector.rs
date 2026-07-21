#![allow(dead_code)]
use anyhow::Result;

use crate::audio::AudioStrategy;
use crate::config::{unsupported_type_message, OutputType};
use crate::image::ImageStrategy;
use crate::strategies::{DocxStrategy, HtmlStrategy, OutputStrategy, PdfStrategy};

/// Select an output strategy based on the given output type.
///
/// The optional `template` name and `template_dir` are forwarded to the chosen
/// strategy so that it can locate the correct template file when rendering.
/// When `template` is `None` the strategy falls back to default pandoc behaviour.
///
/// The optional `profile` is forwarded to audio strategies to control quality
/// settings.  It is ignored for non-audio output types.
pub fn select_strategy(
    output_type: &OutputType,
    template: Option<&str>,
    template_dir: &str,
    profile: Option<&str>,
) -> Result<Box<dyn OutputStrategy + Send + Sync>> {
    match output_type {
        OutputType::Html => Ok(Box::new(HtmlStrategy::new(
            template.map(str::to_owned),
            template_dir.to_owned(),
        ))),
        OutputType::Pdf => Ok(Box::new(PdfStrategy::new(
            template.map(str::to_owned),
            template_dir.to_owned(),
        ))),
        OutputType::Docx => Ok(Box::new(DocxStrategy::new(
            template.map(str::to_owned),
            template_dir.to_owned(),
        ))),
        OutputType::Audio(fmt) => Ok(Box::new(AudioStrategy::new(
            *fmt,
            profile.map(str::to_owned),
        ))),
        OutputType::Image(fmt) => Ok(Box::new(ImageStrategy::new(
            *fmt,
            profile.map(str::to_owned),
        ))),
        OutputType::Unsupported(t) => {
            anyhow::bail!("{}", unsupported_type_message(t.as_str()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioFormat;
    use crate::image::ImageFormat;
    use crate::input_format::InputFormat;
    use crate::strategies::RenderContext;
    use std::collections::HashMap;

    fn default_ctx<'a>(
        input: &'a str,
        output: &'a str,
        vars: &'a HashMap<String, String>,
    ) -> RenderContext<'a> {
        RenderContext {
            input_path: input,
            input_format: InputFormat::Markdown,
            output_path: output,
            variables: vars,
            dry_run: false,
        }
    }

    #[test]
    fn test_select_strategy_html() {
        let result = select_strategy(&OutputType::Html, None, "templates", None);
        assert!(result.is_ok(), "expected html strategy to be selected");
    }

    #[test]
    fn test_select_strategy_pdf() {
        let result = select_strategy(&OutputType::Pdf, None, "templates", None);
        assert!(result.is_ok(), "expected pdf strategy to be selected");
    }

    #[test]
    fn test_select_strategy_html_renders_error_on_missing_input() {
        let vars = HashMap::new();
        let strategy = select_strategy(&OutputType::Html, None, "templates", None).unwrap();
        let ctx = default_ctx("/nonexistent/input.md", "/tmp/output.html", &vars);
        let result = strategy.render(&ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_strategy_passes_template_to_strategy() {
        let strategy = select_strategy(&OutputType::Html, Some("default.html"), "templates", None);
        assert!(strategy.is_ok());
    }

    #[test]
    fn test_select_strategy_docx() {
        let result = select_strategy(&OutputType::Docx, None, "templates", None);
        assert!(result.is_ok(), "expected docx strategy to be selected");
    }

    #[test]
    fn test_select_strategy_unsupported_type_returns_error() {
        // A truly unknown type must return a clear error from select_strategy.
        let result = select_strategy(
            &OutputType::Unsupported("epub".to_string()),
            None,
            "templates",
            None,
        );
        assert!(result.is_err());
        let msg = format!("{}", result.err().expect("expected an error"));
        assert!(
            msg.contains("not a valid output type"),
            "unexpected error: {}",
            msg
        );
    }

    #[test]
    fn test_select_strategy_truly_invalid_type_returns_error() {
        let result = select_strategy(
            &OutputType::Unsupported("jpeg".to_string()),
            None,
            "templates",
            None,
        );
        assert!(result.is_err());
        let msg = format!("{}", result.err().expect("expected an error"));
        assert!(
            msg.contains("not a valid output type"),
            "unexpected error: {}",
            msg
        );
    }

    #[test]
    fn test_select_strategy_html_with_non_markdown_input_format() {
        let vars = HashMap::new();
        let strategy = select_strategy(&OutputType::Html, None, "templates", None).unwrap();
        let ctx = RenderContext {
            input_path: "/nonexistent/input.html",
            input_format: InputFormat::Html,
            output_path: "/tmp/output.html",
            variables: &vars,
            dry_run: false,
        };
        // Strategy can be created and render can be attempted (will fail due to missing file)
        let result = strategy.render(&ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_strategy_docx_with_html_input_format() {
        let vars = HashMap::new();
        let strategy = select_strategy(&OutputType::Docx, None, "templates", None).unwrap();
        let ctx = RenderContext {
            input_path: "/nonexistent/input.html",
            input_format: InputFormat::Html,
            output_path: "/tmp/output.docx",
            variables: &vars,
            dry_run: false,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_strategy_pdf_with_rst_input_format() {
        let vars = HashMap::new();
        let strategy = select_strategy(&OutputType::Pdf, None, "templates", None).unwrap();
        let ctx = RenderContext {
            input_path: "/nonexistent/input.rst",
            input_format: InputFormat::Rst,
            output_path: "/tmp/output.pdf",
            variables: &vars,
            dry_run: false,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_err());
    }

    #[test]
    fn test_select_strategy_audio_mp3() {
        let result = select_strategy(
            &OutputType::Audio(AudioFormat::Mp3),
            None,
            "templates",
            None,
        );
        assert!(result.is_ok(), "expected audio strategy for mp3");
    }

    #[test]
    fn test_select_strategy_audio_flac() {
        let result = select_strategy(
            &OutputType::Audio(AudioFormat::Flac),
            None,
            "templates",
            None,
        );
        assert!(result.is_ok(), "expected audio strategy for flac");
    }

    #[test]
    fn test_select_strategy_audio_dry_run_succeeds() {
        let vars = HashMap::new();
        let strategy = select_strategy(
            &OutputType::Audio(AudioFormat::Flac),
            None,
            "templates",
            Some("broadcast"),
        )
        .unwrap();
        let ctx = RenderContext {
            input_path: "/nonexistent/input.wav",
            input_format: InputFormat::Markdown, // ignored for audio
            output_path: "/tmp/output.flac",
            variables: &vars,
            dry_run: true,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_ok(), "audio dry-run must succeed: {:?}", result);
    }

    #[test]
    fn test_select_strategy_audio_unsupported_encoding_returns_error() {
        let vars = HashMap::new();
        // RealAudio does not support encoding via FFmpeg.
        let strategy =
            select_strategy(&OutputType::Audio(AudioFormat::Ra), None, "templates", None).unwrap();
        let ctx = RenderContext {
            input_path: "/input/test.wav",
            input_format: InputFormat::Markdown,
            output_path: "/tmp/output.ra",
            variables: &vars,
            dry_run: false,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("does not support encoding"), "{msg}");
    }

    #[test]
    fn test_select_strategy_image_png() {
        let result = select_strategy(
            &OutputType::Image(ImageFormat::Png),
            None,
            "templates",
            None,
        );
        assert!(result.is_ok(), "expected image strategy for png");
    }

    #[test]
    fn test_select_strategy_image_jpeg() {
        let result = select_strategy(
            &OutputType::Image(ImageFormat::Jpeg),
            None,
            "templates",
            None,
        );
        assert!(result.is_ok(), "expected image strategy for jpeg");
    }

    #[test]
    fn test_select_strategy_image_dry_run_succeeds() {
        let vars = HashMap::new();
        let strategy = select_strategy(
            &OutputType::Image(ImageFormat::Png),
            None,
            "templates",
            Some("png_max"),
        )
        .unwrap();
        let ctx = RenderContext {
            input_path: "/nonexistent/input.jpg",
            input_format: InputFormat::Markdown, // ignored for image
            output_path: "/tmp/output.png",
            variables: &vars,
            dry_run: true,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_ok(), "image dry-run must succeed: {:?}", result);
    }

    #[test]
    fn test_select_strategy_image_unsupported_encoding_returns_error() {
        let vars = HashMap::new();
        // SVG does not support encoding via FFmpeg.
        let strategy = select_strategy(
            &OutputType::Image(ImageFormat::Svg),
            None,
            "templates",
            None,
        )
        .unwrap();
        let ctx = RenderContext {
            input_path: "/input/test.png",
            input_format: InputFormat::Markdown,
            output_path: "/tmp/output.svg",
            variables: &vars,
            dry_run: false,
        };
        let result = strategy.render(&ctx);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("does not support encoding"), "{msg}");
    }
}
