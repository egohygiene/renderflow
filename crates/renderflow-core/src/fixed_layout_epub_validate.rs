//! Independent, bounded inspection for Renderflow's exact ordered-image EPUB route.
//!
//! This deliberately accepts only the narrow PNG/JPEG publication contract.
//! General EPUB 3.3 conformance remains the job of an independently identified
//! EPUBCheck provider; accepting an arbitrary ZIP or matching text fragments is
//! not evidence that a fixed-layout publication is safe or internally coherent.

use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zip::{CompressionMethod, ZipArchive};

use crate::ebook::{EbookDiagnostic, EbookDiagnosticSeverity};
use crate::graph::Format;
use crate::print_pdf_image::inspect_print_image;

const CONTAINER_NS: &str = "urn:oasis:names:tc:opendocument:xmlns:container";
const OPF_NS: &str = "http://www.idpf.org/2007/opf";
const DC_NS: &str = "http://purl.org/dc/elements/1.1/";
const XHTML_NS: &str = "http://www.w3.org/1999/xhtml";
const EPUB_NS: &str = "http://www.idpf.org/2007/ops";
const XML_NS: &str = "http://www.w3.org/XML/1998/namespace";
const MAX_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_XML_BYTES: u64 = 2 * 1024 * 1024;
const MAX_IMAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_MEMBERS: usize = 2005;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FixedLayoutStatus {
    Validated,
    Invalid,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedLayoutPageEvidence {
    pub page: String,
    pub image: String,
    pub image_sha256: String,
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixedLayoutEvidence {
    pub status: FixedLayoutStatus,
    pub pages: Vec<FixedLayoutPageEvidence>,
}

#[derive(Debug)]
struct Refusal {
    code: &'static str,
    message: String,
}

type Checked<T> = std::result::Result<T, Refusal>;

fn refuse<T>(code: &'static str, message: impl Into<String>) -> Checked<T> {
    Err(Refusal {
        code,
        message: message.into(),
    })
}

fn required(condition: bool, code: &'static str, message: impl Into<String>) -> Checked<()> {
    if condition {
        Ok(())
    } else {
        refuse(code, message)
    }
}

pub(crate) fn inspect_fixed_layout<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &Path,
) -> (FixedLayoutEvidence, Vec<EbookDiagnostic>) {
    match validate(archive, path) {
        Ok(pages) => (
            FixedLayoutEvidence {
                status: FixedLayoutStatus::Validated,
                pages,
            },
            Vec::new(),
        ),
        Err(failure) => (
            FixedLayoutEvidence {
                status: FixedLayoutStatus::Invalid,
                pages: Vec::new(),
            },
            vec![EbookDiagnostic {
                severity: EbookDiagnosticSeverity::Error,
                code: failure.code.to_string(),
                message: failure.message,
            }],
        ),
    }
}

fn validate<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    path: &Path,
) -> Checked<Vec<FixedLayoutPageEvidence>> {
    validate_central_directory(path, archive.central_directory_start(), archive.len())?;
    required(
        (7..=MAX_MEMBERS).contains(&archive.len()),
        "ebook.fixed_layout.member",
        "fixed-layout package member count is outside the exact route bound",
    )?;
    let mut names = Vec::with_capacity(archive.len());
    let mut seen = HashSet::new();
    let mut total = 0_u64;
    for index in 0..archive.len() {
        let member = archive.by_index(index).map_err(|error| Refusal {
            code: "ebook.fixed_layout.member",
            message: format!("unreadable ZIP member {index}: {error}"),
        })?;
        let name = member.name().to_string();
        required(
            safe_member_name(&name) && seen.insert(name.to_ascii_lowercase()),
            "ebook.fixed_layout.member",
            format!("unsafe, duplicate, or case-colliding ZIP member: {name}"),
        )?;
        required(
            member.compression() == CompressionMethod::Stored
                && member
                    .unix_mode()
                    .is_none_or(|mode| mode & 0o170000 != 0o120000),
            "ebook.fixed_layout.member",
            format!("unsupported compressed or symlink ZIP member: {name}"),
        )?;
        let timestamp = member.last_modified();
        required(
            timestamp.is_some_and(|date| {
                date.year() == 1980
                    && date.month() == 1
                    && date.day() == 1
                    && date.hour() == 0
                    && date.minute() == 0
                    && date.second() == 0
            }),
            "ebook.fixed_layout.member",
            format!("non-deterministic ZIP member timestamp: {name}"),
        )?;
        total = total.checked_add(member.size()).ok_or_else(|| Refusal {
            code: "ebook.fixed_layout.bounds",
            message: "ZIP uncompressed size overflowed".to_string(),
        })?;
        required(
            total <= MAX_ARCHIVE_BYTES,
            "ebook.fixed_layout.bounds",
            "fixed-layout package exceeds its uncompressed 512 MiB bound",
        )?;
        names.push(name);
    }

    let mut mimetype = archive.by_index(0).map_err(|error| Refusal {
        code: "ebook.fixed_layout.mimetype",
        message: error.to_string(),
    })?;
    required(
        mimetype.name() == "mimetype"
            && mimetype.compression() == CompressionMethod::Stored
            && mimetype.data_start() == 38
            && mimetype.extra_data().is_none_or(|extra| extra.is_empty()),
        "ebook.fixed_layout.mimetype",
        "mimetype must be the first, stored, extra-free ZIP local entry",
    )?;
    let mut mime = Vec::new();
    mimetype.read_to_end(&mut mime).map_err(|error| Refusal {
        code: "ebook.fixed_layout.mimetype",
        message: error.to_string(),
    })?;
    required(
        mime == b"application/epub+zip",
        "ebook.fixed_layout.mimetype",
        "mimetype bytes must be exact, without BOM or padding",
    )?;
    drop(mimetype);

    let container_text = read_text(archive, "META-INF/container.xml", "ebook.fixed_layout.xml")?;
    let container = parse_xml(&container_text)?;
    let root = container.root_element();
    required(
        element(root, CONTAINER_NS, "container"),
        "ebook.fixed_layout.xml",
        "container.xml has an unexpected root or namespace",
    )?;
    let rootfiles = one_child(root, CONTAINER_NS, "rootfiles", "ebook.fixed_layout.xml")?;
    let rootfile = one_child(
        rootfiles,
        CONTAINER_NS,
        "rootfile",
        "ebook.fixed_layout.xml",
    )?;
    required(
        rootfile.attribute("full-path") == Some("EPUB/book.opf")
            && rootfile.attribute("media-type") == Some("application/oebps-package+xml"),
        "ebook.fixed_layout.resource",
        "container rootfile must resolve to the declared local OPF",
    )?;

    let opf_text = read_text(archive, "EPUB/book.opf", "ebook.fixed_layout.xml")?;
    let opf = parse_xml(&opf_text)?;
    let package = opf.root_element();
    required(
        element(package, OPF_NS, "package") && package.attribute("version") == Some("3.0"),
        "ebook.fixed_layout.metadata",
        "fixed-layout OPF must declare the EPUB 3.3 package version 3.0",
    )?;
    let metadata = one_child(package, OPF_NS, "metadata", "ebook.fixed_layout.metadata")?;
    let manifest = one_child(package, OPF_NS, "manifest", "ebook.fixed_layout.resource")?;
    let spine = one_child(package, OPF_NS, "spine", "ebook.fixed_layout.spine")?;
    for name in ["title", "language", "identifier", "rights"] {
        let node = one_child(metadata, DC_NS, name, "ebook.fixed_layout.metadata")?;
        required(
            !node.text().unwrap_or_default().trim().is_empty(),
            "ebook.fixed_layout.metadata",
            format!("dc:{name} is empty"),
        )?;
        if name == "identifier" {
            required(
                node.attribute("id") == package.attribute("unique-identifier"),
                "ebook.fixed_layout.metadata",
                "unique-identifier does not reference the publication identifier",
            )?;
        }
    }
    required(
        metadata
            .children()
            .filter(|node| element(*node, DC_NS, "creator") || element(*node, DC_NS, "contributor"))
            .any(|node| !node.text().unwrap_or_default().trim().is_empty()),
        "ebook.fixed_layout.metadata",
        "publication contributor metadata is absent",
    )?;
    require_meta(
        metadata,
        "dcterms:modified",
        |value| {
            value.len() == 20 && value.ends_with('Z') && value.as_bytes().get(10) == Some(&b'T')
        },
        "ebook.fixed_layout.metadata",
    )?;
    require_meta(
        metadata,
        "rendition:layout",
        |value| value == "pre-paginated",
        "ebook.fixed_layout.metadata",
    )?;
    require_meta(
        metadata,
        "rendition:spread",
        |value| value == "none",
        "ebook.fixed_layout.metadata",
    )?;
    require_meta(
        metadata,
        "schema:accessibilitySummary",
        |value| !value.trim().is_empty(),
        "ebook.fixed_layout.accessibility",
    )?;
    require_meta(
        metadata,
        "schema:accessMode",
        |value| value == "visual",
        "ebook.fixed_layout.accessibility",
    )?;
    let hazards = metadata
        .children()
        .filter(|node| {
            element(*node, OPF_NS, "meta")
                && node.attribute("property") == Some("schema:accessibilityHazard")
        })
        .collect::<Vec<_>>();
    required(
        !hazards.is_empty()
            && hazards
                .iter()
                .all(|node| !node.text().unwrap_or_default().trim().is_empty()),
        "ebook.fixed_layout.accessibility",
        "nonempty accessibility hazard declarations are required",
    )?;
    let features = metadata
        .children()
        .filter(|node| {
            element(*node, OPF_NS, "meta")
                && node.attribute("property") == Some("schema:accessibilityFeature")
        })
        .map(|node| node.text().unwrap_or_default().trim())
        .collect::<Vec<_>>();
    required(
        features.len() == 2
            && features.contains(&"tableOfContents")
            && features.contains(&"pageNavigation"),
        "ebook.fixed_layout.accessibility",
        "declared table-of-contents and page-navigation features must match the package",
    )?;
    required(
        matches!(
            spine.attribute("page-progression-direction"),
            Some("ltr" | "rtl")
        ),
        "ebook.fixed_layout.spine",
        "page progression must be explicitly ltr or rtl",
    )?;

    let mut ids = HashSet::new();
    let mut hrefs = HashSet::new();
    let mut items = HashMap::new();
    let mut nav_count = 0;
    let mut cover_count = 0;
    for item in manifest.children().filter(|node| node.is_element()) {
        required(
            element(item, OPF_NS, "item"),
            "ebook.fixed_layout.resource",
            "unexpected manifest element",
        )?;
        let id = item.attribute("id").unwrap_or_default();
        let href = item.attribute("href").unwrap_or_default();
        let path = resolve_href("EPUB/book.opf", href)?;
        required(
            !id.is_empty() && ids.insert(id.to_string()) && hrefs.insert(path.clone()),
            "ebook.fixed_layout.resource",
            "manifest IDs and hrefs must be nonempty and unique",
        )?;
        required(
            seen.contains(&path.to_ascii_lowercase()),
            "ebook.fixed_layout.resource",
            format!("manifest href does not resolve to a ZIP member: {href}"),
        )?;
        let properties = item.attribute("properties").unwrap_or_default();
        nav_count += usize::from(
            properties
                .split_ascii_whitespace()
                .any(|token| token == "nav"),
        );
        cover_count += usize::from(
            properties
                .split_ascii_whitespace()
                .any(|token| token == "cover-image"),
        );
        items.insert(
            id.to_string(),
            (
                path,
                item.attribute("media-type").unwrap_or_default().to_string(),
                properties.to_string(),
            ),
        );
    }
    required(
        nav_count == 1,
        "ebook.fixed_layout.navigation",
        "exactly one nav manifest item is required",
    )?;
    required(
        cover_count == 1,
        "ebook.fixed_layout.cover",
        "exactly one cover-image is required",
    )?;
    required(
        items.get("nav").is_some_and(|item| {
            item.0 == "EPUB/nav.xhtml" && item.1 == "application/xhtml+xml" && item.2 == "nav"
        }),
        "ebook.fixed_layout.navigation",
        "navigation manifest relationship is missing or incorrect",
    )?;
    required(
        items
            .get("styles")
            .is_some_and(|item| item.0 == "EPUB/styles.css" && item.1 == "text/css"),
        "ebook.fixed_layout.resource",
        "local stylesheet manifest relationship is missing",
    )?;
    let spine_ids = spine
        .children()
        .filter(|node| node.is_element())
        .map(|node| {
            (
                element(node, OPF_NS, "itemref"),
                node.attribute("idref").unwrap_or_default().to_string(),
            )
        })
        .collect::<Vec<_>>();
    required(
        !spine_ids.is_empty() && spine_ids.len() <= 1000 && spine_ids.iter().all(|item| item.0),
        "ebook.fixed_layout.spine",
        "ordered spine must contain 1 to 1000 XHTML page references",
    )?;
    let count = spine_ids.len();
    required(
        items.len() == 2 + 2 * count,
        "ebook.fixed_layout.resource",
        "manifest must contain exactly nav, stylesheet, and one XHTML/image pair per page",
    )?;
    let mut pages = Vec::with_capacity(count);
    for (index, (_, idref)) in spine_ids.iter().enumerate() {
        let number = index + 1;
        let page_id = format!("page-{number:04}");
        let image_id = format!("image-{number:04}");
        let page_path = format!("EPUB/pages/page-{number:04}.xhtml");
        required(
            idref == &page_id
                && items
                    .get(&page_id)
                    .is_some_and(|item| item.0 == page_path && item.1 == "application/xhtml+xml"),
            "ebook.fixed_layout.spine",
            format!("spine item {number} does not resolve to its ordered XHTML page"),
        )?;
        let image_item = items.get(&image_id).ok_or_else(|| Refusal {
            code: "ebook.fixed_layout.resource",
            message: format!("page {number} has no declared image"),
        })?;
        let format = match image_item.1.as_str() {
            "image/png" if image_item.0 == format!("EPUB/images/page-{number:04}.png") => {
                Format::Png
            }
            "image/jpeg" if image_item.0 == format!("EPUB/images/page-{number:04}.jpg") => {
                Format::Jpeg
            }
            _ => {
                return refuse(
                    "ebook.fixed_layout.resource",
                    format!("page {number} has an unsupported image relationship"),
                )
            }
        };
        required(
            (number == 1 && image_item.2 == "cover-image")
                || (number > 1 && image_item.2.is_empty()),
            "ebook.fixed_layout.cover",
            "the first ordered page image must be the only declared cover",
        )?;
        let (digest, width_px, height_px) = inspect_image(archive, &image_item.0, format)?;
        let page_text = read_text(archive, &page_path, "ebook.fixed_layout.xml")?;
        let page = parse_xml(&page_text)?;
        validate_page(&page, &page_path, &image_item.0, width_px, height_px)?;
        pages.push(FixedLayoutPageEvidence {
            page: page_path,
            image: image_item.0.clone(),
            image_sha256: digest,
            width_px,
            height_px,
        });
    }
    let nav_text = read_text(archive, "EPUB/nav.xhtml", "ebook.fixed_layout.navigation")?;
    let nav = parse_xml(&nav_text)?;
    validate_navigation(&nav, &pages)?;
    let css = read_text(archive, "EPUB/styles.css", "ebook.fixed_layout.resource")?;
    required(
        css == crate::fixed_layout_epub::generated_stylesheet(),
        "ebook.fixed_layout.unsafe_markup",
        "stylesheet differs from the exact generated route",
    )?;
    let mut expected = vec![
        "mimetype".to_string(),
        "META-INF/container.xml".to_string(),
        "EPUB/book.opf".to_string(),
        "EPUB/nav.xhtml".to_string(),
        "EPUB/styles.css".to_string(),
    ];
    for page in &pages {
        expected.push(page.page.clone());
        expected.push(page.image.clone());
    }
    required(
        names == expected,
        "ebook.fixed_layout.member",
        "ZIP member order or membership differs from the declared page sequence",
    )?;
    Ok(pages)
}

/// `zip` keeps an index by filename and collapses duplicate central names.
/// Scan the raw directory independently before trusting its indexed view.
fn validate_central_directory(path: &Path, offset: u64, indexed_count: usize) -> Checked<()> {
    let mut input = std::fs::File::open(path).map_err(|error| Refusal {
        code: "ebook.fixed_layout.member",
        message: format!("cannot read ZIP central directory: {error}"),
    })?;
    input
        .seek(SeekFrom::Start(offset))
        .map_err(|error| Refusal {
            code: "ebook.fixed_layout.member",
            message: format!("cannot seek ZIP central directory: {error}"),
        })?;
    let mut names = HashSet::new();
    let mut count = 0_usize;
    loop {
        let mut signature = [0_u8; 4];
        input.read_exact(&mut signature).map_err(|error| Refusal {
            code: "ebook.fixed_layout.member",
            message: format!("truncated ZIP central directory: {error}"),
        })?;
        if signature != [0x50, 0x4b, 0x01, 0x02] {
            required(
                signature == [0x50, 0x4b, 0x05, 0x06] && count == indexed_count && count >= 7,
                "ebook.fixed_layout.member",
                "ZIP central directory has an unexpected record count or terminator",
            )?;
            return Ok(());
        }
        count += 1;
        required(
            count <= MAX_MEMBERS,
            "ebook.fixed_layout.bounds",
            "ZIP central directory exceeds member bound",
        )?;
        let mut header = [0_u8; 42];
        input.read_exact(&mut header).map_err(|error| Refusal {
            code: "ebook.fixed_layout.member",
            message: format!("truncated ZIP central header: {error}"),
        })?;
        let name_len = u16::from_le_bytes([header[24], header[25]]) as usize;
        let extra_len = u16::from_le_bytes([header[26], header[27]]) as u64;
        let comment_len = u16::from_le_bytes([header[28], header[29]]) as u64;
        required(
            name_len > 0 && name_len <= 256,
            "ebook.fixed_layout.member",
            "ZIP central member name is empty or exceeds 256 bytes",
        )?;
        let mut name = vec![0_u8; name_len];
        input.read_exact(&mut name).map_err(|error| Refusal {
            code: "ebook.fixed_layout.member",
            message: format!("truncated ZIP central filename: {error}"),
        })?;
        required(
            names.insert(name.to_ascii_lowercase()),
            "ebook.fixed_layout.member",
            "ZIP central directory repeats a member name",
        )?;
        input
            .seek(SeekFrom::Current((extra_len + comment_len) as i64))
            .map_err(|error| Refusal {
                code: "ebook.fixed_layout.member",
                message: format!("truncated ZIP central extra/comment: {error}"),
            })?;
    }
}

fn safe_member_name(value: &str) -> bool {
    !value.is_empty()
        && !value.starts_with('/')
        && !value.ends_with('/')
        && !value.contains("//")
        && value
            .split('/')
            .all(|part| part != "." && part != ".." && !part.is_empty())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'.' | b'_' | b'-'))
}

fn resolve_href(base: &str, href: &str) -> Checked<String> {
    if href.is_empty()
        || href.starts_with('/')
        || href.starts_with("//")
        || href.contains(['\\', ':', '%', '#', '?'])
    {
        return refuse(
            "ebook.fixed_layout.unsafe_markup",
            format!("unsafe or external resource locator: {href}"),
        );
    }
    let mut parts = base.split('/').collect::<Vec<_>>();
    parts.pop();
    for part in href.split('/') {
        match part {
            "" | "." => {
                return refuse(
                    "ebook.fixed_layout.unsafe_markup",
                    "empty or ambiguous resource path component",
                )
            }
            ".." => {
                if parts.pop().is_none() {
                    return refuse(
                        "ebook.fixed_layout.unsafe_markup",
                        "resource escapes the EPUB root",
                    );
                }
            }
            _ if part
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')) =>
            {
                parts.push(part)
            }
            _ => {
                return refuse(
                    "ebook.fixed_layout.unsafe_markup",
                    "resource contains unsupported path characters",
                )
            }
        }
    }
    required(
        !parts.is_empty(),
        "ebook.fixed_layout.unsafe_markup",
        "resource escapes EPUB root",
    )?;
    Ok(parts.join("/"))
}

fn read_text<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    code: &'static str,
) -> Checked<String> {
    let member = archive.by_name(name).map_err(|_| Refusal {
        code,
        message: format!("required ZIP member is missing: {name}"),
    })?;
    required(
        member.size() <= MAX_XML_BYTES,
        "ebook.fixed_layout.bounds",
        format!("XML/CSS member exceeds 2 MiB: {name}"),
    )?;
    let mut text = String::new();
    member
        .take(MAX_XML_BYTES + 1)
        .read_to_string(&mut text)
        .map_err(|error| Refusal {
            code,
            message: format!("unreadable UTF-8 member {name}: {error}"),
        })?;
    required(
        text.len() as u64 <= MAX_XML_BYTES,
        "ebook.fixed_layout.bounds",
        "XML/CSS read exceeded bound",
    )?;
    Ok(text)
}

fn parse_xml(text: &str) -> Checked<Document<'_>> {
    // This route emits only XML declarations, elements, text, and attributes.
    // DTD, entities, comments, CDATA, and other processing instructions are
    // deliberately refused before the parser sees them.
    required(
        !text.contains("<!")
            && text.matches("<?").count() <= 1
            && (!text.contains("<?") || text.starts_with("<?xml version=")),
        "ebook.fixed_layout.unsafe_markup",
        "DTD, entity, active, or unsupported XML declaration is refused",
    )?;
    let document = Document::parse(text).map_err(|error| Refusal {
        code: "ebook.fixed_layout.xml",
        message: format!("malformed XML: {error}"),
    })?;
    for node in document.descendants().filter(|node| node.is_element()) {
        required(
            !matches!(
                node.tag_name().name().to_ascii_lowercase().as_str(),
                "script" | "iframe" | "object" | "embed" | "foreignobject" | "audio" | "video"
            ),
            "ebook.fixed_layout.unsafe_markup",
            "active XML/XHTML element is not supported",
        )?;
        for attribute in node.attributes() {
            let name = attribute.name().to_ascii_lowercase();
            required(
                !name.starts_with("on")
                    && !matches!(
                        name.as_str(),
                        "base"
                            | "srcset"
                            | "style"
                            | "poster"
                            | "background"
                            | "action"
                            | "formaction"
                    ),
                "ebook.fixed_layout.unsafe_markup",
                "active or resource-bearing attribute is not supported",
            )?;
            if matches!(name.as_str(), "href" | "src" | "data") {
                let value = attribute.value();
                required(
                    !value.contains([':', '\\', '%', '#', '?'])
                        && !value.starts_with('/')
                        && !value.starts_with("//"),
                    "ebook.fixed_layout.unsafe_markup",
                    format!("external or ambiguous resource reference: {value}"),
                )?;
            }
        }
    }
    Ok(document)
}

fn element(node: Node<'_, '_>, namespace: &str, local_name: &str) -> bool {
    node.is_element()
        && node.tag_name().namespace() == Some(namespace)
        && node.tag_name().name() == local_name
}

fn only_attributes(node: Node<'_, '_>, allowed: &[(&str, Option<&str>)]) -> Checked<()> {
    required(
        node.attributes().all(|attribute| {
            allowed.iter().any(|(name, namespace)| {
                attribute.name() == *name && attribute.namespace() == *namespace
            })
        }),
        "ebook.fixed_layout.unsafe_markup",
        "element contains an undeclared attribute",
    )
}

fn one_child<'a, 'input>(
    node: Node<'a, 'input>,
    namespace: &str,
    local_name: &str,
    code: &'static str,
) -> Checked<Node<'a, 'input>> {
    let mut candidates = node
        .children()
        .filter(|child| element(*child, namespace, local_name));
    let first = candidates.next();
    if first.is_none() || candidates.next().is_some() {
        return refuse(
            code,
            format!("expected exactly one {local_name} child in the {namespace} namespace"),
        );
    }
    Ok(first.expect("checked above"))
}

fn require_meta(
    metadata: Node<'_, '_>,
    property: &str,
    accept: impl Fn(&str) -> bool,
    code: &'static str,
) -> Checked<()> {
    let values = metadata
        .children()
        .filter(|node| {
            element(*node, OPF_NS, "meta") && node.attribute("property") == Some(property)
        })
        .map(|node| node.text().unwrap_or_default().trim().to_string())
        .collect::<Vec<_>>();
    required(
        values.len() == 1 && accept(&values[0]),
        code,
        format!("missing, duplicated, or invalid {property} metadata"),
    )
}

fn inspect_image<R: Read + Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
    format: Format,
) -> Checked<(String, u32, u32)> {
    let mut member = archive.by_name(name).map_err(|_| Refusal {
        code: "ebook.fixed_layout.resource",
        message: format!("declared image is missing: {name}"),
    })?;
    required(
        member.size() <= MAX_IMAGE_BYTES,
        "ebook.fixed_layout.bounds",
        format!("image exceeds 128 MiB: {name}"),
    )?;
    let mut temporary = tempfile::NamedTempFile::new().map_err(|error| Refusal {
        code: "ebook.fixed_layout.resource",
        message: format!("cannot inspect image: {error}"),
    })?;
    let mut hasher = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = member.read(&mut buffer).map_err(|error| Refusal {
            code: "ebook.fixed_layout.resource",
            message: format!("corrupt image member {name}: {error}"),
        })?;
        if count == 0 {
            break;
        }
        copied = copied.checked_add(count as u64).ok_or_else(|| Refusal {
            code: "ebook.fixed_layout.bounds",
            message: "image size overflow".to_string(),
        })?;
        required(
            copied <= MAX_IMAGE_BYTES,
            "ebook.fixed_layout.bounds",
            "image exceeded read bound",
        )?;
        hasher.update(&buffer[..count]);
        temporary
            .write_all(&buffer[..count])
            .map_err(|error| Refusal {
                code: "ebook.fixed_layout.resource",
                message: format!("cannot stage inspected image: {error}"),
            })?;
    }
    required(
        copied == member.size(),
        "ebook.fixed_layout.resource",
        "image size differs from ZIP declaration",
    )?;
    let info = inspect_print_image(temporary.path(), format).map_err(|error| Refusal {
        code: "ebook.fixed_layout.resource",
        message: format!("image {name} failed bounded PNG/JPEG preflight: {error:#}"),
    })?;
    Ok((
        format!("{:x}", hasher.finalize()),
        info.width_px,
        info.height_px,
    ))
}

fn validate_page(
    document: &Document<'_>,
    page_path: &str,
    image_path: &str,
    width_px: u32,
    height_px: u32,
) -> Checked<()> {
    let html = document.root_element();
    required(
        element(html, XHTML_NS, "html"),
        "ebook.fixed_layout.xml",
        "page root must be XHTML",
    )?;
    only_attributes(html, &[("lang", Some(XML_NS))])?;
    required(
        html.children().filter(|node| node.is_element()).count() == 2,
        "ebook.fixed_layout.unsafe_markup",
        "page document contains undeclared markup outside head and body",
    )?;
    let head = one_child(html, XHTML_NS, "head", "ebook.fixed_layout.xml")?;
    let body = one_child(html, XHTML_NS, "body", "ebook.fixed_layout.xml")?;
    only_attributes(head, &[])?;
    only_attributes(body, &[])?;
    required(
        head.children().filter(|node| node.is_element()).count() == 3
            && one_child(head, XHTML_NS, "title", "ebook.fixed_layout.xml")?
                .text()
                .is_some_and(|text| !text.trim().is_empty()),
        "ebook.fixed_layout.unsafe_markup",
        "page head contains unsupported markup or no title",
    )?;
    only_attributes(
        one_child(head, XHTML_NS, "title", "ebook.fixed_layout.xml")?,
        &[],
    )?;
    let viewport = one_child(head, XHTML_NS, "meta", "ebook.fixed_layout.viewport")?;
    only_attributes(viewport, &[("name", None), ("content", None)])?;
    required(
        viewport.attribute("name") == Some("viewport")
            && viewport.attribute("content")
                == Some(format!("width={width_px}, height={height_px}").as_str()),
        "ebook.fixed_layout.viewport",
        "page viewport differs from the decoded PNG/JPEG dimensions",
    )?;
    let style = one_child(head, XHTML_NS, "link", "ebook.fixed_layout.resource")?;
    only_attributes(style, &[("rel", None), ("type", None), ("href", None)])?;
    required(
        style.attribute("rel") == Some("stylesheet")
            && style.attribute("type") == Some("text/css")
            && resolve_href(page_path, style.attribute("href").unwrap_or_default())?
                == "EPUB/styles.css",
        "ebook.fixed_layout.resource",
        "page stylesheet is not the declared local stylesheet",
    )?;
    let image = one_child(body, XHTML_NS, "img", "ebook.fixed_layout.resource")?;
    only_attributes(image, &[("class", None), ("src", None), ("alt", None)])?;
    required(
        resolve_href(page_path, image.attribute("src").unwrap_or_default())? == image_path,
        "ebook.fixed_layout.resource",
        "page image reference does not match its ordered manifest image",
    )?;
    required(
        image
            .attribute("alt")
            .is_some_and(|value| !value.trim().is_empty() && value.len() <= 4096),
        "ebook.fixed_layout.accessibility",
        "page image is missing its bounded accessible description",
    )?;
    required(
        body.children().filter(|node| node.is_element()).count() == 1,
        "ebook.fixed_layout.unsafe_markup",
        "page body contains undeclared content or resources",
    )
}

fn validate_navigation(document: &Document<'_>, pages: &[FixedLayoutPageEvidence]) -> Checked<()> {
    let html = document.root_element();
    required(
        element(html, XHTML_NS, "html"),
        "ebook.fixed_layout.navigation",
        "navigation document must be XHTML",
    )?;
    only_attributes(html, &[("lang", Some(XML_NS))])?;
    let head = one_child(html, XHTML_NS, "head", "ebook.fixed_layout.navigation")?;
    only_attributes(head, &[])?;
    required(
        head.children().filter(|node| node.is_element()).count() == 1
            && one_child(head, XHTML_NS, "title", "ebook.fixed_layout.navigation")?
                .text()
                .is_some_and(|text| !text.trim().is_empty()),
        "ebook.fixed_layout.navigation",
        "navigation head must have only a publication title",
    )?;
    only_attributes(
        one_child(head, XHTML_NS, "title", "ebook.fixed_layout.navigation")?,
        &[],
    )?;
    let body = one_child(html, XHTML_NS, "body", "ebook.fixed_layout.navigation")?;
    only_attributes(body, &[])?;
    required(
        html.children().filter(|node| node.is_element()).count() == 2
            && body.children().filter(|node| node.is_element()).count() == 2,
        "ebook.fixed_layout.navigation",
        "navigation document contains undeclared markup",
    )?;
    for kind in ["toc", "page-list"] {
        let candidates = body
            .children()
            .filter(|node| {
                element(*node, XHTML_NS, "nav") && node.attribute((EPUB_NS, "type")) == Some(kind)
            })
            .collect::<Vec<_>>();
        required(
            candidates.len() == 1,
            "ebook.fixed_layout.navigation",
            format!("navigation requires exactly one {kind}"),
        )?;
        let nav = candidates[0];
        only_attributes(nav, &[("type", Some(EPUB_NS)), ("id", None)])?;
        let heading = if kind == "toc" { "h1" } else { "h2" };
        let title = one_child(nav, XHTML_NS, heading, "ebook.fixed_layout.navigation")?;
        let list = one_child(nav, XHTML_NS, "ol", "ebook.fixed_layout.navigation")?;
        only_attributes(title, &[])?;
        only_attributes(list, &[])?;
        required(
            nav.children().filter(|node| node.is_element()).count() == 2
                && title.text().is_some_and(|text| !text.trim().is_empty()),
            "ebook.fixed_layout.navigation",
            "navigation has unsupported content or an empty heading",
        )?;
        let entries = list
            .children()
            .filter(|node| node.is_element())
            .collect::<Vec<_>>();
        required(
            entries.len() == pages.len(),
            "ebook.fixed_layout.navigation",
            format!("{kind} links do not cover all ordered pages"),
        )?;
        for (index, (entry, page)) in entries.iter().zip(pages).enumerate() {
            only_attributes(*entry, &[])?;
            required(
                element(*entry, XHTML_NS, "li")
                    && entry.children().filter(|node| node.is_element()).count() == 1,
                "ebook.fixed_layout.navigation",
                "navigation list must contain only single-link entries",
            )?;
            let link = one_child(*entry, XHTML_NS, "a", "ebook.fixed_layout.navigation")?;
            only_attributes(link, &[("href", None)])?;
            required(
                resolve_href("EPUB/nav.xhtml", link.attribute("href").unwrap_or_default())?
                    == page.page
                    && !link.text().unwrap_or_default().trim().is_empty(),
                "ebook.fixed_layout.navigation",
                format!("{kind} link {} differs from spine order", index + 1),
            )?;
        }
    }
    Ok(())
}
