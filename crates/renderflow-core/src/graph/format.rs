use std::fmt;
use std::str::FromStr;

use anyhow::Result;

/// Represents all document formats that can participate in transformation edges.
///
/// Formats are modelled as graph nodes: each variant is a unique node in the
/// [`TransformGraph`](super::TransformGraph).  An edge between two nodes
/// indicates that a [`TransformEdge`](super::TransformEdge) exists that can
/// convert from the source format to the target format.
///
/// For detailed capability information (detection signals, MIME types, magic
/// bytes, families, and declared operations), see
/// [`capability::FormatCapabilityRegistry`](super::capability::FormatCapabilityRegistry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Format {
    // ── Document formats ───────────────────────────────────────────────────────
    Markdown,
    Html,
    Pdf,
    Docx,
    Epub,
    Rst,
    Latex,
    /// Fountain screenplay plain-text format.
    Fountain,

    // ── Image formats ──────────────────────────────────────────────────────────
    /// JPEG raster image format.
    Jpeg,
    /// PNG raster image format.
    Png,
    /// TIFF raster image format, commonly used in print/press workflows.
    Tiff,
    /// WebP image format — modern lossy/lossless.
    Webp,
    /// GIF image format — supports animation and transparency.
    Gif,
    /// Windows Bitmap.
    Bmp,
    /// AV1 Image File Format.
    Avif,
    /// Scalable Vector Graphics.
    Svg,
    /// Comic Book ZIP archive (`.cbz`).
    Cbz,

    // ── Audio formats ─────────────────────────────────────────────────────────
    /// Waveform Audio File Format (`.wav`).
    Wav,
    /// Audio Interchange File Format (`.aiff`).
    Aiff,
    /// Broadcast Wave Format (`.bwf`).
    Bwf,
    /// Raw PCM audio (`.pcm`).
    Pcm,
    /// Free Lossless Audio Codec (`.flac`).
    Flac,
    /// Apple Lossless in MPEG-4 container (`.m4a` / ALAC).
    M4aAlac,
    /// WavPack (`.wv`).
    Wv,
    /// Monkey's Audio (`.ape`).
    Ape,
    /// True Audio (`.tta`).
    Tta,
    /// DSD Storage Facility (`.dsf`).
    Dsf,
    /// DSD Interchange File Format (`.dff`).
    Dff,
    /// Shorten legacy lossless (`.shn`).
    Shn,
    /// MPEG-1 Audio Layer III (`.mp3`).
    Mp3,
    /// Advanced Audio Coding in MPEG-4 (`.m4a` / AAC).
    M4aAac,
    /// Advanced Audio Coding raw stream (`.aac`).
    Aac,
    /// Ogg Vorbis (`.ogg`).
    Ogg,
    /// Opus audio codec (`.opus`).
    Opus,
    /// Windows Media Audio (`.wma`).
    Wma,
    /// Adaptive Multi-Rate voice codec (`.amr`).
    Amr,
    /// MPEG-1 Audio Layer II (`.mp2`).
    Mp2,
    /// RealAudio (`.ra`).
    Ra,
    /// Sony ATRAC (`.oma`).
    Oma,
    /// Dolby Digital AC-3 (`.ac3`).
    Ac3,
    /// Dolby Digital Plus E-AC-3 (`.ec3`).
    Ec3,
    /// Dolby TrueHD (`.thd`).
    Thd,
    /// DTS Digital Theater Systems (`.dts`).
    Dts,
    /// DTS-HD Master Audio (`.dtshd`).
    DtsHd,
    /// MIDI (`.mid` / `.midi`).
    Midi,
    /// Amiga Module tracker format (`.mod`).
    Mod,

    // ── Video formats ─────────────────────────────────────────────────────────
    /// MPEG-4 Part 14 video container (`.mp4`).
    Mp4,
    /// QuickTime Movie container (`.mov`).
    Mov,
    /// Matroska Video container (`.mkv`).
    Mkv,
    /// WebM video container (`.webm`).
    WebM,
    /// Audio Video Interleave container (`.avi`).
    Avi,

    // ── Structured data formats ────────────────────────────────────────────────
    /// JavaScript Object Notation (`.json`).
    Json,
    /// YAML Ain't Markup Language (`.yaml` / `.yml`).
    Yaml,
    /// Tom's Obvious Minimal Language (`.toml`).
    Toml,
    /// Comma-Separated Values (`.csv`).
    Csv,
    /// Tab-Separated Values (`.tsv`).
    Tsv,
    /// Extensible Markup Language (`.xml`).
    Xml,

    // ── Archive formats ───────────────────────────────────────────────────────
    /// ZIP archive (`.zip`).
    Zip,
    /// Gzip-compressed TAR archive (`.tar.gz` / `.tgz`).
    TarGz,
    /// XZ-compressed TAR archive (`.tar.xz` / `.txz`).
    TarXz,

    // ── Subtitle / transcript formats ─────────────────────────────────────────
    /// SubRip Text subtitle format (`.srt`).
    Srt,
    /// Web Video Text Tracks (`.vtt`).
    WebVtt,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Format::Markdown => "markdown",
            Format::Html => "html",
            Format::Pdf => "pdf",
            Format::Docx => "docx",
            Format::Epub => "epub",
            Format::Rst => "rst",
            Format::Latex => "latex",
            Format::Fountain => "fountain",
            // Image
            Format::Jpeg => "jpeg",
            Format::Png => "png",
            Format::Tiff => "tiff",
            Format::Webp => "webp",
            Format::Gif => "gif",
            Format::Bmp => "bmp",
            Format::Avif => "avif",
            Format::Svg => "svg",
            Format::Cbz => "cbz",
            // Audio
            Format::Wav => "wav",
            Format::Aiff => "aiff",
            Format::Bwf => "bwf",
            Format::Pcm => "pcm",
            Format::Flac => "flac",
            Format::M4aAlac => "m4a_alac",
            Format::Wv => "wv",
            Format::Ape => "ape",
            Format::Tta => "tta",
            Format::Dsf => "dsf",
            Format::Dff => "dff",
            Format::Shn => "shn",
            Format::Mp3 => "mp3",
            Format::M4aAac => "m4a",
            Format::Aac => "aac",
            Format::Ogg => "ogg",
            Format::Opus => "opus",
            Format::Wma => "wma",
            Format::Amr => "amr",
            Format::Mp2 => "mp2",
            Format::Ra => "ra",
            Format::Oma => "oma",
            Format::Ac3 => "ac3",
            Format::Ec3 => "ec3",
            Format::Thd => "thd",
            Format::Dts => "dts",
            Format::DtsHd => "dtshd",
            Format::Midi => "midi",
            Format::Mod => "mod",
            // Video
            Format::Mp4 => "mp4",
            Format::Mov => "mov",
            Format::Mkv => "mkv",
            Format::WebM => "webm",
            Format::Avi => "avi",
            // Data
            Format::Json => "json",
            Format::Yaml => "yaml",
            Format::Toml => "toml",
            Format::Csv => "csv",
            Format::Tsv => "tsv",
            Format::Xml => "xml",
            // Archives
            Format::Zip => "zip",
            Format::TarGz => "tar.gz",
            Format::TarXz => "tar.xz",
            // Subtitles
            Format::Srt => "srt",
            Format::WebVtt => "vtt",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for Format {
    type Err = anyhow::Error;

    /// Parse a [`Format`] from a case-insensitive string.
    ///
    /// Accepted values include all document, image, audio, video, data, archive,
    /// and subtitle format identifiers.
    ///
    /// Returns an error that lists all supported format families when the string
    /// is unrecognised.
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            // Documents
            "markdown" | "md" => Ok(Format::Markdown),
            "html" => Ok(Format::Html),
            "pdf" => Ok(Format::Pdf),
            "docx" => Ok(Format::Docx),
            "epub" => Ok(Format::Epub),
            "rst" => Ok(Format::Rst),
            "latex" | "tex" => Ok(Format::Latex),
            "fountain" => Ok(Format::Fountain),
            // Images
            "jpeg" | "jpg" => Ok(Format::Jpeg),
            "png" => Ok(Format::Png),
            "tiff" | "tif" => Ok(Format::Tiff),
            "webp" => Ok(Format::Webp),
            "gif" => Ok(Format::Gif),
            "bmp" => Ok(Format::Bmp),
            "avif" => Ok(Format::Avif),
            "svg" => Ok(Format::Svg),
            "cbz" => Ok(Format::Cbz),
            // Audio
            "wav" => Ok(Format::Wav),
            "aif" | "aiff" => Ok(Format::Aiff),
            "bwf" => Ok(Format::Bwf),
            "pcm" => Ok(Format::Pcm),
            "flac" => Ok(Format::Flac),
            "m4a_alac" | "alac" => Ok(Format::M4aAlac),
            "wv" | "wavpack" => Ok(Format::Wv),
            "ape" => Ok(Format::Ape),
            "tta" => Ok(Format::Tta),
            "dsf" => Ok(Format::Dsf),
            "dff" => Ok(Format::Dff),
            "shn" => Ok(Format::Shn),
            "mp3" => Ok(Format::Mp3),
            "m4a" | "m4a_aac" => Ok(Format::M4aAac),
            "aac" => Ok(Format::Aac),
            "ogg" => Ok(Format::Ogg),
            "opus" => Ok(Format::Opus),
            "wma" => Ok(Format::Wma),
            "amr" => Ok(Format::Amr),
            "mp2" => Ok(Format::Mp2),
            "ra" => Ok(Format::Ra),
            "oma" => Ok(Format::Oma),
            "ac3" => Ok(Format::Ac3),
            "ec3" | "eac3" => Ok(Format::Ec3),
            "thd" => Ok(Format::Thd),
            "dts" => Ok(Format::Dts),
            "dtshd" => Ok(Format::DtsHd),
            "mid" | "midi" => Ok(Format::Midi),
            "mod" => Ok(Format::Mod),
            // Video
            "mp4" => Ok(Format::Mp4),
            "mov" => Ok(Format::Mov),
            "mkv" => Ok(Format::Mkv),
            "webm" => Ok(Format::WebM),
            "avi" => Ok(Format::Avi),
            // Data
            "json" => Ok(Format::Json),
            "yaml" | "yml" => Ok(Format::Yaml),
            "toml" => Ok(Format::Toml),
            "csv" => Ok(Format::Csv),
            "tsv" => Ok(Format::Tsv),
            "xml" => Ok(Format::Xml),
            // Archives
            "zip" => Ok(Format::Zip),
            "tar.gz" | "tgz" => Ok(Format::TarGz),
            "tar.xz" | "txz" => Ok(Format::TarXz),
            // Subtitles
            "srt" => Ok(Format::Srt),
            "vtt" | "webvtt" => Ok(Format::WebVtt),
            _ => anyhow::bail!(
                "'{}' is not a known format. \
                 Documents: markdown, html, pdf, docx, epub, rst, latex, fountain. \
                 Images: jpeg, png, tiff, webp, gif, bmp, avif, svg, cbz. \
                 Audio: wav, aiff, bwf, pcm, flac, m4a_alac, wv, ape, tta, dsf, dff, shn, \
                 mp3, m4a, aac, ogg, opus, wma, amr, mp2, ra, oma, \
                 ac3, ec3, thd, dts, dtshd, midi, mod. \
                 Video: mp4, mov, mkv, webm, avi. \
                 Data: json, yaml, toml, csv, tsv, xml. \
                 Archives: zip, tar.gz, tar.xz. \
                 Subtitles: srt, vtt.",
                s
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_markdown() {
        assert_eq!(Format::Markdown.to_string(), "markdown");
    }

    #[test]
    fn test_display_all_variants() {
        assert_eq!(Format::Html.to_string(), "html");
        assert_eq!(Format::Pdf.to_string(), "pdf");
        assert_eq!(Format::Docx.to_string(), "docx");
        assert_eq!(Format::Epub.to_string(), "epub");
        assert_eq!(Format::Rst.to_string(), "rst");
        assert_eq!(Format::Latex.to_string(), "latex");
    }

    #[test]
    fn test_format_equality() {
        assert_eq!(Format::Markdown, Format::Markdown);
        assert_ne!(Format::Markdown, Format::Html);
    }

    #[test]
    fn test_format_clone_copy() {
        let f = Format::Pdf;
        let g = f;
        assert_eq!(f, g);
    }

    #[test]
    fn test_format_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(Format::Markdown);
        set.insert(Format::Markdown);
        set.insert(Format::Html);
        assert_eq!(set.len(), 2);
    }

    // ── FromStr tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_from_str_markdown() {
        assert_eq!("markdown".parse::<Format>().unwrap(), Format::Markdown);
        assert_eq!("md".parse::<Format>().unwrap(), Format::Markdown);
        assert_eq!("Markdown".parse::<Format>().unwrap(), Format::Markdown);
    }

    #[test]
    fn test_from_str_html() {
        assert_eq!("html".parse::<Format>().unwrap(), Format::Html);
        assert_eq!("HTML".parse::<Format>().unwrap(), Format::Html);
    }

    #[test]
    fn test_from_str_pdf() {
        assert_eq!("pdf".parse::<Format>().unwrap(), Format::Pdf);
    }

    #[test]
    fn test_from_str_docx() {
        assert_eq!("docx".parse::<Format>().unwrap(), Format::Docx);
    }

    #[test]
    fn test_from_str_epub() {
        assert_eq!("epub".parse::<Format>().unwrap(), Format::Epub);
    }

    #[test]
    fn test_from_str_rst() {
        assert_eq!("rst".parse::<Format>().unwrap(), Format::Rst);
    }

    #[test]
    fn test_from_str_latex() {
        assert_eq!("latex".parse::<Format>().unwrap(), Format::Latex);
        assert_eq!("tex".parse::<Format>().unwrap(), Format::Latex);
    }

    #[test]
    fn test_from_str_unknown_returns_error() {
        let err = "xyz-unknown".parse::<Format>().unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("'xyz-unknown' is not a known format"),
            "unexpected: {msg}"
        );
        assert!(msg.contains("markdown"), "expected format list: {msg}");
    }

    // ── image / CBZ format tests ──────────────────────────────────────────────

    #[test]
    fn test_from_str_jpeg() {
        assert_eq!("jpeg".parse::<Format>().unwrap(), Format::Jpeg);
        assert_eq!("jpg".parse::<Format>().unwrap(), Format::Jpeg);
        assert_eq!("JPEG".parse::<Format>().unwrap(), Format::Jpeg);
    }

    #[test]
    fn test_from_str_png() {
        assert_eq!("png".parse::<Format>().unwrap(), Format::Png);
        assert_eq!("PNG".parse::<Format>().unwrap(), Format::Png);
    }

    #[test]
    fn test_from_str_tiff() {
        assert_eq!("tiff".parse::<Format>().unwrap(), Format::Tiff);
        assert_eq!("tif".parse::<Format>().unwrap(), Format::Tiff);
        assert_eq!("TIFF".parse::<Format>().unwrap(), Format::Tiff);
    }

    #[test]
    fn test_from_str_cbz() {
        assert_eq!("cbz".parse::<Format>().unwrap(), Format::Cbz);
        assert_eq!("CBZ".parse::<Format>().unwrap(), Format::Cbz);
    }

    #[test]
    fn test_display_jpeg() {
        assert_eq!(Format::Jpeg.to_string(), "jpeg");
    }

    #[test]
    fn test_display_png() {
        assert_eq!(Format::Png.to_string(), "png");
    }

    #[test]
    fn test_display_tiff() {
        assert_eq!(Format::Tiff.to_string(), "tiff");
    }

    #[test]
    fn test_display_cbz() {
        assert_eq!(Format::Cbz.to_string(), "cbz");
    }

    // ── Audio format tests ────────────────────────────────────────────────────

    #[test]
    fn test_from_str_wav() {
        assert_eq!("wav".parse::<Format>().unwrap(), Format::Wav);
    }

    #[test]
    fn test_from_str_mp3() {
        assert_eq!("mp3".parse::<Format>().unwrap(), Format::Mp3);
        assert_eq!("MP3".parse::<Format>().unwrap(), Format::Mp3);
    }

    #[test]
    fn test_from_str_flac() {
        assert_eq!("flac".parse::<Format>().unwrap(), Format::Flac);
    }

    #[test]
    fn test_from_str_opus() {
        assert_eq!("opus".parse::<Format>().unwrap(), Format::Opus);
    }

    #[test]
    fn test_from_str_m4a_is_aac() {
        assert_eq!("m4a".parse::<Format>().unwrap(), Format::M4aAac);
    }

    #[test]
    fn test_from_str_m4a_alac() {
        assert_eq!("m4a_alac".parse::<Format>().unwrap(), Format::M4aAlac);
        assert_eq!("alac".parse::<Format>().unwrap(), Format::M4aAlac);
    }

    #[test]
    fn test_display_audio_mp3() {
        assert_eq!(Format::Mp3.to_string(), "mp3");
    }

    #[test]
    fn test_display_audio_flac() {
        assert_eq!(Format::Flac.to_string(), "flac");
    }

    #[test]
    fn test_display_audio_wav() {
        assert_eq!(Format::Wav.to_string(), "wav");
    }

    // ── New image format tests ────────────────────────────────────────────────

    #[test]
    fn test_from_str_webp() {
        assert_eq!("webp".parse::<Format>().unwrap(), Format::Webp);
        assert_eq!("WEBP".parse::<Format>().unwrap(), Format::Webp);
    }

    #[test]
    fn test_from_str_gif() {
        assert_eq!("gif".parse::<Format>().unwrap(), Format::Gif);
    }

    #[test]
    fn test_from_str_bmp() {
        assert_eq!("bmp".parse::<Format>().unwrap(), Format::Bmp);
    }

    #[test]
    fn test_from_str_avif() {
        assert_eq!("avif".parse::<Format>().unwrap(), Format::Avif);
    }

    #[test]
    fn test_from_str_svg() {
        assert_eq!("svg".parse::<Format>().unwrap(), Format::Svg);
    }

    #[test]
    fn test_display_webp() {
        assert_eq!(Format::Webp.to_string(), "webp");
    }

    #[test]
    fn test_display_gif() {
        assert_eq!(Format::Gif.to_string(), "gif");
    }

    #[test]
    fn test_display_svg() {
        assert_eq!(Format::Svg.to_string(), "svg");
    }

    // ── Video format tests ────────────────────────────────────────────────────

    #[test]
    fn test_from_str_mp4() {
        assert_eq!("mp4".parse::<Format>().unwrap(), Format::Mp4);
    }

    #[test]
    fn test_from_str_mov() {
        assert_eq!("mov".parse::<Format>().unwrap(), Format::Mov);
    }

    #[test]
    fn test_from_str_mkv() {
        assert_eq!("mkv".parse::<Format>().unwrap(), Format::Mkv);
    }

    #[test]
    fn test_from_str_webm() {
        assert_eq!("webm".parse::<Format>().unwrap(), Format::WebM);
    }

    #[test]
    fn test_from_str_avi() {
        assert_eq!("avi".parse::<Format>().unwrap(), Format::Avi);
    }

    #[test]
    fn test_display_mp4() {
        assert_eq!(Format::Mp4.to_string(), "mp4");
    }

    #[test]
    fn test_display_mkv() {
        assert_eq!(Format::Mkv.to_string(), "mkv");
    }

    #[test]
    fn test_display_webm() {
        assert_eq!(Format::WebM.to_string(), "webm");
    }

    // ── Structured data format tests ──────────────────────────────────────────

    #[test]
    fn test_from_str_json() {
        assert_eq!("json".parse::<Format>().unwrap(), Format::Json);
        assert_eq!("JSON".parse::<Format>().unwrap(), Format::Json);
    }

    #[test]
    fn test_from_str_yaml() {
        assert_eq!("yaml".parse::<Format>().unwrap(), Format::Yaml);
        assert_eq!("yml".parse::<Format>().unwrap(), Format::Yaml);
        assert_eq!("YAML".parse::<Format>().unwrap(), Format::Yaml);
    }

    #[test]
    fn test_from_str_toml() {
        assert_eq!("toml".parse::<Format>().unwrap(), Format::Toml);
    }

    #[test]
    fn test_from_str_csv() {
        assert_eq!("csv".parse::<Format>().unwrap(), Format::Csv);
    }

    #[test]
    fn test_from_str_tsv() {
        assert_eq!("tsv".parse::<Format>().unwrap(), Format::Tsv);
    }

    #[test]
    fn test_from_str_xml() {
        assert_eq!("xml".parse::<Format>().unwrap(), Format::Xml);
    }

    #[test]
    fn test_display_json() {
        assert_eq!(Format::Json.to_string(), "json");
    }

    #[test]
    fn test_display_yaml() {
        assert_eq!(Format::Yaml.to_string(), "yaml");
    }

    #[test]
    fn test_display_csv() {
        assert_eq!(Format::Csv.to_string(), "csv");
    }

    // ── Archive format tests ──────────────────────────────────────────────────

    #[test]
    fn test_from_str_zip() {
        assert_eq!("zip".parse::<Format>().unwrap(), Format::Zip);
    }

    #[test]
    fn test_from_str_tar_gz() {
        assert_eq!("tar.gz".parse::<Format>().unwrap(), Format::TarGz);
        assert_eq!("tgz".parse::<Format>().unwrap(), Format::TarGz);
    }

    #[test]
    fn test_from_str_tar_xz() {
        assert_eq!("tar.xz".parse::<Format>().unwrap(), Format::TarXz);
        assert_eq!("txz".parse::<Format>().unwrap(), Format::TarXz);
    }

    #[test]
    fn test_display_zip() {
        assert_eq!(Format::Zip.to_string(), "zip");
    }

    #[test]
    fn test_display_tar_gz() {
        assert_eq!(Format::TarGz.to_string(), "tar.gz");
    }

    #[test]
    fn test_display_tar_xz() {
        assert_eq!(Format::TarXz.to_string(), "tar.xz");
    }

    // ── Subtitle format tests ─────────────────────────────────────────────────

    #[test]
    fn test_from_str_srt() {
        assert_eq!("srt".parse::<Format>().unwrap(), Format::Srt);
    }

    #[test]
    fn test_from_str_vtt() {
        assert_eq!("vtt".parse::<Format>().unwrap(), Format::WebVtt);
        assert_eq!("webvtt".parse::<Format>().unwrap(), Format::WebVtt);
    }

    #[test]
    fn test_display_srt() {
        assert_eq!(Format::Srt.to_string(), "srt");
    }

    #[test]
    fn test_display_vtt() {
        assert_eq!(Format::WebVtt.to_string(), "vtt");
    }

    // ── Error message includes all format families ────────────────────────────

    #[test]
    fn test_from_str_error_message_lists_all_families() {
        let err = "xyz-unknown".parse::<Format>().unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("Documents:"),
            "message must mention Documents: {msg}"
        );
        assert!(
            msg.contains("Images:"),
            "message must mention Images: {msg}"
        );
        assert!(msg.contains("Audio:"), "message must mention Audio: {msg}");
        assert!(msg.contains("Video:"), "message must mention Video: {msg}");
        assert!(msg.contains("Data:"), "message must mention Data: {msg}");
        assert!(
            msg.contains("Archives:"),
            "message must mention Archives: {msg}"
        );
        assert!(
            msg.contains("Subtitles:"),
            "message must mention Subtitles: {msg}"
        );
    }
}
