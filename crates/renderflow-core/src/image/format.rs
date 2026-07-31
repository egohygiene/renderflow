use std::fmt;
use std::str::FromStr;

use anyhow::Result;

/// All image formats that renderflow can produce via FFmpeg.
///
/// # Groupings
///
/// * **Common raster** – JPEG/JPG, PNG, WebP, AVIF, GIF, BMP, TIFF/TIF
/// * **Professional/HDR** – EXR, HDR, DPX, CIN, FITS/FIT
/// * **Legacy raster** – TGA, SGI/RGB, PCX, PBM, PGM, PPM, PNM, PAM, XBM, XPM, WBMP, RAS/SUN
/// * **JPEG variants** – JP2, J2K, JXL, JXR/WDP/HDP, BPG, FLIF
/// * **Animation** – APNG, MNG, FLI, FLC, ANI, CUR
/// * **Special/compressed** – HEIC, HEIF, DDS, ICO, DCM/DICOM, PICT/PCT, IFF/LBM, MAC, CALS/CAL, FAX,
///   JBIG/JB2, PGF, PIC, BLP, VTF, SFW
/// * **Camera RAW** (decode-only) – RAW, DNG, CR2, CR3, NEF, ARW, ORF, RAF, RW2
/// * **Vector/Design** (no FFmpeg support) – SVG, EPS, PDF, PSD/PSB, AI, INDD, XCF, AFPHOTO,
///   AFDESIGN, CDR, SKETCH, FIG
/// * **CAD/vector/other** (no FFmpeg support) – WMF, EMF, SKP, DXF, DWG, PLT, CGM, CMX, DRW,
///   SWF, FLA
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageFormat {
    // ── Common raster ─────────────────────────────────────────────────────────
    /// JPEG image (.jpeg).
    Jpeg,
    /// JPEG image (.jpg) — same codec as `Jpeg`; kept to preserve the original extension.
    Jpg,
    /// Portable Network Graphics (.png).
    Png,
    /// WebP image (.webp) — modern lossy/lossless format.
    Webp,
    /// AV1 Image File Format (.avif) — modern high-efficiency still-image format.
    Avif,
    /// Graphics Interchange Format (.gif) — supports animation and transparency.
    Gif,
    /// Windows Bitmap (.bmp).
    Bmp,
    /// Tagged Image File Format (.tiff).
    Tiff,
    /// Tagged Image File Format (.tif) — same codec as `Tiff`; kept to preserve the extension.
    Tif,

    // ── Professional / HDR ────────────────────────────────────────────────────
    /// OpenEXR high dynamic range image (.exr).
    Exr,
    /// Radiance HDR image (.hdr).
    Hdr,
    /// Digital Picture Exchange (.dpx) — cinema/VFX 10-bit or higher format.
    Dpx,
    /// Cineon digital-film format (.cin).
    Cin,
    /// Flexible Image Transport System (.fits) — astronomy/science format.
    Fits,
    /// Flexible Image Transport System (.fit) — alternate extension for `Fits`.
    Fit,

    // ── Legacy raster ─────────────────────────────────────────────────────────
    /// Truevision TGA / TARGA (.tga).
    Tga,
    /// SGI native image (.sgi).
    Sgi,
    /// SGI RGB image (.rgb) — same codec as `Sgi`; kept to preserve the extension.
    Rgb,
    /// ZSoft PCX bitmap (.pcx).
    Pcx,
    /// Portable Bitmap (black & white) (.pbm).
    Pbm,
    /// Portable Graymap (.pgm).
    Pgm,
    /// Portable Pixmap (.ppm).
    Ppm,
    /// Portable Any Map umbrella (.pnm).
    Pnm,
    /// Portable Arbitrary Map (.pam).
    Pam,
    /// X Window System Bitmap (.xbm).
    Xbm,
    /// X Window System Pixmap (.xpm).
    Xpm,
    /// Wireless Bitmap (.wbmp) — monochrome WAP format.
    Wbmp,
    /// Sun Rasterfile (.ras).
    Ras,
    /// Sun Rasterfile (.sun) — alternate extension for `Ras`.
    Sun,

    // ── JPEG variants ─────────────────────────────────────────────────────────
    /// JPEG 2000 (.jp2) — wavelet-based high-quality format.
    Jp2,
    /// JPEG 2000 codestream (.j2k).
    J2k,
    /// JPEG XL (.jxl) — next-generation image format.
    Jxl,
    /// JPEG XR / HD Photo (.jxr).
    Jxr,
    /// JPEG XR / HD Photo (.wdp) — Windows format name.
    Wdp,
    /// JPEG XR / HD Photo (.hdp) — alternate extension.
    Hdp,
    /// Better Portable Graphics (.bpg) — HEVC-based format.
    Bpg,
    /// Free Lossless Image Format (.flif).
    Flif,

    // ── Animation ─────────────────────────────────────────────────────────────
    /// Animated PNG (.apng).
    Apng,
    /// Multiple-image Network Graphics (.mng).
    Mng,
    /// Autodesk FLIC animation (.fli).
    Fli,
    /// Autodesk FLIC animation (.flc).
    Flc,
    /// Windows animated cursor (.ani).
    Ani,

    // ── Special / compressed ──────────────────────────────────────────────────
    /// High Efficiency Image Container (.heic) — Apple's HEIF container.
    Heic,
    /// High Efficiency Image Format (.heif).
    Heif,
    /// DirectDraw Surface (.dds) — GPU texture format.
    Dds,
    /// Windows icon (.ico).
    Ico,
    /// Windows cursor (.cur).
    Cur,
    /// DICOM medical image (.dcm).
    Dcm,
    /// DICOM medical image (.dicom) — alternate extension.
    Dicom,
    /// PICT / Macintosh QuickDraw image (.pict).
    Pict,
    /// PICT / Macintosh image (.pct) — alternate extension.
    Pct,
    /// Amiga IFF image (.iff).
    Iff,
    /// Amiga IFF / DeluxePaint (.lbm) — alternate extension.
    Lbm,
    /// MacPaint image (.mac).
    Mac,
    /// CALS Group 4 raster (.cals).
    Cals,
    /// CALS Group 4 raster (.cal) — alternate extension.
    Cal,
    /// FAX Group 3/4 image (.fax).
    Fax,
    /// JBIG compressed bitmap (.jbig).
    Jbig,
    /// JBIG2 compressed bitmap (.jb2).
    Jb2,
    /// Progressive Graphics File (.pgf).
    Pgf,
    /// Softimage PIC (.pic).
    Pic,
    /// Blizzard Entertainment texture (.blp).
    Blp,
    /// Valve Texture Format (.vtf).
    Vtf,
    /// Seattle FilmWorks image (.sfw).
    Sfw,

    // ── Camera RAW (decode-only via LibRaw/dcraw) ─────────────────────────────
    /// Generic camera RAW (.raw).
    Raw,
    /// Adobe Digital Negative (.dng) — open RAW standard.
    Dng,
    /// Canon RAW 2 (.cr2).
    Cr2,
    /// Canon RAW 3 (.cr3).
    Cr3,
    /// Nikon Electronic Format (.nef).
    Nef,
    /// Sony Alpha RAW (.arw).
    Arw,
    /// Olympus RAW Format (.orf).
    Orf,
    /// Fujifilm RAW (.raf).
    Raf,
    /// Panasonic RAW 2 (.rw2).
    Rw2,

    // ── Vector / Design (no FFmpeg encode support) ────────────────────────────
    /// Scalable Vector Graphics (.svg).
    Svg,
    /// Encapsulated PostScript (.eps).
    Eps,
    /// Portable Document Format (.pdf).
    Pdf,
    /// Adobe Photoshop Document (.psd).
    Psd,
    /// Adobe Photoshop Large Document (.psb).
    Psb,
    /// Adobe Illustrator (.ai).
    Ai,
    /// Adobe InDesign (.indd).
    Indd,
    /// GIMP native format (.xcf).
    Xcf,
    /// Affinity Photo (.afphoto).
    Afphoto,
    /// Affinity Designer (.afdesign).
    Afdesign,
    /// CorelDRAW (.cdr).
    Cdr,
    /// Sketch app format (.sketch).
    Sketch,
    /// Figma design file (.fig).
    Fig,

    // ── CAD / vector / other (no FFmpeg support) ──────────────────────────────
    /// Windows Metafile (.wmf).
    Wmf,
    /// Enhanced Windows Metafile (.emf).
    Emf,
    /// SketchUp 3D model (.skp).
    Skp,
    /// AutoCAD Drawing Exchange Format (.dxf).
    Dxf,
    /// AutoCAD Drawing (.dwg).
    Dwg,
    /// HPGL plot file (.plt).
    Plt,
    /// Computer Graphics Metafile (.cgm).
    Cgm,
    /// Corel Metafile Exchange (.cmx).
    Cmx,
    /// Micrografx Draw (.drw).
    Drw,
    /// Adobe Flash (.swf).
    Swf,
    /// Adobe Flash source (.fla).
    Fla,
}

impl ImageFormat {
    /// Return the canonical file extension for this format (without leading dot).
    pub fn file_extension(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpeg",
            ImageFormat::Jpg => "jpg",
            ImageFormat::Png => "png",
            ImageFormat::Webp => "webp",
            ImageFormat::Avif => "avif",
            ImageFormat::Gif => "gif",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Tif => "tif",
            ImageFormat::Exr => "exr",
            ImageFormat::Hdr => "hdr",
            ImageFormat::Dpx => "dpx",
            ImageFormat::Cin => "cin",
            ImageFormat::Fits => "fits",
            ImageFormat::Fit => "fit",
            ImageFormat::Tga => "tga",
            ImageFormat::Sgi => "sgi",
            ImageFormat::Rgb => "rgb",
            ImageFormat::Pcx => "pcx",
            ImageFormat::Pbm => "pbm",
            ImageFormat::Pgm => "pgm",
            ImageFormat::Ppm => "ppm",
            ImageFormat::Pnm => "pnm",
            ImageFormat::Pam => "pam",
            ImageFormat::Xbm => "xbm",
            ImageFormat::Xpm => "xpm",
            ImageFormat::Wbmp => "wbmp",
            ImageFormat::Ras => "ras",
            ImageFormat::Sun => "sun",
            ImageFormat::Jp2 => "jp2",
            ImageFormat::J2k => "j2k",
            ImageFormat::Jxl => "jxl",
            ImageFormat::Jxr => "jxr",
            ImageFormat::Wdp => "wdp",
            ImageFormat::Hdp => "hdp",
            ImageFormat::Bpg => "bpg",
            ImageFormat::Flif => "flif",
            ImageFormat::Apng => "apng",
            ImageFormat::Mng => "mng",
            ImageFormat::Fli => "fli",
            ImageFormat::Flc => "flc",
            ImageFormat::Ani => "ani",
            ImageFormat::Heic => "heic",
            ImageFormat::Heif => "heif",
            ImageFormat::Dds => "dds",
            ImageFormat::Ico => "ico",
            ImageFormat::Cur => "cur",
            ImageFormat::Dcm => "dcm",
            ImageFormat::Dicom => "dicom",
            ImageFormat::Pict => "pict",
            ImageFormat::Pct => "pct",
            ImageFormat::Iff => "iff",
            ImageFormat::Lbm => "lbm",
            ImageFormat::Mac => "mac",
            ImageFormat::Cals => "cals",
            ImageFormat::Cal => "cal",
            ImageFormat::Fax => "fax",
            ImageFormat::Jbig => "jbig",
            ImageFormat::Jb2 => "jb2",
            ImageFormat::Pgf => "pgf",
            ImageFormat::Pic => "pic",
            ImageFormat::Blp => "blp",
            ImageFormat::Vtf => "vtf",
            ImageFormat::Sfw => "sfw",
            ImageFormat::Raw => "raw",
            ImageFormat::Dng => "dng",
            ImageFormat::Cr2 => "cr2",
            ImageFormat::Cr3 => "cr3",
            ImageFormat::Nef => "nef",
            ImageFormat::Arw => "arw",
            ImageFormat::Orf => "orf",
            ImageFormat::Raf => "raf",
            ImageFormat::Rw2 => "rw2",
            ImageFormat::Svg => "svg",
            ImageFormat::Eps => "eps",
            ImageFormat::Pdf => "pdf",
            ImageFormat::Psd => "psd",
            ImageFormat::Psb => "psb",
            ImageFormat::Ai => "ai",
            ImageFormat::Indd => "indd",
            ImageFormat::Xcf => "xcf",
            ImageFormat::Afphoto => "afphoto",
            ImageFormat::Afdesign => "afdesign",
            ImageFormat::Cdr => "cdr",
            ImageFormat::Sketch => "sketch",
            ImageFormat::Fig => "fig",
            ImageFormat::Wmf => "wmf",
            ImageFormat::Emf => "emf",
            ImageFormat::Skp => "skp",
            ImageFormat::Dxf => "dxf",
            ImageFormat::Dwg => "dwg",
            ImageFormat::Plt => "plt",
            ImageFormat::Cgm => "cgm",
            ImageFormat::Cmx => "cmx",
            ImageFormat::Drw => "drw",
            ImageFormat::Swf => "swf",
            ImageFormat::Fla => "fla",
        }
    }

    /// Return the FFmpeg video codec name (`-vcodec`), or `None` when FFmpeg
    /// cannot encode to this format.
    pub fn ffmpeg_codec(self) -> Option<&'static str> {
        match self {
            // ── Common raster ─────────────────────────────────────────────────
            ImageFormat::Jpeg | ImageFormat::Jpg => Some("mjpeg"),
            ImageFormat::Png => Some("png"),
            ImageFormat::Webp => Some("libwebp"),
            ImageFormat::Avif => Some("libaom-av1"),
            ImageFormat::Gif => Some("gif"),
            ImageFormat::Bmp => Some("bmp"),
            ImageFormat::Tiff | ImageFormat::Tif => Some("tiff"),

            // ── Professional / HDR ───────────────────────────────────────────
            ImageFormat::Exr => Some("exr"),
            ImageFormat::Hdr => Some("hdr"),
            ImageFormat::Dpx => Some("dpx"),
            ImageFormat::Cin => Some("cineon"),
            ImageFormat::Fits | ImageFormat::Fit => Some("fits"),

            // ── Legacy raster ────────────────────────────────────────────────
            ImageFormat::Tga => Some("tga"),
            ImageFormat::Sgi | ImageFormat::Rgb => Some("sgi"),
            ImageFormat::Pcx => Some("pcx"),
            ImageFormat::Pbm => Some("pbm"),
            ImageFormat::Pgm => Some("pgm"),
            ImageFormat::Ppm => Some("ppm"),
            ImageFormat::Pnm => Some("pam"), // PNM umbrella — use PAM encoder
            ImageFormat::Pam => Some("pam"),
            ImageFormat::Xbm => Some("xbm"),
            ImageFormat::Xpm => Some("xpm"),
            ImageFormat::Wbmp => Some("wbmp"),
            ImageFormat::Ras | ImageFormat::Sun => Some("sunrast"),

            // ── JPEG variants ────────────────────────────────────────────────
            ImageFormat::Jp2 | ImageFormat::J2k => Some("jpeg2000"),
            ImageFormat::Jxl => Some("libjxl"), // requires libjxl in FFmpeg build
            // JPEG XR has no FFmpeg encoder
            ImageFormat::Jxr | ImageFormat::Wdp | ImageFormat::Hdp => None,
            // BPG/FLIF have no FFmpeg encoder
            ImageFormat::Bpg | ImageFormat::Flif => None,

            // ── Animation ────────────────────────────────────────────────────
            ImageFormat::Apng => Some("apng"),
            // MNG has no FFmpeg encoder
            ImageFormat::Mng => None,
            // FLI/FLC have no FFmpeg encoder
            ImageFormat::Fli | ImageFormat::Flc => None,
            // ANI has no FFmpeg encoder
            ImageFormat::Ani => None,

            // ── Special / compressed ─────────────────────────────────────────
            // HEIC/HEIF: FFmpeg can encode to HEIF via libaom in newer builds,
            // but this is not universally available; treat as decode-only here.
            ImageFormat::Heic | ImageFormat::Heif => None,
            ImageFormat::Dds => Some("dds"),
            ImageFormat::Ico => Some("bmp"), // ICO muxer wraps BMP frames
            // CUR has no FFmpeg encoder
            ImageFormat::Cur => None,
            // DICOM has no FFmpeg encoder
            ImageFormat::Dcm | ImageFormat::Dicom => None,
            // PICT has no FFmpeg encoder
            ImageFormat::Pict | ImageFormat::Pct => None,
            // IFF/LBM have no FFmpeg encoder
            ImageFormat::Iff | ImageFormat::Lbm => None,
            // MacPaint has no FFmpeg encoder
            ImageFormat::Mac => None,
            // CALS has no FFmpeg encoder
            ImageFormat::Cals | ImageFormat::Cal => None,
            // FAX has no FFmpeg encoder
            ImageFormat::Fax => None,
            // JBIG has no FFmpeg encoder
            ImageFormat::Jbig | ImageFormat::Jb2 => None,
            // PGF has no FFmpeg support
            ImageFormat::Pgf => None,
            // PIC (Softimage) has no FFmpeg encoder
            ImageFormat::Pic => None,
            // BLP/VTF/SFW are decode-only
            ImageFormat::Blp | ImageFormat::Vtf | ImageFormat::Sfw => None,

            // ── Camera RAW (decode-only) ──────────────────────────────────────
            ImageFormat::Raw
            | ImageFormat::Dng
            | ImageFormat::Cr2
            | ImageFormat::Cr3
            | ImageFormat::Nef
            | ImageFormat::Arw
            | ImageFormat::Orf
            | ImageFormat::Raf
            | ImageFormat::Rw2 => None,

            // ── Vector / Design (no FFmpeg support) ───────────────────────────
            ImageFormat::Svg
            | ImageFormat::Eps
            | ImageFormat::Pdf
            | ImageFormat::Psd
            | ImageFormat::Psb
            | ImageFormat::Ai
            | ImageFormat::Indd
            | ImageFormat::Xcf
            | ImageFormat::Afphoto
            | ImageFormat::Afdesign
            | ImageFormat::Cdr
            | ImageFormat::Sketch
            | ImageFormat::Fig => None,

            // ── CAD / vector / other ─────────────────────────────────────────
            ImageFormat::Wmf
            | ImageFormat::Emf
            | ImageFormat::Skp
            | ImageFormat::Dxf
            | ImageFormat::Dwg
            | ImageFormat::Plt
            | ImageFormat::Cgm
            | ImageFormat::Cmx
            | ImageFormat::Drw
            | ImageFormat::Swf
            | ImageFormat::Fla => None,
        }
    }

    /// Return the FFmpeg muxer format name used with `-f`, or `None` when FFmpeg
    /// does not support encoding to this format.
    ///
    /// Most image formats use the `image2` muxer for single-frame output;
    /// a few need a specific muxer name.
    #[allow(dead_code)]
    pub fn ffmpeg_format(self) -> Option<&'static str> {
        match self {
            ImageFormat::Jpeg | ImageFormat::Jpg => Some("image2"),
            ImageFormat::Png => Some("image2"),
            ImageFormat::Webp => Some("image2"),
            ImageFormat::Avif => Some("image2"),
            ImageFormat::Gif => Some("gif"),
            ImageFormat::Bmp => Some("image2"),
            ImageFormat::Tiff | ImageFormat::Tif => Some("image2"),
            ImageFormat::Exr => Some("image2"),
            ImageFormat::Hdr => Some("image2"),
            ImageFormat::Dpx => Some("image2"),
            ImageFormat::Cin => Some("image2"),
            ImageFormat::Fits | ImageFormat::Fit => Some("image2"),
            ImageFormat::Tga => Some("image2"),
            ImageFormat::Sgi | ImageFormat::Rgb => Some("image2"),
            ImageFormat::Pcx => Some("image2"),
            ImageFormat::Pbm
            | ImageFormat::Pgm
            | ImageFormat::Ppm
            | ImageFormat::Pam
            | ImageFormat::Pnm => Some("image2"),
            ImageFormat::Xbm => Some("image2"),
            ImageFormat::Xpm => Some("image2"),
            ImageFormat::Wbmp => Some("image2"),
            ImageFormat::Ras | ImageFormat::Sun => Some("image2"),
            ImageFormat::Jp2 | ImageFormat::J2k => Some("image2"),
            ImageFormat::Jxl => Some("image2"),
            ImageFormat::Apng => Some("apng"),
            ImageFormat::Dds => Some("image2"),
            ImageFormat::Ico => Some("ico"),
            _ => None,
        }
    }

    /// Return `true` when this format stores pixel data without perceptual loss.
    #[allow(dead_code)]
    pub fn is_lossless(self) -> bool {
        matches!(
            self,
            ImageFormat::Png
                | ImageFormat::Bmp
                | ImageFormat::Tiff
                | ImageFormat::Tif
                | ImageFormat::Exr
                | ImageFormat::Hdr
                | ImageFormat::Dpx
                | ImageFormat::Cin
                | ImageFormat::Fits
                | ImageFormat::Fit
                | ImageFormat::Tga
                | ImageFormat::Sgi
                | ImageFormat::Rgb
                | ImageFormat::Pbm
                | ImageFormat::Pgm
                | ImageFormat::Ppm
                | ImageFormat::Pnm
                | ImageFormat::Pam
                | ImageFormat::Xbm
                | ImageFormat::Ras
                | ImageFormat::Sun
                | ImageFormat::Psd
                | ImageFormat::Psb
                | ImageFormat::Xcf
                | ImageFormat::Afphoto
                | ImageFormat::Afdesign
                | ImageFormat::Dng
                | ImageFormat::Raw
                | ImageFormat::Cr2
                | ImageFormat::Cr3
                | ImageFormat::Nef
                | ImageFormat::Arw
                | ImageFormat::Orf
                | ImageFormat::Raf
                | ImageFormat::Rw2
        )
    }

    /// Return `true` when this format natively supports animation / multiple frames.
    pub fn is_animated(self) -> bool {
        matches!(
            self,
            ImageFormat::Gif
                | ImageFormat::Apng
                | ImageFormat::Mng
                | ImageFormat::Fli
                | ImageFormat::Flc
                | ImageFormat::Ani
        )
    }

    /// Return `true` when FFmpeg can encode to this format.
    pub fn supports_encoding(self) -> bool {
        self.ffmpeg_codec().is_some()
    }

    /// Detect an `ImageFormat` from a file-extension string (without the dot).
    ///
    /// Returns `None` when the extension is not a known image format.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            // Common raster
            "jpeg" => Some(ImageFormat::Jpeg),
            "jpg" => Some(ImageFormat::Jpg),
            "png" => Some(ImageFormat::Png),
            "webp" => Some(ImageFormat::Webp),
            "avif" => Some(ImageFormat::Avif),
            "gif" => Some(ImageFormat::Gif),
            "bmp" => Some(ImageFormat::Bmp),
            "tiff" => Some(ImageFormat::Tiff),
            "tif" => Some(ImageFormat::Tif),
            // Professional / HDR
            "exr" => Some(ImageFormat::Exr),
            "hdr" => Some(ImageFormat::Hdr),
            "dpx" => Some(ImageFormat::Dpx),
            "cin" => Some(ImageFormat::Cin),
            "fits" => Some(ImageFormat::Fits),
            "fit" => Some(ImageFormat::Fit),
            // Legacy raster
            "tga" => Some(ImageFormat::Tga),
            "sgi" => Some(ImageFormat::Sgi),
            "rgb" => Some(ImageFormat::Rgb),
            "pcx" => Some(ImageFormat::Pcx),
            "pbm" => Some(ImageFormat::Pbm),
            "pgm" => Some(ImageFormat::Pgm),
            "ppm" => Some(ImageFormat::Ppm),
            "pnm" => Some(ImageFormat::Pnm),
            "pam" => Some(ImageFormat::Pam),
            "xbm" => Some(ImageFormat::Xbm),
            "xpm" => Some(ImageFormat::Xpm),
            "wbmp" => Some(ImageFormat::Wbmp),
            "ras" => Some(ImageFormat::Ras),
            "sun" => Some(ImageFormat::Sun),
            // JPEG variants
            "jp2" => Some(ImageFormat::Jp2),
            "j2k" | "j2c" => Some(ImageFormat::J2k),
            "jxl" => Some(ImageFormat::Jxl),
            "jxr" => Some(ImageFormat::Jxr),
            "wdp" => Some(ImageFormat::Wdp),
            "hdp" => Some(ImageFormat::Hdp),
            "bpg" => Some(ImageFormat::Bpg),
            "flif" => Some(ImageFormat::Flif),
            // Animation
            "apng" => Some(ImageFormat::Apng),
            "mng" => Some(ImageFormat::Mng),
            "fli" => Some(ImageFormat::Fli),
            "flc" => Some(ImageFormat::Flc),
            "ani" => Some(ImageFormat::Ani),
            // Special / compressed
            "heic" => Some(ImageFormat::Heic),
            "heif" => Some(ImageFormat::Heif),
            "dds" => Some(ImageFormat::Dds),
            "ico" => Some(ImageFormat::Ico),
            "cur" => Some(ImageFormat::Cur),
            "dcm" => Some(ImageFormat::Dcm),
            "dicom" => Some(ImageFormat::Dicom),
            "pict" => Some(ImageFormat::Pict),
            "pct" => Some(ImageFormat::Pct),
            "iff" => Some(ImageFormat::Iff),
            "lbm" => Some(ImageFormat::Lbm),
            "mac" => Some(ImageFormat::Mac),
            "cals" => Some(ImageFormat::Cals),
            "cal" => Some(ImageFormat::Cal),
            "fax" => Some(ImageFormat::Fax),
            "jbig" => Some(ImageFormat::Jbig),
            "jb2" => Some(ImageFormat::Jb2),
            "pgf" => Some(ImageFormat::Pgf),
            "pic" => Some(ImageFormat::Pic),
            "blp" => Some(ImageFormat::Blp),
            "vtf" => Some(ImageFormat::Vtf),
            "sfw" => Some(ImageFormat::Sfw),
            // Camera RAW
            "raw" => Some(ImageFormat::Raw),
            "dng" => Some(ImageFormat::Dng),
            "cr2" => Some(ImageFormat::Cr2),
            "cr3" => Some(ImageFormat::Cr3),
            "nef" => Some(ImageFormat::Nef),
            "arw" => Some(ImageFormat::Arw),
            "orf" => Some(ImageFormat::Orf),
            "raf" => Some(ImageFormat::Raf),
            "rw2" => Some(ImageFormat::Rw2),
            // Vector / Design
            "svg" => Some(ImageFormat::Svg),
            "eps" => Some(ImageFormat::Eps),
            "pdf" => Some(ImageFormat::Pdf),
            "psd" => Some(ImageFormat::Psd),
            "psb" => Some(ImageFormat::Psb),
            "ai" => Some(ImageFormat::Ai),
            "indd" => Some(ImageFormat::Indd),
            "xcf" => Some(ImageFormat::Xcf),
            "afphoto" => Some(ImageFormat::Afphoto),
            "afdesign" => Some(ImageFormat::Afdesign),
            "cdr" => Some(ImageFormat::Cdr),
            "sketch" => Some(ImageFormat::Sketch),
            "fig" => Some(ImageFormat::Fig),
            // CAD / vector / other
            "wmf" => Some(ImageFormat::Wmf),
            "emf" => Some(ImageFormat::Emf),
            "skp" => Some(ImageFormat::Skp),
            "dxf" => Some(ImageFormat::Dxf),
            "dwg" => Some(ImageFormat::Dwg),
            "plt" => Some(ImageFormat::Plt),
            "cgm" => Some(ImageFormat::Cgm),
            "cmx" => Some(ImageFormat::Cmx),
            "drw" => Some(ImageFormat::Drw),
            "swf" => Some(ImageFormat::Swf),
            "fla" => Some(ImageFormat::Fla),
            _ => None,
        }
    }

    /// Return a human-readable description of this format.
    pub fn description(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "JPEG image (.jpeg)",
            ImageFormat::Jpg => "JPEG image (.jpg)",
            ImageFormat::Png => "Portable Network Graphics (PNG)",
            ImageFormat::Webp => "WebP image",
            ImageFormat::Avif => "AV1 Image File Format (AVIF)",
            ImageFormat::Gif => "Graphics Interchange Format (GIF)",
            ImageFormat::Bmp => "Windows Bitmap (BMP)",
            ImageFormat::Tiff => "Tagged Image File Format (.tiff)",
            ImageFormat::Tif => "Tagged Image File Format (.tif)",
            ImageFormat::Exr => "OpenEXR High Dynamic Range (EXR)",
            ImageFormat::Hdr => "Radiance HDR image",
            ImageFormat::Dpx => "Digital Picture Exchange (DPX)",
            ImageFormat::Cin => "Cineon digital-film format",
            ImageFormat::Fits => "Flexible Image Transport System (.fits)",
            ImageFormat::Fit => "Flexible Image Transport System (.fit)",
            ImageFormat::Tga => "Truevision TGA / TARGA",
            ImageFormat::Sgi => "SGI native image (.sgi)",
            ImageFormat::Rgb => "SGI RGB image (.rgb)",
            ImageFormat::Pcx => "ZSoft PCX bitmap",
            ImageFormat::Pbm => "Portable Bitmap (PBM)",
            ImageFormat::Pgm => "Portable Graymap (PGM)",
            ImageFormat::Ppm => "Portable Pixmap (PPM)",
            ImageFormat::Pnm => "Portable Any Map (PNM)",
            ImageFormat::Pam => "Portable Arbitrary Map (PAM)",
            ImageFormat::Xbm => "X Window System Bitmap (XBM)",
            ImageFormat::Xpm => "X Window System Pixmap (XPM)",
            ImageFormat::Wbmp => "Wireless Bitmap (WBMP)",
            ImageFormat::Ras => "Sun Rasterfile (.ras)",
            ImageFormat::Sun => "Sun Rasterfile (.sun)",
            ImageFormat::Jp2 => "JPEG 2000 (.jp2)",
            ImageFormat::J2k => "JPEG 2000 codestream (.j2k)",
            ImageFormat::Jxl => "JPEG XL",
            ImageFormat::Jxr => "JPEG XR / HD Photo (.jxr)",
            ImageFormat::Wdp => "JPEG XR / HD Photo (.wdp)",
            ImageFormat::Hdp => "JPEG XR / HD Photo (.hdp)",
            ImageFormat::Bpg => "Better Portable Graphics (BPG)",
            ImageFormat::Flif => "Free Lossless Image Format (FLIF)",
            ImageFormat::Apng => "Animated PNG (APNG)",
            ImageFormat::Mng => "Multiple-image Network Graphics (MNG)",
            ImageFormat::Fli => "Autodesk FLIC animation (.fli)",
            ImageFormat::Flc => "Autodesk FLIC animation (.flc)",
            ImageFormat::Ani => "Windows animated cursor (ANI)",
            ImageFormat::Heic => "High Efficiency Image Container (HEIC)",
            ImageFormat::Heif => "High Efficiency Image Format (HEIF)",
            ImageFormat::Dds => "DirectDraw Surface (DDS)",
            ImageFormat::Ico => "Windows icon (ICO)",
            ImageFormat::Cur => "Windows cursor (CUR)",
            ImageFormat::Dcm => "DICOM medical image (.dcm)",
            ImageFormat::Dicom => "DICOM medical image (.dicom)",
            ImageFormat::Pict => "PICT / Macintosh image (.pict)",
            ImageFormat::Pct => "PICT / Macintosh image (.pct)",
            ImageFormat::Iff => "Amiga IFF image",
            ImageFormat::Lbm => "Amiga IFF / DeluxePaint (LBM)",
            ImageFormat::Mac => "MacPaint image",
            ImageFormat::Cals => "CALS Group 4 raster (.cals)",
            ImageFormat::Cal => "CALS Group 4 raster (.cal)",
            ImageFormat::Fax => "FAX Group 3/4 image",
            ImageFormat::Jbig => "JBIG compressed bitmap (.jbig)",
            ImageFormat::Jb2 => "JBIG2 compressed bitmap (.jb2)",
            ImageFormat::Pgf => "Progressive Graphics File (PGF)",
            ImageFormat::Pic => "Softimage PIC",
            ImageFormat::Blp => "Blizzard Entertainment texture (BLP)",
            ImageFormat::Vtf => "Valve Texture Format (VTF)",
            ImageFormat::Sfw => "Seattle FilmWorks image (SFW)",
            ImageFormat::Raw => "Generic camera RAW",
            ImageFormat::Dng => "Adobe Digital Negative (DNG)",
            ImageFormat::Cr2 => "Canon RAW 2 (CR2)",
            ImageFormat::Cr3 => "Canon RAW 3 (CR3)",
            ImageFormat::Nef => "Nikon Electronic Format (NEF)",
            ImageFormat::Arw => "Sony Alpha RAW (ARW)",
            ImageFormat::Orf => "Olympus RAW Format (ORF)",
            ImageFormat::Raf => "Fujifilm RAW (RAF)",
            ImageFormat::Rw2 => "Panasonic RAW 2 (RW2)",
            ImageFormat::Svg => "Scalable Vector Graphics (SVG)",
            ImageFormat::Eps => "Encapsulated PostScript (EPS)",
            ImageFormat::Pdf => "Portable Document Format (PDF)",
            ImageFormat::Psd => "Adobe Photoshop Document (PSD)",
            ImageFormat::Psb => "Adobe Photoshop Large Document (PSB)",
            ImageFormat::Ai => "Adobe Illustrator (AI)",
            ImageFormat::Indd => "Adobe InDesign (INDD)",
            ImageFormat::Xcf => "GIMP native format (XCF)",
            ImageFormat::Afphoto => "Affinity Photo (.afphoto)",
            ImageFormat::Afdesign => "Affinity Designer (.afdesign)",
            ImageFormat::Cdr => "CorelDRAW (CDR)",
            ImageFormat::Sketch => "Sketch app format",
            ImageFormat::Fig => "Figma design file (FIG)",
            ImageFormat::Wmf => "Windows Metafile (WMF)",
            ImageFormat::Emf => "Enhanced Windows Metafile (EMF)",
            ImageFormat::Skp => "SketchUp 3D model (SKP)",
            ImageFormat::Dxf => "AutoCAD Drawing Exchange Format (DXF)",
            ImageFormat::Dwg => "AutoCAD Drawing (DWG)",
            ImageFormat::Plt => "HPGL plot file (PLT)",
            ImageFormat::Cgm => "Computer Graphics Metafile (CGM)",
            ImageFormat::Cmx => "Corel Metafile Exchange (CMX)",
            ImageFormat::Drw => "Micrografx Draw (DRW)",
            ImageFormat::Swf => "Adobe Flash (SWF)",
            ImageFormat::Fla => "Adobe Flash source (FLA)",
        }
    }

    /// All image formats that can be encoded by FFmpeg (encode-capable subset).
    #[allow(dead_code)]
    pub fn encodable_formats() -> &'static [ImageFormat] {
        &[
            ImageFormat::Jpeg,
            ImageFormat::Jpg,
            ImageFormat::Png,
            ImageFormat::Webp,
            ImageFormat::Avif,
            ImageFormat::Gif,
            ImageFormat::Bmp,
            ImageFormat::Tiff,
            ImageFormat::Tif,
            ImageFormat::Exr,
            ImageFormat::Hdr,
            ImageFormat::Dpx,
            ImageFormat::Cin,
            ImageFormat::Fits,
            ImageFormat::Fit,
            ImageFormat::Tga,
            ImageFormat::Sgi,
            ImageFormat::Rgb,
            ImageFormat::Pcx,
            ImageFormat::Pbm,
            ImageFormat::Pgm,
            ImageFormat::Ppm,
            ImageFormat::Pnm,
            ImageFormat::Pam,
            ImageFormat::Xbm,
            ImageFormat::Xpm,
            ImageFormat::Wbmp,
            ImageFormat::Ras,
            ImageFormat::Sun,
            ImageFormat::Jp2,
            ImageFormat::J2k,
            ImageFormat::Jxl,
            ImageFormat::Apng,
            ImageFormat::Dds,
            ImageFormat::Ico,
        ]
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.file_extension())
    }
}

impl FromStr for ImageFormat {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            // Common raster
            "jpeg" => Ok(ImageFormat::Jpeg),
            "jpg" => Ok(ImageFormat::Jpg),
            "png" => Ok(ImageFormat::Png),
            "webp" => Ok(ImageFormat::Webp),
            "avif" => Ok(ImageFormat::Avif),
            "gif" => Ok(ImageFormat::Gif),
            "bmp" => Ok(ImageFormat::Bmp),
            "tiff" => Ok(ImageFormat::Tiff),
            "tif" => Ok(ImageFormat::Tif),
            // Professional / HDR
            "exr" => Ok(ImageFormat::Exr),
            "hdr" => Ok(ImageFormat::Hdr),
            "dpx" => Ok(ImageFormat::Dpx),
            "cin" => Ok(ImageFormat::Cin),
            "fits" => Ok(ImageFormat::Fits),
            "fit" => Ok(ImageFormat::Fit),
            // Legacy raster
            "tga" => Ok(ImageFormat::Tga),
            "sgi" => Ok(ImageFormat::Sgi),
            "rgb" => Ok(ImageFormat::Rgb),
            "pcx" => Ok(ImageFormat::Pcx),
            "pbm" => Ok(ImageFormat::Pbm),
            "pgm" => Ok(ImageFormat::Pgm),
            "ppm" => Ok(ImageFormat::Ppm),
            "pnm" => Ok(ImageFormat::Pnm),
            "pam" => Ok(ImageFormat::Pam),
            "xbm" => Ok(ImageFormat::Xbm),
            "xpm" => Ok(ImageFormat::Xpm),
            "wbmp" => Ok(ImageFormat::Wbmp),
            "ras" => Ok(ImageFormat::Ras),
            "sun" => Ok(ImageFormat::Sun),
            // JPEG variants
            "jp2" => Ok(ImageFormat::Jp2),
            "j2k" | "j2c" => Ok(ImageFormat::J2k),
            "jxl" => Ok(ImageFormat::Jxl),
            "jxr" => Ok(ImageFormat::Jxr),
            "wdp" => Ok(ImageFormat::Wdp),
            "hdp" => Ok(ImageFormat::Hdp),
            "bpg" => Ok(ImageFormat::Bpg),
            "flif" => Ok(ImageFormat::Flif),
            // Animation
            "apng" => Ok(ImageFormat::Apng),
            "mng" => Ok(ImageFormat::Mng),
            "fli" => Ok(ImageFormat::Fli),
            "flc" => Ok(ImageFormat::Flc),
            "ani" => Ok(ImageFormat::Ani),
            // Special / compressed
            "heic" => Ok(ImageFormat::Heic),
            "heif" => Ok(ImageFormat::Heif),
            "dds" => Ok(ImageFormat::Dds),
            "ico" => Ok(ImageFormat::Ico),
            "cur" => Ok(ImageFormat::Cur),
            "dcm" => Ok(ImageFormat::Dcm),
            "dicom" => Ok(ImageFormat::Dicom),
            "pict" => Ok(ImageFormat::Pict),
            "pct" => Ok(ImageFormat::Pct),
            "iff" => Ok(ImageFormat::Iff),
            "lbm" => Ok(ImageFormat::Lbm),
            "mac" => Ok(ImageFormat::Mac),
            "cals" => Ok(ImageFormat::Cals),
            "cal" => Ok(ImageFormat::Cal),
            "fax" => Ok(ImageFormat::Fax),
            "jbig" => Ok(ImageFormat::Jbig),
            "jb2" => Ok(ImageFormat::Jb2),
            "pgf" => Ok(ImageFormat::Pgf),
            "pic" => Ok(ImageFormat::Pic),
            "blp" => Ok(ImageFormat::Blp),
            "vtf" => Ok(ImageFormat::Vtf),
            "sfw" => Ok(ImageFormat::Sfw),
            // Camera RAW
            "raw" => Ok(ImageFormat::Raw),
            "dng" => Ok(ImageFormat::Dng),
            "cr2" => Ok(ImageFormat::Cr2),
            "cr3" => Ok(ImageFormat::Cr3),
            "nef" => Ok(ImageFormat::Nef),
            "arw" => Ok(ImageFormat::Arw),
            "orf" => Ok(ImageFormat::Orf),
            "raf" => Ok(ImageFormat::Raf),
            "rw2" => Ok(ImageFormat::Rw2),
            // Vector / Design
            "svg" => Ok(ImageFormat::Svg),
            "eps" => Ok(ImageFormat::Eps),
            "pdf" => Ok(ImageFormat::Pdf),
            "psd" => Ok(ImageFormat::Psd),
            "psb" => Ok(ImageFormat::Psb),
            "ai" => Ok(ImageFormat::Ai),
            "indd" => Ok(ImageFormat::Indd),
            "xcf" => Ok(ImageFormat::Xcf),
            "afphoto" => Ok(ImageFormat::Afphoto),
            "afdesign" => Ok(ImageFormat::Afdesign),
            "cdr" => Ok(ImageFormat::Cdr),
            "sketch" => Ok(ImageFormat::Sketch),
            "fig" => Ok(ImageFormat::Fig),
            // CAD / vector / other
            "wmf" => Ok(ImageFormat::Wmf),
            "emf" => Ok(ImageFormat::Emf),
            "skp" => Ok(ImageFormat::Skp),
            "dxf" => Ok(ImageFormat::Dxf),
            "dwg" => Ok(ImageFormat::Dwg),
            "plt" => Ok(ImageFormat::Plt),
            "cgm" => Ok(ImageFormat::Cgm),
            "cmx" => Ok(ImageFormat::Cmx),
            "drw" => Ok(ImageFormat::Drw),
            "swf" => Ok(ImageFormat::Swf),
            "fla" => Ok(ImageFormat::Fla),
            _ => anyhow::bail!(
                "'{}' is not a known image format. \
                 Supported image output formats include: jpeg, jpg, png, webp, avif, gif, bmp, \
                 tiff, tif, exr, hdr, dpx, tga, sgi, pbm, pgm, ppm, pam, jp2, j2k, jxl, apng, \
                 dds, ico, and many others.",
                s
            ),
        }
    }
}

/// Return `true` when the file-path extension identifies an image file that
/// renderflow can process as an image input.
#[allow(dead_code)]
pub fn is_image_path(path: &str) -> bool {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    ImageFormat::from_extension(ext).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── from_extension ────────────────────────────────────────────────────────

    #[test]
    fn test_from_extension_jpeg() {
        assert_eq!(ImageFormat::from_extension("jpeg"), Some(ImageFormat::Jpeg));
    }

    #[test]
    fn test_from_extension_jpg() {
        assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpg));
    }

    #[test]
    fn test_from_extension_png() {
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
    }

    #[test]
    fn test_from_extension_webp() {
        assert_eq!(ImageFormat::from_extension("webp"), Some(ImageFormat::Webp));
    }

    #[test]
    fn test_from_extension_avif() {
        assert_eq!(ImageFormat::from_extension("avif"), Some(ImageFormat::Avif));
    }

    #[test]
    fn test_from_extension_tiff_and_tif() {
        assert_eq!(ImageFormat::from_extension("tiff"), Some(ImageFormat::Tiff));
        assert_eq!(ImageFormat::from_extension("tif"), Some(ImageFormat::Tif));
    }

    #[test]
    fn test_from_extension_case_insensitive() {
        assert_eq!(ImageFormat::from_extension("JPEG"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("PNG"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("GIF"), Some(ImageFormat::Gif));
    }

    #[test]
    fn test_from_extension_raw_formats() {
        assert_eq!(ImageFormat::from_extension("cr2"), Some(ImageFormat::Cr2));
        assert_eq!(ImageFormat::from_extension("nef"), Some(ImageFormat::Nef));
        assert_eq!(ImageFormat::from_extension("arw"), Some(ImageFormat::Arw));
        assert_eq!(ImageFormat::from_extension("dng"), Some(ImageFormat::Dng));
    }

    #[test]
    fn test_from_extension_vector_formats() {
        assert_eq!(ImageFormat::from_extension("svg"), Some(ImageFormat::Svg));
        assert_eq!(ImageFormat::from_extension("eps"), Some(ImageFormat::Eps));
        assert_eq!(ImageFormat::from_extension("pdf"), Some(ImageFormat::Pdf));
        assert_eq!(ImageFormat::from_extension("psd"), Some(ImageFormat::Psd));
        assert_eq!(ImageFormat::from_extension("ai"), Some(ImageFormat::Ai));
    }

    #[test]
    fn test_from_extension_unknown_returns_none() {
        assert_eq!(ImageFormat::from_extension("xyz"), None);
        assert_eq!(ImageFormat::from_extension("mp3"), None);
        assert_eq!(ImageFormat::from_extension("md"), None);
    }

    // ── file_extension ────────────────────────────────────────────────────────

    #[test]
    fn test_file_extension_jpeg() {
        assert_eq!(ImageFormat::Jpeg.file_extension(), "jpeg");
    }

    #[test]
    fn test_file_extension_jpg() {
        assert_eq!(ImageFormat::Jpg.file_extension(), "jpg");
    }

    #[test]
    fn test_file_extension_png() {
        assert_eq!(ImageFormat::Png.file_extension(), "png");
    }

    // ── supports_encoding ─────────────────────────────────────────────────────

    #[test]
    fn test_supports_encoding_jpeg() {
        assert!(ImageFormat::Jpeg.supports_encoding());
    }

    #[test]
    fn test_supports_encoding_png() {
        assert!(ImageFormat::Png.supports_encoding());
    }

    #[test]
    fn test_supports_encoding_webp() {
        assert!(ImageFormat::Webp.supports_encoding());
    }

    #[test]
    fn test_supports_encoding_svg_false() {
        assert!(!ImageFormat::Svg.supports_encoding());
    }

    #[test]
    fn test_supports_encoding_psd_false() {
        assert!(!ImageFormat::Psd.supports_encoding());
    }

    #[test]
    fn test_supports_encoding_cr2_false() {
        assert!(!ImageFormat::Cr2.supports_encoding());
    }

    #[test]
    fn test_supports_encoding_heic_false() {
        assert!(!ImageFormat::Heic.supports_encoding());
    }

    // ── is_animated ──────────────────────────────────────────────────────────

    #[test]
    fn test_is_animated_gif() {
        assert!(ImageFormat::Gif.is_animated());
    }

    #[test]
    fn test_is_animated_apng() {
        assert!(ImageFormat::Apng.is_animated());
    }

    #[test]
    fn test_is_animated_png_false() {
        assert!(!ImageFormat::Png.is_animated());
    }

    // ── is_image_path ─────────────────────────────────────────────────────────

    #[test]
    fn test_is_image_path_jpeg() {
        assert!(is_image_path("photo.jpg"));
        assert!(is_image_path("photo.jpeg"));
    }

    #[test]
    fn test_is_image_path_png() {
        assert!(is_image_path("image.png"));
    }

    #[test]
    fn test_is_image_path_unknown() {
        assert!(!is_image_path("audio.mp3"));
        assert!(!is_image_path("document.md"));
    }

    // ── from_str ──────────────────────────────────────────────────────────────

    #[test]
    fn test_from_str_jpeg() {
        assert_eq!("jpeg".parse::<ImageFormat>().unwrap(), ImageFormat::Jpeg);
    }

    #[test]
    fn test_from_str_png() {
        assert_eq!("png".parse::<ImageFormat>().unwrap(), ImageFormat::Png);
    }

    #[test]
    fn test_from_str_unknown_returns_error() {
        let result = "unknown_xyz".parse::<ImageFormat>();
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not a known image format"),
            "unexpected error: {msg}"
        );
    }

    // ── display ───────────────────────────────────────────────────────────────

    #[test]
    fn test_display_uses_extension() {
        assert_eq!(format!("{}", ImageFormat::Png), "png");
        assert_eq!(format!("{}", ImageFormat::Jpeg), "jpeg");
    }
}
