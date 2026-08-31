//! Format capability model for Renderflow.
//!
//! This module defines the machine-readable capability model that describes
//! what operations each format supports.  The model is shared by the Rust
//! library, CLI, graph planner, documentation generator, and test infrastructure.
//!
//! # Design
//!
//! Format support is not binary.  A format may support one or more distinct
//! [`ArtifactCapability`] values.  For example, PDF support may include
//! detecting, profiling, inspecting, extracting text, and generating — but
//! those capabilities do not imply lossless round-trip reconstruction.
//!
//! Use [`FormatCapabilityRegistry::global`] to access the built-in registry of
//! all declared format descriptors, then query it by [`super::Format`] or by
//! family.
//!
//! # Example
//!
//! ```rust
//! use renderflow::graph::capability::{ArtifactCapability, FormatCapabilityRegistry};
//! use renderflow::graph::Format;
//!
//! let registry = FormatCapabilityRegistry::global();
//! if let Some(desc) = registry.get(Format::Pdf) {
//!     assert!(desc.capabilities.contains(&ArtifactCapability::Detect));
//! }
//! ```

use std::collections::HashMap;

use super::Format;

/// A discrete operation that Renderflow can perform on or with a format.
///
/// Not every format supports every capability.  Capabilities that are listed
/// in a [`FormatDescriptor`] must be implemented, tested, and accurate — they
/// must not be theoretical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactCapability {
    /// Identify the format reliably, beyond extension matching.
    Detect,
    /// Classify the artifact's structural and content characteristics.
    Profile,
    /// Read structural and technical metadata from the artifact.
    Inspect,
    /// Produce embedded or derived artifacts from the source.
    Extract,
    /// Transform the artifact into another representation.
    Convert,
    /// Produce a valid, structurally correct artifact in this format.
    Generate,
    /// Verify that a produced artifact is structurally usable.
    Validate,
    /// Preserve defined information through conversion and reconstruction.
    RoundTrip,
}

impl ArtifactCapability {
    /// Return a short identifier string for this capability.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Detect => "detect",
            Self::Profile => "profile",
            Self::Inspect => "inspect",
            Self::Extract => "extract",
            Self::Convert => "convert",
            Self::Generate => "generate",
            Self::Validate => "validate",
            Self::RoundTrip => "round_trip",
        }
    }
}

impl std::fmt::Display for ArtifactCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Broad family classification for a format.
///
/// A format may belong to more than one family.  For example, EPUB is both
/// a document format and an archive format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FormatFamily {
    /// Text documents, manuscripts, and publishing formats.
    Document,
    /// Raster and vector image formats.
    Image,
    /// Audio formats (containers and codecs).
    Audio,
    /// Video formats (containers and streams).
    Video,
    /// Container formats that hold other files.
    Archive,
    /// Structured data exchange formats.
    Data,
    /// Timed-text, subtitle, and transcript formats.
    Subtitle,
    /// Slide presentation formats.
    Presentation,
    /// Tabular data and spreadsheet formats.
    Spreadsheet,
}

impl FormatFamily {
    /// Return a short identifier string for this family.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Image => "image",
            Self::Audio => "audio",
            Self::Video => "video",
            Self::Archive => "archive",
            Self::Data => "data",
            Self::Subtitle => "subtitle",
            Self::Presentation => "presentation",
            Self::Spreadsheet => "spreadsheet",
        }
    }
}

impl std::fmt::Display for FormatFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Describes the expected information loss when converting FROM this format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LossProfile {
    /// Conversion preserves all defined information.
    Lossless,
    /// Conversion discards some information irreversibly.
    Lossy,
    /// Conversion preserves most information but may lose metadata or structure.
    PartialLoss,
    /// Loss characteristics depend on the specific conversion path chosen.
    PathDependent,
}

impl LossProfile {
    /// Return a short identifier string for this loss profile.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lossless => "lossless",
            Self::Lossy => "lossy",
            Self::PartialLoss => "partial_loss",
            Self::PathDependent => "path_dependent",
        }
    }
}

impl std::fmt::Display for LossProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// An external tool or system required for one or more capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExternalTool {
    /// Pandoc document converter.
    Pandoc,
    /// Tectonic LaTeX typesetter (required for PDF via LaTeX).
    Tectonic,
    /// FFmpeg multimedia framework (audio, video, image conversion).
    Ffmpeg,
}

impl ExternalTool {
    /// Return the executable name used to probe this tool's availability.
    pub fn executable_name(self) -> &'static str {
        match self {
            Self::Pandoc => "pandoc",
            Self::Tectonic => "tectonic",
            Self::Ffmpeg => "ffmpeg",
        }
    }

    /// Return a short identifier string for this tool.
    pub fn as_str(self) -> &'static str {
        self.executable_name()
    }

    /// Return the stable provider ID used by the runtime tool registry.
    pub fn stable_id(self) -> &'static str {
        match self {
            Self::Pandoc => "tool.pandoc",
            Self::Tectonic => "tool.tectonic",
            Self::Ffmpeg => "tool.ffmpeg",
        }
    }
}

impl std::fmt::Display for ExternalTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A magic-byte signature that can be used to identify a format.
///
/// `offset` is the byte position within the file where `bytes` is expected.
/// Most signatures have `offset = 0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MagicSignature {
    /// The expected byte sequence.
    pub bytes: &'static [u8],
    /// The byte offset in the file where the sequence must appear.
    pub offset: usize,
}

impl MagicSignature {
    /// Create a signature at offset 0.
    pub const fn at_start(bytes: &'static [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    /// Create a signature at an explicit offset.
    pub const fn at_offset(bytes: &'static [u8], offset: usize) -> Self {
        Self { bytes, offset }
    }

    /// Return `true` when `buf` contains this signature at the specified offset.
    pub fn matches(&self, buf: &[u8]) -> bool {
        let start = self.offset;
        let end = start + self.bytes.len();
        if buf.len() < end {
            return false;
        }
        &buf[start..end] == self.bytes
    }
}

/// A complete description of a format's identity, families, and capabilities.
///
/// `FormatDescriptor` is the canonical source of truth for a format within
/// Renderflow.  Only capabilities that are implemented, tested, and accurate
/// should appear in the `capabilities` slice.
///
/// Use [`FormatCapabilityRegistry`] to look up descriptors by [`Format`].
#[derive(Debug, Clone)]
pub struct FormatDescriptor {
    /// Short canonical identifier used in CLI output and configuration (e.g. `"pdf"`).
    pub id: &'static str,
    /// Human-readable name (e.g. `"Portable Document Format"`).
    pub name: &'static str,
    /// Known file extensions without the leading dot (e.g. `&["pdf"]`).
    pub extensions: &'static [&'static str],
    /// MIME types associated with this format (e.g. `&["application/pdf"]`).
    pub media_types: &'static [&'static str],
    /// Format family classifications.
    pub families: &'static [FormatFamily],
    /// Capabilities that are declared as implemented and tested.
    pub capabilities: Vec<ArtifactCapability>,
    /// Magic-byte signatures for reliable detection.
    pub magic_signatures: Vec<MagicSignature>,
    /// Expected information loss when converting from this format.
    pub loss_profile: LossProfile,
    /// External tools required for any declared capability.
    pub external_requirements: Vec<ExternalTool>,
}

impl FormatDescriptor {
    /// Return `true` when this descriptor declares the given capability.
    pub fn has_capability(&self, cap: ArtifactCapability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Return `true` when this format belongs to the given family.
    pub fn is_in_family(&self, family: FormatFamily) -> bool {
        self.families.contains(&family)
    }

    /// Attempt to match any of this format's magic signatures against `buf`.
    ///
    /// Returns `true` when at least one signature matches.  Returns `false`
    /// when no signatures are registered (extension-only detection) or when
    /// none match.
    pub fn matches_magic(&self, buf: &[u8]) -> bool {
        self.magic_signatures.iter().any(|sig| sig.matches(buf))
    }
}

/// A registry mapping every known [`Format`] variant to its [`FormatDescriptor`].
///
/// Use [`FormatCapabilityRegistry::global`] to obtain the built-in registry.
pub struct FormatCapabilityRegistry {
    descriptors: HashMap<Format, FormatDescriptor>,
}

impl FormatCapabilityRegistry {
    /// Return the globally pre-populated registry containing all built-in
    /// format descriptors.
    pub fn global() -> Self {
        let mut reg = Self {
            descriptors: HashMap::new(),
        };
        reg.register_all();
        reg
    }

    /// Look up the descriptor for `format`.
    ///
    /// Returns `None` when no descriptor has been registered for the format.
    pub fn get(&self, format: Format) -> Option<&FormatDescriptor> {
        self.descriptors.get(&format)
    }

    /// Return all descriptors belonging to the given family.
    pub fn by_family(&self, family: FormatFamily) -> Vec<&FormatDescriptor> {
        self.descriptors
            .values()
            .filter(|d| d.is_in_family(family))
            .collect()
    }

    /// Return all descriptors that declare the given capability.
    pub fn by_capability(&self, cap: ArtifactCapability) -> Vec<&FormatDescriptor> {
        self.descriptors
            .values()
            .filter(|d| d.has_capability(cap))
            .collect()
    }

    /// Return all registered descriptors.
    pub fn all(&self) -> impl Iterator<Item = &FormatDescriptor> {
        self.descriptors.values()
    }

    /// Insert or replace a descriptor for the given format.
    pub fn register(&mut self, format: Format, descriptor: FormatDescriptor) {
        self.descriptors.insert(format, descriptor);
    }

    fn register_all(&mut self) {
        use ArtifactCapability::*;
        use ExternalTool::*;
        use FormatFamily::*;
        use LossProfile::*;

        // ── Document formats ──────────────────────────────────────────────────

        self.register(
            Format::Markdown,
            FormatDescriptor {
                id: "markdown",
                name: "Markdown",
                extensions: &["md", "markdown"],
                media_types: &["text/markdown", "text/x-markdown"],
                families: &[Document],
                capabilities: vec![Detect, Convert, Generate],
                magic_signatures: vec![],
                loss_profile: PartialLoss,
                external_requirements: vec![Pandoc],
            },
        );

        self.register(
            Format::Html,
            FormatDescriptor {
                id: "html",
                name: "HyperText Markup Language",
                extensions: &["html", "htm"],
                media_types: &["text/html"],
                families: &[Document],
                capabilities: vec![Detect, Convert, Generate],
                magic_signatures: vec![],
                loss_profile: PartialLoss,
                external_requirements: vec![Pandoc],
            },
        );

        self.register(
            Format::Pdf,
            FormatDescriptor {
                id: "pdf",
                name: "Portable Document Format",
                extensions: &["pdf"],
                media_types: &["application/pdf"],
                families: &[Document],
                capabilities: vec![Detect, Generate],
                magic_signatures: vec![MagicSignature::at_start(b"%PDF")],
                loss_profile: PartialLoss,
                external_requirements: vec![Pandoc, Tectonic],
            },
        );

        self.register(
            Format::Docx,
            FormatDescriptor {
                id: "docx",
                name: "Office Open XML Document",
                extensions: &["docx"],
                media_types: &[
                    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
                ],
                families: &[Document],
                capabilities: vec![Detect, Convert, Generate],
                // DOCX is a ZIP file — matches ZIP magic bytes.
                magic_signatures: vec![MagicSignature::at_start(b"PK\x03\x04")],
                loss_profile: PartialLoss,
                external_requirements: vec![Pandoc],
            },
        );

        self.register(
            Format::Epub,
            FormatDescriptor {
                id: "epub",
                name: "Electronic Publication",
                extensions: &["epub"],
                media_types: &["application/epub+zip"],
                families: &[Document, Archive],
                capabilities: vec![Detect, Convert],
                // EPUB is a ZIP file — matches ZIP magic bytes.
                magic_signatures: vec![MagicSignature::at_start(b"PK\x03\x04")],
                loss_profile: PartialLoss,
                external_requirements: vec![Pandoc],
            },
        );

        self.register(
            Format::Rst,
            FormatDescriptor {
                id: "rst",
                name: "reStructuredText",
                extensions: &["rst"],
                media_types: &["text/x-rst"],
                families: &[Document],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![],
                loss_profile: PartialLoss,
                external_requirements: vec![Pandoc],
            },
        );

        self.register(
            Format::Latex,
            FormatDescriptor {
                id: "latex",
                name: "LaTeX",
                extensions: &["tex"],
                media_types: &["application/x-tex", "text/x-tex"],
                families: &[Document],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![],
                loss_profile: PartialLoss,
                external_requirements: vec![Pandoc],
            },
        );

        self.register(
            Format::Fountain,
            FormatDescriptor {
                id: "fountain",
                name: "Fountain Screenplay",
                extensions: &["fountain"],
                media_types: &["text/x-fountain"],
                families: &[Document],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![],
                loss_profile: PartialLoss,
                external_requirements: vec![Pandoc],
            },
        );

        // ── Image formats ─────────────────────────────────────────────────────

        self.register(
            Format::Jpeg,
            FormatDescriptor {
                id: "jpeg",
                name: "JPEG Image",
                extensions: &["jpeg", "jpg"],
                media_types: &["image/jpeg"],
                families: &[Image],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"\xFF\xD8\xFF")],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Png,
            FormatDescriptor {
                id: "png",
                name: "Portable Network Graphics",
                extensions: &["png"],
                media_types: &["image/png"],
                families: &[Image],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"\x89PNG\r\n\x1A\n")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Tiff,
            FormatDescriptor {
                id: "tiff",
                name: "Tagged Image File Format",
                extensions: &["tiff", "tif"],
                media_types: &["image/tiff"],
                families: &[Image],
                capabilities: vec![Detect, Convert],
                // TIFF can be little-endian (II) or big-endian (MM).
                magic_signatures: vec![
                    MagicSignature::at_start(b"II*\x00"),
                    MagicSignature::at_start(b"MM\x00*"),
                ],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Cbz,
            FormatDescriptor {
                id: "cbz",
                name: "Comic Book ZIP Archive",
                extensions: &["cbz"],
                media_types: &["application/vnd.comicbook+zip"],
                families: &[Image, Archive],
                capabilities: vec![Detect],
                magic_signatures: vec![MagicSignature::at_start(b"PK\x03\x04")],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        // ── New image formats ─────────────────────────────────────────────────

        self.register(
            Format::Webp,
            FormatDescriptor {
                id: "webp",
                name: "WebP Image",
                extensions: &["webp"],
                media_types: &["image/webp"],
                families: &[Image],
                capabilities: vec![Detect, Convert],
                // WebP: RIFF header at 0 + "WEBP" at offset 8.
                magic_signatures: vec![MagicSignature::at_start(b"RIFF")],
                loss_profile: PathDependent,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Gif,
            FormatDescriptor {
                id: "gif",
                name: "Graphics Interchange Format",
                extensions: &["gif"],
                media_types: &["image/gif"],
                families: &[Image],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![
                    MagicSignature::at_start(b"GIF87a"),
                    MagicSignature::at_start(b"GIF89a"),
                ],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Bmp,
            FormatDescriptor {
                id: "bmp",
                name: "Windows Bitmap",
                extensions: &["bmp"],
                media_types: &["image/bmp"],
                families: &[Image],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"BM")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Avif,
            FormatDescriptor {
                id: "avif",
                name: "AV1 Image File Format",
                extensions: &["avif"],
                media_types: &["image/avif"],
                families: &[Image],
                capabilities: vec![Detect, Convert],
                // AVIF: 'ftyp' box at byte 4.
                magic_signatures: vec![MagicSignature::at_offset(b"ftyp", 4)],
                loss_profile: PathDependent,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Svg,
            FormatDescriptor {
                id: "svg",
                name: "Scalable Vector Graphics",
                extensions: &["svg"],
                media_types: &["image/svg+xml"],
                families: &[Image],
                capabilities: vec![Detect],
                magic_signatures: vec![],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        // ── Audio formats ─────────────────────────────────────────────────────

        self.register(
            Format::Wav,
            FormatDescriptor {
                id: "wav",
                name: "Waveform Audio File Format",
                extensions: &["wav"],
                media_types: &["audio/wav", "audio/x-wav"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"RIFF")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Flac,
            FormatDescriptor {
                id: "flac",
                name: "Free Lossless Audio Codec",
                extensions: &["flac"],
                media_types: &["audio/flac"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"fLaC")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Mp3,
            FormatDescriptor {
                id: "mp3",
                name: "MPEG-1 Audio Layer III",
                extensions: &["mp3"],
                media_types: &["audio/mpeg"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                // MP3: ID3 tag or sync word.
                magic_signatures: vec![
                    MagicSignature::at_start(b"ID3"),
                    MagicSignature::at_start(b"\xFF\xFB"),
                    MagicSignature::at_start(b"\xFF\xFA"),
                    MagicSignature::at_start(b"\xFF\xF3"),
                    MagicSignature::at_start(b"\xFF\xF2"),
                ],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Ogg,
            FormatDescriptor {
                id: "ogg",
                name: "Ogg Vorbis",
                extensions: &["ogg"],
                media_types: &["audio/ogg"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"OggS")],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Opus,
            FormatDescriptor {
                id: "opus",
                name: "Opus Audio",
                extensions: &["opus"],
                media_types: &["audio/opus"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"OggS")],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Aiff,
            FormatDescriptor {
                id: "aiff",
                name: "Audio Interchange File Format",
                extensions: &["aiff", "aif"],
                media_types: &["audio/aiff", "audio/x-aiff"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"FORM")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::M4aAac,
            FormatDescriptor {
                id: "m4a",
                name: "AAC in MPEG-4 Container",
                extensions: &["m4a"],
                media_types: &["audio/mp4", "audio/x-m4a"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_offset(b"ftyp", 4)],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Aac,
            FormatDescriptor {
                id: "aac",
                name: "Advanced Audio Coding",
                extensions: &["aac"],
                media_types: &["audio/aac"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::M4aAlac,
            FormatDescriptor {
                id: "m4a_alac",
                name: "Apple Lossless (ALAC) in MPEG-4 Container",
                extensions: &["m4a"],
                media_types: &["audio/mp4"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_offset(b"ftyp", 4)],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Wma,
            FormatDescriptor {
                id: "wma",
                name: "Windows Media Audio",
                extensions: &["wma"],
                media_types: &["audio/x-ms-wma"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(
                    b"\x30\x26\xB2\x75\x8E\x66\xCF\x11",
                )],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Bwf,
            FormatDescriptor {
                id: "bwf",
                name: "Broadcast Wave Format",
                extensions: &["bwf"],
                media_types: &["audio/x-bwf"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"RIFF")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Pcm,
            FormatDescriptor {
                id: "pcm",
                name: "Raw PCM Audio",
                extensions: &["pcm"],
                media_types: &["audio/L16"],
                families: &[Audio],
                capabilities: vec![Convert],
                magic_signatures: vec![],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Wv,
            FormatDescriptor {
                id: "wv",
                name: "WavPack",
                extensions: &["wv"],
                media_types: &["audio/x-wavpack"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"wvpk")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Ape,
            FormatDescriptor {
                id: "ape",
                name: "Monkey's Audio",
                extensions: &["ape"],
                media_types: &["audio/x-ape"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"MAC ")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Tta,
            FormatDescriptor {
                id: "tta",
                name: "True Audio",
                extensions: &["tta"],
                media_types: &["audio/x-tta"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"TTA1")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Dsf,
            FormatDescriptor {
                id: "dsf",
                name: "DSD Storage Facility",
                extensions: &["dsf"],
                media_types: &["audio/x-dsf"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"DSD ")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Dff,
            FormatDescriptor {
                id: "dff",
                name: "DSD Interchange File Format",
                extensions: &["dff"],
                media_types: &["audio/x-dff"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"FRM8")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Shn,
            FormatDescriptor {
                id: "shn",
                name: "Shorten",
                extensions: &["shn"],
                media_types: &["audio/x-shorten"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"ajkg")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Mp2,
            FormatDescriptor {
                id: "mp2",
                name: "MPEG-1 Audio Layer II",
                extensions: &["mp2"],
                media_types: &["audio/mpeg"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Amr,
            FormatDescriptor {
                id: "amr",
                name: "Adaptive Multi-Rate",
                extensions: &["amr"],
                media_types: &["audio/amr"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"#!AMR")],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Ra,
            FormatDescriptor {
                id: "ra",
                name: "RealAudio",
                extensions: &["ra"],
                media_types: &["audio/x-realaudio"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b".RMF")],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Oma,
            FormatDescriptor {
                id: "oma",
                name: "Sony ATRAC",
                extensions: &["oma"],
                media_types: &["audio/x-oma"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"ea3\x03")],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Ac3,
            FormatDescriptor {
                id: "ac3",
                name: "Dolby Digital AC-3",
                extensions: &["ac3"],
                media_types: &["audio/ac3"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"\x0B\x77")],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Ec3,
            FormatDescriptor {
                id: "ec3",
                name: "Dolby Digital Plus (E-AC-3)",
                extensions: &["ec3"],
                media_types: &["audio/eac3"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Thd,
            FormatDescriptor {
                id: "thd",
                name: "Dolby TrueHD",
                extensions: &["thd"],
                media_types: &["audio/truehd"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Dts,
            FormatDescriptor {
                id: "dts",
                name: "DTS Digital Theater Systems",
                extensions: &["dts"],
                media_types: &["audio/vnd.dts"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"\x7F\xFE\x80\x01")],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::DtsHd,
            FormatDescriptor {
                id: "dtshd",
                name: "DTS-HD Master Audio",
                extensions: &["dtshd"],
                media_types: &["audio/vnd.dts.hd"],
                families: &[Audio],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Midi,
            FormatDescriptor {
                id: "midi",
                name: "MIDI",
                extensions: &["mid", "midi"],
                media_types: &["audio/midi", "audio/x-midi"],
                families: &[Audio],
                capabilities: vec![Detect],
                magic_signatures: vec![MagicSignature::at_start(b"MThd")],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        self.register(
            Format::Mod,
            FormatDescriptor {
                id: "mod",
                name: "Amiga Module",
                extensions: &["mod"],
                media_types: &["audio/mod", "audio/x-mod"],
                families: &[Audio],
                capabilities: vec![Detect],
                magic_signatures: vec![],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        // ── New video formats ─────────────────────────────────────────────────

        self.register(
            Format::Mp4,
            FormatDescriptor {
                id: "mp4",
                name: "MPEG-4 Part 14",
                extensions: &["mp4"],
                media_types: &["video/mp4"],
                families: &[Video],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_offset(b"ftyp", 4)],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Mov,
            FormatDescriptor {
                id: "mov",
                name: "QuickTime Movie",
                extensions: &["mov"],
                media_types: &["video/quicktime"],
                families: &[Video],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_offset(b"ftyp", 4)],
                loss_profile: PathDependent,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Mkv,
            FormatDescriptor {
                id: "mkv",
                name: "Matroska Video",
                extensions: &["mkv"],
                media_types: &["video/x-matroska"],
                families: &[Video],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"\x1A\x45\xDF\xA3")],
                loss_profile: Lossless,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::WebM,
            FormatDescriptor {
                id: "webm",
                name: "WebM",
                extensions: &["webm"],
                media_types: &["video/webm"],
                families: &[Video],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"\x1A\x45\xDF\xA3")],
                loss_profile: Lossy,
                external_requirements: vec![Ffmpeg],
            },
        );

        self.register(
            Format::Avi,
            FormatDescriptor {
                id: "avi",
                name: "Audio Video Interleave",
                extensions: &["avi"],
                media_types: &["video/x-msvideo"],
                families: &[Video],
                capabilities: vec![Detect, Convert],
                magic_signatures: vec![MagicSignature::at_start(b"RIFF")],
                loss_profile: PathDependent,
                external_requirements: vec![Ffmpeg],
            },
        );

        // ── New structured data formats ────────────────────────────────────────

        self.register(
            Format::Json,
            FormatDescriptor {
                id: "json",
                name: "JavaScript Object Notation",
                extensions: &["json"],
                media_types: &["application/json"],
                families: &[Data],
                capabilities: vec![Detect, Inspect, Validate],
                magic_signatures: vec![],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        self.register(
            Format::Yaml,
            FormatDescriptor {
                id: "yaml",
                name: "YAML Ain't Markup Language",
                extensions: &["yaml", "yml"],
                media_types: &["application/yaml", "text/yaml"],
                families: &[Data],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        self.register(
            Format::Toml,
            FormatDescriptor {
                id: "toml",
                name: "Tom's Obvious Minimal Language",
                extensions: &["toml"],
                media_types: &["application/toml"],
                families: &[Data],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        self.register(
            Format::Csv,
            FormatDescriptor {
                id: "csv",
                name: "Comma-Separated Values",
                extensions: &["csv"],
                media_types: &["text/csv"],
                families: &[Data, Spreadsheet],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![],
                loss_profile: Lossy,
                external_requirements: vec![],
            },
        );

        self.register(
            Format::Tsv,
            FormatDescriptor {
                id: "tsv",
                name: "Tab-Separated Values",
                extensions: &["tsv"],
                media_types: &["text/tab-separated-values"],
                families: &[Data, Spreadsheet],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![],
                loss_profile: Lossy,
                external_requirements: vec![],
            },
        );

        self.register(
            Format::Xml,
            FormatDescriptor {
                id: "xml",
                name: "Extensible Markup Language",
                extensions: &["xml"],
                media_types: &["application/xml", "text/xml"],
                families: &[Data],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![MagicSignature::at_start(b"<?xml")],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        // ── New archive formats ───────────────────────────────────────────────

        self.register(
            Format::Zip,
            FormatDescriptor {
                id: "zip",
                name: "ZIP Archive",
                extensions: &["zip"],
                media_types: &["application/zip"],
                families: &[Archive],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![MagicSignature::at_start(b"PK\x03\x04")],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        self.register(
            Format::TarGz,
            FormatDescriptor {
                id: "tar.gz",
                name: "Gzip-compressed TAR Archive",
                extensions: &["tar.gz", "tgz"],
                media_types: &["application/gzip", "application/x-tar"],
                families: &[Archive],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![MagicSignature::at_start(b"\x1F\x8B")],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        self.register(
            Format::TarXz,
            FormatDescriptor {
                id: "tar.xz",
                name: "XZ-compressed TAR Archive",
                extensions: &["tar.xz", "txz"],
                media_types: &["application/x-xz"],
                families: &[Archive],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![MagicSignature::at_start(b"\xFD7zXZ\x00")],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        // ── New subtitle/transcript formats ───────────────────────────────────

        self.register(
            Format::Srt,
            FormatDescriptor {
                id: "srt",
                name: "SubRip Text",
                extensions: &["srt"],
                media_types: &["application/x-subrip", "text/plain"],
                families: &[Subtitle],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );

        self.register(
            Format::WebVtt,
            FormatDescriptor {
                id: "vtt",
                name: "Web Video Text Tracks",
                extensions: &["vtt"],
                media_types: &["text/vtt"],
                families: &[Subtitle],
                capabilities: vec![Detect, Inspect],
                magic_signatures: vec![MagicSignature::at_start(b"WEBVTT")],
                loss_profile: Lossless,
                external_requirements: vec![],
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ArtifactCapability ────────────────────────────────────────────────────

    #[test]
    fn capability_as_str_returns_expected_values() {
        assert_eq!(ArtifactCapability::Detect.as_str(), "detect");
        assert_eq!(ArtifactCapability::Profile.as_str(), "profile");
        assert_eq!(ArtifactCapability::Inspect.as_str(), "inspect");
        assert_eq!(ArtifactCapability::Extract.as_str(), "extract");
        assert_eq!(ArtifactCapability::Convert.as_str(), "convert");
        assert_eq!(ArtifactCapability::Generate.as_str(), "generate");
        assert_eq!(ArtifactCapability::Validate.as_str(), "validate");
        assert_eq!(ArtifactCapability::RoundTrip.as_str(), "round_trip");
    }

    #[test]
    fn capability_display_matches_as_str() {
        assert_eq!(
            format!("{}", ArtifactCapability::Detect),
            ArtifactCapability::Detect.as_str()
        );
    }

    // ── FormatFamily ──────────────────────────────────────────────────────────

    #[test]
    fn family_as_str_returns_expected_values() {
        assert_eq!(FormatFamily::Document.as_str(), "document");
        assert_eq!(FormatFamily::Image.as_str(), "image");
        assert_eq!(FormatFamily::Audio.as_str(), "audio");
        assert_eq!(FormatFamily::Video.as_str(), "video");
        assert_eq!(FormatFamily::Archive.as_str(), "archive");
        assert_eq!(FormatFamily::Data.as_str(), "data");
        assert_eq!(FormatFamily::Subtitle.as_str(), "subtitle");
        assert_eq!(FormatFamily::Presentation.as_str(), "presentation");
        assert_eq!(FormatFamily::Spreadsheet.as_str(), "spreadsheet");
    }

    // ── LossProfile ───────────────────────────────────────────────────────────

    #[test]
    fn loss_profile_as_str_returns_expected_values() {
        assert_eq!(LossProfile::Lossless.as_str(), "lossless");
        assert_eq!(LossProfile::Lossy.as_str(), "lossy");
        assert_eq!(LossProfile::PartialLoss.as_str(), "partial_loss");
        assert_eq!(LossProfile::PathDependent.as_str(), "path_dependent");
    }

    // ── ExternalTool ──────────────────────────────────────────────────────────

    #[test]
    fn external_tool_executable_names() {
        assert_eq!(ExternalTool::Pandoc.executable_name(), "pandoc");
        assert_eq!(ExternalTool::Tectonic.executable_name(), "tectonic");
        assert_eq!(ExternalTool::Ffmpeg.executable_name(), "ffmpeg");
    }

    // ── MagicSignature ────────────────────────────────────────────────────────

    #[test]
    fn magic_signature_at_start_matches_correct_bytes() {
        let sig = MagicSignature::at_start(b"\xFF\xD8\xFF");
        let jpeg = b"\xFF\xD8\xFF\xE0some-jpeg-data";
        let not_jpeg = b"\x89PNG\r\n\x1A\n";
        assert!(sig.matches(jpeg));
        assert!(!sig.matches(not_jpeg));
    }

    #[test]
    fn magic_signature_at_offset_matches_at_correct_position() {
        let sig = MagicSignature::at_offset(b"ftyp", 4);
        let mp4_like = b"\x00\x00\x00\x20ftypsome-more";
        assert!(sig.matches(mp4_like));
    }

    #[test]
    fn magic_signature_rejects_too_short_buffer() {
        let sig = MagicSignature::at_start(b"RIFF");
        assert!(!sig.matches(b"RI")); // only 2 bytes
    }

    #[test]
    fn magic_signature_at_offset_rejects_too_short_buffer() {
        let sig = MagicSignature::at_offset(b"ftyp", 4);
        assert!(!sig.matches(b"\x00\x00\x00")); // only 3 bytes, need 8
    }

    // ── FormatDescriptor ──────────────────────────────────────────────────────

    #[test]
    fn format_descriptor_has_capability() {
        let desc = FormatDescriptor {
            id: "pdf",
            name: "PDF",
            extensions: &["pdf"],
            media_types: &["application/pdf"],
            families: &[FormatFamily::Document],
            capabilities: vec![ArtifactCapability::Detect, ArtifactCapability::Generate],
            magic_signatures: vec![MagicSignature::at_start(b"%PDF")],
            loss_profile: LossProfile::PartialLoss,
            external_requirements: vec![ExternalTool::Pandoc],
        };
        assert!(desc.has_capability(ArtifactCapability::Detect));
        assert!(desc.has_capability(ArtifactCapability::Generate));
        assert!(!desc.has_capability(ArtifactCapability::Extract));
    }

    #[test]
    fn format_descriptor_is_in_family() {
        let desc = FormatDescriptor {
            id: "epub",
            name: "EPUB",
            extensions: &["epub"],
            media_types: &["application/epub+zip"],
            families: &[FormatFamily::Document, FormatFamily::Archive],
            capabilities: vec![ArtifactCapability::Detect],
            magic_signatures: vec![],
            loss_profile: LossProfile::PartialLoss,
            external_requirements: vec![],
        };
        assert!(desc.is_in_family(FormatFamily::Document));
        assert!(desc.is_in_family(FormatFamily::Archive));
        assert!(!desc.is_in_family(FormatFamily::Audio));
    }

    #[test]
    fn format_descriptor_matches_magic() {
        let desc = FormatDescriptor {
            id: "png",
            name: "PNG",
            extensions: &["png"],
            media_types: &["image/png"],
            families: &[FormatFamily::Image],
            capabilities: vec![ArtifactCapability::Detect],
            magic_signatures: vec![MagicSignature::at_start(b"\x89PNG\r\n\x1A\n")],
            loss_profile: LossProfile::Lossless,
            external_requirements: vec![],
        };
        assert!(desc.matches_magic(b"\x89PNG\r\n\x1A\nsome-png-data"));
        assert!(!desc.matches_magic(b"\xFF\xD8\xFF")); // JPEG
    }

    #[test]
    fn format_descriptor_no_magic_never_matches() {
        let desc = FormatDescriptor {
            id: "markdown",
            name: "Markdown",
            extensions: &["md"],
            media_types: &["text/markdown"],
            families: &[FormatFamily::Document],
            capabilities: vec![ArtifactCapability::Detect],
            magic_signatures: vec![],
            loss_profile: LossProfile::PartialLoss,
            external_requirements: vec![],
        };
        // No magic signatures → never matches any buffer.
        assert!(!desc.matches_magic(b"# Hello"));
    }

    // ── FormatCapabilityRegistry ──────────────────────────────────────────────

    #[test]
    fn global_registry_has_pdf_descriptor() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry.get(Format::Pdf).expect("PDF must be registered");
        assert_eq!(desc.id, "pdf");
        assert!(desc.has_capability(ArtifactCapability::Detect));
        assert!(desc.has_capability(ArtifactCapability::Generate));
        assert!(desc.matches_magic(b"%PDF-1.7\n..."));
    }

    #[test]
    fn global_registry_has_markdown_descriptor() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry
            .get(Format::Markdown)
            .expect("Markdown must be registered");
        assert_eq!(desc.id, "markdown");
        assert!(desc.has_capability(ArtifactCapability::Detect));
    }

    #[test]
    fn global_registry_has_mp3_descriptor_with_ffmpeg_requirement() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry.get(Format::Mp3).expect("MP3 must be registered");
        assert!(desc.external_requirements.contains(&ExternalTool::Ffmpeg));
    }

    #[test]
    fn global_registry_by_family_returns_audio_formats() {
        let registry = FormatCapabilityRegistry::global();
        let audio = registry.by_family(FormatFamily::Audio);
        // WAV, FLAC, MP3, AAC, Ogg, Opus, etc. must all be present.
        assert!(
            audio.len() >= 5,
            "expected at least 5 audio formats, got {}",
            audio.len()
        );
    }

    #[test]
    fn global_registry_by_family_returns_video_formats() {
        let registry = FormatCapabilityRegistry::global();
        let video = registry.by_family(FormatFamily::Video);
        assert!(
            video.len() >= 3,
            "expected at least 3 video formats, got {}",
            video.len()
        );
    }

    #[test]
    fn global_registry_by_family_returns_data_formats() {
        let registry = FormatCapabilityRegistry::global();
        let data = registry.by_family(FormatFamily::Data);
        assert!(
            data.len() >= 4,
            "expected JSON, YAML, TOML, CSV, XML in data family, got {}",
            data.len()
        );
    }

    #[test]
    fn global_registry_by_capability_detect_includes_pdf_and_png() {
        let registry = FormatCapabilityRegistry::global();
        let detectable = registry.by_capability(ArtifactCapability::Detect);
        let ids: Vec<&str> = detectable.iter().map(|d| d.id).collect();
        assert!(ids.contains(&"pdf"), "PDF must support Detect");
        assert!(ids.contains(&"png"), "PNG must support Detect");
    }

    #[test]
    fn global_registry_epub_is_in_document_and_archive_family() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry.get(Format::Epub).expect("EPUB must be registered");
        assert!(desc.is_in_family(FormatFamily::Document));
        assert!(desc.is_in_family(FormatFamily::Archive));
    }

    #[test]
    fn global_registry_all_returns_nonempty_iterator() {
        let registry = FormatCapabilityRegistry::global();
        let count = registry.all().count();
        assert!(count > 20, "expected many formats, got {count}");
    }

    #[test]
    fn global_registry_json_has_inspect_and_validate() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry.get(Format::Json).expect("JSON must be registered");
        assert!(desc.has_capability(ArtifactCapability::Inspect));
        assert!(desc.has_capability(ArtifactCapability::Validate));
    }

    #[test]
    fn global_registry_png_matches_correct_magic() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry.get(Format::Png).expect("PNG must be registered");
        let png_header = b"\x89PNG\r\n\x1A\nfake-data";
        let jpeg_header = b"\xFF\xD8\xFF\xE0fake-data";
        assert!(
            desc.matches_magic(png_header),
            "PNG magic must match PNG header"
        );
        assert!(
            !desc.matches_magic(jpeg_header),
            "PNG magic must not match JPEG header"
        );
    }

    #[test]
    fn global_registry_webvtt_matches_magic() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry
            .get(Format::WebVtt)
            .expect("WebVTT must be registered");
        assert!(desc.matches_magic(b"WEBVTT\n\nfake-cue"));
    }

    #[test]
    fn global_registry_zip_matches_magic() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry.get(Format::Zip).expect("ZIP must be registered");
        assert!(desc.matches_magic(b"PK\x03\x04fake-zip-data"));
    }

    #[test]
    fn global_registry_tar_gz_matches_magic() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry
            .get(Format::TarGz)
            .expect("TAR.GZ must be registered");
        assert!(desc.matches_magic(b"\x1F\x8Bfake-gz-data"));
    }

    #[test]
    fn global_registry_mp4_family_is_video() {
        let registry = FormatCapabilityRegistry::global();
        let desc = registry.get(Format::Mp4).expect("MP4 must be registered");
        assert!(desc.is_in_family(FormatFamily::Video));
        assert_eq!(desc.loss_profile, LossProfile::Lossy);
    }

    #[test]
    fn custom_descriptor_can_be_registered() {
        let mut registry = FormatCapabilityRegistry::global();
        let custom = FormatDescriptor {
            id: "custom",
            name: "Custom Format",
            extensions: &["cst"],
            media_types: &["application/x-custom"],
            families: &[FormatFamily::Document],
            capabilities: vec![ArtifactCapability::Detect],
            magic_signatures: vec![],
            loss_profile: LossProfile::Lossless,
            external_requirements: vec![],
        };
        // Register under an existing format variant to verify override.
        registry.register(Format::Srt, custom);
        let desc = registry.get(Format::Srt).unwrap();
        assert_eq!(desc.id, "custom");
    }
}
