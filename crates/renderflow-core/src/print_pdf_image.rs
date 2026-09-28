//! Bounded, read-only preflight for images embedded without raster conversion.
//!
//! This module deliberately accepts a narrow subset. A caller must still bind the
//! inspected bytes to the planned source digest before writing a publication.

use std::fs::File;
use std::io::{Read, Take};
use std::path::Path;

use anyhow::{bail, Context, Result};
use flate2::read::ZlibDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::graph::Format;

const MAX_IMAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;
const MAX_DIMENSION: u32 = 100_000;
const MAX_PNG_CHUNKS: usize = 65_536;

/// Pixel space used by the PDF image XObject. JPEG's YCbCr is decoded to RGB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PrintImageColorSpace {
    #[serde(rename = "RGB")]
    Rgb,
    #[serde(rename = "Gray")]
    Gray,
}

/// The immutable image facts frozen into an image-only print PDF plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrintImageInfo {
    pub width_px: u32,
    pub height_px: u32,
    pub color_space: PrintImageColorSpace,
    /// SHA-256 over the bytes passed to the PDF image stream: complete JPEG or
    /// concatenated PNG IDAT payloads (including the zlib wrapper).
    pub image_stream_sha256: String,
}

/// Inspect one local PNG or JPEG without modifying or transcoding it.
///
/// The file read is bounded even if another process grows it during inspection.
/// Input ownership, root confinement, and planned-source digest checks belong to
/// the caller. Structural rejection uses stable `print_pdf.image.*` prefixes.
pub fn inspect_print_image(path: &Path, format: Format) -> Result<PrintImageInfo> {
    if !matches!(format, Format::Png | Format::Jpeg) {
        bail!("print_pdf.image.unsupported_format: expected PNG or JPEG, got {format}");
    }
    let metadata = path.symlink_metadata().with_context(|| {
        format!(
            "print_pdf.image.read_failed: cannot stat {}",
            path.display()
        )
    })?;
    if !metadata.file_type().is_file() {
        bail!("print_pdf.image.invalid_source: expected a regular image file");
    }
    if metadata.len() > MAX_IMAGE_BYTES {
        bail!("print_pdf.image.size_limit: image exceeds 128 MiB");
    }
    let file = File::open(path).with_context(|| {
        format!(
            "print_pdf.image.read_failed: cannot open {}",
            path.display()
        )
    })?;
    let mut bytes = Vec::new();
    let mut bounded: Take<File> = file.take(MAX_IMAGE_BYTES + 1);
    bounded.read_to_end(&mut bytes).with_context(|| {
        format!(
            "print_pdf.image.read_failed: cannot read {}",
            path.display()
        )
    })?;
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        bail!("print_pdf.image.size_limit: image exceeds 128 MiB");
    }
    match format {
        Format::Png => inspect_png(&bytes),
        Format::Jpeg => inspect_jpeg(&bytes),
        _ => unreachable!(),
    }
}

fn checked_dimensions(width: u32, height: u32) -> Result<()> {
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        bail!("print_pdf.image.dimensions: zero or out-of-range pixel dimensions");
    }
    Ok(())
}

fn inspect_png(bytes: &[u8]) -> Result<PrintImageInfo> {
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        bail!("print_pdf.image.png_header: invalid PNG signature");
    }
    let mut pos = 8usize;
    let mut dimensions = None;
    let mut idat = Vec::new();
    let mut saw_idat = false;
    let mut after_idat = false;
    let mut saw_end = false;
    let mut chunk_count = 0usize;
    while pos < bytes.len() {
        chunk_count += 1;
        if chunk_count > MAX_PNG_CHUNKS {
            bail!("print_pdf.image.png_chunks: too many PNG chunks");
        }
        let header = bytes
            .get(pos..pos.saturating_add(8))
            .ok_or_else(|| anyhow::anyhow!("print_pdf.image.png_chunks: truncated chunk header"))?;
        let len = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let kind = &header[4..8];
        if !kind.iter().all(u8::is_ascii_alphabetic) || !kind[2].is_ascii_uppercase() {
            bail!("print_pdf.image.png_chunks: invalid or reserved chunk type");
        }
        let data_start = pos + 8;
        let data_end = data_start
            .checked_add(len)
            .ok_or_else(|| anyhow::anyhow!("print_pdf.image.png_chunks: chunk length overflow"))?;
        let end = data_end
            .checked_add(4)
            .ok_or_else(|| anyhow::anyhow!("print_pdf.image.png_chunks: chunk CRC overflow"))?;
        let data = bytes.get(data_start..data_end).ok_or_else(|| {
            anyhow::anyhow!("print_pdf.image.png_chunks: truncated chunk payload")
        })?;
        let expected_crc = bytes
            .get(data_end..end)
            .ok_or_else(|| anyhow::anyhow!("print_pdf.image.png_chunks: truncated chunk CRC"))?;
        let mut crc = crc32fast::Hasher::new();
        crc.update(kind);
        crc.update(data);
        if crc.finalize() != u32::from_be_bytes(expected_crc.try_into().unwrap()) {
            bail!("print_pdf.image.png_crc: PNG chunk CRC mismatch");
        }
        if dimensions.is_none() && kind != b"IHDR" {
            bail!("print_pdf.image.png_chunks: IHDR must be first");
        }
        if kind != b"IDAT" && saw_idat {
            after_idat = true;
        }
        match kind {
            b"IHDR" => {
                if dimensions.is_some() || pos != 8 || data.len() != 13 {
                    bail!("print_pdf.image.png_header: duplicate or malformed IHDR");
                }
                let width = u32::from_be_bytes(data[..4].try_into().unwrap());
                let height = u32::from_be_bytes(data[4..8].try_into().unwrap());
                checked_dimensions(width, height)?;
                let color_space = match (data[8], data[9]) {
                    (8, 0) => PrintImageColorSpace::Gray,
                    (8, 2) => PrintImageColorSpace::Rgb,
                    _ => bail!("print_pdf.image.png_color: only 8-bit grayscale or RGB without alpha/palette is supported"),
                };
                if data[10..13] != [0, 0, 0] {
                    bail!(
                        "print_pdf.image.png_layout: unsupported compression, filter, or interlace"
                    );
                }
                dimensions = Some((width, height, color_space));
            }
            b"IDAT" => {
                if after_idat {
                    bail!("print_pdf.image.png_chunks: IDAT chunks must be contiguous");
                }
                saw_idat = true;
                idat.extend_from_slice(data);
            }
            b"IEND" => {
                if !data.is_empty() || idat.is_empty() || end != bytes.len() {
                    bail!("print_pdf.image.png_chunks: invalid IEND or trailing bytes");
                }
                saw_end = true;
            }
            b"PLTE" | b"tRNS" | b"iCCP" | b"eXIf" | b"acTL" | b"fcTL" | b"fdAT" => {
                bail!("print_pdf.image.png_metadata: palette, transparency, profile, EXIF, and animation are unsupported");
            }
            b"sRGB" | b"gAMA" | b"cHRM" | b"cICP" | b"sBIT" => {
                bail!("print_pdf.image.png_color: embedded color intent or significant-bit metadata cannot be preserved by the device-color PDF route");
            }
            b"pHYs" => {
                if data.len() != 9
                    || u32::from_be_bytes(data[..4].try_into().unwrap())
                        != u32::from_be_bytes(data[4..8].try_into().unwrap())
                    || data[8] > 1
                {
                    bail!("print_pdf.image.png_aspect: non-square or malformed pixel aspect metadata is unsupported");
                }
            }
            _ if kind[0].is_ascii_uppercase() => {
                bail!("print_pdf.image.png_chunks: unknown critical PNG chunk");
            }
            _ => {}
        }
        pos = end;
        if saw_end {
            break;
        }
    }
    if !saw_end {
        bail!("print_pdf.image.png_chunks: missing IEND");
    }
    let (width_px, height_px, color_space) = dimensions.expect("IHDR required above");
    check_png_pixels(&idat, width_px, height_px, color_space)?;
    Ok(PrintImageInfo {
        width_px,
        height_px,
        color_space,
        image_stream_sha256: format!("{:x}", Sha256::digest(&idat)),
    })
}

fn check_png_pixels(
    idat: &[u8],
    width: u32,
    height: u32,
    color_space: PrintImageColorSpace,
) -> Result<()> {
    let channels = match color_space {
        PrintImageColorSpace::Gray => 1u64,
        PrintImageColorSpace::Rgb => 3,
    };
    let row_size = u64::from(width) * channels + 1;
    let expected = row_size * u64::from(height);
    if expected > MAX_DECODED_BYTES {
        bail!("print_pdf.image.decoded_size_limit: decoded PNG exceeds 512 MiB");
    }
    let mut row = vec![0u8; row_size as usize];
    let mut decoder = ZlibDecoder::new(idat);
    for _ in 0..height {
        decoder.read_exact(&mut row).map_err(|err| {
            anyhow::anyhow!("print_pdf.image.png_zlib: corrupt or short image data: {err}")
        })?;
        if row[0] > 4 {
            bail!("print_pdf.image.png_filter: invalid PNG scanline filter");
        }
    }
    let mut extra = [0u8; 1];
    if decoder.read(&mut extra).map_err(|err| {
        anyhow::anyhow!("print_pdf.image.png_zlib: corrupt compressed image data: {err}")
    })? != 0
        || decoder.total_in() != idat.len() as u64
    {
        bail!("print_pdf.image.png_zlib: unexpected extra compressed or decoded bytes");
    }
    Ok(())
}

fn inspect_jpeg(bytes: &[u8]) -> Result<PrintImageInfo> {
    if !bytes.starts_with(&[0xff, 0xd8]) {
        bail!("print_pdf.image.jpeg_header: missing JPEG SOI");
    }
    let mut pos = 2usize;
    let mut frame: Option<(u32, u32, PrintImageColorSpace, Vec<u8>, bool)> = None;
    let mut saw_scan = false;
    let mut saw_exif = false;
    let mut saw_jfif = false;
    loop {
        let marker = read_jpeg_marker(bytes, &mut pos)?;
        if marker == 0xd9 {
            if !saw_scan || pos != bytes.len() {
                bail!("print_pdf.image.jpeg_structure: missing scan or trailing bytes after EOI");
            }
            break;
        }
        if marker == 0xd8 || marker == 0x01 || (0xd0..=0xd7).contains(&marker) {
            bail!("print_pdf.image.jpeg_structure: unexpected standalone marker");
        }
        let len_bytes = bytes.get(pos..pos.saturating_add(2)).ok_or_else(|| {
            anyhow::anyhow!("print_pdf.image.jpeg_structure: truncated segment length")
        })?;
        let len = u16::from_be_bytes(len_bytes.try_into().unwrap()) as usize;
        if len < 2 {
            bail!("print_pdf.image.jpeg_structure: invalid segment length");
        }
        let data_start = pos + 2;
        let data_end = pos.checked_add(len).ok_or_else(|| {
            anyhow::anyhow!("print_pdf.image.jpeg_structure: segment length overflow")
        })?;
        let data = bytes
            .get(data_start..data_end)
            .ok_or_else(|| anyhow::anyhow!("print_pdf.image.jpeg_structure: truncated segment"))?;
        pos = data_end;
        match marker {
            0xe0 if data.starts_with(b"JFIF\0") => {
                if saw_jfif || data.len() < 14 {
                    bail!("print_pdf.image.jpeg_jfif: repeated or malformed JFIF header");
                }
                saw_jfif = true;
                let x_density = u16::from_be_bytes([data[8], data[9]]);
                let y_density = u16::from_be_bytes([data[10], data[11]]);
                let thumbnail_bytes = usize::from(data[12]) * usize::from(data[13]) * 3;
                if data[7] > 2
                    || x_density == 0
                    || x_density != y_density
                    || data.len() != 14 + thumbnail_bytes
                {
                    bail!("print_pdf.image.jpeg_aspect: non-square or malformed JFIF pixel aspect is unsupported");
                }
            }
            0xc0 | 0xc2 => {
                if frame.is_some() || saw_scan || data.len() < 6 || data[0] != 8 {
                    bail!("print_pdf.image.jpeg_frame: duplicate or unsupported frame");
                }
                let height = u16::from_be_bytes([data[1], data[2]]) as u32;
                let width = u16::from_be_bytes([data[3], data[4]]) as u32;
                checked_dimensions(width, height)?;
                let count = data[5] as usize;
                let color_space = match count {
                    1 => PrintImageColorSpace::Gray,
                    3 => PrintImageColorSpace::Rgb,
                    _ => bail!("print_pdf.image.jpeg_color: only grayscale or three-component JPEG is supported"),
                };
                if data.len() != 6 + 3 * count {
                    bail!("print_pdf.image.jpeg_frame: malformed frame components");
                }
                let components: Vec<u8> = data[6..]
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|part| part[0])
                    .collect();
                if components
                    .iter()
                    .enumerate()
                    .any(|(i, id)| components[..i].contains(id))
                {
                    bail!("print_pdf.image.jpeg_frame: duplicate frame component IDs");
                }
                frame = Some((width, height, color_space, components, marker == 0xc2));
            }
            0xc1 | 0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf => {
                bail!("print_pdf.image.jpeg_frame: unsupported JPEG coding mode");
            }
            0xda => {
                let (_, _, _, components, progressive) = frame.as_ref().ok_or_else(|| {
                    anyhow::anyhow!("print_pdf.image.jpeg_structure: SOS precedes SOF")
                })?;
                validate_scan_header(data, components, *progressive)?;
                saw_scan = true;
                // Entropy-coded bytes may contain stuffed FF 00 and restart
                // markers. The next non-restart marker starts a new segment.
                let entropy_start = pos;
                loop {
                    let value = *bytes.get(pos).ok_or_else(|| {
                        anyhow::anyhow!("print_pdf.image.jpeg_structure: truncated scan")
                    })?;
                    if value != 0xff {
                        pos += 1;
                        continue;
                    }
                    let marker_pos = pos;
                    while bytes.get(pos) == Some(&0xff) {
                        pos += 1;
                    }
                    let next = *bytes.get(pos).ok_or_else(|| {
                        anyhow::anyhow!("print_pdf.image.jpeg_structure: truncated scan marker")
                    })?;
                    if next == 0x00 || (0xd0..=0xd7).contains(&next) {
                        pos += 1;
                        continue;
                    }
                    if marker_pos == entropy_start {
                        bail!("print_pdf.image.jpeg_scan: empty entropy-coded scan");
                    }
                    pos = marker_pos;
                    break;
                }
            }
            0xe1 if data.starts_with(b"Exif\0\0") => {
                if saw_exif {
                    bail!("print_pdf.image.jpeg_exif: multiple EXIF segments");
                }
                saw_exif = true;
                check_exif_orientation(&data[6..])?;
            }
            0xe2 if data.starts_with(b"ICC_PROFILE\0") => {
                bail!("print_pdf.image.jpeg_icc: embedded ICC profile is unsupported");
            }
            0xee if data.starts_with(b"Adobe") => {
                // Adobe APP14 can change the interpretation of a three-component
                // DCT stream. The narrow DeviceRGB/Gray PDF contract excludes it.
                bail!("print_pdf.image.jpeg_color: Adobe APP14 color transforms are unsupported");
            }
            0xdc => bail!("print_pdf.image.jpeg_frame: DNL dimensions are unsupported"),
            _ => {}
        }
    }
    let (width_px, height_px, color_space, _, _) = frame.ok_or_else(|| {
        anyhow::anyhow!("print_pdf.image.jpeg_frame: missing baseline or progressive SOF")
    })?;
    check_jpeg_pixels(bytes, width_px, height_px, color_space)?;
    Ok(PrintImageInfo {
        width_px,
        height_px,
        color_space,
        image_stream_sha256: format!("{:x}", Sha256::digest(bytes)),
    })
}

fn check_jpeg_pixels(
    bytes: &[u8],
    width: u32,
    height: u32,
    color_space: PrintImageColorSpace,
) -> Result<()> {
    let (channels, expected_pixel_format) = match color_space {
        PrintImageColorSpace::Gray => (1u64, jpeg_decoder::PixelFormat::L8),
        PrintImageColorSpace::Rgb => (3u64, jpeg_decoder::PixelFormat::RGB24),
    };
    let expected = u64::from(width) * u64::from(height) * channels;
    if expected > MAX_DECODED_BYTES {
        bail!("print_pdf.image.decoded_size_limit: decoded JPEG exceeds 512 MiB");
    }
    let mut decoder = jpeg_decoder::Decoder::new(bytes);
    decoder.set_max_decoding_buffer_size(MAX_DECODED_BYTES as usize);
    let decoded = decoder.decode().map_err(|err| {
        anyhow::anyhow!("print_pdf.image.jpeg_decode: unreadable JPEG entropy: {err}")
    })?;
    let info = decoder.info().ok_or_else(|| {
        anyhow::anyhow!("print_pdf.image.jpeg_decode: missing decoded image properties")
    })?;
    if u32::from(info.width) != width
        || u32::from(info.height) != height
        || info.pixel_format != expected_pixel_format
        || decoded.len() as u64 != expected
    {
        bail!("print_pdf.image.jpeg_decode: decoded image differs from inspected frame");
    }
    Ok(())
}

fn read_jpeg_marker(bytes: &[u8], pos: &mut usize) -> Result<u8> {
    if bytes.get(*pos) != Some(&0xff) {
        bail!("print_pdf.image.jpeg_structure: expected marker");
    }
    while bytes.get(*pos) == Some(&0xff) {
        *pos += 1;
    }
    let marker = *bytes
        .get(*pos)
        .ok_or_else(|| anyhow::anyhow!("print_pdf.image.jpeg_structure: truncated marker"))?;
    if marker == 0x00 {
        bail!("print_pdf.image.jpeg_structure: stuffed byte outside scan");
    }
    *pos += 1;
    Ok(marker)
}

fn validate_scan_header(data: &[u8], components: &[u8], progressive: bool) -> Result<()> {
    let count =
        usize::from(*data.first().ok_or_else(|| {
            anyhow::anyhow!("print_pdf.image.jpeg_scan: missing scan components")
        })?);
    if count == 0 || count > components.len() || data.len() != 4 + 2 * count {
        bail!("print_pdf.image.jpeg_scan: malformed scan components");
    }
    let mut seen = Vec::new();
    for chunk in data[1..1 + count * 2].as_chunks::<2>().0 {
        if !components.contains(&chunk[0]) || seen.contains(&chunk[0]) {
            bail!("print_pdf.image.jpeg_scan: unknown or repeated scan component");
        }
        seen.push(chunk[0]);
    }
    let ss = data[1 + count * 2];
    let se = data[2 + count * 2];
    let approximation = data[3 + count * 2];
    if progressive {
        if ss > se
            || se > 63
            || (ss == 0 && se != 0)
            || (ss != 0 && count != 1)
            || approximation >> 4 > 13
            || approximation & 0x0f > 13
        {
            bail!("print_pdf.image.jpeg_scan: invalid progressive scan parameters");
        }
    } else if (ss, se, approximation) != (0, 63, 0) {
        bail!("print_pdf.image.jpeg_scan: invalid baseline scan parameters");
    }
    Ok(())
}

fn check_exif_orientation(tiff: &[u8]) -> Result<()> {
    let endian = match tiff.get(..2) {
        Some(b"II") => true,
        Some(b"MM") => false,
        _ => bail!("print_pdf.image.jpeg_exif: invalid TIFF byte order"),
    };
    let read_u16 = |bytes: &[u8]| -> u16 {
        if endian {
            u16::from_le_bytes([bytes[0], bytes[1]])
        } else {
            u16::from_be_bytes([bytes[0], bytes[1]])
        }
    };
    let read_u32 = |bytes: &[u8]| -> u32 {
        if endian {
            u32::from_le_bytes(bytes.try_into().unwrap())
        } else {
            u32::from_be_bytes(bytes.try_into().unwrap())
        }
    };
    let header = tiff
        .get(..8)
        .ok_or_else(|| anyhow::anyhow!("print_pdf.image.jpeg_exif: short TIFF header"))?;
    if read_u16(&header[2..4]) != 42 {
        bail!("print_pdf.image.jpeg_exif: invalid TIFF magic");
    }
    let offset = read_u32(&header[4..8]) as usize;
    let count_bytes = tiff
        .get(offset..offset.saturating_add(2))
        .ok_or_else(|| anyhow::anyhow!("print_pdf.image.jpeg_exif: missing IFD0"))?;
    let count = read_u16(count_bytes) as usize;
    let entries_end = offset
        .checked_add(2)
        .and_then(|value| value.checked_add(count * 12))
        .and_then(|value| value.checked_add(4))
        .ok_or_else(|| anyhow::anyhow!("print_pdf.image.jpeg_exif: IFD0 offset overflow"))?;
    if tiff.get(..entries_end).is_none() {
        bail!("print_pdf.image.jpeg_exif: truncated IFD0");
    }
    let mut orientation = None;
    for i in 0..count {
        let start = offset
            .checked_add(2)
            .and_then(|n| n.checked_add(i * 12))
            .ok_or_else(|| anyhow::anyhow!("print_pdf.image.jpeg_exif: IFD0 offset overflow"))?;
        let field = tiff
            .get(start..start.saturating_add(12))
            .ok_or_else(|| anyhow::anyhow!("print_pdf.image.jpeg_exif: truncated IFD0"))?;
        if read_u16(&field[..2]) == 0x0112 {
            if orientation.is_some() || read_u16(&field[2..4]) != 3 || read_u32(&field[4..8]) != 1 {
                bail!("print_pdf.image.jpeg_exif: malformed or repeated orientation");
            }
            orientation = Some(read_u16(&field[8..10]));
        } else {
            // EXIF may carry a profile, pixel-aspect, or color interpretation
            // in other IFD entries. Keep this route limited to an explicit
            // orientation declaration, which does not change image color.
            bail!(
                "print_pdf.image.jpeg_exif: only an identity-orientation EXIF entry is supported"
            );
        }
    }
    if read_u32(&tiff[entries_end - 4..entries_end]) != 0 {
        bail!("print_pdf.image.jpeg_exif: linked EXIF directories are unsupported");
    }
    if orientation.is_some_and(|value| value != 1) {
        bail!("print_pdf.image.jpeg_orientation: nonidentity EXIF orientation is unsupported");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    fn png_chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut chunk = Vec::new();
        chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
        chunk.extend_from_slice(kind);
        chunk.extend_from_slice(data);
        let mut crc = crc32fast::Hasher::new();
        crc.update(kind);
        crc.update(data);
        chunk.extend_from_slice(&crc.finalize().to_be_bytes());
        chunk
    }

    fn png(width: u32, height: u32, color_type: u8) -> Vec<u8> {
        let channels = if color_type == 0 { 1 } else { 3 };
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, color_type, 0, 0, 0]);
        bytes.extend(png_chunk(b"IHDR", &ihdr));
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        for _ in 0..height {
            encoder
                .write_all(&vec![0; 1 + width as usize * channels])
                .unwrap();
        }
        bytes.extend(png_chunk(b"IDAT", &encoder.finish().unwrap()));
        bytes.extend(png_chunk(b"IEND", &[]));
        bytes
    }

    fn jpeg_marker_structure(components: u8) -> Vec<u8> {
        // This fixture exercises unsupported channels before entropy decoding.
        let mut bytes = vec![0xff, 0xd8];
        let mut sof = vec![8, 0, 1, 0, 1, components];
        for component in 1..=components {
            sof.extend([component, 0x11, 0]);
        }
        bytes.extend([0xff, 0xc0]);
        bytes.extend(((sof.len() + 2) as u16).to_be_bytes());
        bytes.extend(sof);
        let mut sos = vec![components];
        for component in 1..=components {
            sos.extend([component, 0]);
        }
        sos.extend([0, 63, 0]);
        bytes.extend([0xff, 0xda]);
        bytes.extend(((sos.len() + 2) as u16).to_be_bytes());
        bytes.extend(sos);
        bytes.extend([0x42, 0xff, 0x00, 0x17, 0xff, 0xd9]);
        bytes
    }

    fn valid_jpeg_rgb() -> Vec<u8> {
        // Optimized 1x1 JPEG made with Pillow; checked-in bytes keep the test
        // deterministic without a runtime image encoder.
        decode_hex(concat!(
            "ffd8ffe000104a46494600010100000100010000ffdb004300080606070605080707070909080a0c140d0c0b0b0c1912130f141d1a1f1e1d1a1c1c20242e2720222c231c1c2837292c30313434341f27393d38323c2e333432",
            "ffdb0043010909090c0b0c180d0d1832211c213232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232323232ffc00011080001000103012200021101031101",
            "ffc4001500010100000000000000000000000000000007ffc40014100100000000000000000000000000000000ffc40014010100000000000000000000000000000004ffc40014110100000000000000000000000000000000ffda000c03010002110311003f009680608fffd9"
        ))
    }

    fn valid_jpeg_gray() -> Vec<u8> {
        decode_hex(concat!(
            "ffd8ffe000104a46494600010100000100010000ffdb004300080606070605080707070909080a0c140d0c0b0b0c1912130f141d1a1f1e1d1a1c1c20242e2720222c231c1c2837292c30313434341f27393d38323c2e333432",
            "ffc0000b080001000101011100ffc40014000100000000000000000000000000000000ffc40014100100000000000000000000000000000000ffda0008010100003f003fffd9"
        ))
    }

    fn valid_jpeg_progressive_gray() -> Vec<u8> {
        decode_hex(concat!(
            "ffd8ffe000104a46494600010100000100010000ffdb004300080606070605080707070909080a0c140d0c0b0b0c1912130f141d1a1f1e1d1a1c1c20242e2720222c231c1c2837292c30313434341f27393d38323c2e333432",
            "ffc2000b080001000101011100ffc40014000100000000000000000000000000000000ffda00080101000000017fffc40014100100000000000000000000000000000000ffda00080101000105027fff",
            "c40014100100000000000000000000000000000000ffda0008010100063f027fffc40014100100000000000000000000000000000000ffda0008010100013f217fffda00080101000000107fffc40014100100000000000000000000000000000000ffda0008010100013f107fffd9"
        ))
    }

    fn decode_hex(hex: &str) -> Vec<u8> {
        hex.as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect()
    }

    fn inspect_fixture(bytes: &[u8], format: Format) -> Result<PrintImageInfo> {
        let path = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(path.path(), bytes).unwrap();
        inspect_print_image(path.path(), format)
    }

    #[test]
    fn png_rgb_gray_crc_and_stream_hash() {
        for (kind, expected_color) in [
            (2, PrintImageColorSpace::Rgb),
            (0, PrintImageColorSpace::Gray),
        ] {
            let bytes = png(2, 3, kind);
            let image = inspect_fixture(&bytes, Format::Png).unwrap();
            assert_eq!(
                (image.width_px, image.height_px, image.color_space),
                (2, 3, expected_color)
            );
            assert_eq!(image.image_stream_sha256.len(), 64);
            assert!(serde_json::to_string(&image)
                .unwrap()
                .contains(if kind == 2 { "RGB" } else { "Gray" }));
        }
    }

    #[test]
    fn png_refuses_bad_crc_zlib_color_or_metadata() {
        let mut bad_crc = png(1, 1, 2);
        bad_crc[29] ^= 1;
        assert!(inspect_fixture(&bad_crc, Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.png_crc"));
        let mut bad_data = png(1, 1, 2);
        let idat_pos = 8 + 25 + 8;
        bad_data[idat_pos] ^= 1;
        let idat_len = u32::from_be_bytes(bad_data[33..37].try_into().unwrap()) as usize;
        let mut crc = crc32fast::Hasher::new();
        crc.update(b"IDAT");
        crc.update(&bad_data[idat_pos..idat_pos + idat_len]);
        bad_data[idat_pos + idat_len..idat_pos + idat_len + 4]
            .copy_from_slice(&crc.finalize().to_be_bytes());
        assert!(inspect_fixture(&bad_data, Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.png_zlib"));
        let alpha = png(1, 1, 6);
        assert!(inspect_fixture(&alpha, Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.png_color"));
        let mut icc = png(1, 1, 2);
        let first_idat = 8 + 25;
        icc.splice(first_idat..first_idat, png_chunk(b"iCCP", b"profile\0"));
        assert!(inspect_fixture(&icc, Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.png_metadata"));
        let mut srgb = png(1, 1, 2);
        srgb.splice(first_idat..first_idat, png_chunk(b"sRGB", &[0]));
        assert!(inspect_fixture(&srgb, Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.png_color"));
        let mut nonsquare = png(1, 1, 2);
        nonsquare.splice(
            first_idat..first_idat,
            png_chunk(b"pHYs", &[0, 0, 0, 1, 0, 0, 0, 2, 1]),
        );
        assert!(inspect_fixture(&nonsquare, Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.png_aspect"));
    }

    #[test]
    fn jpeg_refuses_cmyk_icc_and_nonidentity_orientation() {
        let rgb = valid_jpeg_rgb();
        let info = inspect_fixture(&rgb, Format::Jpeg).unwrap();
        assert_eq!(
            (info.width_px, info.height_px, info.color_space),
            (1, 1, PrintImageColorSpace::Rgb)
        );
        assert_eq!(
            info.image_stream_sha256,
            format!("{:x}", Sha256::digest(&rgb))
        );
        let gray = inspect_fixture(&valid_jpeg_gray(), Format::Jpeg).unwrap();
        assert_eq!(gray.color_space, PrintImageColorSpace::Gray);
        let progressive = inspect_fixture(&valid_jpeg_progressive_gray(), Format::Jpeg).unwrap();
        assert_eq!(progressive.color_space, PrintImageColorSpace::Gray);
        assert!(inspect_fixture(&jpeg_marker_structure(4), Format::Jpeg)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.jpeg_color"));
        let mut icc = rgb.clone();
        let app = b"ICC_PROFILE\0\x01\x01";
        icc.splice(
            2..2,
            [
                vec![0xff, 0xe2],
                ((app.len() + 2) as u16).to_be_bytes().to_vec(),
                app.to_vec(),
            ]
            .concat(),
        );
        assert!(inspect_fixture(&icc, Format::Jpeg)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.jpeg_icc"));
        let mut rotated = rgb;
        let tiff = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0\x06\0\0\0\0\0\0\0";
        rotated.splice(
            2..2,
            [
                vec![0xff, 0xe1],
                ((tiff.len() + 2) as u16).to_be_bytes().to_vec(),
                tiff.to_vec(),
            ]
            .concat(),
        );
        assert!(inspect_fixture(&rotated, Format::Jpeg)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.jpeg_orientation"));
        rotated[30] = 1;
        assert_eq!(
            inspect_fixture(&rotated, Format::Jpeg).unwrap().color_space,
            PrintImageColorSpace::Rgb
        );
        let mut nonsquare = valid_jpeg_rgb();
        nonsquare[17] = 2;
        assert!(inspect_fixture(&nonsquare, Format::Jpeg)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.jpeg_aspect"));
    }

    #[test]
    fn jpeg_refuses_unreadable_tables_before_embedding() {
        let mut jpeg = valid_jpeg_rgb();
        // The DQT table header follows the JFIF APP0 marker.
        jpeg[24] = 0xff;
        assert!(inspect_fixture(&jpeg, Format::Jpeg)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.jpeg_decode"));
    }

    #[test]
    fn file_bounds_and_wrong_format_fail_without_mutation() {
        let path = tempfile::NamedTempFile::new().unwrap();
        let bytes = png(1, 1, 2);
        std::fs::write(path.path(), &bytes).unwrap();
        assert!(inspect_print_image(path.path(), Format::Tiff)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.unsupported_format"));
        assert!(inspect_print_image(path.path(), Format::Jpeg)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.jpeg_header"));
        assert_eq!(std::fs::read(path.path()).unwrap(), bytes);

        let too_wide = png(MAX_DIMENSION + 1, 0, 2);
        assert!(inspect_fixture(&too_wide, Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.dimensions"));
        let truncated = &bytes[..bytes.len() - 1];
        assert!(inspect_fixture(truncated, Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.png_chunks"));

        let oversized = tempfile::NamedTempFile::new().unwrap();
        oversized.as_file().set_len(MAX_IMAGE_BYTES + 1).unwrap();
        assert!(inspect_print_image(oversized.path(), Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.size_limit"));
    }

    #[cfg(unix)]
    #[test]
    fn symlink_source_is_refused() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("image.png");
        std::fs::write(&source, png(1, 1, 2)).unwrap();
        let link = directory.path().join("link.png");
        std::os::unix::fs::symlink(&source, &link).unwrap();
        assert!(inspect_print_image(&link, Format::Png)
            .unwrap_err()
            .to_string()
            .contains("print_pdf.image.invalid_source"));
    }
}
