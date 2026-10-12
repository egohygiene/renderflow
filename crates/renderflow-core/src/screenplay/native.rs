//! Independent readers for the deliberately narrow scripttool import profile.
//!
//! These readers do not invoke a converter. They refuse constructs for which
//! this checkpoint cannot compare meaning independently. Parsing is evidence
//! about the accepted semantic subset, not native-application qualification.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use roxmltree::{Document, Node, ParsingOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::{CompressionMethod, ZipArchive};

use super::{
    validate, Screenplay, ScreenplayElement, ScreenplayElementKind, ScreenplaySource,
    MAX_ELEMENTS, MAX_INPUT_BYTES, MAX_TEXT_BYTES,
};

const MAX_XML_NODES: u32 = 500_000;
const MAX_XML_DEPTH: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NativeScreenplayFormat {
    Fountain,
    Fdx,
    Osf,
    #[serde(rename = "fadein")]
    FadeIn,
}

impl NativeScreenplayFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fountain => "fountain",
            Self::Fdx => "fdx",
            Self::Osf => "osf",
            Self::FadeIn => "fadein",
        }
    }

    pub fn extension(self) -> &'static str { self.as_str() }
}

/// A stable diagnostic code and a description of the refused input.
#[derive(Debug, Error)]
#[error("{code}: {message}")]
pub struct NativeScreenplayError {
    pub code: &'static str,
    pub message: String,
}

type Checked<T> = Result<T, NativeScreenplayError>;

fn failure(code: &'static str, message: impl Into<String>) -> NativeScreenplayError {
    NativeScreenplayError { code, message: message.into() }
}

fn require(condition: bool, code: &'static str, message: impl Into<String>) -> Checked<()> {
    if condition { Ok(()) } else { Err(failure(code, message)) }
}

fn utf8(bytes: &[u8]) -> Checked<&str> {
    require(bytes.len() <= MAX_INPUT_BYTES, "screenplay.native.bounds", "input exceeds 16 MiB")?;
    std::str::from_utf8(bytes).map_err(|error| failure("screenplay.native.encoding", format!("UTF-8 is required: {error}")))
}

/// Read an observed source into the common body-only, unstyled profile.
///
/// Only FDX version 1, OSF version 20, a single-member Fade In wrapper of that
/// OSF profile, and the corresponding plain Fountain subset are accepted.
/// Title pages, notes, styling, scene numbers, dual dialogue, revision metadata,
/// application layout, and unknown fields fail explicitly; none are dropped.
pub fn parse_native(bytes: &[u8], format: NativeScreenplayFormat) -> Checked<Screenplay> {
    require(bytes.len() <= MAX_INPUT_BYTES, "screenplay.native.bounds", "input exceeds 16 MiB")?;
    let elements = match format {
        NativeScreenplayFormat::Fountain => parse_fountain(utf8(bytes)?)?,
        NativeScreenplayFormat::Fdx => parse_fdx(utf8(bytes)?)?,
        NativeScreenplayFormat::Osf => parse_osf(utf8(bytes)?)?,
        NativeScreenplayFormat::FadeIn => {
            let document = fadein_document(bytes)?;
            parse_osf(utf8(&document)?)?
        }
    };
    let screenplay = Screenplay::new(ScreenplaySource {
        sha256: format!("{:x}", Sha256::digest(bytes)),
        format: format.as_str().to_owned(),
        extensions: BTreeMap::new(),
    }, elements);
    validate(&screenplay).map_err(|error| failure("screenplay.native.structure", error.to_string()))?;
    Ok(screenplay)
}

/// Compare all accepted semantics, excluding only generated IDs and source
/// identity. This comparison cannot certify an arbitrary canonical model.
/// Unsupported canonical extensions/losses/title pages are refused first.
pub fn equivalent_semantics(left: &Screenplay, right: &Screenplay) -> Checked<()> {
    fn projection(screenplay: &Screenplay) -> Checked<Vec<(ScreenplayElementKind, String, Option<usize>)>> {
        validate(screenplay).map_err(|error| failure("screenplay.native.structure", error.to_string()))?;
        require(screenplay.title_page.is_empty() && screenplay.dual_dialogue.is_empty()
            && screenplay.losses.is_empty() && screenplay.extensions.is_empty()
            && screenplay.source.extensions.is_empty(), "screenplay.native.unsupported",
            "semantic comparison supports only the body-only native profile without extensions or historical loss entries")?;
        let ids: BTreeMap<&str, usize> = screenplay.elements.iter().enumerate()
            .map(|(index, element)| (element.id.as_str(), index)).collect();
        let mut output = Vec::with_capacity(screenplay.elements.len());
        for element in &screenplay.elements {
            require(supported_kind(element.kind) && element.scene_number.is_none()
                && element.section_level.is_none() && element.extensions.is_empty(),
                "screenplay.native.unsupported", "semantic comparison encountered a construct outside the native profile")?;
            let owner = element.character_id.as_ref().map(|id| {
                ids.get(id.as_str()).copied()
                    .ok_or_else(|| failure("screenplay.native.structure", "missing character owner"))
            }).transpose()?;
            output.push((element.kind, element.text.clone(), owner));
        }
        Ok(output)
    }
    require(projection(left)? == projection(right)?, "screenplay.native.semantic_mismatch",
        "converter output changed paragraph order, kind, text, or character ownership")
}

fn supported_kind(kind: ScreenplayElementKind) -> bool {
    matches!(kind, ScreenplayElementKind::SceneHeading | ScreenplayElementKind::Action
        | ScreenplayElementKind::Character | ScreenplayElementKind::Dialogue
        | ScreenplayElementKind::Parenthetical | ScreenplayElementKind::Transition)
}

fn push_element(elements: &mut Vec<ScreenplayElement>, kind: ScreenplayElementKind, value: &str) -> Checked<()> {
    require(elements.len() < MAX_ELEMENTS, "screenplay.native.bounds", "more than 100000 screenplay elements")?;
    require(!value.is_empty() && value.len() <= MAX_TEXT_BYTES && value.trim() == value
        && !value.chars().any(char::is_control), "screenplay.native.text",
        "native profile text must be one nonempty line without surrounding whitespace or controls, at most 65536 bytes")?;
    let character_id = if matches!(kind, ScreenplayElementKind::Dialogue | ScreenplayElementKind::Parenthetical) {
        let previous = elements.last().ok_or_else(|| failure("screenplay.native.structure", "dialogue has no preceding character"))?;
        match previous.kind {
            ScreenplayElementKind::Character => Some(previous.id.clone()),
            ScreenplayElementKind::Parenthetical | ScreenplayElementKind::Dialogue => previous.character_id.clone(),
            _ => return Err(failure("screenplay.native.structure", "dialogue has no active character")),
        }
    } else { None };
    elements.push(ScreenplayElement {
        id: format!("native-element-{:06}", elements.len() + 1), kind,
        text: value.to_owned(), character_id, scene_number: None, section_level: None,
        extensions: BTreeMap::new(),
    });
    Ok(())
}

fn xml_document(input: &str) -> Checked<Document<'_>> {
    require(!input.contains("<!") && input.matches("<?").count() <= 1
        && (!input.contains("<?") || input.starts_with("<?xml ")),
        "screenplay.native.xml_markup", "DTDs, entity declarations, CDATA, comments and processing instructions are unsupported")?;
    let document = Document::parse_with_options(input, ParsingOptions {
        allow_dtd: false, nodes_limit: MAX_XML_NODES,
    }).map_err(|error| failure("screenplay.native.xml", format!("invalid bounded XML: {error}")))?;
    for node in document.descendants() {
        require(node.ancestors().count() <= MAX_XML_DEPTH, "screenplay.native.bounds", "XML depth exceeds eight nodes")?;
        if node.is_element() {
            require(node.tag_name().namespace().is_none() && node.namespaces().len() == 0,
                "screenplay.native.xml_namespace", "namespaces are outside this native profile")?;
            require(node.attributes().all(|attribute| attribute.namespace().is_none()),
                "screenplay.native.xml_namespace", "namespaced attributes are outside this native profile")?;
        }
    }
    Ok(document)
}

fn attributes(node: Node<'_, '_>, allowed: &[(&str, &str)]) -> Checked<()> {
    require(node.attributes().len() == allowed.len()
        && allowed.iter().all(|(key, value)| node.attribute(*key) == Some(*value)),
        "screenplay.native.xml_attributes", format!("unsupported or missing attributes on {}", node.tag_name().name()))
}

fn child_elements<'a, 'input>(node: Node<'a, 'input>) -> Checked<Vec<Node<'a, 'input>>> {
    require(node.children().filter(|child| child.is_text()).all(|child| child.text().unwrap_or_default().trim().is_empty()),
        "screenplay.native.xml_structure", format!("unexpected mixed text in {}", node.tag_name().name()))?;
    Ok(node.children().filter(Node::is_element).collect())
}

fn one_child<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Checked<Node<'a, 'input>> {
    let children = child_elements(node)?;
    require(children.len() == 1 && children[0].tag_name().name() == name,
        "screenplay.native.xml_structure", format!("{} must contain only one {name}", node.tag_name().name()))?;
    Ok(children[0])
}

fn plain_text(node: Node<'_, '_>) -> Checked<String> {
    attributes(node, &[])?;
    require(node.children().all(|child| child.is_text()), "screenplay.native.xml_structure",
        "text runs must contain only plain text, without nested styling")?;
    Ok(node.children().filter_map(|child| child.text()).collect())
}

fn paragraph_kind(value: &str) -> Checked<ScreenplayElementKind> {
    match value {
        "Scene Heading" => Ok(ScreenplayElementKind::SceneHeading),
        "Action" => Ok(ScreenplayElementKind::Action),
        "Character" => Ok(ScreenplayElementKind::Character),
        "Dialogue" => Ok(ScreenplayElementKind::Dialogue),
        "Parenthetical" => Ok(ScreenplayElementKind::Parenthetical),
        "Transition" => Ok(ScreenplayElementKind::Transition),
        _ => Err(failure("screenplay.native.paragraph_kind", format!("unsupported paragraph kind: {value}"))),
    }
}

fn parse_fdx(input: &str) -> Checked<Vec<ScreenplayElement>> {
    let document = xml_document(input)?;
    let root = document.root_element();
    require(root.tag_name().name() == "FinalDraft", "screenplay.native.xml_root", "expected FinalDraft root")?;
    attributes(root, &[("DocumentType", "Script"), ("Template", "No"), ("Version", "1")])?;
    let content = one_child(root, "Content")?;
    attributes(content, &[])?;
    let mut elements = Vec::new();
    for paragraph in child_elements(content)? {
        require(paragraph.tag_name().name() == "Paragraph", "screenplay.native.xml_structure", "Content accepts only Paragraph children")?;
        require(paragraph.attributes().len() == 1, "screenplay.native.xml_attributes", "Paragraph accepts only Type")?;
        let kind_name = paragraph.attribute("Type").ok_or_else(|| failure("screenplay.native.xml_attributes", "Paragraph requires Type"))?;
        let text = plain_text(one_child(paragraph, "Text")?)?;
        if kind_name == "Empty" {
            require(text.trim().is_empty(), "screenplay.native.text", "Empty paragraph contains nonempty text")?;
            continue;
        }
        let kind = paragraph_kind(kind_name)?;
        let text = decode_parenthetical(kind, &text, false)?;
        push_element(&mut elements, kind, &text)?;
    }
    Ok(elements)
}

fn parse_osf(input: &str) -> Checked<Vec<ScreenplayElement>> {
    let document = xml_document(input)?;
    let root = document.root_element();
    require(root.tag_name().name() == "document", "screenplay.native.xml_root", "expected document root")?;
    attributes(root, &[("type", "Open Screenplay Format document"), ("version", "20")])?;
    let paragraphs = one_child(root, "paragraphs")?;
    attributes(paragraphs, &[])?;
    let mut elements = Vec::new();
    for paragraph in child_elements(paragraphs)? {
        require(paragraph.tag_name().name() == "para", "screenplay.native.xml_structure", "paragraphs accepts only para children")?;
        attributes(paragraph, &[])?;
        let children = child_elements(paragraph)?;
        require(children.len() == 2 && children[0].tag_name().name() == "style"
            && children[1].tag_name().name() == "text", "screenplay.native.xml_structure",
            "para must contain one style followed by one text run")?;
        let style = children[0];
        require(style.attributes().len() == 1 && style.attribute("basestylename").is_some()
            && style.children().next().is_none(), "screenplay.native.xml_attributes",
            "style accepts only basestylename and must be empty")?;
        let kind = paragraph_kind(style.attribute("basestylename").unwrap_or_default())?;
        let text = plain_text(children[1])?;
        let text = decode_parenthetical(kind, &text, true)?;
        push_element(&mut elements, kind, &text)?;
    }
    Ok(elements)
}

fn decode_parenthetical(kind: ScreenplayElementKind, value: &str, allow_unwrapped: bool) -> Checked<String> {
    if kind != ScreenplayElementKind::Parenthetical { return Ok(value.to_owned()); }
    if let Some(content) = value.strip_prefix('(').and_then(|value| value.strip_suffix(')')) {
        require(!content.contains(['(', ')']), "screenplay.native.unsupported", "nested parenthetical syntax is unsupported")?;
        return Ok(content.to_owned());
    }
    require(allow_unwrapped && !value.contains(['(', ')']), "screenplay.native.unsupported",
        "parenthetical must have one complete pair of parentheses")?;
    Ok(value.to_owned())
}

fn fadein_document(bytes: &[u8]) -> Checked<Vec<u8>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| failure("screenplay.native.archive", format!("invalid Fade In ZIP: {error}")))?;
    require(archive.len() == 1, "screenplay.native.archive_members",
        "this Fade In profile requires exactly one document.xml member; additional data is unsupported")?;
    let mut member = archive.by_index(0)
        .map_err(|error| failure("screenplay.native.archive", format!("unreadable or encrypted ZIP member: {error}")))?;
    require(member.name() == "document.xml" && member.enclosed_name().is_some()
        && !member.is_dir() && !member.encrypted()
        && member.unix_mode().is_none_or(|mode| matches!(mode & 0o170000, 0 | 0o100000)),
        "screenplay.native.archive_member", "Fade In member must be a regular, unencrypted root document.xml")?;
    require(matches!(member.compression(), CompressionMethod::Stored | CompressionMethod::Deflated),
        "screenplay.native.archive_compression", "only stored or deflated Fade In members are accepted")?;
    require(member.size() <= MAX_INPUT_BYTES as u64, "screenplay.native.bounds", "document.xml exceeds 16 MiB uncompressed")?;
    let declared_size = member.size();
    let mut output = Vec::new();
    member.by_ref().take(MAX_INPUT_BYTES as u64 + 1).read_to_end(&mut output)
        .map_err(|error| failure("screenplay.native.archive", format!("document.xml could not be read completely: {error}")))?;
    require(output.len() <= MAX_INPUT_BYTES && output.len() as u64 == declared_size,
        "screenplay.native.bounds", "uncompressed document.xml size differs from the bounded declaration")?;
    Ok(output)
}

fn scene_heading(value: &str) -> bool {
    ["INT. ", "EXT. ", "INT./EXT. ", "EXT./INT. ", "INT/EXT. ", "I/E. "]
        .iter().any(|prefix| value.starts_with(prefix))
}

fn character_cue(value: &str) -> bool {
    value.chars().any(char::is_alphabetic) && value.to_uppercase() == value
        && !value.ends_with(':') && !scene_heading(value)
}

fn fountain_line(value: &str) -> Checked<()> {
    require(value.trim() == value && !value.is_empty() && !value.chars().any(char::is_control),
        "screenplay.native.text", "Fountain lines must have no indentation, trailing whitespace, or control characters")?;
    require(!value.contains(['*', '_', '[', ']', '#', '^', '~', '\\'])
        && !value.contains("/*") && !value.contains("*/")
        && !value.starts_with(['@', '!', '.', '>', '<', '=']),
        "screenplay.native.unsupported", "Fountain styling, notes, forced kinds, escapes, scene numbers, sections, and dual dialogue are unsupported")
}

fn parse_fountain(input: &str) -> Checked<Vec<ScreenplayElement>> {
    require(!input.contains('\r') && !input.starts_with('\u{feff}'), "screenplay.native.encoding",
        "this Fountain profile requires UTF-8 without BOM and LF line endings")?;
    let mut elements = Vec::new();
    let mut block = Vec::new();
    fn finish(block: &mut Vec<&str>, elements: &mut Vec<ScreenplayElement>) -> Checked<()> {
        if block.is_empty() { return Ok(()); }
        let first = block[0];
        require(!elements.is_empty() || scene_heading(first), "screenplay.native.unsupported",
            "body-only Fountain must begin with a conventional uppercase scene heading; title pages and leading authoring metadata are unsupported")?;
        if scene_heading(first) {
            require(block.len() == 1, "screenplay.native.unsupported", "scene heading must be separated by blank lines")?;
            push_element(elements, ScreenplayElementKind::SceneHeading, first)?;
        } else if first.ends_with("TO:") || first == "FADE OUT." {
            require(block.len() == 1, "screenplay.native.unsupported", "transition must occupy its own paragraph")?;
            push_element(elements, ScreenplayElementKind::Transition, first)?;
        } else if character_cue(first) && block.len() >= 2 {
            push_element(elements, ScreenplayElementKind::Character, first)?;
            let mut index = 1;
            if block[index].starts_with('(') {
                let text = decode_parenthetical(ScreenplayElementKind::Parenthetical, block[index], false)?;
                push_element(elements, ScreenplayElementKind::Parenthetical, &text)?;
                index += 1;
            }
            require(index + 1 == block.len(), "screenplay.native.unsupported",
                "a character block requires exactly one dialogue line, optionally preceded by one parenthetical")?;
            require(!block[index].starts_with('('), "screenplay.native.unsupported", "dialogue cannot be another parenthetical")?;
            push_element(elements, ScreenplayElementKind::Dialogue, block[index])?;
        } else {
            require(block.len() == 1 && !first.starts_with('('), "screenplay.native.unsupported",
                "only one-line action paragraphs and complete character blocks are accepted")?;
            push_element(elements, ScreenplayElementKind::Action, first)?;
        }
        block.clear();
        Ok(())
    }
    for line in input.split('\n') {
        if line.is_empty() { finish(&mut block, &mut elements)?; }
        else { fountain_line(line)?; block.push(line); }
    }
    finish(&mut block, &mut elements)?;
    Ok(elements)
}

#[cfg(test)]
mod tests;
