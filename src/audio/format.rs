use std::fmt;
use std::str::FromStr;

use anyhow::Result;

/// All audio formats that renderflow can produce via FFmpeg (or SoX).
///
/// # Groupings
///
/// * **Uncompressed** – WAV, AIFF, BWF, PCM
/// * **Lossless compressed** – FLAC, ALAC (M4A), WavPack, APE, TrueAudio, DSD (DSF/DFF), Shorten
/// * **Lossy compressed** – MP3, AAC (M4A), Ogg Vorbis, Opus, WMA, AMR, MP2, RealAudio, ATRAC
/// * **Multichannel / Cinema** – AC-3, E-AC-3, Dolby TrueHD, DTS, DTS-HD
/// * **MIDI / Sequence** – MIDI, MOD (read-only decoding via FFmpeg)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioFormat {
    // ── Uncompressed ──────────────────────────────────────────────────────────
    /// Waveform Audio File Format (.wav) — universal PCM container.
    Wav,
    /// Audio Interchange File Format (.aif) — Mac/Apple PCM container.
    Aif,
    /// Audio Interchange File Format (.aiff) — standard extension variant.
    Aiff,
    /// Broadcast Wave Format (.bwf) — WAV with embedded metadata for broadcast.
    Bwf,
    /// Raw Pulse Code Modulation (.pcm) — headerless raw samples.
    Pcm,

    // ── Lossless compressed ───────────────────────────────────────────────────
    /// Free Lossless Audio Codec (.flac).
    Flac,
    /// Apple Lossless Audio Codec in MPEG-4 container (.m4a / ALAC).
    M4aAlac,
    /// WavPack (.wv) — hybrid lossless/lossy codec.
    Wv,
    /// Monkey's Audio (.ape) — high-ratio lossless compression.
    Ape,
    /// True Audio (.tta) — symmetric lossless codec.
    Tta,
    /// DSD Storage Facility (.dsf) — Direct Stream Digital container.
    Dsf,
    /// DSD Interchange File Format (.dff) — alternative DSD container.
    Dff,
    /// Shorten (.shn) — legacy lossless format.
    Shn,

    // ── Lossy compressed ──────────────────────────────────────────────────────
    /// MPEG-1 Audio Layer III (.mp3).
    Mp3,
    /// Advanced Audio Coding in MPEG-4 container (.m4a / AAC).
    M4aAac,
    /// Advanced Audio Coding raw stream (.aac).
    Aac,
    /// Ogg Vorbis (.ogg).
    Ogg,
    /// Opus audio codec (.opus) — low-latency, high-quality.
    Opus,
    /// Windows Media Audio (.wma).
    Wma,
    /// Adaptive Multi-Rate voice codec (.amr).
    Amr,
    /// MPEG-1 Audio Layer II (.mp2) — broadcast / legacy.
    Mp2,
    /// RealAudio (.ra) — legacy streaming format.
    Ra,
    /// Sony ATRAC (.oma) — MiniDisc successor.
    Oma,

    // ── Multichannel / Cinema ─────────────────────────────────────────────────
    /// Dolby Digital AC-3 (.ac3).
    Ac3,
    /// Dolby Digital Plus E-AC-3 (.ec3).
    Ec3,
    /// Dolby TrueHD (.thd) — lossless Dolby surround.
    Thd,
    /// DTS Digital Theater Systems (.dts).
    Dts,
    /// DTS-HD Master Audio (.dtshd) — lossless DTS surround.
    DtsHd,

    // ── MIDI / Sequence (limited encode support via FFmpeg) ───────────────────
    /// Musical Instrument Digital Interface (.mid).
    Mid,
    /// MIDI — alternate extension (.midi).
    Midi,
    /// Amiga Module tracker format (.mod).
    Mod,
}

impl AudioFormat {
    /// Return the canonical file extension for this format (without leading dot).
    pub fn file_extension(self) -> &'static str {        match self {
            AudioFormat::Wav => "wav",
            AudioFormat::Aif => "aif",
            AudioFormat::Aiff => "aiff",
            AudioFormat::Bwf => "wav", // BWF shares the .wav extension
            AudioFormat::Pcm => "pcm",
            AudioFormat::Flac => "flac",
            AudioFormat::M4aAlac => "m4a",
            AudioFormat::Wv => "wv",
            AudioFormat::Ape => "ape",
            AudioFormat::Tta => "tta",
            AudioFormat::Dsf => "dsf",
            AudioFormat::Dff => "dff",
            AudioFormat::Shn => "shn",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::M4aAac => "m4a",
            AudioFormat::Aac => "aac",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Opus => "opus",
            AudioFormat::Wma => "wma",
            AudioFormat::Amr => "amr",
            AudioFormat::Mp2 => "mp2",
            AudioFormat::Ra => "ra",
            AudioFormat::Oma => "oma",
            AudioFormat::Ac3 => "ac3",
            AudioFormat::Ec3 => "ec3",
            AudioFormat::Thd => "thd",
            AudioFormat::Dts => "dts",
            AudioFormat::DtsHd => "dtshd",
            AudioFormat::Mid => "mid",
            AudioFormat::Midi => "midi",
            AudioFormat::Mod => "mod",
        }
    }

    /// Return the FFmpeg format name used with `-f`, or `None` when FFmpeg does
    /// not support encoding to this format.
    #[allow(dead_code)]
    pub fn ffmpeg_format(self) -> Option<&'static str> {
        match self {
            AudioFormat::Wav | AudioFormat::Bwf => Some("wav"),
            AudioFormat::Aif | AudioFormat::Aiff => Some("aiff"),
            AudioFormat::Pcm => Some("s16le"), // raw signed 16-bit PCM
            AudioFormat::Flac => Some("flac"),
            AudioFormat::M4aAlac | AudioFormat::M4aAac => Some("ipod"),
            AudioFormat::Wv => Some("wv"),
            AudioFormat::Ape => Some("ape"),
            AudioFormat::Tta => Some("tta"),
            AudioFormat::Dsf | AudioFormat::Dff => None, // DSD encode not supported
            AudioFormat::Shn => None,               // Shorten encode not supported
            AudioFormat::Mp3 => Some("mp3"),
            AudioFormat::Aac => Some("adts"),
            AudioFormat::Ogg => Some("ogg"),
            AudioFormat::Opus => Some("opus"),
            AudioFormat::Wma => Some("asf"),
            AudioFormat::Amr => Some("amr"),
            AudioFormat::Mp2 => Some("mp2"),
            AudioFormat::Ra => None,   // RealAudio encode not supported
            AudioFormat::Oma => None,  // ATRAC encode not supported
            AudioFormat::Ac3 => Some("ac3"),
            AudioFormat::Ec3 => Some("eac3"),
            AudioFormat::Thd => None,  // TrueHD encode requires special build
            AudioFormat::Dts => Some("dts"),
            AudioFormat::DtsHd => None, // DTS-HD encode not generally available
            AudioFormat::Mid => Some("midi"),
            AudioFormat::Midi => Some("midi"),
            AudioFormat::Mod => None,  // MOD encode not supported
        }
    }

    /// Return the FFmpeg audio codec name (`-acodec`), or `None` for formats
    /// where encode is unsupported.
    pub fn ffmpeg_codec(self) -> Option<&'static str> {
        match self {
            AudioFormat::Wav | AudioFormat::Bwf => Some("pcm_s16le"), // overridden per profile
            AudioFormat::Aif | AudioFormat::Aiff => Some("pcm_s16le"),
            AudioFormat::Pcm => Some("pcm_s16le"),
            AudioFormat::Flac => Some("flac"),
            AudioFormat::M4aAlac => Some("alac"),
            AudioFormat::Wv => Some("wavpack"),
            AudioFormat::Ape => Some("ape"),
            AudioFormat::Tta => Some("tta"),
            AudioFormat::Dsf | AudioFormat::Dff | AudioFormat::Shn => None,
            AudioFormat::Mp3 => Some("libmp3lame"),
            AudioFormat::M4aAac | AudioFormat::Aac => Some("aac"),
            AudioFormat::Ogg => Some("libvorbis"),
            AudioFormat::Opus => Some("libopus"),
            AudioFormat::Wma => Some("wmav2"),
            AudioFormat::Amr => Some("libopencore_amrnb"),
            AudioFormat::Mp2 => Some("mp2"),
            AudioFormat::Ra | AudioFormat::Oma => None,
            AudioFormat::Ac3 => Some("ac3"),
            AudioFormat::Ec3 => Some("eac3"),
            AudioFormat::Thd | AudioFormat::DtsHd => None,
            AudioFormat::Dts => Some("dca"),
            AudioFormat::Mid | AudioFormat::Midi => Some("midi"),
            AudioFormat::Mod => None,
        }
    }

    /// Return `true` when this format stores audio without perceptual loss.
    #[allow(dead_code)]
    pub fn is_lossless(self) -> bool {
        matches!(
            self,
            AudioFormat::Wav
                | AudioFormat::Aif
                | AudioFormat::Aiff
                | AudioFormat::Bwf
                | AudioFormat::Pcm
                | AudioFormat::Flac
                | AudioFormat::M4aAlac
                | AudioFormat::Wv
                | AudioFormat::Ape
                | AudioFormat::Tta
                | AudioFormat::Dsf
                | AudioFormat::Dff
                | AudioFormat::Shn
                | AudioFormat::Thd
                | AudioFormat::DtsHd
        )
    }

    /// Return `true` when FFmpeg can encode to this format.
    pub fn supports_encoding(self) -> bool {
        self.ffmpeg_codec().is_some()
    }

    /// Detect an `AudioFormat` from a file-extension string (without the dot).
    ///
    /// Returns `None` when the extension is not a known audio format.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "wav" => Some(AudioFormat::Wav),
            "aif" => Some(AudioFormat::Aif),
            "aiff" => Some(AudioFormat::Aiff),
            "bwf" => Some(AudioFormat::Bwf),
            "pcm" => Some(AudioFormat::Pcm),
            "flac" => Some(AudioFormat::Flac),
            "wv" => Some(AudioFormat::Wv),
            "ape" => Some(AudioFormat::Ape),
            "tta" => Some(AudioFormat::Tta),
            "dsf" => Some(AudioFormat::Dsf),
            "dff" => Some(AudioFormat::Dff),
            "shn" => Some(AudioFormat::Shn),
            "mp3" => Some(AudioFormat::Mp3),
            "aac" => Some(AudioFormat::Aac),
            "ogg" => Some(AudioFormat::Ogg),
            "opus" => Some(AudioFormat::Opus),
            "wma" => Some(AudioFormat::Wma),
            "amr" => Some(AudioFormat::Amr),
            "mp2" => Some(AudioFormat::Mp2),
            "ra" => Some(AudioFormat::Ra),
            "oma" => Some(AudioFormat::Oma),
            "ac3" => Some(AudioFormat::Ac3),
            "ec3" | "eac3" => Some(AudioFormat::Ec3),
            "thd" => Some(AudioFormat::Thd),
            "dts" => Some(AudioFormat::Dts),
            "dtshd" => Some(AudioFormat::DtsHd),
            "mid" => Some(AudioFormat::Mid),
            "midi" => Some(AudioFormat::Midi),
            "mod" => Some(AudioFormat::Mod),
            // m4a is ambiguous (ALAC or AAC); default to AAC as it is more common.
            "m4a" => Some(AudioFormat::M4aAac),
            _ => None,
        }
    }

    /// Return a human-readable description of this format.
    pub fn description(self) -> &'static str {
        match self {
            AudioFormat::Wav => "Waveform Audio File Format (WAV)",
            AudioFormat::Aif => "Audio Interchange File Format (AIFF, .aif)",
            AudioFormat::Aiff => "Audio Interchange File Format (AIFF)",
            AudioFormat::Bwf => "Broadcast Wave Format (BWF/WAV)",
            AudioFormat::Pcm => "Raw PCM (headerless)",
            AudioFormat::Flac => "Free Lossless Audio Codec (FLAC)",
            AudioFormat::M4aAlac => "Apple Lossless Audio Codec in M4A (ALAC)",
            AudioFormat::Wv => "WavPack",
            AudioFormat::Ape => "Monkey's Audio (APE)",
            AudioFormat::Tta => "True Audio (TTA)",
            AudioFormat::Dsf => "DSD Storage Facility (DSF)",
            AudioFormat::Dff => "DSD Interchange File Format (DFF)",
            AudioFormat::Shn => "Shorten (SHN)",
            AudioFormat::Mp3 => "MPEG-1 Audio Layer III (MP3)",
            AudioFormat::M4aAac => "Advanced Audio Coding in M4A (AAC)",
            AudioFormat::Aac => "Advanced Audio Coding raw stream (AAC)",
            AudioFormat::Ogg => "Ogg Vorbis",
            AudioFormat::Opus => "Opus Audio Codec",
            AudioFormat::Wma => "Windows Media Audio (WMA)",
            AudioFormat::Amr => "Adaptive Multi-Rate (AMR)",
            AudioFormat::Mp2 => "MPEG-1 Audio Layer II (MP2)",
            AudioFormat::Ra => "RealAudio (RA)",
            AudioFormat::Oma => "Sony ATRAC (OMA)",
            AudioFormat::Ac3 => "Dolby Digital AC-3",
            AudioFormat::Ec3 => "Dolby Digital Plus E-AC-3",
            AudioFormat::Thd => "Dolby TrueHD",
            AudioFormat::Dts => "DTS Digital Theater Systems",
            AudioFormat::DtsHd => "DTS-HD Master Audio",
            AudioFormat::Mid => "MIDI (.mid)",
            AudioFormat::Midi => "MIDI (.midi)",
            AudioFormat::Mod => "Amiga Module (.mod)",
        }
    }

    /// All audio formats that can be encoded by FFmpeg (encode-capable subset).
    #[allow(dead_code)]
    pub fn encodable_formats() -> &'static [AudioFormat] {
        &[
            AudioFormat::Wav,
            AudioFormat::Aif,
            AudioFormat::Aiff,
            AudioFormat::Bwf,
            AudioFormat::Pcm,
            AudioFormat::Flac,
            AudioFormat::M4aAlac,
            AudioFormat::Wv,
            AudioFormat::Ape,
            AudioFormat::Tta,
            AudioFormat::Mp3,
            AudioFormat::M4aAac,
            AudioFormat::Aac,
            AudioFormat::Ogg,
            AudioFormat::Opus,
            AudioFormat::Wma,
            AudioFormat::Amr,
            AudioFormat::Mp2,
            AudioFormat::Ac3,
            AudioFormat::Ec3,
            AudioFormat::Dts,
        ]
    }
}

impl fmt::Display for AudioFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file_extension())
    }
}

impl FromStr for AudioFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "wav" => Ok(AudioFormat::Wav),
            "aif" => Ok(AudioFormat::Aif),
            "aiff" => Ok(AudioFormat::Aiff),
            "bwf" => Ok(AudioFormat::Bwf),
            "pcm" => Ok(AudioFormat::Pcm),
            "flac" => Ok(AudioFormat::Flac),
            "m4a_alac" | "alac" => Ok(AudioFormat::M4aAlac),
            "wv" | "wavpack" => Ok(AudioFormat::Wv),
            "ape" => Ok(AudioFormat::Ape),
            "tta" => Ok(AudioFormat::Tta),
            "dsf" => Ok(AudioFormat::Dsf),
            "dff" => Ok(AudioFormat::Dff),
            "shn" => Ok(AudioFormat::Shn),
            "mp3" => Ok(AudioFormat::Mp3),
            "m4a" | "m4a_aac" => Ok(AudioFormat::M4aAac),
            "aac" => Ok(AudioFormat::Aac),
            "ogg" => Ok(AudioFormat::Ogg),
            "opus" => Ok(AudioFormat::Opus),
            "wma" => Ok(AudioFormat::Wma),
            "amr" => Ok(AudioFormat::Amr),
            "mp2" => Ok(AudioFormat::Mp2),
            "ra" => Ok(AudioFormat::Ra),
            "oma" => Ok(AudioFormat::Oma),
            "ac3" => Ok(AudioFormat::Ac3),
            "ec3" | "eac3" => Ok(AudioFormat::Ec3),
            "thd" => Ok(AudioFormat::Thd),
            "dts" => Ok(AudioFormat::Dts),
            "dtshd" => Ok(AudioFormat::DtsHd),
            "mid" | "midi" => Ok(AudioFormat::Mid),
            "mod" => Ok(AudioFormat::Mod),
            _ => anyhow::bail!(
                "'{}' is not a known audio format. \
                 Supported audio output formats: wav, aif, aiff, bwf, pcm, flac, m4a, m4a_alac, \
                 wv, ape, tta, dsf, dff, shn, mp3, aac, ogg, opus, wma, amr, mp2, ra, oma, \
                 ac3, ec3, thd, dts, dtshd, mid, midi, mod",
                s
            ),
        }
    }
}

/// Return `true` when the file-path extension identifies an audio file that
/// renderflow can process as an audio input.
pub fn is_audio_path(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    AudioFormat::from_extension(ext).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── from_extension ────────────────────────────────────────────────────────

    #[test]
    fn test_from_extension_wav() {
        assert_eq!(AudioFormat::from_extension("wav"), Some(AudioFormat::Wav));
    }

    #[test]
    fn test_from_extension_mp3() {
        assert_eq!(AudioFormat::from_extension("mp3"), Some(AudioFormat::Mp3));
    }

    #[test]
    fn test_from_extension_flac() {
        assert_eq!(AudioFormat::from_extension("flac"), Some(AudioFormat::Flac));
    }

    #[test]
    fn test_from_extension_opus() {
        assert_eq!(AudioFormat::from_extension("opus"), Some(AudioFormat::Opus));
    }

    #[test]
    fn test_from_extension_m4a_defaults_to_aac() {
        assert_eq!(AudioFormat::from_extension("m4a"), Some(AudioFormat::M4aAac));
    }

    #[test]
    fn test_from_extension_case_insensitive() {
        assert_eq!(AudioFormat::from_extension("WAV"), Some(AudioFormat::Wav));
        assert_eq!(AudioFormat::from_extension("MP3"), Some(AudioFormat::Mp3));
        assert_eq!(AudioFormat::from_extension("FLAC"), Some(AudioFormat::Flac));
    }

    #[test]
    fn test_from_extension_unknown_returns_none() {
        assert_eq!(AudioFormat::from_extension("xyz"), None);
        assert_eq!(AudioFormat::from_extension("pdf"), None);
        assert_eq!(AudioFormat::from_extension("md"), None);
    }

    // ── file_extension ────────────────────────────────────────────────────────

    #[test]
    fn test_file_extension_mp3() {
        assert_eq!(AudioFormat::Mp3.file_extension(), "mp3");
    }

    #[test]
    fn test_file_extension_bwf_uses_wav() {
        assert_eq!(AudioFormat::Bwf.file_extension(), "wav");
    }

    #[test]
    fn test_file_extension_m4a_alac() {
        assert_eq!(AudioFormat::M4aAlac.file_extension(), "m4a");
    }

    #[test]
    fn test_file_extension_m4a_aac() {
        assert_eq!(AudioFormat::M4aAac.file_extension(), "m4a");
    }

    // ── ffmpeg_codec ──────────────────────────────────────────────────────────

    #[test]
    fn test_ffmpeg_codec_mp3() {
        assert_eq!(AudioFormat::Mp3.ffmpeg_codec(), Some("libmp3lame"));
    }

    #[test]
    fn test_ffmpeg_codec_flac() {
        assert_eq!(AudioFormat::Flac.ffmpeg_codec(), Some("flac"));
    }

    #[test]
    fn test_ffmpeg_codec_dsf_is_none() {
        assert_eq!(AudioFormat::Dsf.ffmpeg_codec(), None);
    }

    #[test]
    fn test_ffmpeg_codec_ra_is_none() {
        assert_eq!(AudioFormat::Ra.ffmpeg_codec(), None);
    }

    // ── is_lossless ───────────────────────────────────────────────────────────

    #[test]
    fn test_wav_is_lossless() {
        assert!(AudioFormat::Wav.is_lossless());
    }

    #[test]
    fn test_flac_is_lossless() {
        assert!(AudioFormat::Flac.is_lossless());
    }

    #[test]
    fn test_mp3_is_not_lossless() {
        assert!(!AudioFormat::Mp3.is_lossless());
    }

    #[test]
    fn test_aac_is_not_lossless() {
        assert!(!AudioFormat::Aac.is_lossless());
    }

    // ── supports_encoding ─────────────────────────────────────────────────────

    #[test]
    fn test_mp3_supports_encoding() {
        assert!(AudioFormat::Mp3.supports_encoding());
    }

    #[test]
    fn test_ra_does_not_support_encoding() {
        assert!(!AudioFormat::Ra.supports_encoding());
    }

    #[test]
    fn test_dsf_does_not_support_encoding() {
        assert!(!AudioFormat::Dsf.supports_encoding());
    }

    // ── is_audio_path ─────────────────────────────────────────────────────────

    #[test]
    fn test_is_audio_path_wav() {
        assert!(is_audio_path("recording.wav"));
    }

    #[test]
    fn test_is_audio_path_mp3() {
        assert!(is_audio_path("/home/user/music/track.mp3"));
    }

    #[test]
    fn test_is_audio_path_flac_uppercase() {
        assert!(is_audio_path("song.FLAC"));
    }

    #[test]
    fn test_is_audio_path_rejects_markdown() {
        assert!(!is_audio_path("document.md"));
    }

    #[test]
    fn test_is_audio_path_rejects_pdf() {
        assert!(!is_audio_path("output.pdf"));
    }

    // ── FromStr ───────────────────────────────────────────────────────────────

    #[test]
    fn test_from_str_wav() {
        assert_eq!("wav".parse::<AudioFormat>().unwrap(), AudioFormat::Wav);
    }

    #[test]
    fn test_from_str_mp3() {
        assert_eq!("mp3".parse::<AudioFormat>().unwrap(), AudioFormat::Mp3);
    }

    #[test]
    fn test_from_str_m4a_alac() {
        assert_eq!("m4a_alac".parse::<AudioFormat>().unwrap(), AudioFormat::M4aAlac);
        assert_eq!("alac".parse::<AudioFormat>().unwrap(), AudioFormat::M4aAlac);
    }

    #[test]
    fn test_from_str_unknown_returns_error() {
        let result = "xyz".parse::<AudioFormat>();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("not a known audio format"), "unexpected: {msg}");
    }

    // ── Display ───────────────────────────────────────────────────────────────

    #[test]
    fn test_display_mp3() {
        assert_eq!(AudioFormat::Mp3.to_string(), "mp3");
    }

    #[test]
    fn test_display_flac() {
        assert_eq!(AudioFormat::Flac.to_string(), "flac");
    }

    // ── encodable_formats ─────────────────────────────────────────────────────

    #[test]
    fn test_encodable_formats_contains_mp3_and_flac() {
        let encodable = AudioFormat::encodable_formats();
        assert!(encodable.contains(&AudioFormat::Mp3));
        assert!(encodable.contains(&AudioFormat::Flac));
    }

    #[test]
    fn test_encodable_formats_does_not_contain_ra() {
        let encodable = AudioFormat::encodable_formats();
        assert!(!encodable.contains(&AudioFormat::Ra));
    }
}
