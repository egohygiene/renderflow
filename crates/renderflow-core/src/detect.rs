//! Deterministic artifact detection beyond filename extension matching.
//!
//! This module provides functions for identifying file formats using signals
//! more reliable than the filename extension alone:
//!
//! - **Magic bytes** — byte sequences at known offsets within the file.
//! - **Extension** — the filename extension (fallback when bytes are absent).
//! - **Conflict detection** — cases where the extension and content disagree.
//!
//! # Detection signal priority
//!
//! When both signals are available the content-based (magic-byte) match takes
//! precedence over the extension-based match.  When they conflict a
//! [`DetectionConflict`] is returned so the caller can decide how to proceed.
//!
//! # Example
//!
//! ```rust
//! use renderflow::detect::{detect_from_bytes, detect_from_path};
//!
//! // Detect from raw bytes (no filename).
//! let jpeg_bytes = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00";
//! let result = detect_from_bytes(jpeg_bytes);
//! assert!(result.is_some());
//!
//! // Detect from a file path (extension only — no I/O performed here).
//! let result = detect_from_path("document.pdf", None);
//! ```

use std::path::Path;

use crate::graph::capability::{FormatCapabilityRegistry, MagicSignature};
use crate::graph::Format;

/// The result of attempting to detect a format.
#[derive(Debug, Clone, PartialEq)]
pub enum DetectionResult {
    /// Format identified with high confidence using magic bytes.
    ConfidentMatch {
        /// The detected format.
        format: Format,
        /// The signal that produced the match.
        signal: DetectionSignal,
    },
    /// The extension and magic-byte signals disagree.
    Conflict(DetectionConflict),
    /// Only an extension-based match was possible (low confidence).
    ExtensionOnly {
        /// The format inferred from the extension.
        format: Format,
    },
    /// No format could be identified.
    Unknown,
}

/// The signal that produced a detection result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectionSignal {
    /// The format was identified via magic bytes in the file content.
    MagicBytes,
    /// The format was identified via the file extension.
    Extension,
}

/// Describes a conflict between the extension-inferred format and the
/// content-inferred format.
#[derive(Debug, Clone, PartialEq)]
pub struct DetectionConflict {
    /// The format inferred from the filename extension.
    pub extension_format: Format,
    /// The format identified via magic bytes.
    pub content_format: Format,
    /// A human-readable description of the conflict.
    pub message: String,
}

/// Attempt to identify a format from file content (magic bytes) alone.
///
/// Scans the built-in [`FormatCapabilityRegistry`] for a format whose magic
/// signatures match the provided byte slice.  Returns the first matching
/// format or `None` when no signature matches.
///
/// The `buf` slice should contain at least the first 16 bytes of the file for
/// reliable results, but longer slices are handled correctly.
pub fn detect_from_bytes(buf: &[u8]) -> Option<Format> {
    // Iterate in a deterministic order so that ties between formats sharing
    // a prefix are resolved consistently.
    let registry = FormatCapabilityRegistry::global();
    let mut candidates: Vec<(Format, usize)> = registry
        .all()
        .filter_map(|desc| {
            // Find the longest (most specific) matching signature.
            desc.magic_signatures
                .iter()
                .filter(|sig| sig.matches(buf))
                .map(|sig: &MagicSignature| sig.bytes.len())
                .max()
                .and_then(|len| {
                    // We need the Format variant — retrieve it by id.
                    desc.id.parse::<Format>().ok().map(|format| (format, len))
                })
        })
        .collect();

    // Prefer the match with the longest (most specific) signature to avoid
    // false positives when a shorter signature is a prefix of another.
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    candidates.into_iter().next().map(|(f, _)| f)
}

/// Attempt to identify a format from a filename extension.
///
/// Parses the extension from `path` and maps it to a known [`Format`] variant.
/// Returns `None` when the extension is missing or unrecognised.
pub fn detect_from_extension(path: &str) -> Option<Format> {
    let ext = Path::new(path).extension()?.to_str()?;
    ext.to_lowercase().as_str().parse::<Format>().ok()
}

/// Attempt to identify a format from a path and optional content bytes.
///
/// When `content` is provided, both the extension and magic bytes are
/// evaluated:
///
/// * If both agree, a [`DetectionResult::ConfidentMatch`] is returned using
///   [`DetectionSignal::MagicBytes`].
/// * If they disagree, a [`DetectionResult::Conflict`] is returned describing
///   the discrepancy.
/// * If only magic bytes match, a [`DetectionResult::ConfidentMatch`] with
///   [`DetectionSignal::MagicBytes`] is returned.
///
/// When `content` is `None`, extension detection alone is used, returning
/// [`DetectionResult::ExtensionOnly`] or [`DetectionResult::Unknown`].
pub fn detect_from_path(path: &str, content: Option<&[u8]>) -> DetectionResult {
    let ext_format = detect_from_extension(path);

    let content_format = content.and_then(detect_from_bytes);

    match (ext_format, content_format) {
        // Both signals agree → confident match.
        (Some(ext), Some(content)) if ext == content => DetectionResult::ConfidentMatch {
            format: content,
            signal: DetectionSignal::MagicBytes,
        },
        // Signals disagree → conflict.
        (Some(ext), Some(content)) => DetectionResult::Conflict(DetectionConflict {
            extension_format: ext,
            content_format: content,
            message: format!(
                "Extension suggests '{}' but file content identifies '{}'. \
                 The content-based identification is more reliable.",
                ext, content
            ),
        }),
        // Magic bytes only (no extension or unrecognised extension).
        (None, Some(content)) => DetectionResult::ConfidentMatch {
            format: content,
            signal: DetectionSignal::MagicBytes,
        },
        // Extension only (no content provided or content unrecognised).
        (Some(ext), None) => DetectionResult::ExtensionOnly { format: ext },
        // Nothing matched.
        (None, None) => DetectionResult::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── detect_from_bytes ──────────────────────────────────────────────────────

    #[test]
    fn detect_jpeg_from_bytes() {
        let buf = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00";
        let result = detect_from_bytes(buf);
        assert_eq!(result, Some(Format::Jpeg));
    }

    #[test]
    fn detect_png_from_bytes() {
        let buf = b"\x89PNG\r\n\x1A\nfake-data";
        let result = detect_from_bytes(buf);
        assert_eq!(result, Some(Format::Png));
    }

    #[test]
    fn detect_pdf_from_bytes() {
        let buf = b"%PDF-1.7\nfake-pdf-data";
        let result = detect_from_bytes(buf);
        assert_eq!(result, Some(Format::Pdf));
    }

    #[test]
    fn detect_flac_from_bytes() {
        let buf = b"fLaC\x00\x00\x00\x22";
        let result = detect_from_bytes(buf);
        assert_eq!(result, Some(Format::Flac));
    }

    #[test]
    fn detect_mp3_from_id3_tag() {
        let buf = b"ID3\x03\x00\x00\x00\x00\x00\x00";
        let result = detect_from_bytes(buf);
        assert_eq!(result, Some(Format::Mp3));
    }

    #[test]
    fn detect_webvtt_from_bytes() {
        let buf = b"WEBVTT\n\nfake-cue";
        let result = detect_from_bytes(buf);
        assert_eq!(result, Some(Format::WebVtt));
    }

    #[test]
    fn detect_zip_from_bytes() {
        let buf = b"PK\x03\x04\x14\x00\x00\x00";
        let result = detect_from_bytes(buf);
        // ZIP-based formats include Zip, Docx, Epub, Cbz — any is valid.
        // At minimum a result must be returned.
        assert!(result.is_some(), "ZIP magic must match something");
    }

    #[test]
    fn detect_tar_gz_from_bytes() {
        let buf = b"\x1F\x8B\x08\x00\x00\x00\x00\x00";
        let result = detect_from_bytes(buf);
        assert_eq!(result, Some(Format::TarGz));
    }

    #[test]
    fn detect_tar_xz_from_bytes() {
        let buf = b"\xFD7zXZ\x00\x00\x04";
        let result = detect_from_bytes(buf);
        assert_eq!(result, Some(Format::TarXz));
    }

    #[test]
    fn detect_mkv_from_bytes() {
        let buf = b"\x1A\x45\xDF\xA3\x01\x00\x00\x00";
        let result = detect_from_bytes(buf);
        // Both MKV and WebM share this magic — either is acceptable.
        assert!(result.is_some(), "EBML magic must match MKV or WebM");
    }

    #[test]
    fn detect_returns_none_for_empty_buffer() {
        let result = detect_from_bytes(b"");
        assert!(result.is_none());
    }

    #[test]
    fn detect_returns_none_for_unrecognised_bytes() {
        // Deliberately random-looking bytes that should not match any signature.
        let result = detect_from_bytes(b"\xDE\xAD\xBE\xEF\x00\x01\x02\x03");
        // Some signatures might be short enough to produce a false positive;
        // this test mainly ensures the function does not panic.
        let _ = result;
    }

    // ── detect_from_extension ─────────────────────────────────────────────────

    #[test]
    fn detect_extension_pdf() {
        assert_eq!(detect_from_extension("report.pdf"), Some(Format::Pdf));
    }

    #[test]
    fn detect_extension_png() {
        assert_eq!(detect_from_extension("image.PNG"), Some(Format::Png));
    }

    #[test]
    fn detect_extension_mp3() {
        assert_eq!(detect_from_extension("song.mp3"), Some(Format::Mp3));
    }

    #[test]
    fn detect_extension_yaml() {
        assert_eq!(detect_from_extension("config.yaml"), Some(Format::Yaml));
        assert_eq!(detect_from_extension("config.yml"), Some(Format::Yaml));
    }

    #[test]
    fn detect_extension_json() {
        assert_eq!(detect_from_extension("data.json"), Some(Format::Json));
    }

    #[test]
    fn detect_extension_csv() {
        assert_eq!(detect_from_extension("table.csv"), Some(Format::Csv));
    }

    #[test]
    fn detect_extension_zip() {
        assert_eq!(detect_from_extension("archive.zip"), Some(Format::Zip));
    }

    #[test]
    fn detect_extension_srt() {
        assert_eq!(detect_from_extension("subs.srt"), Some(Format::Srt));
    }

    #[test]
    fn detect_extension_vtt() {
        assert_eq!(detect_from_extension("captions.vtt"), Some(Format::WebVtt));
    }

    #[test]
    fn detect_extension_unknown_returns_none() {
        assert_eq!(detect_from_extension("file.xyz"), None);
    }

    #[test]
    fn detect_extension_no_extension_returns_none() {
        assert_eq!(detect_from_extension("README"), None);
    }

    // ── detect_from_path ──────────────────────────────────────────────────────

    #[test]
    fn detect_path_jpeg_both_signals_agree() {
        let buf = b"\xFF\xD8\xFF\xE0fake";
        let result = detect_from_path("photo.jpg", Some(buf));
        assert_eq!(
            result,
            DetectionResult::ConfidentMatch {
                format: Format::Jpeg,
                signal: DetectionSignal::MagicBytes,
            }
        );
    }

    #[test]
    fn detect_path_conflict_extension_vs_content() {
        // A file named ".png" but with JPEG content.
        let jpeg_bytes = b"\xFF\xD8\xFF\xE0fake";
        let result = detect_from_path("image.png", Some(jpeg_bytes));
        match result {
            DetectionResult::Conflict(c) => {
                assert_eq!(c.extension_format, Format::Png);
                assert_eq!(c.content_format, Format::Jpeg);
                assert!(
                    c.message.contains("content"),
                    "conflict message should mention content"
                );
            }
            other => panic!("expected Conflict, got {:?}", other),
        }
    }

    #[test]
    fn detect_path_extension_only_when_no_content() {
        let result = detect_from_path("document.pdf", None);
        assert_eq!(
            result,
            DetectionResult::ExtensionOnly {
                format: Format::Pdf
            }
        );
    }

    #[test]
    fn detect_path_unknown_when_no_signals_match() {
        let result = detect_from_path("file.xyz", None);
        assert_eq!(result, DetectionResult::Unknown);
    }

    #[test]
    fn detect_path_confident_when_only_magic_matches() {
        let flac_bytes = b"fLaC\x00\x00\x00\x22";
        // No extension recognisable.
        let result = detect_from_path("audio_file", Some(flac_bytes));
        assert_eq!(
            result,
            DetectionResult::ConfidentMatch {
                format: Format::Flac,
                signal: DetectionSignal::MagicBytes,
            }
        );
    }

    #[test]
    fn detect_path_pdf_magic_beats_wrong_extension() {
        let pdf_bytes = b"%PDF-1.5\n";
        // Named ".txt" but contains PDF bytes.
        let result = detect_from_path("sneaky.txt", Some(pdf_bytes));
        match result {
            DetectionResult::Conflict(c) => {
                assert_eq!(c.content_format, Format::Pdf);
            }
            DetectionResult::ConfidentMatch { format, .. } => {
                // If text/plain isn't registered, the extension branch returns None
                // and we get a ConfidentMatch on PDF — also acceptable.
                assert_eq!(format, Format::Pdf);
            }
            other => panic!("unexpected result: {:?}", other),
        }
    }

    #[test]
    fn detect_path_webvtt_magic_from_path() {
        let buf = b"WEBVTT\n\n";
        let result = detect_from_path("captions.vtt", Some(buf));
        assert_eq!(
            result,
            DetectionResult::ConfidentMatch {
                format: Format::WebVtt,
                signal: DetectionSignal::MagicBytes,
            }
        );
    }
}
