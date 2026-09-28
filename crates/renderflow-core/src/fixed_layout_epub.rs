//! Deterministic EPUB 3.3 fixed-layout packaging for a bounded ordered raster
//! collection. The input images are copied byte-for-byte into a local EPUB;
//! generated XHTML supplies one page and a declared viewport per image.

use std::collections::HashSet;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use anyhow::Result;
use serde_json::json;
use sha2::{Digest, Sha256};
use zip::{write::SimpleFileOptions, CompressionMethod, DateTime, ZipArchive, ZipWriter};

use crate::artifact::{
    Artifact, ArtifactCollection, ArtifactCollectionTransform, ArtifactDescriptor,
    ArtifactStorageClass, ArtifactStore,
};
use crate::evidence::FidelityDeclaration;
use crate::graph::Format;
use crate::print_pdf_image::inspect_print_image;
use crate::publication::PublicationContract;
use crate::spec::FixedLayoutEpubPolicy;

pub const FIXED_EPUB_CAPABILITY: &str = "ebook.generate.epub.fixed-layout";
pub const FIXED_EPUB_PROVIDER: &str = "tool.renderflow-epub";

const MIMETYPE: &[u8] = b"application/epub+zip";
const CONTAINER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\"><rootfiles><rootfile full-path=\"EPUB/book.opf\" media-type=\"application/oebps-package+xml\"/></rootfiles></container>\n";
const CSS: &str = "@charset \"UTF-8\";\nhtml,body{width:100%;height:100%;margin:0;padding:0;}\nbody{overflow:hidden;}\nimg.page{display:block;width:100%;height:100%;object-fit:contain;}\n";
pub(crate) fn generated_stylesheet() -> &'static str {
    CSS
}
const BUFFER_SIZE: usize = 64 * 1024;

/// A machine-readable refusal that can be propagated into DAG step evidence.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {message}")]
pub struct FixedEpubError {
    pub code: &'static str,
    pub message: String,
    pub cancelled: bool,
}

fn refusal(code: &'static str, message: impl Into<String>) -> anyhow::Error {
    FixedEpubError {
        code,
        message: message.into(),
        cancelled: code == "fixed_epub.cancelled",
    }
    .into()
}

fn zip_write_error(error: impl std::fmt::Display) -> anyhow::Error {
    let message = error.to_string();
    refusal(
        if message.contains("fixed_epub.output.bounds") {
            "fixed_epub.output.bounds"
        } else {
            "fixed_epub.output.write"
        },
        message,
    )
}

/// Planned identity and accessible label for one immutable raster page.
#[derive(Debug, Clone)]
pub struct FixedEpubPage {
    pub source_id: String,
    pub format: Format,
    pub width_px: u32,
    pub height_px: u32,
    pub alt_text: String,
    /// Lowercase hexadecimal SHA-256 of the exact PNG/JPEG bytes.
    pub digest: String,
}

pub struct FixedLayoutEpubTransform {
    policy: FixedLayoutEpubPolicy,
    publication: PublicationContract,
    pages: Vec<FixedEpubPage>,
    input_digests: Vec<String>,
    cache_identity: String,
    cancellation: Option<Arc<AtomicBool>>,
}

impl FixedLayoutEpubTransform {
    pub fn new(
        policy: FixedLayoutEpubPolicy,
        publication: PublicationContract,
        pages: Vec<FixedEpubPage>,
        input_digests: Vec<String>,
        cancellation: Option<Arc<AtomicBool>>,
    ) -> Result<Self> {
        policy
            .validate()
            .map_err(|error| refusal("fixed_epub.policy", error.to_string()))?;
        if pages.is_empty() || pages.len() > policy.max_pages || pages.len() != input_digests.len()
        {
            return Err(refusal(
                "fixed_epub.bounds",
                "ordered pages and digests must be nonempty, have matching counts, and fit max_pages",
            ));
        }
        if pages[0].source_id != policy.cover_member_id {
            return Err(refusal(
                "fixed_epub.cover",
                "cover_member_id must name the first ordered page",
            ));
        }
        validate_publication(&publication)?;
        let mut source_ids = HashSet::new();
        for (index, (page, digest)) in pages.iter().zip(&input_digests).enumerate() {
            if !matches!(page.format, Format::Png | Format::Jpeg) {
                return Err(refusal(
                    "fixed_epub.image.unsupported_format",
                    format!("page {} requires a PNG or JPEG source; SVG and other formats are not accepted", index + 1),
                ));
            }
            if page.source_id.trim().is_empty() || !source_ids.insert(page.source_id.as_str()) {
                return Err(refusal(
                    "fixed_epub.input.identity",
                    "page source IDs must be nonempty and unique",
                ));
            }
            if page.width_px == 0
                || page.height_px == 0
                || page.width_px > 100_000
                || page.height_px > 100_000
            {
                return Err(refusal(
                    "fixed_epub.image.geometry",
                    "page dimensions must be positive and within the image preflight limit",
                ));
            }
            ensure_xml_text(&page.alt_text, "fixed_epub.accessibility.alt_text")?;
            if page.alt_text.trim().is_empty() {
                return Err(refusal(
                    "fixed_epub.accessibility.alt_text",
                    format!("page {} has no reviewed alt text", index + 1),
                ));
            }
            if !valid_digest(&page.digest) || strip_sha256_prefix(digest) != page.digest {
                return Err(refusal(
                    "fixed_epub.input.identity",
                    format!(
                        "page {} digest does not match its frozen input identity",
                        index + 1
                    ),
                ));
            }
        }
        let cache_identity = serde_json::to_string(&json!({
            "policy": policy,
            "publication": publication,
            "pages": pages.iter().map(|page| json!({
                "source_id": page.source_id,
                "format": page.format.to_string(),
                "width_px": page.width_px,
                "height_px": page.height_px,
                "alt_text": page.alt_text,
                "digest": page.digest,
            })).collect::<Vec<_>>(),
            "input_digests": input_digests,
            "package_version": 1,
        }))?;
        Ok(Self {
            policy,
            publication,
            pages,
            input_digests,
            cache_identity,
            cancellation,
        })
    }

    fn check_cancelled(&self) -> Result<()> {
        if self
            .cancellation
            .as_ref()
            .is_some_and(|flag| flag.load(Ordering::SeqCst))
        {
            Err(refusal(
                "fixed_epub.cancelled",
                "EPUB packaging was cancelled before artifact import",
            ))
        } else {
            Ok(())
        }
    }

    fn write_epub(
        &self,
        inputs: &ArtifactCollection,
        store: &ArtifactStore,
        output: &mut File,
    ) -> Result<()> {
        let bounded = BoundedWriter::new(output, self.policy.max_output_bytes);
        let mut zip = ZipWriter::new(bounded);
        // Stored entries, a fixed DOS timestamp, fixed mode, fixed order, and
        // no arbitrary source file names make clean builds byte-identical.
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .last_modified_time(DateTime::default())
            .unix_permissions(0o644);

        write_entry(&mut zip, "mimetype", MIMETYPE, options)?;
        write_entry(
            &mut zip,
            "META-INF/container.xml",
            CONTAINER.as_bytes(),
            options,
        )?;
        write_entry(&mut zip, "EPUB/book.opf", self.opf()?.as_bytes(), options)?;
        write_entry(&mut zip, "EPUB/nav.xhtml", self.nav().as_bytes(), options)?;
        write_entry(&mut zip, "EPUB/styles.css", CSS.as_bytes(), options)?;

        for (index, (page, artifact)) in self.pages.iter().zip(inputs.iter()).enumerate() {
            self.check_cancelled()?;
            if artifact.format().as_str() != page.format.to_string()
                || artifact.digest().value() != page.digest
            {
                return Err(refusal(
                    "fixed_epub.input.changed",
                    format!("page {} does not match its frozen format/digest", index + 1),
                ));
            }
            if strip_sha256_prefix(&self.input_digests[index]) != artifact.digest().value() {
                return Err(refusal(
                    "fixed_epub.input.changed",
                    format!("page {} input digest changed", index + 1),
                ));
            }
            let input_path = store
                .payload_path(artifact)
                .map_err(|error| refusal("fixed_epub.input.unavailable", error.to_string()))?;
            let metadata = input_path
                .symlink_metadata()
                .map_err(|error| refusal("fixed_epub.input.unavailable", error.to_string()))?;
            if !metadata.file_type().is_file() || metadata.len() != artifact.size_bytes() {
                return Err(refusal(
                    "fixed_epub.input.changed",
                    format!(
                        "page {} source is not a regular file of the planned size",
                        index + 1
                    ),
                ));
            }
            let image = inspect_print_image(&input_path, page.format).map_err(|error| {
                refusal(
                    "fixed_epub.image.unreadable",
                    format!("page {}: {error:#}", index + 1),
                )
            })?;
            if image.width_px != page.width_px || image.height_px != page.height_px {
                return Err(refusal(
                    "fixed_epub.image.geometry",
                    format!("page {} dimensions changed from the plan", index + 1),
                ));
            }

            let number = index + 1;
            write_entry(
                &mut zip,
                &format!("EPUB/pages/page-{number:04}.xhtml"),
                self.page_xhtml(number, page).as_bytes(),
                options,
            )?;
            zip.start_file(
                format!("EPUB/images/page-{number:04}.{}", extension(page.format)),
                options,
            )
            .map_err(zip_write_error)?;
            let mut file = File::open(&input_path)
                .map_err(|error| refusal("fixed_epub.input.unavailable", error.to_string()))?;
            let mut digest = Sha256::new();
            let mut copied = 0_u64;
            let mut buffer = [0u8; BUFFER_SIZE];
            loop {
                self.check_cancelled()?;
                let read = file
                    .read(&mut buffer)
                    .map_err(|error| refusal("fixed_epub.input.unavailable", error.to_string()))?;
                if read == 0 {
                    break;
                }
                copied = copied
                    .checked_add(read as u64)
                    .ok_or_else(|| refusal("fixed_epub.bounds", "source size overflow"))?;
                if copied > artifact.size_bytes() {
                    return Err(refusal(
                        "fixed_epub.input.changed",
                        format!("page {} grew during packaging", number),
                    ));
                }
                digest.update(&buffer[..read]);
                zip.write_all(&buffer[..read]).map_err(zip_write_error)?;
            }
            if copied != artifact.size_bytes() || format!("{:x}", digest.finalize()) != page.digest
            {
                return Err(refusal(
                    "fixed_epub.input.changed",
                    format!("page {} bytes changed during packaging", number),
                ));
            }
        }
        self.check_cancelled()?;
        zip.finish().map_err(zip_write_error)?;
        Ok(())
    }

    fn opf(&self) -> Result<String> {
        let title = xml_escape(&self.publication.title);
        let id = xml_escape(&self.publication.issue_id);
        let lang = xml_escape(&self.publication.language);
        let rights = xml_escape(
            self.publication
                .rights
                .rights_holder
                .as_deref()
                .unwrap_or_default(),
        );
        let license = xml_escape(
            self.publication
                .rights
                .license
                .as_deref()
                .unwrap_or_default(),
        );
        let summary = xml_escape(
            self.publication
                .accessibility
                .summary
                .as_deref()
                .unwrap_or_default(),
        );
        let modified = modified_date(&self.publication.publication_date)?;
        let mut xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<package xmlns=\"http://www.idpf.org/2007/opf\" version=\"3.0\" unique-identifier=\"pub-id\" prefix=\"dcterms: http://purl.org/dc/terms/ rendition: http://www.idpf.org/vocab/rendition/# schema: http://schema.org/\">\n<metadata xmlns:dc=\"http://purl.org/dc/elements/1.1/\">\n<dc:identifier id=\"pub-id\">{id}</dc:identifier>\n<dc:title>{title}</dc:title>\n<dc:language>{lang}</dc:language>\n<dc:date>{}</dc:date>\n<dc:rights>{rights}; {license}</dc:rights>\n<meta property=\"dcterms:modified\">{modified}</meta>\n<meta property=\"rendition:layout\">pre-paginated</meta>\n<meta property=\"rendition:spread\">none</meta>\n<meta property=\"schema:accessibilityFeature\">tableOfContents</meta>\n<meta property=\"schema:accessibilityFeature\">pageNavigation</meta>\n<meta property=\"schema:accessibilitySummary\">{summary}</meta>\n", xml_escape(&self.publication.publication_date));
        for (index, contributor) in self.publication.contributors.iter().enumerate() {
            let element = if contributor.role.eq_ignore_ascii_case("author")
                || contributor.role.eq_ignore_ascii_case("creator")
            {
                "dc:creator"
            } else {
                "dc:contributor"
            };
            let number = index + 1;
            xml.push_str(&format!(
                "<{element} id=\"person-{number:04}\">{}</{element}>\n<meta refines=\"#person-{number:04}\" property=\"role\">{}</meta>\n",
                xml_escape(&contributor.name),
                xml_escape(&contributor.role)
            ));
        }
        for mode in &self.publication.accessibility.access_modes {
            xml.push_str(&format!(
                "<meta property=\"schema:accessMode\">{}</meta>\n",
                xml_escape(mode)
            ));
        }
        for hazard in &self.publication.accessibility.hazards {
            xml.push_str(&format!(
                "<meta property=\"schema:accessibilityHazard\">{}</meta>\n",
                xml_escape(hazard)
            ));
        }
        xml.push_str("</metadata>\n<manifest>\n<item id=\"nav\" href=\"nav.xhtml\" media-type=\"application/xhtml+xml\" properties=\"nav\"/>\n<item id=\"styles\" href=\"styles.css\" media-type=\"text/css\"/>\n");
        for (index, page) in self.pages.iter().enumerate() {
            let number = index + 1;
            xml.push_str(&format!("<item id=\"page-{number:04}\" href=\"pages/page-{number:04}.xhtml\" media-type=\"application/xhtml+xml\"/>\n"));
            let cover = if index == 0 {
                " properties=\"cover-image\""
            } else {
                ""
            };
            xml.push_str(&format!("<item id=\"image-{number:04}\" href=\"images/page-{number:04}.{}\" media-type=\"{}\"{cover}/>\n", extension(page.format), media_type(page.format)));
        }
        xml.push_str(&format!(
            "</manifest>\n<spine page-progression-direction=\"{}\">\n",
            self.policy.page_progression_direction
        ));
        for number in 1..=self.pages.len() {
            xml.push_str(&format!("<itemref idref=\"page-{number:04}\"/>\n"));
        }
        xml.push_str("</spine>\n</package>\n");
        Ok(xml)
    }

    fn nav(&self) -> String {
        let mut xml = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:epub=\"http://www.idpf.org/2007/ops\" xml:lang=\"{}\"><head><title>{}</title></head><body>\n<nav epub:type=\"toc\" id=\"toc\"><h1>Contents</h1><ol>\n", xml_escape(&self.publication.language), xml_escape(&self.publication.title));
        for number in 1..=self.pages.len() {
            xml.push_str(&format!(
                "<li><a href=\"pages/page-{number:04}.xhtml\">Page {number}</a></li>\n"
            ));
        }
        xml.push_str("</ol></nav>\n<nav epub:type=\"page-list\"><h2>Pages</h2><ol>\n");
        for number in 1..=self.pages.len() {
            xml.push_str(&format!(
                "<li><a href=\"pages/page-{number:04}.xhtml\">{number}</a></li>\n"
            ));
        }
        xml.push_str("</ol></nav>\n</body></html>\n");
        xml
    }

    fn page_xhtml(&self, number: usize, page: &FixedEpubPage) -> String {
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<html xmlns=\"http://www.w3.org/1999/xhtml\" xml:lang=\"{}\"><head><title>Page {number}</title><meta name=\"viewport\" content=\"width={}, height={}\"/><link rel=\"stylesheet\" type=\"text/css\" href=\"../styles.css\"/></head><body><img class=\"page\" src=\"../images/page-{number:04}.{}\" alt=\"{}\"/></body></html>\n", xml_escape(&self.publication.language), page.width_px, page.height_px, extension(page.format), xml_escape(&page.alt_text))
    }
}

impl ArtifactCollectionTransform for FixedLayoutEpubTransform {
    fn name(&self) -> &str {
        FIXED_EPUB_CAPABILITY
    }

    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    fn cache_identity(&self) -> String {
        self.cache_identity.clone()
    }

    fn fidelity(&self) -> Option<FidelityDeclaration> {
        Some(FidelityDeclaration::Lossless)
    }

    fn apply(
        &self,
        inputs: &ArtifactCollection,
        output_format: Format,
        store: &ArtifactStore,
    ) -> Result<Artifact> {
        if output_format != Format::Epub || inputs.len() != self.pages.len() {
            return Err(refusal(
                "fixed_epub.input.count",
                "expected one EPUB target and the exact ordered page collection",
            ));
        }
        self.check_cancelled()?;
        let mut total = 0u64;
        for input in inputs.iter() {
            total = total
                .checked_add(input.size_bytes())
                .ok_or_else(|| refusal("fixed_epub.bounds", "source sizes overflowed u64"))?;
            if total > self.policy.max_input_bytes {
                return Err(refusal(
                    "fixed_epub.bounds",
                    "source bytes exceed max_input_bytes",
                ));
            }
        }
        let mut temporary = tempfile::NamedTempFile::new_in(store.temporary_directory())
            .map_err(|error| refusal("fixed_epub.output.temporary", error.to_string()))?;
        self.write_epub(inputs, store, temporary.as_file_mut())?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|error| refusal("fixed_epub.output.sync", error.to_string()))?;
        let size = temporary
            .as_file()
            .metadata()
            .map_err(|error| refusal("fixed_epub.output.write", error.to_string()))?
            .len();
        if size == 0 || size > self.policy.max_output_bytes {
            return Err(refusal(
                "fixed_epub.output.bounds",
                "packaged EPUB exceeds max_output_bytes",
            ));
        }
        inspect_container(&temporary, self.pages.len())?;
        self.check_cancelled()?;
        let ordered_pages = self
            .pages
            .iter()
            .enumerate()
            .map(|(index, page)| {
                json!({
                    "index": index, "source_id": page.source_id, "sha256": page.digest,
                    "width_px": page.width_px, "height_px": page.height_px,
                    "format": page.format.to_string(),
                })
            })
            .collect::<Vec<_>>();
        store
            .import_path(
                temporary.path(),
                ArtifactDescriptor::for_format(Format::Epub, ArtifactStorageClass::Intermediate)
                    .with_sources(inputs.iter().map(|artifact| artifact.id().clone()))
                    .with_metadata("renderflow.transform", FIXED_EPUB_CAPABILITY)
                    .with_metadata("renderflow.fixed_epub.provider", FIXED_EPUB_PROVIDER)
                    .with_metadata(
                        "renderflow.fixed_epub.policy",
                        serde_json::to_value(&self.policy)?,
                    )
                    .with_metadata("renderflow.fixed_epub.layout", "pre-paginated")
                    .with_metadata("renderflow.fixed_epub.package_version", "3.0")
                    .with_metadata(
                        "renderflow.fixed_epub.publication_id",
                        self.publication.issue_id.clone(),
                    )
                    .with_metadata("renderflow.fixed_epub.ordered_pages", json!(ordered_pages)),
            )
            .map_err(|error| refusal("fixed_epub.output.import", error.to_string()))
    }
}

fn write_entry<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    name: &str,
    bytes: &[u8],
    options: SimpleFileOptions,
) -> Result<()> {
    zip.start_file(name, options).map_err(zip_write_error)?;
    zip.write_all(bytes).map_err(zip_write_error)?;
    Ok(())
}

fn inspect_container(temporary: &tempfile::NamedTempFile, expected_pages: usize) -> Result<()> {
    let file = temporary
        .reopen()
        .map_err(|error| refusal("fixed_epub.output.invalid", error.to_string()))?;
    let mut zip = ZipArchive::new(file)
        .map_err(|error| refusal("fixed_epub.output.invalid", error.to_string()))?;
    if zip.len() != 5 + 2 * expected_pages {
        return Err(refusal(
            "fixed_epub.output.invalid",
            "ZIP entry count differs from the package contract",
        ));
    }
    let mut mimetype = zip
        .by_index(0)
        .map_err(|error| refusal("fixed_epub.output.invalid", error.to_string()))?;
    if mimetype.name() != "mimetype"
        || mimetype.compression() != CompressionMethod::Stored
        || mimetype.extra_data().is_some_and(|extra| !extra.is_empty())
        || mimetype.data_start() != 38
    {
        return Err(refusal(
            "fixed_epub.output.invalid",
            "mimetype is not the first, stored, extra-free ZIP entry",
        ));
    }
    let mut bytes = Vec::new();
    mimetype
        .read_to_end(&mut bytes)
        .map_err(|error| refusal("fixed_epub.output.invalid", error.to_string()))?;
    if bytes != MIMETYPE {
        return Err(refusal(
            "fixed_epub.output.invalid",
            "mimetype entry does not contain the exact EPUB media type",
        ));
    }
    Ok(())
}

/// Validate the exact metadata envelope required by this fixed-layout route.
/// Planning calls this before executing so refusals retain a planning cause.
pub(crate) fn validate_publication(publication: &PublicationContract) -> Result<()> {
    let required = [
        ("issue_id", publication.issue_id.as_str()),
        ("title", publication.title.as_str()),
        ("language", publication.language.as_str()),
        ("publication_date", publication.publication_date.as_str()),
        (
            "rights_holder",
            publication
                .rights
                .rights_holder
                .as_deref()
                .unwrap_or_default(),
        ),
        (
            "license",
            publication.rights.license.as_deref().unwrap_or_default(),
        ),
        (
            "accessibility_summary",
            publication
                .accessibility
                .summary
                .as_deref()
                .unwrap_or_default(),
        ),
    ];
    for (name, value) in required {
        ensure_xml_text(value, "fixed_epub.metadata.invalid")?;
        if value.trim().is_empty() {
            return Err(refusal(
                "fixed_epub.metadata.missing",
                format!("publication {name} is required"),
            ));
        }
    }
    if publication.contributors.is_empty()
        || publication.accessibility.access_modes.is_empty()
        || publication.accessibility.hazards.is_empty()
    {
        return Err(refusal(
            "fixed_epub.metadata.missing",
            "contributors, access modes, and accessibility hazards are required",
        ));
    }
    for contributor in &publication.contributors {
        ensure_xml_text(&contributor.name, "fixed_epub.metadata.invalid")?;
        ensure_xml_text(&contributor.role, "fixed_epub.metadata.invalid")?;
        if contributor.name.trim().is_empty() || contributor.role.trim().is_empty() {
            return Err(refusal(
                "fixed_epub.metadata.missing",
                "contributor name and role are required",
            ));
        }
    }
    for value in publication
        .accessibility
        .access_modes
        .iter()
        .chain(&publication.accessibility.hazards)
    {
        ensure_xml_text(value, "fixed_epub.metadata.invalid")?;
        if value.trim().is_empty() {
            return Err(refusal(
                "fixed_epub.metadata.missing",
                "access modes and hazards must be nonempty",
            ));
        }
    }
    if !publication
        .accessibility
        .access_modes
        .iter()
        .any(|mode| mode == "visual")
        || publication
            .accessibility
            .access_modes
            .iter()
            .any(|mode| mode != "visual")
    {
        return Err(refusal("fixed_epub.accessibility.claim", "raster-only pages support only the declared visual access mode; other modes need independent reviewed content"));
    }
    modified_date(&publication.publication_date)?;
    Ok(())
}

fn modified_date(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    let valid_date = bytes.len() >= 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
        && valid_calendar_date(bytes);
    if !valid_date || (bytes.len() != 10 && !valid_utc_timestamp(bytes)) {
        return Err(refusal(
            "fixed_epub.metadata.date",
            "publication_date must be YYYY-MM-DD or YYYY-MM-DDThh:mm:ssZ",
        ));
    }
    Ok(if bytes.len() == 10 {
        format!("{value}T00:00:00Z")
    } else {
        value.to_string()
    })
}

fn valid_calendar_date(bytes: &[u8]) -> bool {
    let year = std::str::from_utf8(&bytes[0..4])
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(0);
    let month = parse_u8(&bytes[5..7]);
    let day = parse_u8(&bytes[8..10]);
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    year != 0 && (1..=max_day).contains(&day)
}

fn valid_utc_timestamp(bytes: &[u8]) -> bool {
    bytes.len() == 20
        && bytes[10] == b'T'
        && bytes[11..13].iter().all(u8::is_ascii_digit)
        && bytes[13] == b':'
        && bytes[14..16].iter().all(u8::is_ascii_digit)
        && bytes[16] == b':'
        && bytes[17..19].iter().all(u8::is_ascii_digit)
        && bytes[19] == b'Z'
        && (0..=23).contains(&parse_u8(&bytes[11..13]))
        && (0..=59).contains(&parse_u8(&bytes[14..16]))
        && (0..=59).contains(&parse_u8(&bytes[17..19]))
}

fn parse_u8(bytes: &[u8]) -> u8 {
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(255)
}

fn ensure_xml_text(text: &str, code: &'static str) -> Result<()> {
    if text.chars().any(|ch| !matches!(ch as u32, 0x9 | 0xA | 0xD | 0x20..=0xD7FF | 0xE000..=0xFFFD | 0x10000..=0x10FFFF)) {
        return Err(refusal(code, "publication text contains a character XML 1.0 cannot represent"));
    }
    Ok(())
}

fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn valid_digest(digest: &str) -> bool {
    digest.len() == 64
        && digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn strip_sha256_prefix(digest: &str) -> &str {
    digest.strip_prefix("sha256:").unwrap_or(digest)
}

fn extension(format: Format) -> &'static str {
    match format {
        Format::Png => "png",
        Format::Jpeg => "jpg",
        _ => unreachable!("validated page format"),
    }
}

fn media_type(format: Format) -> &'static str {
    match format {
        Format::Png => "image/png",
        Format::Jpeg => "image/jpeg",
        _ => unreachable!("validated page format"),
    }
}

/// Enforce the bound *during* ZIP writes, including headers and the central
/// directory. ZipWriter seeks backwards when completing local file headers.
struct BoundedWriter<W> {
    inner: W,
    max_bytes: u64,
}

impl<W> BoundedWriter<W> {
    fn new(inner: W, max_bytes: u64) -> Self {
        Self { inner, max_bytes }
    }
}

impl<W: Write + Seek> Write for BoundedWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let position = self.inner.stream_position()?;
        if position
            .checked_add(bytes.len() as u64)
            .is_none_or(|end| end > self.max_bytes)
        {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                "fixed_epub.output.bounds: ZIP exceeds max_output_bytes",
            ));
        }
        self.inner.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl<W: Seek> Seek for BoundedWriter<W> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        // Backward header rewrites are expected. The next write is independently
        // bounded, so seeking alone can never enlarge the resulting file.
        self.inner.seek(position)
    }
}
