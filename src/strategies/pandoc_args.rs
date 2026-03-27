/// Builder for pandoc command-line arguments.
///
/// Centralises argument construction so that all strategies produce a
/// consistent invocation of `pandoc`.  Call [`PandocArgs::new`] with the
/// mandatory arguments, chain optional modifiers, then call [`PandocArgs::build`]
/// to obtain the final `Vec<String>` ready to pass to a command runner.
///
/// # Example
///
/// ```rust
/// use renderflow::strategies::pandoc_args::PandocArgs;
///
/// let args = PandocArgs::new("markdown", "/tmp/input.md", "/tmp/output.html")
///     .with_template("/tmp/templates/default.html")
///     .build();
/// ```
pub struct PandocArgs {
    input_format: String,
    input_path: String,
    output_path: String,
    template: Option<String>,
    pdf_engine: Option<String>,
    reference_doc: Option<String>,
}

impl PandocArgs {
    /// Create a new builder with the three mandatory pandoc arguments.
    ///
    /// * `input_format` – value passed to `--from` (e.g. `"markdown"`)
    /// * `input_path`   – path of the source document
    /// * `output_path`  – destination path for the rendered output
    pub fn new(input_format: &str, input_path: &str, output_path: &str) -> Self {
        Self {
            input_format: input_format.to_owned(),
            input_path: input_path.to_owned(),
            output_path: output_path.to_owned(),
            template: None,
            pdf_engine: None,
            reference_doc: None,
        }
    }

    /// Add a `--template <path>` argument (used by HTML and PDF strategies).
    pub fn with_template(mut self, path: impl Into<String>) -> Self {
        self.template = Some(path.into());
        self
    }

    /// Add a `--pdf-engine=<engine>` argument (used by the PDF strategy).
    pub fn with_pdf_engine(mut self, engine: impl Into<String>) -> Self {
        self.pdf_engine = Some(engine.into());
        self
    }

    /// Add a `--reference-doc <path>` argument (used by the DOCX strategy).
    pub fn with_reference_doc(mut self, path: impl Into<String>) -> Self {
        self.reference_doc = Some(path.into());
        self
    }

    /// Consume the builder and return the assembled argument list.
    ///
    /// The returned `Vec<String>` can be converted to `Vec<&str>` for use with
    /// [`crate::adapters::command::run_command`]:
    ///
    /// ```rust,ignore
    /// let args = builder.build();
    /// let args_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    /// run_command("pandoc", &args_refs)?;
    /// ```
    pub fn build(self) -> Vec<String> {
        let mut args = vec![
            "--from".to_owned(),
            self.input_format,
            self.input_path,
            "-o".to_owned(),
            self.output_path,
        ];

        if let Some(engine) = self.pdf_engine {
            args.push(format!("--pdf-engine={engine}"));
        }

        if let Some(template) = self.template {
            args.push("--template".to_owned());
            args.push(template);
        }

        if let Some(reference_doc) = self.reference_doc {
            args.push("--reference-doc".to_owned());
            args.push(reference_doc);
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_minimal_args() {
        let args = PandocArgs::new("markdown", "input.md", "output.html").build();
        assert_eq!(args, vec!["--from", "markdown", "input.md", "-o", "output.html"]);
    }

    #[test]
    fn test_build_with_template() {
        let args = PandocArgs::new("markdown", "input.md", "output.html")
            .with_template("/templates/default.html")
            .build();
        assert_eq!(
            args,
            vec!["--from", "markdown", "input.md", "-o", "output.html", "--template", "/templates/default.html"]
        );
    }

    #[test]
    fn test_build_with_pdf_engine() {
        let args = PandocArgs::new("markdown", "input.md", "output.pdf")
            .with_pdf_engine("tectonic")
            .build();
        assert_eq!(
            args,
            vec!["--from", "markdown", "input.md", "-o", "output.pdf", "--pdf-engine=tectonic"]
        );
    }

    #[test]
    fn test_build_with_pdf_engine_and_template() {
        let args = PandocArgs::new("markdown", "input.md", "output.pdf")
            .with_pdf_engine("tectonic")
            .with_template("/templates/default.tex")
            .build();
        assert_eq!(
            args,
            vec![
                "--from", "markdown", "input.md", "-o", "output.pdf",
                "--pdf-engine=tectonic",
                "--template", "/templates/default.tex",
            ]
        );
    }

    #[test]
    fn test_build_with_reference_doc() {
        let args = PandocArgs::new("markdown", "input.md", "output.docx")
            .with_reference_doc("/templates/reference.docx")
            .build();
        assert_eq!(
            args,
            vec![
                "--from", "markdown", "input.md", "-o", "output.docx",
                "--reference-doc", "/templates/reference.docx",
            ]
        );
    }

    #[test]
    fn test_build_different_input_formats() {
        for (format, expected) in [("rst", "rst"), ("html", "html"), ("latex", "latex"), ("docx", "docx")] {
            let args = PandocArgs::new(format, "input", "output").build();
            assert_eq!(args[1], expected, "input format should be passed as-is");
        }
    }

    #[test]
    fn test_no_optional_flags_when_not_set() {
        let args = PandocArgs::new("markdown", "input.md", "output.html").build();
        assert!(!args.iter().any(|a| a.contains("--template")), "should not have --template");
        assert!(!args.iter().any(|a| a.contains("--pdf-engine")), "should not have --pdf-engine");
        assert!(!args.iter().any(|a| a.contains("--reference-doc")), "should not have --reference-doc");
    }
}
