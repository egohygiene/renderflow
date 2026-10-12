# Local audio preparation source evaluation — #428

Status: source inspection only, 2026-10-12 UTC. No FFmpeg/FFprobe executable,
compiler, test, model, or media conversion was run for this checkpoint. This
document explains the bounded preparation contract; it does not qualify a
provider build or certify the resulting media.

## Primary sources

The source snapshot is
[`FFmpeg/FFmpeg` at `615b07170e30d79ad5906418ea49f81c3cbd8457`](https://github.com/FFmpeg/FFmpeg/tree/615b07170e30d79ad5906418ea49f81c3cbd8457),
observed on 2026-10-12. Read at that revision:

- [FFprobe implementation](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/fftools/ffprobe.c)
  and [argument documentation](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/doc/ffprobe.texi).
- [FFmpeg argument documentation](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/doc/ffmpeg.texi),
  [input handling](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/fftools/ffmpeg_demux.c),
  and [output audio filters](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/fftools/ffmpeg_filter.c).
- [MOV demuxer](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/mov.c),
  [Matroska demuxer](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/matroskadec.c),
  [WAV demuxer](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/wavdec.c),
  [FLAC demuxer](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/flacdec.c),
  and [MP3 demuxer](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/mp3dec.c).
- [Timestamp filter](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavfilter/setpts.c),
  [WAV muxer](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/wavenc.c),
  and [RIFF header writer](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/riffenc.c).
- [AVI](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/avidec.c),
  [Ogg](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/oggdec.c),
  and [AAC](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/aacdec.c)
  demuxers, plus the [input format allowlist](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavformat/demux.c)
  and [list matching](https://github.com/FFmpeg/FFmpeg/blob/615b07170e30d79ad5906418ea49f81c3cbd8457/libavutil/avstring.c).

The published [FFprobe](https://ffmpeg.org/ffprobe.html),
[FFmpeg](https://ffmpeg.org/ffmpeg.html),
[formats](https://ffmpeg.org/ffmpeg-formats.html),
[filters](https://ffmpeg.org/ffmpeg-filters.html),
[resampler](https://ffmpeg.org/ffmpeg-resampler.html), and
[protocols](https://ffmpeg.org/ffmpeg-protocols.html) manuals were also consulted.
Those manuals move over time. The snapshot is a source reference, not a claim
that an operator-supplied executable was built from it; record the actual
executable hash and version separately.

## Probe and selection contract

The intended probe requests JSON with these entries:

```text
format=format_name:stream=index,codec_name,codec_type,channels,sample_rate,duration,start_time:stream_tags=language,title
```

`-select_streams a` narrows the reported streams. The printed `index` remains
the original global `AVStream.index`, so a selected index of 2 must become
`-map 0:2`, not `-map 0:a:2`. Selection must remain explicit even if there is
only one audio track. Tags describe the source; they are not reliable language
detection or authority to choose a track automatically.

FFprobe emits `sample_rate` and time fields as strings. JSON can omit optional
unavailable values; other writer settings can expose `N/A`. Missing duration is
unknown, not zero. Known duration is metadata, not a complete decode result.
Reject malformed/nonfinite/out-of-policy values, and keep absent values absent
in evidence. Output field order is not the requested entry order.

Force the selected input demuxer with `-f`, using only the bounded profile's
allowlisted names. The source registers these names:

| Forced name | Reported demuxer name |
| --- | --- |
| `matroska` | `matroska,webm` |
| `mov` | `mov,mp4,m4a,3gp,3g2,mj2` |
| `wav` | `wav` |
| `flac` | `flac` |
| `mp3` | `mp3` |
| `avi` | `avi` |
| `ogg` | `ogg` |
| `aac` | `aac` |

An extension-to-demuxer choice limits the attempted parser; an extension is not
proof of content. A forced-parser mismatch must fail rather than retrying with
an unrestricted parser. The matching `-format_whitelist` permits the selected
demuxer; the inspected matcher handles comma-separated aliases on both sides.
`-show_entries` bounds the selected fields, not their
total bytes. Moreover, FFprobe applies stream selection after opening the
container, finding stream information, and setting up decoders. Capture limits,
stream-count limits, process timeout, and cancellation remain necessary.

## Conversion contract

Use one input, one explicit global audio stream, direct argument execution,
and an isolated, new output file. Options before `-i` configure the input;
output options follow it. Relevant settings:

| Setting | Intended behavior |
| --- | --- |
| `-protocol_whitelist file` | Limit libavformat input protocols to local files. |
| MOV `-enable_drefs 0 -use_absolute_path 0` | Keep external track references disabled; supply these private options only for MOV. |
| `-xerror` | Stop on an error instead of accepting ordinary decode-error recovery. |
| `-map 0:N -vn -sn -dn` | Map the selected audio stream and suppress other media types. |
| `-threads 1` before and after `-i`, `-filter_threads 1` | Request bounded decoder/encoder and simple-filter concurrency. |
| `-ac 1 -ar 16000 -codec:a pcm_s16le` | Downmix/resample to the speech adapter's mono 16 kHz PCM16 profile. |
| `-filter:a aresample=16000:async=0,asetpts=N/SR/TB` | Explicitly resample without timestamp-driven filling/trimming, then regenerate the timeline from decoded sample count. |
| Output `-t 601` | Stop after a sentinel duration beyond the 600-second acceptance limit. |
| `-map_metadata -1 -map_metadata:s:a -1 -map_chapters -1` | Disable source global metadata, stream metadata, and chapter copying. |
| `-f wav -rf64 never` | Write ordinary RIFF/WAV, with no RF64 conversion. |

The native options retain FFmpeg's supported single-dash spelling. These are
requested native settings, not operating-system CPU, memory, disk, network, or
filesystem confinement. In particular, allowing `file` is not a path allowlist;
the forced demuxer and MOV reference settings narrow behavior but do not sandbox
an arbitrary supplied executable. Its shared libraries and build configuration
also affect effective behavior. A thread count is not a total process resource
limit. Suppressing copied metadata does not promise an entirely metadata-free
WAV: the muxer can write its own encoder identification.

With mono 16 kHz PCM16, the inspected RIFF writer takes the conventional PCM
header path rather than WAVEFORMATEXTENSIBLE. The WAV muxer updates sizes on a
seekable output during finalization. Independently validate the completed file:
RIFF/WAVE signature, chunk bounds, PCM format, rate/channel/sample alignment,
nonempty audio, byte cap, and decoded sample duration. Successful exit and a
`.wav` extension do not replace those checks.

## Duration and timestamp semantics

The sentinel is an implementation bound, not a request to accept the first ten
minutes of a longer stream. A completed WAV longer than 600 seconds must be
rejected even if FFmpeg exits successfully at 601 seconds. This lets sources
with unknown duration be considered without treating unknown metadata as
permission for an unbounded decode. Keep a separate wall-clock timeout; `-t`
limits media duration, not elapsed time or the bytes a malformed provider writes.
Source metadata can reject an obviously overlong request early, but only the
decoded sample count governs the final prepared-audio duration check.

**Transcript times in this profile are relative to the prepared WAV's sample
timeline. They are not qualified original-video subtitle times.**

The timestamp expression counts decoded samples from zero. This intentionally
removes a source start offset and collapses timestamp gaps/discontinuities;
downmixing and resampling are transformations too. MOV edit lists and codec
padding may already affect which samples reach the filter. Retain the reported
stream start time, selected global index, original media digest, probe output,
extraction identity/settings, and prepared WAV digest as provenance. Adding the
reported start time to every cue cannot generally reconstruct the original
timeline. A later graph/mux route needs an explicit timeline mapping contract,
or an equally explicit refusal for inputs whose alignment cannot be established.

## Deferred evidence

The user deferred execution checks. Qualification still needs compilation,
parser/refusal coverage execution, licensed native fixtures, unknown/incorrect
duration cases, multiple tracks, nonzero starts and gaps, edit lists, sample-rate
changes, oversize output, interruption, and artifact linkage checks. The source
review does not establish compatibility across FFmpeg releases or platforms.
