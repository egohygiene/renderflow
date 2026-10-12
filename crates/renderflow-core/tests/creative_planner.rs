//! Source-authored regressions for optional creative provider isolation (#429).
//! The registry is assessed through synthetic probes; no external tools run.

use renderflow::graph::{Format, TransformEdge, TransformGraph};
use renderflow::process::{
    ProcessPlatform, ProcessTreeTermination, ToolProbeEvidence, ToolProbeStatus,
};
use renderflow::toolchain::{ToolProbe, ToolRegistry, ToolRuntimeContext};

struct MediaProbe {
    ffmpeg_version: &'static str,
    ffprobe_missing: bool,
}

impl ToolProbe for MediaProbe {
    fn probe(&self, executable: &str, _version_args: &[String]) -> ToolProbeEvidence {
        let missing = executable == "ffprobe" && self.ffprobe_missing;
        ToolProbeEvidence {
            executable: executable.to_owned(),
            status: if missing { ToolProbeStatus::Missing } else { ToolProbeStatus::Available },
            version_line: (!missing).then(|| {
                if executable == "ffmpeg" { self.ffmpeg_version.to_owned() }
                else { "ffprobe version 7.1.1".to_owned() }
            }),
            duration_ms: 0,
            platform: ProcessPlatform {
                os: "linux", arch: "x86_64",
                tree_termination: ProcessTreeTermination::DirectChild,
            },
            diagnostic: missing.then(|| "synthetic missing FFprobe".to_owned()),
        }
    }
}

fn graph() -> TransformGraph {
    let mut graph = TransformGraph::new();
    graph.add_transform(
        TransformEdge::new(Format::Mkv, Format::TimedTranscriptJson, 1.0, 0.9)
            .with_provider("tool.ffmpeg", "subtitle.extract.plain")
            .with_required_provider("tool.ffprobe"),
    );
    graph.add_transform(
        TransformEdge::new(Format::Mkv, Format::Mp4, 1.0, 0.9)
            .with_provider("tool.ffmpeg", "video.convert"),
    );
    graph.add_transform(
        TransformEdge::new(Format::Srt, Format::TimedTranscriptJson, 1.0, 1.0)
            .with_provider("tool.renderflow-timed-text", "subtitle.convert.plain"),
    );
    graph.add_transform(
        TransformEdge::new(Format::TimedTranscriptJson, Format::WebVtt, 1.0, 0.9)
            .with_provider("tool.renderflow-timed-text", "subtitle.convert.plain"),
    );
    graph
}

#[test]
fn missing_inspection_provider_preserves_unrelated_media_and_pure_text_routes() {
    let graph = graph();
    let inventory = ToolRegistry::builtins().assess_ids_with(
        graph.provider_ids(),
        &MediaProbe { ffmpeg_version: "ffmpeg version 7.1.1", ffprobe_missing: true },
        &ToolRuntimeContext::for_platform("linux", "x86_64"),
    );
    let available = graph.filtered_by_available_providers(&inventory.available_ids());
    assert!(!available.has_transform(Format::Mkv, Format::TimedTranscriptJson));
    assert!(available.has_transform(Format::Mkv, Format::Mp4));
    assert!(available.reachable_from(Format::Srt).contains(&Format::WebVtt));
}

#[test]
fn incompatible_media_provider_preserves_pure_timed_text_conversion() {
    let graph = graph();
    let inventory = ToolRegistry::builtins().assess_ids_with(
        graph.provider_ids(),
        &MediaProbe { ffmpeg_version: "ffmpeg version 3.0.0", ffprobe_missing: false },
        &ToolRuntimeContext::for_platform("linux", "x86_64"),
    );
    let available = graph.filtered_by_available_providers(&inventory.available_ids());
    assert!(!available.has_transform(Format::Mkv, Format::TimedTranscriptJson));
    assert!(!available.has_transform(Format::Mkv, Format::Mp4));
    assert!(available.reachable_from(Format::Srt).contains(&Format::WebVtt));
}
