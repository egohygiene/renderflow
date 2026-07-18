use super::format::AudioFormat;

/// Named industry-standard audio output quality profiles.
///
/// Each profile maps to a specific combination of bitrate, sample-rate,
/// bit-depth and codec options, chosen to match a recognised professional
/// or consumer delivery standard.
///
/// Profiles are optional; when no profile is specified the strategy falls back
/// to [`AudioProfile::Default`], which selects sensible quality settings for
/// the target format.
///
/// # YAML configuration example
///
/// ```yaml
/// outputs:
///   - type: mp3
///     profile: streaming_128k
///   - type: mp3
///     profile: hq_320k
///   - type: wav
///     profile: broadcast
///   - type: flac
///     profile: lossless
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioProfile {
    /// Format-appropriate defaults: maximum quality within the codec's
    /// typical range (e.g. VBR V0 for MP3, highest FLAC compression, etc.).
    Default,

    // ── MP3 variants ─────────────────────────────────────────────────────────
    /// MP3 at 64 kbps CBR — voice / low-bandwidth streaming.
    Mp3_64k,
    /// MP3 at 96 kbps CBR — acceptable stereo streaming.
    Mp3_96k,
    /// MP3 at 128 kbps CBR — standard streaming quality.
    Mp3_128k,
    /// MP3 at 192 kbps CBR — good stereo quality.
    Mp3_192k,
    /// MP3 at 256 kbps CBR — high-quality streaming.
    Mp3_256k,
    /// MP3 at 320 kbps CBR — maximum CBR, near-lossless for most listeners.
    Mp3_320k,
    /// MP3 VBR V0 (~245 kbps avg) — best variable-bitrate quality.
    Mp3VbrV0,
    /// MP3 VBR V2 (~190 kbps avg) — transparent VBR quality.
    Mp3VbrV2,

    // ── AAC / M4A variants ───────────────────────────────────────────────────
    /// AAC at 96 kbps — podcast / voice streaming.
    Aac96k,
    /// AAC at 128 kbps — standard streaming quality.
    Aac128k,
    /// AAC at 192 kbps — high-quality streaming.
    Aac192k,
    /// AAC at 256 kbps — Apple Music / iTunes Plus quality.
    Aac256k,
    /// AAC at 320 kbps — maximum AAC CBR quality.
    Aac320k,

    // ── Ogg Vorbis variants ──────────────────────────────────────────────────
    /// Ogg Vorbis quality 1 (~80 kbps) — low bitrate / voice.
    OggQ1,
    /// Ogg Vorbis quality 3 (~112 kbps) — acceptable music quality.
    OggQ3,
    /// Ogg Vorbis quality 6 (~192 kbps) — high-quality stereo.
    OggQ6,
    /// Ogg Vorbis quality 8 (~256 kbps) — near-lossless.
    OggQ8,
    /// Ogg Vorbis quality 10 (~500 kbps) — archival / maximum VBR quality.
    OggQ10,

    // ── Opus variants ────────────────────────────────────────────────────────
    /// Opus at 32 kbps — voice / low-bandwidth.
    Opus32k,
    /// Opus at 64 kbps — good voice quality.
    Opus64k,
    /// Opus at 96 kbps — transparent voice, acceptable music.
    Opus96k,
    /// Opus at 128 kbps — transparent music quality.
    Opus128k,
    /// Opus at 192 kbps — high-quality music.
    Opus192k,
    /// Opus at 256 kbps — near-lossless music.
    Opus256k,
    /// Opus at 510 kbps — maximum bitrate.
    Opus510k,

    // ── WAV / PCM sample-rate and bit-depth presets ──────────────────────────
    /// WAV 8-bit 8 kHz mono — telephony / maximum compression.
    WavTelephony,
    /// WAV 16-bit 44.1 kHz stereo — standard CD-quality.
    WavCdQuality,
    /// WAV 16-bit 48 kHz stereo — standard digital video / broadcast.
    WavDvQuality,
    /// WAV 24-bit 48 kHz stereo — broadcast / professional production.
    WavBroadcast,
    /// WAV 24-bit 96 kHz stereo — high-resolution audio (studio mastering).
    WavHiRes96k,
    /// WAV 32-bit float 96 kHz stereo — floating-point studio master.
    WavFloat96k,
    /// WAV 24-bit 192 kHz stereo — ultra-high-resolution mastering.
    WavHiRes192k,

    // ── FLAC variants ────────────────────────────────────────────────────────
    /// FLAC compression level 0 — fastest encode, largest file.
    FlacLevel0,
    /// FLAC compression level 5 (default) — balanced speed and size.
    FlacLevel5,
    /// FLAC compression level 8 — maximum compression, smallest file.
    FlacLevel8,
    /// FLAC 24-bit 48 kHz — high-resolution broadcast-grade lossless.
    FlacBroadcast,
    /// FLAC 24-bit 96 kHz — high-resolution studio-grade lossless.
    FlacHiRes,

    // ── ALAC variants ────────────────────────────────────────────────────────
    /// Apple Lossless 16-bit 44.1 kHz — CD lossless in M4A container.
    AlacCdQuality,
    /// Apple Lossless 24-bit 48 kHz — broadcast lossless in M4A container.
    AlacBroadcast,
    /// Apple Lossless 24-bit 96 kHz — hi-res lossless in M4A container.
    AlacHiRes,

    // ── Dolby Digital / DTS cinema presets ──────────────────────────────────
    /// Dolby Digital AC-3 5.1 at 448 kbps — DVD standard surround.
    Ac3DvdSurround,
    /// Dolby Digital AC-3 5.1 at 640 kbps — maximum Blu-ray AC-3.
    Ac3BluRay,
    /// Dolby Digital Plus E-AC-3 5.1 at 1.5 Mbps — streaming surround.
    Ec3Streaming,
    /// DTS 5.1 at 1509 kbps (DTS core) — cinema / disc standard.
    DtsCinema,
}

/// A resolved set of FFmpeg options produced by a profile for a given format.
#[derive(Debug, Clone, Default)]
pub struct ProfileOptions {
    /// Codec name passed to `-acodec` (overrides the format default when set).
    pub codec: Option<String>,
    /// Constant bitrate in bits per second, e.g. `128000`.
    pub bitrate: Option<u32>,
    /// VBR quality level, codec-specific string (e.g. `"0"` for lame VBR V0).
    pub vbr_quality: Option<String>,
    /// Output sample rate in Hz, e.g. `44100`.
    pub sample_rate: Option<u32>,
    /// PCM bit depth, e.g. `16` or `24`.  Only relevant for PCM-based codecs.
    pub bit_depth: Option<u8>,
    /// FLAC compression level (0–8).
    pub flac_compression: Option<u8>,
    /// Number of audio channels.
    pub channels: Option<u8>,
    /// Additional raw FFmpeg options appended verbatim as key=value pairs.
    pub extra_args: Vec<(String, String)>,
}

impl ProfileOptions {
    /// Convert the resolved options into FFmpeg command-line arguments.
    ///
    /// Returns a flat list of argument strings ready for `run_command`.
    pub fn to_ffmpeg_args(&self, format: AudioFormat) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();

        // Codec — either from the profile or from the format default.
        let codec = self.codec.as_deref().or_else(|| format.ffmpeg_codec());
        if let Some(c) = codec {
            // For PCM bit-depth variants, override the codec name.
            let resolved_codec = if let Some(depth) = self.bit_depth {
                match (c, depth) {
                    ("pcm_s16le", 8) => "pcm_u8",
                    ("pcm_s16le", 16) => "pcm_s16le",
                    ("pcm_s16le", 24) => "pcm_s24le",
                    ("pcm_s16le", 32) => "pcm_f32le",
                    _ => c,
                }
            } else {
                c
            };
            args.extend(["-acodec".to_string(), resolved_codec.to_string()]);
        }

        // Bitrate.
        if let Some(br) = self.bitrate {
            args.extend(["-b:a".to_string(), format!("{}k", br / 1000)]);
        }

        // VBR quality.
        if let Some(ref q) = self.vbr_quality {
            // -q:a is the universal VBR quality flag for most codecs.
            args.extend(["-q:a".to_string(), q.clone()]);
        }

        // Sample rate.
        if let Some(sr) = self.sample_rate {
            args.extend(["-ar".to_string(), sr.to_string()]);
        }

        // Channels.
        if let Some(ch) = self.channels {
            args.extend(["-ac".to_string(), ch.to_string()]);
        }

        // FLAC compression level.
        if let Some(level) = self.flac_compression {
            args.extend(["-compression_level".to_string(), level.to_string()]);
        }

        // Extra codec-specific options.  Each (key, value) pair becomes two
        // separate arguments so that FFmpeg receives them as `-key value`.
        for (k, v) in &self.extra_args {
            args.push(format!("-{}", k));
            args.push(v.clone());
        }

        args
    }
}

impl AudioProfile {
    /// Parse a profile identifier from the string used in YAML configuration.
    ///
    /// Returns `None` when the string does not match a known profile; the
    /// caller should treat `None` as [`AudioProfile::Default`].
    pub fn from_config_str(s: &str) -> Option<Self> {
        match s.to_lowercase().replace('-', "_").as_str() {
            // MP3
            "mp3_64k" => Some(AudioProfile::Mp3_64k),
            "mp3_96k" => Some(AudioProfile::Mp3_96k),
            "128k" | "mp3_128k" | "streaming" | "streaming_128k" => Some(AudioProfile::Mp3_128k),
            "192k" | "mp3_192k" => Some(AudioProfile::Mp3_192k),
            "256k" | "mp3_256k" => Some(AudioProfile::Mp3_256k),
            "320k" | "mp3_320k" | "hq_320k" | "hq" => Some(AudioProfile::Mp3_320k),
            "vbr_v0" | "mp3_vbr_v0" => Some(AudioProfile::Mp3VbrV0),
            "vbr_v2" | "mp3_vbr_v2" => Some(AudioProfile::Mp3VbrV2),
            // AAC
            "aac_96k" | "96k" => Some(AudioProfile::Aac96k),
            "aac_128k" => Some(AudioProfile::Aac128k),
            "aac_192k" => Some(AudioProfile::Aac192k),
            "aac_256k" | "itunes_plus" => Some(AudioProfile::Aac256k),
            "aac_320k" => Some(AudioProfile::Aac320k),
            // Ogg Vorbis
            "ogg_q1" | "q1" => Some(AudioProfile::OggQ1),
            "ogg_q3" | "q3" => Some(AudioProfile::OggQ3),
            "ogg_q6" | "q6" => Some(AudioProfile::OggQ6),
            "ogg_q8" | "q8" => Some(AudioProfile::OggQ8),
            "ogg_q10" | "q10" => Some(AudioProfile::OggQ10),
            // Opus
            "opus_32k" => Some(AudioProfile::Opus32k),
            "opus_64k" => Some(AudioProfile::Opus64k),
            "opus_96k" => Some(AudioProfile::Opus96k),
            "opus_128k" => Some(AudioProfile::Opus128k),
            "opus_192k" => Some(AudioProfile::Opus192k),
            "opus_256k" => Some(AudioProfile::Opus256k),
            "opus_510k" | "opus_max" => Some(AudioProfile::Opus510k),
            // WAV
            "telephony" | "wav_telephony" => Some(AudioProfile::WavTelephony),
            "cd" | "cd_quality" | "wav_cd" | "wav_16bit_44100" | "16bit_44100" => {
                Some(AudioProfile::WavCdQuality)
            }
            "dv" | "dv_quality" | "wav_dv" | "wav_16bit_48000" | "16bit_48000" => {
                Some(AudioProfile::WavDvQuality)
            }
            "broadcast" | "wav_broadcast" | "wav_24bit_48000" | "24bit_48000" => {
                Some(AudioProfile::WavBroadcast)
            }
            "hires_96k" | "wav_hires" | "wav_hires_96k" | "wav_24bit_96000" | "24bit_96000" => {
                Some(AudioProfile::WavHiRes96k)
            }
            "float_96k" | "wav_float" | "wav_float_96k" | "32bit_float_96000" => {
                Some(AudioProfile::WavFloat96k)
            }
            "hires_192k" | "wav_hires_192k" | "wav_24bit_192000" | "24bit_192000" => {
                Some(AudioProfile::WavHiRes192k)
            }
            // FLAC
            "flac_0" | "flac_fast" => Some(AudioProfile::FlacLevel0),
            "flac_5" | "flac_default" => Some(AudioProfile::FlacLevel5),
            "flac_8" | "flac_max" => Some(AudioProfile::FlacLevel8),
            "flac_broadcast" | "flac_24bit_48k" => Some(AudioProfile::FlacBroadcast),
            "flac_hires" | "flac_24bit_96k" | "lossless" => Some(AudioProfile::FlacHiRes),
            // ALAC
            "alac_cd" | "alac_16bit_44100" => Some(AudioProfile::AlacCdQuality),
            "alac_broadcast" | "alac_24bit_48k" => Some(AudioProfile::AlacBroadcast),
            "alac_hires" | "alac_24bit_96k" => Some(AudioProfile::AlacHiRes),
            // Dolby / DTS
            "ac3_dvd" | "ac3_surround" => Some(AudioProfile::Ac3DvdSurround),
            "ac3_bluray" | "ac3_640k" => Some(AudioProfile::Ac3BluRay),
            "ec3_streaming" => Some(AudioProfile::Ec3Streaming),
            "dts_cinema" | "dts_core" => Some(AudioProfile::DtsCinema),
            _ => None,
        }
    }

    /// Resolve this profile to a set of FFmpeg options for the given format.
    ///
    /// Returns [`ProfileOptions::default`] when the profile has no specific
    /// options for the target format (i.e. use codec defaults).
    pub fn resolve(&self, format: AudioFormat) -> ProfileOptions {
        match self {
            AudioProfile::Default => Self::default_options(format),

            // ── MP3 ───────────────────────────────────────────────────────────
            AudioProfile::Mp3_64k => ProfileOptions {
                bitrate: Some(64_000),
                sample_rate: Some(44100),
                ..Default::default()
            },
            AudioProfile::Mp3_96k => ProfileOptions {
                bitrate: Some(96_000),
                sample_rate: Some(44100),
                ..Default::default()
            },
            AudioProfile::Mp3_128k => ProfileOptions {
                bitrate: Some(128_000),
                sample_rate: Some(44100),
                ..Default::default()
            },
            AudioProfile::Mp3_192k => ProfileOptions {
                bitrate: Some(192_000),
                sample_rate: Some(44100),
                ..Default::default()
            },
            AudioProfile::Mp3_256k => ProfileOptions {
                bitrate: Some(256_000),
                sample_rate: Some(44100),
                ..Default::default()
            },
            AudioProfile::Mp3_320k => ProfileOptions {
                bitrate: Some(320_000),
                sample_rate: Some(44100),
                ..Default::default()
            },
            AudioProfile::Mp3VbrV0 => ProfileOptions {
                vbr_quality: Some("0".to_string()),
                sample_rate: Some(44100),
                ..Default::default()
            },
            AudioProfile::Mp3VbrV2 => ProfileOptions {
                vbr_quality: Some("2".to_string()),
                sample_rate: Some(44100),
                ..Default::default()
            },

            // ── AAC ───────────────────────────────────────────────────────────
            AudioProfile::Aac96k => ProfileOptions {
                bitrate: Some(96_000),
                ..Default::default()
            },
            AudioProfile::Aac128k => ProfileOptions {
                bitrate: Some(128_000),
                ..Default::default()
            },
            AudioProfile::Aac192k => ProfileOptions {
                bitrate: Some(192_000),
                ..Default::default()
            },
            AudioProfile::Aac256k => ProfileOptions {
                bitrate: Some(256_000),
                ..Default::default()
            },
            AudioProfile::Aac320k => ProfileOptions {
                bitrate: Some(320_000),
                ..Default::default()
            },

            // ── Ogg Vorbis ────────────────────────────────────────────────────
            AudioProfile::OggQ1 => ProfileOptions {
                vbr_quality: Some("1".to_string()),
                ..Default::default()
            },
            AudioProfile::OggQ3 => ProfileOptions {
                vbr_quality: Some("3".to_string()),
                ..Default::default()
            },
            AudioProfile::OggQ6 => ProfileOptions {
                vbr_quality: Some("6".to_string()),
                ..Default::default()
            },
            AudioProfile::OggQ8 => ProfileOptions {
                vbr_quality: Some("8".to_string()),
                ..Default::default()
            },
            AudioProfile::OggQ10 => ProfileOptions {
                vbr_quality: Some("10".to_string()),
                ..Default::default()
            },

            // ── Opus ──────────────────────────────────────────────────────────
            AudioProfile::Opus32k => ProfileOptions {
                bitrate: Some(32_000),
                ..Default::default()
            },
            AudioProfile::Opus64k => ProfileOptions {
                bitrate: Some(64_000),
                ..Default::default()
            },
            AudioProfile::Opus96k => ProfileOptions {
                bitrate: Some(96_000),
                ..Default::default()
            },
            AudioProfile::Opus128k => ProfileOptions {
                bitrate: Some(128_000),
                ..Default::default()
            },
            AudioProfile::Opus192k => ProfileOptions {
                bitrate: Some(192_000),
                ..Default::default()
            },
            AudioProfile::Opus256k => ProfileOptions {
                bitrate: Some(256_000),
                ..Default::default()
            },
            AudioProfile::Opus510k => ProfileOptions {
                bitrate: Some(510_000),
                ..Default::default()
            },

            // ── WAV ───────────────────────────────────────────────────────────
            AudioProfile::WavTelephony => ProfileOptions {
                codec: Some("pcm_u8".to_string()),
                sample_rate: Some(8000),
                channels: Some(1),
                ..Default::default()
            },
            AudioProfile::WavCdQuality => ProfileOptions {
                codec: Some("pcm_s16le".to_string()),
                sample_rate: Some(44100),
                channels: Some(2),
                ..Default::default()
            },
            AudioProfile::WavDvQuality => ProfileOptions {
                codec: Some("pcm_s16le".to_string()),
                sample_rate: Some(48000),
                channels: Some(2),
                ..Default::default()
            },
            AudioProfile::WavBroadcast => ProfileOptions {
                codec: Some("pcm_s24le".to_string()),
                sample_rate: Some(48000),
                channels: Some(2),
                ..Default::default()
            },
            AudioProfile::WavHiRes96k => ProfileOptions {
                codec: Some("pcm_s24le".to_string()),
                sample_rate: Some(96000),
                channels: Some(2),
                ..Default::default()
            },
            AudioProfile::WavFloat96k => ProfileOptions {
                codec: Some("pcm_f32le".to_string()),
                sample_rate: Some(96000),
                channels: Some(2),
                ..Default::default()
            },
            AudioProfile::WavHiRes192k => ProfileOptions {
                codec: Some("pcm_s24le".to_string()),
                sample_rate: Some(192000),
                channels: Some(2),
                ..Default::default()
            },

            // ── FLAC ──────────────────────────────────────────────────────────
            AudioProfile::FlacLevel0 => ProfileOptions {
                flac_compression: Some(0),
                ..Default::default()
            },
            AudioProfile::FlacLevel5 => ProfileOptions {
                flac_compression: Some(5),
                ..Default::default()
            },
            AudioProfile::FlacLevel8 => ProfileOptions {
                flac_compression: Some(8),
                ..Default::default()
            },
            AudioProfile::FlacBroadcast => ProfileOptions {
                flac_compression: Some(8),
                sample_rate: Some(48000),
                bit_depth: Some(24),
                extra_args: vec![("sample_fmt".to_string(), "s32".to_string())],
                ..Default::default()
            },
            AudioProfile::FlacHiRes => ProfileOptions {
                flac_compression: Some(8),
                sample_rate: Some(96000),
                bit_depth: Some(24),
                extra_args: vec![("sample_fmt".to_string(), "s32".to_string())],
                ..Default::default()
            },

            // ── ALAC ──────────────────────────────────────────────────────────
            AudioProfile::AlacCdQuality => ProfileOptions {
                codec: Some("alac".to_string()),
                sample_rate: Some(44100),
                bit_depth: Some(16),
                ..Default::default()
            },
            AudioProfile::AlacBroadcast => ProfileOptions {
                codec: Some("alac".to_string()),
                sample_rate: Some(48000),
                bit_depth: Some(24),
                ..Default::default()
            },
            AudioProfile::AlacHiRes => ProfileOptions {
                codec: Some("alac".to_string()),
                sample_rate: Some(96000),
                bit_depth: Some(24),
                ..Default::default()
            },

            // ── Dolby / DTS ───────────────────────────────────────────────────
            AudioProfile::Ac3DvdSurround => ProfileOptions {
                bitrate: Some(448_000),
                channels: Some(6),
                ..Default::default()
            },
            AudioProfile::Ac3BluRay => ProfileOptions {
                bitrate: Some(640_000),
                channels: Some(6),
                ..Default::default()
            },
            AudioProfile::Ec3Streaming => ProfileOptions {
                bitrate: Some(1_500_000),
                channels: Some(6),
                ..Default::default()
            },
            AudioProfile::DtsCinema => ProfileOptions {
                bitrate: Some(1_509_000),
                channels: Some(6),
                ..Default::default()
            },
        }
    }

    /// Return the default [`ProfileOptions`] for a given format when no
    /// explicit profile is specified.
    fn default_options(format: AudioFormat) -> ProfileOptions {
        match format {
            // MP3 defaults: VBR V2 (~190 kbps) — high-quality transparent encode.
            AudioFormat::Mp3 => ProfileOptions {
                vbr_quality: Some("2".to_string()),
                sample_rate: Some(44100),
                ..Default::default()
            },
            // AAC default: 192 kbps — good balance of size and quality.
            AudioFormat::M4aAac | AudioFormat::Aac => ProfileOptions {
                bitrate: Some(192_000),
                ..Default::default()
            },
            // Ogg Vorbis default: quality 6 (~192 kbps).
            AudioFormat::Ogg => ProfileOptions {
                vbr_quality: Some("6".to_string()),
                ..Default::default()
            },
            // Opus default: 128 kbps — transparent quality for most content.
            AudioFormat::Opus => ProfileOptions {
                bitrate: Some(128_000),
                ..Default::default()
            },
            // WMA default: 192 kbps.
            AudioFormat::Wma => ProfileOptions {
                bitrate: Some(192_000),
                ..Default::default()
            },
            // MP2 default: 192 kbps (broadcast standard).
            AudioFormat::Mp2 => ProfileOptions {
                bitrate: Some(192_000),
                sample_rate: Some(48000),
                ..Default::default()
            },
            // WAV default: 16-bit 44.1 kHz — CD quality.
            AudioFormat::Wav | AudioFormat::Bwf => ProfileOptions {
                codec: Some("pcm_s16le".to_string()),
                sample_rate: Some(44100),
                ..Default::default()
            },
            // AIFF default: 16-bit 44.1 kHz. Both Aif (.aif) and Aiff (.aiff) are
            // the same format — variants exist to preserve the original file extension.
            AudioFormat::Aif | AudioFormat::Aiff => ProfileOptions {
                codec: Some("pcm_s16le".to_string()),
                sample_rate: Some(44100),
                ..Default::default()
            },
            // FLAC default: compression level 5.
            AudioFormat::Flac => ProfileOptions {
                flac_compression: Some(5),
                ..Default::default()
            },
            // ALAC default: 16-bit 44.1 kHz.
            AudioFormat::M4aAlac => ProfileOptions {
                codec: Some("alac".to_string()),
                sample_rate: Some(44100),
                ..Default::default()
            },
            // AMR default: 12.2 kbps narrow-band (standard voice).
            AudioFormat::Amr => ProfileOptions {
                bitrate: Some(12_200),
                sample_rate: Some(8000),
                channels: Some(1),
                ..Default::default()
            },
            // AC-3 default: 5.1 surround at 448 kbps.
            AudioFormat::Ac3 => ProfileOptions {
                bitrate: Some(448_000),
                channels: Some(6),
                ..Default::default()
            },
            // E-AC-3 default: 1 Mbps.
            AudioFormat::Ec3 => ProfileOptions {
                bitrate: Some(1_000_000),
                channels: Some(6),
                ..Default::default()
            },
            // DTS default: core at 1509 kbps.
            AudioFormat::Dts => ProfileOptions {
                bitrate: Some(1_509_000),
                channels: Some(6),
                ..Default::default()
            },
            // All other formats: let FFmpeg use its built-in defaults.
            _ => ProfileOptions::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── from_config_str ───────────────────────────────────────────────────────

    #[test]
    fn test_from_config_str_128k_is_mp3_streaming() {
        assert_eq!(
            AudioProfile::from_config_str("128k"),
            Some(AudioProfile::Mp3_128k)
        );
    }

    #[test]
    fn test_from_config_str_320k() {
        assert_eq!(
            AudioProfile::from_config_str("320k"),
            Some(AudioProfile::Mp3_320k)
        );
    }

    #[test]
    fn test_from_config_str_broadcast() {
        assert_eq!(
            AudioProfile::from_config_str("broadcast"),
            Some(AudioProfile::WavBroadcast)
        );
    }

    #[test]
    fn test_from_config_str_lossless() {
        assert_eq!(
            AudioProfile::from_config_str("lossless"),
            Some(AudioProfile::FlacHiRes)
        );
    }

    #[test]
    fn test_from_config_str_cd_quality() {
        assert_eq!(
            AudioProfile::from_config_str("cd_quality"),
            Some(AudioProfile::WavCdQuality)
        );
    }

    #[test]
    fn test_from_config_str_itunes_plus() {
        assert_eq!(
            AudioProfile::from_config_str("itunes_plus"),
            Some(AudioProfile::Aac256k)
        );
    }

    #[test]
    fn test_from_config_str_unknown_returns_none() {
        assert_eq!(AudioProfile::from_config_str("unknown_profile"), None);
    }

    #[test]
    fn test_from_config_str_case_insensitive() {
        assert_eq!(
            AudioProfile::from_config_str("BROADCAST"),
            Some(AudioProfile::WavBroadcast)
        );
    }

    // ── resolve ───────────────────────────────────────────────────────────────

    #[test]
    fn test_mp3_128k_resolve_sets_bitrate() {
        let opts = AudioProfile::Mp3_128k.resolve(AudioFormat::Mp3);
        assert_eq!(opts.bitrate, Some(128_000));
        assert_eq!(opts.sample_rate, Some(44100));
        assert!(opts.vbr_quality.is_none());
    }

    #[test]
    fn test_mp3_320k_resolve_sets_bitrate_320k() {
        let opts = AudioProfile::Mp3_320k.resolve(AudioFormat::Mp3);
        assert_eq!(opts.bitrate, Some(320_000));
    }

    #[test]
    fn test_mp3_vbr_v0_sets_q_not_bitrate() {
        let opts = AudioProfile::Mp3VbrV0.resolve(AudioFormat::Mp3);
        assert!(opts.bitrate.is_none(), "VBR must not set a fixed bitrate");
        assert_eq!(opts.vbr_quality.as_deref(), Some("0"));
    }

    #[test]
    fn test_wav_broadcast_sets_24bit_48k() {
        let opts = AudioProfile::WavBroadcast.resolve(AudioFormat::Wav);
        assert_eq!(opts.codec.as_deref(), Some("pcm_s24le"));
        assert_eq!(opts.sample_rate, Some(48000));
    }

    #[test]
    fn test_flac_level8_sets_compression_level() {
        let opts = AudioProfile::FlacLevel8.resolve(AudioFormat::Flac);
        assert_eq!(opts.flac_compression, Some(8));
    }

    #[test]
    fn test_default_mp3_uses_vbr_v2() {
        let opts = AudioProfile::Default.resolve(AudioFormat::Mp3);
        assert_eq!(opts.vbr_quality.as_deref(), Some("2"));
    }

    #[test]
    fn test_default_opus_uses_128k_bitrate() {
        let opts = AudioProfile::Default.resolve(AudioFormat::Opus);
        assert_eq!(opts.bitrate, Some(128_000));
    }

    // ── ProfileOptions::to_ffmpeg_args ────────────────────────────────────────

    #[test]
    fn test_to_ffmpeg_args_bitrate() {
        let opts = ProfileOptions {
            bitrate: Some(128_000),
            ..Default::default()
        };
        let args = opts.to_ffmpeg_args(AudioFormat::Mp3);
        let joined = args.join(" ");
        assert!(joined.contains("-b:a 128k"), "expected -b:a 128k, got: {joined}");
    }

    #[test]
    fn test_to_ffmpeg_args_sample_rate() {
        let opts = ProfileOptions {
            sample_rate: Some(48000),
            ..Default::default()
        };
        let args = opts.to_ffmpeg_args(AudioFormat::Flac);
        let joined = args.join(" ");
        assert!(joined.contains("-ar 48000"), "expected -ar 48000, got: {joined}");
    }

    #[test]
    fn test_to_ffmpeg_args_vbr_quality() {
        let opts = ProfileOptions {
            vbr_quality: Some("0".to_string()),
            ..Default::default()
        };
        let args = opts.to_ffmpeg_args(AudioFormat::Mp3);
        let joined = args.join(" ");
        assert!(joined.contains("-q:a 0"), "expected -q:a 0, got: {joined}");
    }

    #[test]
    fn test_to_ffmpeg_args_channels() {
        let opts = ProfileOptions {
            channels: Some(2),
            ..Default::default()
        };
        let args = opts.to_ffmpeg_args(AudioFormat::Wav);
        let joined = args.join(" ");
        assert!(joined.contains("-ac 2"), "expected -ac 2, got: {joined}");
    }

    #[test]
    fn test_to_ffmpeg_args_flac_compression() {
        let opts = ProfileOptions {
            flac_compression: Some(8),
            ..Default::default()
        };
        let args = opts.to_ffmpeg_args(AudioFormat::Flac);
        let joined = args.join(" ");
        assert!(
            joined.contains("-compression_level 8"),
            "expected -compression_level 8, got: {joined}"
        );
    }
}
