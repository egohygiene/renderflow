//! Closed FFprobe8.0.1 audio clocks, expressed in integer output-sample ticks.
use serde::{Deserialize, Serialize};

use super::{Failure, MAX_PROBE_BYTES};
use crate::graph::Format;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Probe {
    pub streams: Vec<Stream>,
    pub packets: Vec<Packet>,
    #[serde(default)]
    programs: Vec<serde_json::Value>,
    #[serde(default)]
    stream_groups: Vec<serde_json::Value>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Stream {
    index: u32,
    pub codec_name: String,
    #[serde(default)]
    profile: Option<String>,
    codec_type: String,
    pub sample_rate: String,
    pub channels: u16,
    bits_per_sample: u16,
    #[serde(default)]
    bits_per_raw_sample: Option<String>,
    time_base: String,
    #[serde(default)]
    start_pts: Option<i64>,
    duration_ts: i64,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Packet {
    codec_type: String,
    pts: i64,
    duration: i64,
    #[serde(default)]
    side_data_list: Vec<SideData>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SideData {
    side_data_type: String,
    skip_samples: u64,
    discard_padding: u64,
}
#[derive(Debug, Clone, Serialize)]
pub(super) struct Clock {
    pub rate_hz: u32,
    pub channels: u16,
    pub output_precision_bits: Option<u16>,
    pub stream_duration_samples: u64,
    pub stream_start_sample: i64,
    pub packet_start_sample: i64,
    pub packet_end_sample: i64,
    pub skip_samples: u64,
    pub discard_padding: u64,
    pub presentation_samples: u64,
    pub packet_count: usize,
}

fn invalid(code: &'static str) -> Failure {
    Failure::invalid(code)
}
fn decimal(value: &str, code: &'static str) -> Result<u32, Failure> {
    if value.is_empty() || value.len() > 9 || !value.bytes().all(|v| v.is_ascii_digit()) {
        return Err(invalid(code));
    }
    value.parse().map_err(|_| invalid(code))
}
impl Probe {
    pub fn parse(bytes: &[u8]) -> Result<Self, Failure> {
        let value: Self = crate::evidence::json::object(bytes, MAX_PROBE_BYTES)
            .map_err(|_| invalid("audio.native.probe.json"))?;
        if value.streams.len() != 1
            || !value.programs.is_empty()
            || !value.stream_groups.is_empty()
            || value.packets.is_empty()
            || value.packets.len() > 32_000
        {
            return Err(invalid("audio.native.probe.stream_or_packet_count"));
        }
        Ok(value)
    }
    pub fn clock(&self, format: Format) -> Result<Clock, Failure> {
        let stream = &self.streams[0];
        let codec = match format {
            Format::Flac => "flac",
            Format::M4aAlac => "alac",
            Format::Mp3 => "mp3",
            Format::M4aAac => "aac",
            _ => return Err(Failure::unavailable("audio.native.format_unsupported")),
        };
        let rate = decimal(&stream.sample_rate, "audio.native.probe.rate")?;
        if stream.index != 0
            || stream.codec_type != "audio"
            || stream.codec_name != codec
            || !(8_000..=192_000).contains(&rate)
            || !(1..=8).contains(&stream.channels)
        {
            return Err(invalid("audio.native.probe.codec_or_layout"));
        }
        if format == Format::M4aAac && stream.profile.as_deref() != Some("LC") {
            return Err(Failure::unavailable("audio.native.aac_profile_unsupported"));
        }
        if format == Format::Mp3 && !matches!(rate, 32_000 | 44_100 | 48_000) {
            return Err(Failure::unavailable("audio.native.mp3_profile_unsupported"));
        }
        let (num, den) = stream
            .time_base
            .split_once('/')
            .ok_or(invalid("audio.native.probe.time_base"))?;
        let num = decimal(num, "audio.native.probe.time_base")?;
        let den = decimal(den, "audio.native.probe.time_base")?;
        if num == 0 || den == 0 {
            return Err(invalid("audio.native.probe.time_base"));
        }
        let ticks = |value: i64| -> Result<i64, Failure> {
            let scaled = i128::from(value)
                .checked_mul(i128::from(num))
                .and_then(|v| v.checked_mul(i128::from(rate)))
                .ok_or(invalid("audio.native.probe.clock_overflow"))?;
            if scaled % i128::from(den) != 0 {
                return Err(invalid("audio.native.probe.fractional_sample_clock"));
            }
            let result = i64::try_from(scaled / i128::from(den))
                .map_err(|_| invalid("audio.native.probe.clock_overflow"))?;
            if result.unsigned_abs() > u64::from(rate) * 601 + 2304 {
                return Err(invalid("audio.native.probe.clock_limit"));
            }
            Ok(result)
        };
        let duration = ticks(stream.duration_ts)?;
        if duration <= 0 {
            return Err(invalid("audio.native.probe.duration"));
        }
        let first = ticks(self.packets[0].pts)?;
        let mut end = first;
        let mut skip = 0u64;
        let mut discard = 0u64;
        for (index, packet) in self.packets.iter().enumerate() {
            let start = ticks(packet.pts)?;
            let span = ticks(packet.duration)?;
            if packet.codec_type != "audio" || span <= 0 || start != end {
                return Err(invalid("audio.native.probe.packet_sequence"));
            }
            if format == Format::Mp3 && span != 1152
                || format == Format::M4aAac
                    && (span > 1024 || index + 1 < self.packets.len() && span != 1024)
            {
                return Err(invalid("audio.native.probe.packet_codec_clock"));
            }
            end = start
                .checked_add(span)
                .ok_or(invalid("audio.native.probe.clock_overflow"))?;
            if packet.side_data_list.len() > 1 {
                return Err(invalid("audio.native.probe.skip_data"));
            }
            for side in &packet.side_data_list {
                if side.side_data_type != "Skip Samples"
                    || side.skip_samples > 0 && index != 0
                    || side.discard_padding > 0 && index + 1 != self.packets.len()
                {
                    return Err(invalid("audio.native.probe.skip_data"));
                }
                skip = skip
                    .checked_add(side.skip_samples)
                    .ok_or(invalid("audio.native.probe.clock_overflow"))?;
                discard = discard
                    .checked_add(side.discard_padding)
                    .ok_or(invalid("audio.native.probe.clock_overflow"))?;
            }
        }
        let presentation =
            i128::from(end) - i128::from(first) - i128::from(skip) - i128::from(discard);
        if presentation <= 0 || presentation > i128::from(rate) * 600 {
            return Err(invalid("audio.native.probe.presentation_limit"));
        }
        let stream_start = ticks(stream.start_pts.unwrap_or(0))?;
        let bits = stream
            .bits_per_raw_sample
            .as_deref()
            .map(|v| decimal(v, "audio.native.probe.precision"))
            .transpose()?
            .unwrap_or(u32::from(stream.bits_per_sample));
        let precision = if matches!(format, Format::Flac | Format::M4aAlac) {
            if !matches!(bits, 16 | 24) {
                return Err(Failure::unavailable("audio.native.precision_unsupported"));
            }
            if first != 0
                || skip != 0
                || discard != 0
                || stream_start != 0
                || i128::from(duration) != presentation
            {
                return Err(invalid("audio.native.probe.lossless_clock"));
            }
            Some(bits as u16)
        } else if format == Format::Mp3 {
            if first != 0
                || skip == 0
                || skip > 1152
                || discard >= 1152
                || stream_start < 0
                || stream_start as u64 != skip
                || i128::from(duration) < presentation
                || duration > end
            {
                return Err(invalid("audio.native.probe.mp3_clock"));
            }
            None
        } else {
            if first != -1024
                || skip != 1024
                || discard != 0
                || stream_start != 0
                || i128::from(duration) != presentation
            {
                return Err(invalid("audio.native.probe.aac_clock"));
            }
            None
        };
        Ok(Clock {
            rate_hz: rate,
            channels: stream.channels,
            output_precision_bits: precision,
            stream_duration_samples: duration as u64,
            stream_start_sample: stream_start,
            packet_start_sample: first,
            packet_end_sample: end,
            skip_samples: skip,
            discard_padding: discard,
            presentation_samples: presentation as u64,
            packet_count: self.packets.len(),
        })
    }
}
