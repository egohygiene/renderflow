//! Structural checks for the existing native exports, never general importers.
//! Exact root/policy/lineage/byte admission remains in graph::validate_published.

use crate::graph::Format;
use crate::timed_text::{self, TimedTextFormat};
use anyhow::{ensure, Context, Result};

const TTML: &str = "http://www.w3.org/ns/ttml";
const XML: &str = "http://www.w3.org/XML/1998/namespace";
const MAX_XML_NODES: u32 = 1_000_016;

pub(crate) fn validate(bytes: &[u8], format: Format) -> Result<()> {
    ensure!(
        !bytes.is_empty() && bytes.len() <= super::MAX_DOCUMENT_BYTES,
        "lyrics.export.limit"
    );
    if format == Format::LyricsJson {
        super::parse_document(bytes)?;
        return Ok(());
    }
    let text = std::str::from_utf8(bytes).context("lyrics.export.utf8")?;
    match format {
        Format::Lrc => lrc(text),
        Format::Ttml => ttml(text),
        _ => anyhow::bail!("lyrics.export.format"),
    }
}

fn lrc(text: &str) -> Result<()> {
    ensure!(text.ends_with('\n'), "lyrics.lrc.terminal_newline");
    let mut previous = 0;
    for (index, line) in text.split_terminator('\n').enumerate() {
        let (stamp, words) = line.split_once(']').context("lyrics.lrc.timestamp")?;
        ensure!(
            words.len() <= timed_text::MAX_CUE_TEXT_BYTES,
            "lyrics.lrc.text_limit"
        );
        let stamp = stamp.strip_prefix('[').context("lyrics.lrc.timestamp")?;
        let (minutes, seconds) = stamp.split_once(':').context("lyrics.lrc.timestamp")?;
        ensure!(
            (2..=4).contains(&minutes.len()) && minutes.bytes().all(|c| c.is_ascii_digit()),
            "lyrics.lrc.minutes"
        );
        let minutes: u64 = minutes.parse()?;
        ensure!(minutes <= 5999, "lyrics.lrc.clock_range");
        ensure!(
            seconds.len() == 6 && seconds.as_bytes()[2] == b'.',
            "lyrics.lrc.clock_shape"
        );
        let clock = format!("{:02}:{:02}:{seconds}", minutes / 60, minutes % 60);
        let start = timed_text::parse_timestamp(&clock, TimedTextFormat::WebVtt)
            .context("lyrics.lrc.clock")?;
        ensure!(
            stamp
                == format!(
                    "{:02}:{:02}.{:03}",
                    minutes,
                    start / 1000 % 60,
                    start % 1000
                ),
            "lyrics.lrc.native_clock"
        );
        ensure!(index == 0 || start >= previous, "lyrics.lrc.order");
        timed_text::validate_text(words).context("lyrics.lrc.plain_text")?;
        previous = start;
    }
    Ok(())
}

fn ttml(text: &str) -> Result<()> {
    ensure!(
        !text
            .as_bytes()
            .windows(9)
            .any(|v| v.eq_ignore_ascii_case(b"<!DOCTYPE"))
            && !text
                .as_bytes()
                .windows(8)
                .any(|v| v.eq_ignore_ascii_case(b"<!ENTITY")),
        "lyrics.ttml.dtd_entity"
    );
    let document = roxmltree::Document::parse_with_options(
        text,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XML_NODES,
            ..Default::default()
        },
    )
    .context("lyrics.ttml.xml")?;
    let root = document.root_element();
    ensure!(
        root.has_tag_name((TTML, "tt")) && root.attribute((XML, "space")) == Some("preserve"),
        "lyrics.ttml.root"
    );
    for attr in root.attributes() {
        ensure!(
            attr.namespace() == Some(XML) && matches!(attr.name(), "space" | "lang"),
            "lyrics.ttml.root_attribute"
        );
    }
    if let Some(language) = root.attribute((XML, "lang")) {
        super::validate_language(language)?;
    }
    let bodies = root
        .children()
        .filter(|n| n.is_element())
        .collect::<Vec<_>>();
    ensure!(
        bodies.len() == 1 && bodies[0].has_tag_name((TTML, "body")),
        "lyrics.ttml.body"
    );
    let divs = bodies[0]
        .children()
        .filter(|n| n.is_element())
        .collect::<Vec<_>>();
    ensure!(
        divs.len() == 1 && divs[0].has_tag_name((TTML, "div")),
        "lyrics.ttml.div"
    );
    ensure!(
        bodies[0].attributes().len() == 0 && divs[0].attributes().len() == 0,
        "lyrics.ttml.container_attribute"
    );
    let mut count = 0;
    let mut previous = 0;
    for node in document.descendants() {
        ensure!(
            !node.is_pi() && !node.is_comment(),
            "lyrics.ttml.extra_node"
        );
        if node.is_text() && !node.parent().is_some_and(|p| p.has_tag_name((TTML, "p"))) {
            ensure!(
                node.text().unwrap_or_default().trim().is_empty(),
                "lyrics.ttml.detached_text"
            );
        }
        if !node.is_element() || node == root || node == bodies[0] || node == divs[0] {
            continue;
        }
        if node.has_tag_name((TTML, "br")) {
            ensure!(
                node.parent().is_some_and(|p| p.has_tag_name((TTML, "p")))
                    && node.attributes().len() == 0
                    && !node.has_children(),
                "lyrics.ttml.line_break"
            );
            continue;
        }
        ensure!(
            node.has_tag_name((TTML, "p")) && node.parent() == Some(divs[0]),
            "lyrics.ttml.element"
        );
        count += 1;
        ensure!(count <= timed_text::MAX_CUES, "lyrics.ttml.cue_limit");
        let expected_id = format!("cue-{count}");
        ensure!(
            node.attribute((XML, "id")) == Some(expected_id.as_str())
                && node.attributes().len() == 3,
            "lyrics.ttml.cue_attributes"
        );
        let begin = node.attribute("begin").context("lyrics.ttml.begin")?;
        let end = node.attribute("end").context("lyrics.ttml.end")?;
        ensure!(
            begin.len() == 12 && end.len() == 12,
            "lyrics.ttml.clock_shape"
        );
        let begin = timed_text::parse_timestamp(begin, TimedTextFormat::WebVtt)
            .context("lyrics.ttml.clock")?;
        let end = timed_text::parse_timestamp(end, TimedTextFormat::WebVtt)
            .context("lyrics.ttml.clock")?;
        ensure!(
            end > begin && (count == 1 || begin >= previous),
            "lyrics.ttml.clock_order"
        );
        previous = begin;
        let mut words = String::new();
        for child in node.children() {
            if child.is_text() {
                ensure!(
                    words
                        .len()
                        .saturating_add(child.text().unwrap_or_default().len())
                        <= timed_text::MAX_CUE_TEXT_BYTES,
                    "lyrics.ttml.text_limit"
                );
                words.push_str(child.text().unwrap_or_default());
            } else {
                ensure!(child.has_tag_name((TTML, "br")), "lyrics.ttml.content");
                ensure!(
                    words.len() < timed_text::MAX_CUE_TEXT_BYTES,
                    "lyrics.ttml.text_limit"
                );
                words.push('\n');
            }
            ensure!(
                words.len() <= timed_text::MAX_CUE_TEXT_BYTES,
                "lyrics.ttml.text_limit"
            );
        }
        timed_text::validate_text(&words).context("lyrics.ttml.plain_text")?;
    }
    ensure!(count > 0, "lyrics.ttml.empty");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn refuses(bytes: &[u8], format: Format, expected: &str) {
        let error = validate(bytes, format).expect_err("malformed native export was accepted");
        assert_eq!(error.to_string(), expected);
    }

    fn document() -> super::super::LyricsDocument {
        let bytes = b"7\n00:00:00,125 --> 00:00:01,250\nMoon\nSecond line\n\n9\n00:00:00,650 --> 00:00:01,875\nRepeat\n";
        let policy = super::super::LyricsPolicy {
            source_sha256: super::super::sha256(bytes),
            review_state: super::super::ReviewState::Unreviewed,
            language: Some("en".into()),
            revision: None,
            sections: BTreeMap::new(),
        };
        super::super::normalize(bytes, Format::Srt, &policy).unwrap()
    }

    #[test]
    fn native_exports_pass_and_cross_format_payloads_refuse() {
        let doc = document();
        for format in [Format::LyricsJson, Format::Lrc, Format::Ttml] {
            let bytes = super::super::render(&doc, format).unwrap();
            validate(&bytes, format).unwrap();
            for other in [Format::LyricsJson, Format::Lrc, Format::Ttml] {
                if other != format {
                    refuses(
                        &bytes,
                        other,
                        match other {
                            Format::LyricsJson => "lyrics.document.json",
                            Format::Lrc => "lyrics.lrc.timestamp",
                            Format::Ttml => "lyrics.ttml.xml",
                            _ => unreachable!(),
                        },
                    );
                }
            }
        }
    }

    #[test]
    fn lrc_refuses_non_native_clocks_order_and_plain_text() {
        for (bytes, expected) in [
            (b"ordinary text\n".as_slice(), "lyrics.lrc.timestamp"),
            (b"[00:60.000]Wrong\n", "lyrics.lrc.clock"),
            (b"[00:00.12]Wrong\n", "lyrics.lrc.clock_shape"),
            (b"[000:00.125]Wrong\n", "lyrics.lrc.native_clock"),
            (
                b"[00:01.000]Later\n[00:00.125]Earlier\n",
                "lyrics.lrc.order",
            ),
            (b"[00:00.125]<i>Styled</i>\n", "lyrics.lrc.plain_text"),
        ] {
            refuses(bytes, Format::Lrc, expected);
        }
        validate(b"[5999:59.999]Last\n", Format::Lrc).unwrap();
    }

    #[test]
    fn ttml_refuses_wrong_namespace_dtd_entities_timing_and_nested_styles() {
        let bytes = super::super::render(&document(), Format::Ttml).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        for (changed, expected) in [
            (
                text.replace(TTML, "https://wrong.invalid/ttml"),
                "lyrics.ttml.root",
            ),
            (
                text.replace("<tt ", "<!DOCTYPE tt [<!ENTITY secret 'words'>]><tt "),
                "lyrics.ttml.dtd_entity",
            ),
            (
                text.replace("00:00:00.125", "00:00:00.xx5"),
                "lyrics.ttml.clock",
            ),
            (
                text.replace("00:00:01.250", "00:00:00.001"),
                "lyrics.ttml.clock_order",
            ),
            (
                text.replace("Moon", "<span>Moon</span>"),
                "lyrics.ttml.content",
            ),
            (
                text.replace("cue-1", "cue-999"),
                "lyrics.ttml.cue_attributes",
            ),
        ] {
            refuses(changed.as_bytes(), Format::Ttml, expected);
        }
    }

    #[test]
    fn canonical_json_rejects_unknown_schema_fields_and_detached_policy() {
        let doc = document();
        let good = super::super::render(&doc, Format::LyricsJson).unwrap();
        let mut value: serde_json::Value = serde_json::from_slice(&good).unwrap();
        value["unknown"] = true.into();
        refuses(
            &serde_json::to_vec(&value).unwrap(),
            Format::LyricsJson,
            "lyrics.document.json",
        );
        value.as_object_mut().unwrap().remove("unknown");
        value["policy"]["source_sha256"] = "a".repeat(64).into();
        refuses(
            &serde_json::to_vec(&value).unwrap(),
            Format::LyricsJson,
            "lyrics.source.binding",
        );
    }

    #[test]
    fn ttml_node_budget_and_lrc_preparse_bounds_refuse_with_the_owning_reason() {
        let mut text = String::from(
            "<tt xmlns=\"http://www.w3.org/ns/ttml\" xml:space=\"preserve\"><body><div><p>",
        );
        text.push_str(&"<br/>".repeat(MAX_XML_NODES as usize));
        text.push_str("</p></div></body></tt>");
        let error = validate(text.as_bytes(), Format::Ttml).unwrap_err();
        assert_eq!(error.to_string(), "lyrics.ttml.xml");
        assert!(matches!(
            error.downcast_ref::<roxmltree::Error>(),
            Some(roxmltree::Error::NodesLimitReached)
        ));
        refuses(
            format!("[00:{}]words\n", ":".repeat(100_000)).as_bytes(),
            Format::Lrc,
            "lyrics.lrc.clock_shape",
        );
        refuses(
            format!(
                "[00:00.125]{}\n",
                "x".repeat(timed_text::MAX_CUE_TEXT_BYTES + 1)
            )
            .as_bytes(),
            Format::Lrc,
            "lyrics.lrc.text_limit",
        );
    }
}
