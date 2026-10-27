//! Deterministic body-only codecs shared by original and recovery exports.
//!
//! This layer never removes metadata, joins elements, or invents source
//! fidelity. Recovery callers must verify and retain their own envelope before
//! passing an explicitly chosen body projection. Original callers must retain
//! any declared provenance/ID/layout losses in their own export evidence.

use std::io::{Cursor, Write};

use zip::{write::SimpleFileOptions, CompressionMethod, DateTime, ZipWriter};

use super::{
    equivalent_semantics, failure, parse_native, validate_trelby_neighbors,
    Checked, NativeScreenplayFormat, TRELBY_BODY_PREFIX,
};
use crate::screenplay::{validate, Screenplay, ScreenplayElementKind, MAX_INPUT_BYTES};

/// Serialize a strict, unstyled body and independently recover its semantics.
///
/// Unsupported title pages, extensions, prior loss declarations and paragraph
/// constructs fail before serialization. No input is changed or projected by
/// this function. Success establishes only bounded structural equivalence;
/// it does not qualify a native application, editing workflow or rendering.
pub fn export_body(body: &Screenplay, format: NativeScreenplayFormat) -> Checked<Vec<u8>> {
    validate(body).map_err(|error| failure("screenplay.native.export.structure", error.to_string()))?;
    equivalent_semantics(body, body).map_err(|error| failure(
        "screenplay.native.export.unsupported", error.to_string(),
    ))?;
    for element in &body.elements {
        if element.text.is_empty() || element.text.trim() != element.text
            || element.text.chars().any(char::is_control)
        {
            return Err(failure("screenplay.native.export.text",
                "native body paragraphs must contain one nonempty trimmed line without controls"));
        }
    }
    let bytes = match format {
        NativeScreenplayFormat::Fountain => fountain_bytes(body)?,
        NativeScreenplayFormat::Fdx | NativeScreenplayFormat::Osf => xml_bytes(body, format)?,
        NativeScreenplayFormat::FadeIn => fadein_bytes(body)?,
        NativeScreenplayFormat::Trelby => trelby_bytes(body)?,
    };
    if bytes.is_empty() || bytes.len() > MAX_INPUT_BYTES {
        return Err(failure("screenplay.native.export.bounds", "native output is empty or exceeds 16 MiB"));
    }
    let parsed = parse_native(&bytes, format).map_err(|error| failure(
        "screenplay.native.export.unsupported",
        format!("{} body could not be independently accepted: {error}", format.as_str()),
    ))?;
    equivalent_semantics(body, &parsed).map_err(|error| failure(
        "screenplay.native.export.semantic_mismatch",
        format!("{} representation changed the chosen body: {error}", format.as_str()),
    ))?;
    Ok(bytes)
}

fn append(output: &mut String, value: &str) -> Checked<()> {
    if output.len().saturating_add(value.len()) > MAX_INPUT_BYTES {
        return Err(failure("screenplay.native.export.bounds", "native output exceeds 16 MiB"));
    }
    output.push_str(value);
    Ok(())
}

fn append_xml(output: &mut String, value: &str) -> Checked<()> {
    for character in value.chars() {
        match character {
            '&' => append(output, "&amp;")?,
            '<' => append(output, "&lt;")?,
            '>' => append(output, "&gt;")?,
            '"' => append(output, "&quot;")?,
            '\'' => append(output, "&apos;")?,
            other => append(output, other.encode_utf8(&mut [0; 4]))?,
        }
    }
    Ok(())
}

fn kind_label(kind: ScreenplayElementKind) -> Checked<&'static str> {
    match kind {
        ScreenplayElementKind::SceneHeading => Ok("Scene Heading"),
        ScreenplayElementKind::Action => Ok("Action"),
        ScreenplayElementKind::Character => Ok("Character"),
        ScreenplayElementKind::Dialogue => Ok("Dialogue"),
        ScreenplayElementKind::Parenthetical => Ok("Parenthetical"),
        ScreenplayElementKind::Transition => Ok("Transition"),
        _ => Err(failure("screenplay.native.export.unsupported", "unsupported native paragraph kind")),
    }
}

fn xml_bytes(body: &Screenplay, format: NativeScreenplayFormat) -> Checked<Vec<u8>> {
    let fdx = format == NativeScreenplayFormat::Fdx;
    let mut output = String::new();
    append(&mut output, "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n")?;
    append(&mut output, if fdx {
        "<FinalDraft DocumentType=\"Script\" Template=\"No\" Version=\"1\"><Content>\n"
    } else {
        "<document type=\"Open Screenplay Format document\" version=\"20\"><paragraphs>\n"
    })?;
    for element in &body.elements {
        append(&mut output, if fdx { "<Paragraph Type=\"" } else { "<para><style basestylename=\"" })?;
        append(&mut output, kind_label(element.kind)?)?;
        append(&mut output, if fdx { "\"><Text>" } else { "\"/><text>" })?;
        if element.kind == ScreenplayElementKind::Parenthetical {
            if element.text.contains(['(', ')']) {
                return Err(failure("screenplay.native.export.unsupported", "nested parentheses cannot be represented by the verified native parenthetical profile"));
            }
            append(&mut output, "(")?;
        }
        append_xml(&mut output, &element.text)?;
        if element.kind == ScreenplayElementKind::Parenthetical { append(&mut output, ")")?; }
        append(&mut output, if fdx { "</Text></Paragraph>\n" } else { "</text></para>\n" })?;
    }
    append(&mut output, if fdx { "</Content></FinalDraft>\n" } else { "</paragraphs></document>\n" })?;
    Ok(output.into_bytes())
}

fn fountain_bytes(body: &Screenplay) -> Checked<Vec<u8>> {
    let mut output = String::new();
    for element in &body.elements {
        if !matches!(element.kind, ScreenplayElementKind::Dialogue | ScreenplayElementKind::Parenthetical)
            && !output.is_empty()
        {
            append(&mut output, "\n")?;
        }
        if element.kind == ScreenplayElementKind::Parenthetical { append(&mut output, "(")?; }
        append(&mut output, &element.text)?;
        if element.kind == ScreenplayElementKind::Parenthetical { append(&mut output, ")")?; }
        append(&mut output, "\n")?;
    }
    Ok(output.into_bytes())
}

fn fadein_bytes(body: &Screenplay) -> Checked<Vec<u8>> {
    let xml = xml_bytes(body, NativeScreenplayFormat::Osf)?;
    // Reserve more than the fixed one-member ZIP header/directory overhead.
    if xml.len() > MAX_INPUT_BYTES - 1024 {
        return Err(failure("screenplay.native.export.bounds", "Fade In XML leaves insufficient bounded archive space"));
    }
    let mut archive = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    archive.start_file("document.xml", options)
        .map_err(|error| failure("screenplay.native.export.archive", error.to_string()))?;
    archive.write_all(&xml).map_err(|error| failure("screenplay.native.export.archive", error.to_string()))?;
    let output = archive.finish()
        .map_err(|error| failure("screenplay.native.export.archive", error.to_string()))?.into_inner();
    if output.len() > MAX_INPUT_BYTES {
        return Err(failure("screenplay.native.export.bounds", "Fade In archive exceeds 16 MiB"));
    }
    Ok(output)
}

fn trelby_bytes(body: &Screenplay) -> Checked<Vec<u8>> {
    validate_trelby_neighbors(&body.elements).map_err(|error| failure(
        "screenplay.native.export.unsupported", error.to_string(),
    ))?;
    let mut output = String::new();
    append(&mut output, TRELBY_BODY_PREFIX)?;
    for element in &body.elements {
        let kind_code = match element.kind {
            ScreenplayElementKind::SceneHeading => ".\\",
            ScreenplayElementKind::Action => "..",
            ScreenplayElementKind::Character => "._",
            ScreenplayElementKind::Dialogue => ".:",
            ScreenplayElementKind::Parenthetical => ".(",
            ScreenplayElementKind::Transition => "./",
            _ => return Err(failure("screenplay.native.export.unsupported", "unsupported Trelby paragraph kind")),
        };
        if element.text.contains(['\u{2028}', '\u{2029}']) {
            return Err(failure("screenplay.native.export.text", "Trelby paragraph text cannot contain Unicode line separators"));
        }
        append(&mut output, kind_code)?;
        if element.kind == ScreenplayElementKind::Parenthetical {
            if element.text.contains(['(', ')']) {
                return Err(failure("screenplay.native.export.unsupported", "nested parentheses cannot be represented by the verified native parenthetical profile"));
            }
            append(&mut output, "(")?;
        }
        append(&mut output, &element.text)?;
        if element.kind == ScreenplayElementKind::Parenthetical { append(&mut output, ")")?; }
        append(&mut output, "\n")?;
    }
    Ok(output.into_bytes())
}
