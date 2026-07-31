/// Domain-specific errors for the renderflow rendering engine.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// The `pandoc` executable was not found in `PATH`.
    #[error("Pandoc is not installed.\n\nInstall:\n  sudo apt install pandoc\nor see: https://pandoc.org/installing.html")]
    PandocNotFound,

    /// The `tectonic` executable was not found in `PATH`.
    #[error("Tectonic not found. Please install tectonic to continue.\nSee: https://tectonic-typesetting.github.io/en-US/")]
    TectonicNotFound,

    /// The `ffmpeg` executable was not found in `PATH`.
    #[error(
        "FFmpeg is not installed.\n\n\
         Install:\n  \
           sudo apt install ffmpeg          # Debian / Ubuntu\n  \
           brew install ffmpeg              # macOS (Homebrew)\n  \
           choco install ffmpeg             # Windows (Chocolatey)\n\
         or see: https://ffmpeg.org/download.html"
    )]
    FfmpegNotFound,

    /// A required template file was not found on disk.
    #[error("Template not found: {path}")]
    TemplateNotFound { path: String },
}
