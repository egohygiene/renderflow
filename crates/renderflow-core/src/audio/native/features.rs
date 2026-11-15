//! Bounded advertised prerequisites for this profile, not a capability registry.
use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use super::{hash, invalid, request, unavailable, Failure};
use crate::graph::Format;
use crate::process::{ProcessCancellationToken, ProcessExecutor, ProcessOutputMode};
use crate::speech::provider::FileSnapshot;

const MAX_QUERY_BYTES: usize = 128 * 1024;
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Query {
    tool_id: String,
    argv: Vec<String>,
    stdout_sha256: Option<String>,
    stderr_sha256: Option<String>,
    stdout_bytes: usize,
    stderr_bytes: usize,
    capture_complete: bool,
    recognized: bool,
    matched_profile_ids: Vec<String>,
    unavailable_code: Option<&'static str>,
}
#[derive(Debug, Clone)]
pub(super) struct Features {
    queries: Vec<Query>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Required {
    tool_id: &'static str,
    table: &'static str,
    alternatives: Vec<String>,
    matched: Vec<String>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Admission {
    scope: &'static str,
    required: Vec<Required>,
    queries: Vec<Query>,
    admitted: bool,
    unavailable_code: Option<&'static str>,
}
impl Admission {
    pub fn ensure(&self) -> Result<(), Failure> {
        self.unavailable_code
            .map_or(Ok(()), |code| Err(unavailable(code)))
    }
}
fn wanted(table: &str) -> &'static [&'static str] {
    match table {
        "-decoders" => &["flac", "alac", "aac", "mp3", "mp3float"],
        "-demuxers" => &["flac", "mov", "mp3"],
        "-encoders" => &["pcm_s16le", "pcm_s24le"],
        "-muxers" => &["s16le", "s24le"],
        _ => &[],
    }
}
fn names(bytes: &[u8], table: &str) -> Result<Vec<String>, Failure> {
    if bytes.len() > MAX_QUERY_BYTES {
        return Err(unavailable("audio.native.feature_evidence_limit"));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| unavailable("audio.native.feature_evidence_unrecognized"))?;
    let codec = matches!(table, "-decoders" | "-encoders");
    let separator = if codec { "------" } else { "---" };
    let mut header = false;
    let mut matched = BTreeSet::new();
    let mut count = 0;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !header {
            if line == separator {
                header = true;
            }
            continue;
        }
        count += 1;
        if count > 16_384 || line.len() > 4096 {
            return Err(unavailable("audio.native.feature_evidence_limit"));
        }
        let mut fields = line.split_whitespace();
        let flags = fields.next().unwrap_or("");
        let aliases = fields.next().unwrap_or("");
        let valid_flags = if codec {
            flags.len() == 6
                && matches!(flags.as_bytes()[0], b'A' | b'V' | b'S')
                && flags.bytes().all(|v| v == b'.' || v.is_ascii_uppercase())
        } else {
            flags.contains(if table == "-demuxers" { 'D' } else { 'E' })
                && flags
                    .bytes()
                    .all(|v| matches!(v, b'D' | b'E' | b'd' | b'.'))
        };
        if !valid_flags || aliases.is_empty() || aliases.len() > 256 {
            return Err(unavailable("audio.native.feature_evidence_unrecognized"));
        }
        for name in aliases.split(',') {
            if name.is_empty()
                || !name.bytes().all(|v| {
                    v.is_ascii_lowercase() || v.is_ascii_digit() || matches!(v, b'_' | b'-')
                })
            {
                return Err(unavailable("audio.native.feature_evidence_unrecognized"));
            }
            if (!codec || flags.starts_with('A')) && wanted(table).contains(&name) {
                matched.insert(name.to_owned());
            }
        }
    }
    if !header {
        return Err(unavailable("audio.native.feature_evidence_unrecognized"));
    }
    Ok(matched.into_iter().collect())
}
impl Features {
    pub fn observe(
        tool: &FileSnapshot,
        name: &str,
        directory: &Path,
        token: &ProcessCancellationToken,
    ) -> Result<Self, Failure> {
        let tables = if name == "ffmpeg" {
            &["-decoders", "-demuxers", "-encoders", "-muxers"][..]
        } else {
            &["-decoders", "-demuxers"][..]
        };
        let mut queries = Vec::new();
        for table in tables {
            tool.recheck(token)
                .map_err(|_| invalid("audio.native.verifier_changed"))?;
            let argv = ["-hide_banner", "-v", "error", table]
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>();
            let observed = ProcessExecutor::new().execute(
                request(tool.path(), directory, token)?
                    .args(argv.clone())
                    .timeout(Duration::from_secs(5))
                    .stdout(ProcessOutputMode::capture(MAX_QUERY_BYTES))
                    .stderr(ProcessOutputMode::capture(16 * 1024)),
            );
            let mut query = Query {
                tool_id: format!("tool.{name}"),
                argv,
                stdout_sha256: None,
                stderr_sha256: None,
                stdout_bytes: 0,
                stderr_bytes: 0,
                capture_complete: false,
                recognized: false,
                matched_profile_ids: Vec::new(),
                unavailable_code: Some("audio.native.feature_query_unavailable"),
            };
            if let Ok(observed) = observed {
                query.stdout_sha256 = Some(hash(observed.stdout().bytes()));
                query.stderr_sha256 = Some(hash(observed.stderr().bytes()));
                query.stdout_bytes = observed.stdout().bytes().len();
                query.stderr_bytes = observed.stderr().bytes().len();
                query.capture_complete = observed.is_success()
                    && !observed.stdout().truncated()
                    && !observed.stderr().truncated();
                if query.capture_complete {
                    match names(observed.stdout().bytes(), table) {
                        Ok(names) => {
                            query.recognized = true;
                            query.matched_profile_ids = names;
                            query.unavailable_code = None;
                        }
                        Err(failure) => query.unavailable_code = Some(failure.code),
                    }
                }
            }
            tool.recheck(token)
                .map_err(|_| invalid("audio.native.verifier_changed"))?;
            queries.push(query);
        }
        Ok(Self { queries })
    }
    fn require(
        &self,
        tool_id: &'static str,
        table: &'static str,
        alternatives: &[&str],
    ) -> (Required, Option<&'static str>) {
        let query = self
            .queries
            .iter()
            .find(|q| q.argv.last().is_some_and(|arg| arg == table));
        let matched = query
            .filter(|q| q.capture_complete && q.recognized)
            .map(|q| {
                alternatives
                    .iter()
                    .filter(|id| q.matched_profile_ids.iter().any(|actual| actual == *id))
                    .map(|id| (*id).to_owned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let failure = query
            .and_then(|q| q.unavailable_code)
            .or_else(|| matched.is_empty().then_some("audio.native.feature_missing"));
        (
            Required {
                tool_id,
                table,
                alternatives: alternatives.iter().map(|id| (*id).to_owned()).collect(),
                matched,
            },
            failure,
        )
    }
}
pub(super) fn admit(
    ffmpeg: &Features,
    ffprobe: &Features,
    format: Format,
    bits: u16,
) -> Option<Admission> {
    let (demux, decoders) = match format {
        Format::Flac => ("flac", &["flac"][..]),
        Format::M4aAlac => ("mov", &["alac"][..]),
        Format::M4aAac => ("mov", &["aac"][..]),
        Format::Mp3 => ("mp3", &["mp3", "mp3float"][..]),
        _ => return None,
    };
    let (encoder, muxer) = match bits {
        16 => ("pcm_s16le", "s16le"),
        24 => ("pcm_s24le", "s24le"),
        _ => return None,
    };
    let mut required = Vec::new();
    let mut failure = None;
    for (features, tool_id, table, ids) in [
        (ffmpeg, "tool.ffmpeg", "-demuxers", &[demux][..]),
        (ffmpeg, "tool.ffmpeg", "-decoders", decoders),
        (ffmpeg, "tool.ffmpeg", "-encoders", &[encoder][..]),
        (ffmpeg, "tool.ffmpeg", "-muxers", &[muxer][..]),
        (ffprobe, "tool.ffprobe", "-demuxers", &[demux][..]),
        (ffprobe, "tool.ffprobe", "-decoders", decoders),
    ] {
        let (observation, unavailable) = features.require(tool_id, table, ids);
        if failure.is_none() {
            failure = unavailable;
        }
        required.push(observation);
    }
    Some(Admission {scope:"Advertised fixed-profile prerequisites only; actual media is still probed/decoded before acceptance.",required,
        queries:ffmpeg.queries.iter().chain(ffprobe.queries.iter()).cloned().collect(),admitted:failure.is_none(),unavailable_code:failure})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_audio_features_use_exact_audio_names_and_comma_aliases() {
        assert_eq!(names(b"Formats:\n ---\n D  mov,mp4,m4a,3gp aliases\n D  flac_suffix wrong\n D  flac actual\n","-demuxers").unwrap(),["flac","mov"]);
        assert_eq!(names(b"Decoders:\n ------\n A....D flac exact\n V....D aac non_audio\n A....D mp3float exact\n","-decoders").unwrap(),["flac","mp3float"]);
        assert_eq!(
            names(b"no bounded table header", "-decoders").unwrap_err(),
            unavailable("audio.native.feature_evidence_unrecognized")
        );
        assert_eq!(
            names(b"Formats:\n ---\n not-a-table flac bad\n", "-demuxers").unwrap_err(),
            unavailable("audio.native.feature_evidence_unrecognized")
        );
    }
}
