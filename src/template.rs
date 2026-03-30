use anyhow::{Context, Result};
use std::path::Path;
use tera::Tera;

use crate::config::OutputConfig;

/// Initialise a Tera template engine and load all `*.html` files found under
/// `template_dir`.
///
/// If the directory does not exist or contains no matching files, the function
/// still succeeds and returns an empty Tera instance so that the rest of the
/// pipeline can continue without templates.  An error is only returned when
/// Tera encounters an invalid glob pattern or a template that fails to parse.
pub fn init_tera(template_dir: &str) -> Result<Tera> {
    let glob = format!("{}/**/*.html", template_dir);
    let tera = Tera::new(&glob)
        .with_context(|| format!("Failed to initialise Tera from template directory: {}", template_dir))?;
    Ok(tera)
}

/// Validate that every configured template file exists in `template_dir`.
///
/// This covers all output types (HTML, PDF, DOCX).  All missing-template
/// errors are collected and reported together so that users see every problem
/// at once instead of discovering them one at a time during rendering.
///
/// Returns `Ok(())` when no templates are configured or all configured
/// templates are present on disk.
pub fn validate_templates(outputs: &[OutputConfig], template_dir: &str) -> Result<()> {
    let errors: Vec<String> = outputs
        .iter()
        .filter_map(|output| {
            let name = output.template.as_ref()?;
            let path = Path::new(template_dir).join(name);
            if !path.exists() {
                Some(format!(
                    "  - {} template '{}' not found at '{}'",
                    output.output_type,
                    name,
                    path.display()
                ))
            } else {
                None
            }
        })
        .collect();

    if !errors.is_empty() {
        anyhow::bail!(
            "Template validation failed — the following templates were not found:\n{}",
            errors.join("\n")
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OutputConfig, OutputType};
    use std::fs;
    use tempfile::TempDir;

    fn write_template(dir: &TempDir, name: &str, content: &str) {
        let path = dir.path().join(name);
        fs::write(path, content).expect("failed to write template");
    }

    #[test]
    fn test_init_tera_with_valid_template_dir() {
        let dir = TempDir::new().unwrap();
        write_template(&dir, "default.html", "<html>{{ body }}</html>");
        let tera = init_tera(dir.path().to_str().unwrap());
        assert!(tera.is_ok(), "expected Tera to initialise successfully");
        let tera = tera.unwrap();
        assert!(
            tera.get_template_names().any(|n| n.contains("default.html")),
            "expected 'default.html' to be loaded"
        );
    }

    #[test]
    fn test_init_tera_with_empty_template_dir() {
        let dir = TempDir::new().unwrap();
        let tera = init_tera(dir.path().to_str().unwrap());
        assert!(tera.is_ok(), "expected Tera to initialise with no templates");
    }

    #[test]
    fn test_init_tera_with_nonexistent_dir() {
        let tera = init_tera("/nonexistent/template/dir");
        assert!(tera.is_ok(), "expected Tera to handle missing directory gracefully");
    }

    // ── validate_templates ────────────────────────────────────────────────────

    #[test]
    fn test_validate_templates_passes_when_no_templates_configured() {
        // Outputs without a template field must always pass, even when the
        // template directory does not exist.
        let outputs = vec![
            OutputConfig { output_type: OutputType::Html, template: None },
            OutputConfig { output_type: OutputType::Pdf, template: None },
            OutputConfig { output_type: OutputType::Docx, template: None },
        ];
        assert!(
            validate_templates(&outputs, "/nonexistent/dir").is_ok(),
            "validation should pass when no templates are configured"
        );
    }

    #[test]
    fn test_validate_templates_passes_when_all_templates_exist() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("custom.html"), "").unwrap();
        fs::write(dir.path().join("template.tex"), "").unwrap();
        fs::write(dir.path().join("reference.docx"), "").unwrap();

        let outputs = vec![
            OutputConfig { output_type: OutputType::Html, template: Some("custom.html".to_string()) },
            OutputConfig { output_type: OutputType::Pdf, template: Some("template.tex".to_string()) },
            OutputConfig { output_type: OutputType::Docx, template: Some("reference.docx".to_string()) },
        ];
        assert!(
            validate_templates(&outputs, dir.path().to_str().unwrap()).is_ok(),
            "validation should pass when all templates exist"
        );
    }

    #[test]
    fn test_validate_templates_fails_on_missing_pdf_template() {
        let dir = TempDir::new().unwrap();
        let outputs = vec![
            OutputConfig { output_type: OutputType::Pdf, template: Some("missing.tex".to_string()) },
        ];
        let result = validate_templates(&outputs, dir.path().to_str().unwrap());
        assert!(result.is_err(), "expected error for missing PDF template");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("missing.tex"),
            "error should mention the missing template file: {}",
            msg
        );
    }

    #[test]
    fn test_validate_templates_fails_on_missing_docx_template() {
        let dir = TempDir::new().unwrap();
        let outputs = vec![
            OutputConfig { output_type: OutputType::Docx, template: Some("missing.docx".to_string()) },
        ];
        let result = validate_templates(&outputs, dir.path().to_str().unwrap());
        assert!(result.is_err(), "expected error for missing DOCX template");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("missing.docx"),
            "error should mention the missing template file: {}",
            msg
        );
    }

    #[test]
    fn test_validate_templates_fails_on_missing_html_template() {
        let dir = TempDir::new().unwrap();
        let outputs = vec![
            OutputConfig { output_type: OutputType::Html, template: Some("missing.html".to_string()) },
        ];
        let result = validate_templates(&outputs, dir.path().to_str().unwrap());
        assert!(result.is_err(), "expected error for missing HTML template");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("missing.html"),
            "error should mention the missing template file: {}",
            msg
        );
    }

    #[test]
    fn test_validate_templates_reports_all_missing_templates() {
        // All missing templates should be listed in a single error so users
        // can fix all problems at once without repeated build-fail cycles.
        let dir = TempDir::new().unwrap();
        let outputs = vec![
            OutputConfig { output_type: OutputType::Pdf, template: Some("missing.tex".to_string()) },
            OutputConfig { output_type: OutputType::Docx, template: Some("missing.docx".to_string()) },
        ];
        let result = validate_templates(&outputs, dir.path().to_str().unwrap());
        assert!(result.is_err(), "expected error when multiple templates are missing");
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("missing.tex"), "error should mention missing.tex: {}", msg);
        assert!(msg.contains("missing.docx"), "error should mention missing.docx: {}", msg);
    }

    #[test]
    fn test_validate_templates_error_message_includes_output_type() {
        let dir = TempDir::new().unwrap();
        let outputs = vec![
            OutputConfig { output_type: OutputType::Pdf, template: Some("my.tex".to_string()) },
        ];
        let result = validate_templates(&outputs, dir.path().to_str().unwrap());
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("pdf"),
            "error should mention the output type so users know which output is affected: {}",
            msg
        );
    }

    // ── research template ─────────────────────────────────────────────────────

    #[test]
    fn test_research_tex_template_exists() {
        let path = std::path::Path::new("templates/research/research.tex");
        assert!(
            path.exists(),
            "research LaTeX template must exist at templates/research/research.tex"
        );
    }

    #[test]
    fn test_research_html_template_exists() {
        let path = std::path::Path::new("templates/research/research.html");
        assert!(
            path.exists(),
            "research HTML template must exist at templates/research/research.html"
        );
    }

    #[test]
    fn test_research_tex_template_contains_pandoc_variables() {
        let content = fs::read_to_string("templates/research/research.tex")
            .expect("failed to read research.tex template");
        assert!(content.contains("$title$"), "research.tex should interpolate $$title$$");
        assert!(content.contains("$author$"), "research.tex should interpolate $$author$$");
        assert!(content.contains("$date$"), "research.tex should interpolate $$date$$");
        assert!(content.contains("$abstract$"), "research.tex should interpolate $$abstract$$");
        assert!(content.contains("$body$"), "research.tex should contain $$body$$ for document content");
        assert!(content.contains("$toc$") || content.contains("$if(toc)$"), "research.tex should support optional table of contents");
    }

    #[test]
    fn test_research_tex_template_is_valid_latex_document() {
        let content = fs::read_to_string("templates/research/research.tex")
            .expect("failed to read research.tex template");
        assert!(
            content.contains("\\documentclass"),
            "research.tex should begin with a \\documentclass declaration"
        );
        assert!(content.contains("\\begin{document}"), "research.tex must have \\begin{{document}}");
        assert!(content.contains("\\end{document}"), "research.tex must have \\end{{document}}");
    }

    #[test]
    fn test_validate_templates_accepts_research_tex() {
        let outputs = vec![
            OutputConfig {
                output_type: OutputType::Pdf,
                template: Some("research/research.tex".to_string()),
            },
        ];
        // The template_dir is the workspace-relative "templates" folder.
        let result = validate_templates(&outputs, "templates");
        assert!(
            result.is_ok(),
            "validate_templates should accept the research LaTeX template: {:?}",
            result.err()
        );
    }

    #[test]
    fn test_validate_templates_accepts_research_html() {
        let outputs = vec![
            OutputConfig {
                output_type: OutputType::Html,
                template: Some("research/research.html".to_string()),
            },
        ];
        let result = validate_templates(&outputs, "templates");
        assert!(
            result.is_ok(),
            "validate_templates should accept the research HTML template: {:?}",
            result.err()
        );
    }
}
