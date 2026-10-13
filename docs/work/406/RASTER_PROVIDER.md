# Composition PNG provider decision

Date: 2026-10-13. Issue: #406. Qualification: source review only.

## Decision

Use Resvg **0.48.1**, its matching Usvg **0.48.1** parser and Tiny Skia
**0.12.0** rasterizer to composite the SVG generated from the measured page.
Use PNG **0.18.1** for explicit decode preflight and output encoding,
Zune JPEG **0.5.15** for JPEG decode preflight, and SVGTypes **0.16.1** for
strict source path and number parsing before Usvg's permissive conversion.
ImageSize **0.15.0** is a direct pin of Usvg's selected header parser, used to
check that renderer admission agrees with the fully decoded source dimensions.
Resvg reexports both Usvg and Tiny Skia; there is one selected version of each.

The PNG branch consumes the same positioned glyph paths, layer geometry,
clipping, opacity and captured artwork that the SVG branch serializes. It does
not lay out the page again or discover fonts. The existing typography provider
remains responsible for shaping and measured outlines.

Resvg's default features are disabled. Only `raster-images` is enabled; `text`,
`system-fonts`, `memmap-fonts` and `svgz` are not enabled. The raster-images
feature also compiles upstream GIF and WebP support, but Renderflow's admitted
input profile refuses those formats. This feature is required even for PNG
rendering: disabling it would make Resvg omit raster image nodes altogether.

This is the maintained Linebender renderer family. Resvg's declared Rust 1.85
minimum is below this workspace's Rust 1.94 floor. No browser, native renderer,
image command, system font registry or external resource loader is involved.

## Exact provider boundary

- `usvg::Tree::from_data(bytes, &options)` parses the generated measured SVG.
- `Options::image_href_resolver` replaces both resolver callbacks. The string
  callback refuses every relative, file and network reference. The data
  callback admits only captured, preflighted PNG/JPEG and inert SVG data.
- Nested SVG uses `Tree::from_data_nested`, which preserves the custom data
  resolver and denies string references in the nested parse.
- `resvg::render(&tree, transform, &mut pixmap.as_mut())` paints into a bounded
  RGBA pixmap. The point-to-pixel transform uses the requested DPI.
- `Pixmap::take_demultiplied()` converts Tiny Skia's premultiplied internal
  storage to straight RGBA for the custom PNG encoder. Feeding `Pixmap::data`
  directly to this encoder would give incorrect translucent edge colors.
- `png::Encoder` explicitly selects RGBA, eight-bit samples, compression,
  filtering, sRGB intent and physical pixel resolution. Encoder timestamps and
  other variable metadata are not added.

Usvg's default string resolver reads files. Setting `resources_dir` to `None`
alone would not close that path, so replacing the resolver is mandatory.
Its parser also allows XML DTDs internally. The composition loader and strict
generated-SVG preflight reject DTD/entity surfaces before provider parsing.

## Reject omissions before rendering

Upstream image parsing and painting can log and omit an unsupported or corrupt
image instead of returning an error. A successful tree parse, or Resvg's
unit-returning render call, is consequently not sufficient evidence of a
complete page. Renderflow preflights admitted sources and propagates resolver
failures as composition diagnostics before publishing bytes.

Usvg independently obtains raster dimensions using `imagesize::blob_size`
after the custom resolver returns an image. Renderflow calls that same pinned
header parser and compares its dimensions with preflight before rendering.
This closes a separate omission path where the full codec could accept bytes
that Usvg's header reader rejects. ImageSize returns public `usize` width and
height fields; it does not replace full PNG/JPEG decoding.

PNG preflight reads image data with the pinned PNG decoder, applies explicit
memory and dimension limits, and calls `Reader::finish` after `next_frame`.
The final call consumes remaining chunks and catches errors after the first
frame. Header sniffing alone cannot establish a decodable source. PNG decoder
limits cover its intermediate allocations; the caller separately bounds the
output buffer.

JPEG preflight uses the same Zune decoder family as Resvg, with explicit strict
mode, source dimensions, scan limits and RGBA output. The output colorspace is
checked after header decoding because JPEG markers can affect that choice.
The accepted source dimensions must stay within Resvg's internal decoder
defaults as well as Renderflow's stricter limits; otherwise preflight could
accept an image that the renderer later omits.

SVGTypes' `PathParser` and `NumberListParser` provide fallible syntax parsing,
with finite coordinate and work budgets enforced by Renderflow. Usvg remains
the SVG interpreter and Resvg remains the rasterizer; this preflight does not
introduce a second custom drawing engine. The parsed tree is also inspected
for unsupported paint/effect surfaces and excessive allocation estimates.

## Authored PNG profile

The default is **144 DPI with a transparent background**. An integer DPI from
36 through 600 and either `transparent` or an opaque `#RRGGBB` background are
explicit inputs. Hex colors are normalized to lowercase. Width and height are
`ceil(page_mm * dpi / 25.4)`; a uniform `dpi / 72` transform preserves the page
aspect ratio while any fractional pixel padding remains at the right/bottom.
Usvg interprets the generated physical dimensions at 72 DPI, matching the
page's point-based viewport.

Output is straight-alpha RGBA8. Encoding selects `Compression::Fast` followed
by `Filter::Sub`, a perceptual sRGB declaration, and a `pHYs` density rounded
to the nearest integer pixels per metre (`round(dpi / 0.0254)`). The output is
fully decoded and its dimensions, sample format, sRGB declaration and physical
resolution are checked before publication; cached PNG candidates receive the
same output checks. The receipt records `runtime_qualified: false` and the
versioned provider/encoding profile.

PNG input is restricted to eight-bit grayscale, RGB, palette, grayscale-alpha
or RGBA, including interlaced sources. The metadata preflight checks every
chunk CRC and rejects animation, ICC profiles, EXIF and HDR/color metadata
outside the admitted assumed-sRGB policy. A declared gamma must be 45455 and
declared chromaticities must match the sRGB primaries/white point. JPEG input
is one eight-bit baseline or progressive frame with one or three components;
EXIF/XMP APP1, ICC APP2 and Adobe APP14 inputs are refused. No orientation
rewrite or ICC conversion is implied. JPEG decoding is strict with at most
64 scans, below the renderer's default scan limit.

| Limit | Value |
| --- | --- |
| Source raster or output axis | 8,192 pixels |
| Source raster or output area | 16,777,216 pixels |
| Raster instances across page, including repeated use | 33,554,432 decoded pixels |
| Captured asset size | 16 MiB |
| Encoded output or generated SVG size | 64 MiB |
| Decoder intermediate/output budget | 64 MiB each |
| PNG chunks or JPEG markers | 16,384 |
| Generated SVG XML nodes | 32,768 |
| Captured SVG nesting | 16 elements |
| Captured SVG path segments or coordinate pairs | 65,536 |
| Captured SVG coordinate magnitude | 10,000,000 user units |
| Captured SVG explicit root/viewBox dimension minimum | 0.000001 user units |
| Parsed rendering tree nodes including repetitions | 65,536 |
| Parsed rendering tree depth | 32 |
| Parsed path segments including repetitions | 524,288 |
| Estimated simultaneously active pixel surfaces | 384 MiB |

The tree preflight additionally refuses unsupported filters, masks, non-normal
blend modes, gradient/pattern paints and nested clip-path constructs. Isolated
groups are bounded against the output canvas before Resvg's larger internal
clipping range can cause excessive intermediate allocations.

## Qualification and limits

The software composite assumes the admitted image samples describe sRGB.
Declaring sRGB in the output is not an ICC conversion, wide-gamut workflow or
print proof. Source metadata, animation and orientation policies are enforced
at the input boundary rather than delegated to provider best-effort behavior.

The output surface, decoded sources, tree depth, repeated path work and active
intermediate pixel surfaces are bounded before painting. Isolated opacity
groups, clipping and embedded SVG can allocate additional surfaces, so the
output pixel count alone is not an adequate memory estimate. These checks are
conservative admission limits, not total-process memory or elapsed-time
isolation. The synchronous provider call is not internally cancellable.

Version pins and explicit encoding settings make the inputs and policy
reviewable. Compilation, repeat-byte behavior, image fidelity and
cross-platform raster equivalence have not been established in this
checkpoint. Authored regression sources remain unrun for the deferred testing
session. No build, tests, lint, schema checks or CI runs were requested or run.

## Dependency provenance and lock strategy

No Cargo resolution, fetch or crate archive download was run. `Cargo.lock` was
authored from official crates.io index release records: checksums, required
normal dependencies and the selected feature graph. There are 27 new nodes.
The direct ImageSize dependency reuses its existing new transitive node; it
adds a Renderflow edge without adding another package version.
Existing package versions are retained, including SipHasher 1.0.2. Existing
ROXMLTree 0.20.0 and Weezl 0.2.1 remain for their existing users; their references
are version-qualified alongside the new 0.21.1 and 0.1.12 nodes.

| Package | Version | Registry release metadata |
| --- | --- | --- |
| resvg | 0.48.1 | [index](https://github.com/rust-lang/crates.io-index/blob/master/re/sv/resvg) |
| usvg | 0.48.1 | [index](https://github.com/rust-lang/crates.io-index/blob/master/us/vg/usvg) |
| tiny-skia | 0.12.0 | [index](https://github.com/rust-lang/crates.io-index/blob/master/ti/ny/tiny-skia) |
| tiny-skia-path | 0.12.0 | [index](https://github.com/rust-lang/crates.io-index/blob/master/ti/ny/tiny-skia-path) |
| png | 0.18.1 | [index](https://github.com/rust-lang/crates.io-index/blob/master/3/p/png) |
| zune-jpeg | 0.5.15 | [index](https://github.com/rust-lang/crates.io-index/blob/master/zu/ne/zune-jpeg) |
| zune-core | 0.5.3 | [index](https://github.com/rust-lang/crates.io-index/blob/master/zu/ne/zune-core) |
| svgtypes | 0.16.1 | [index](https://github.com/rust-lang/crates.io-index/blob/master/sv/gt/svgtypes) |
| arrayref | 0.3.9 | [index](https://github.com/rust-lang/crates.io-index/blob/master/ar/ra/arrayref) |
| arrayvec | 0.7.8 | [index](https://github.com/rust-lang/crates.io-index/blob/master/ar/ra/arrayvec) |
| byteorder-lite | 0.1.0 | [index](https://github.com/rust-lang/crates.io-index/blob/master/by/te/byteorder-lite) |
| color_quant | 1.1.0 | [index](https://github.com/rust-lang/crates.io-index/blob/master/co/lo/color_quant) |
| data-url | 0.3.2 | [index](https://github.com/rust-lang/crates.io-index/blob/master/da/ta/data-url) |
| fdeflate | 0.3.7 | [index](https://github.com/rust-lang/crates.io-index/blob/master/fd/ef/fdeflate) |
| float-cmp | 0.9.0 | [index](https://github.com/rust-lang/crates.io-index/blob/master/fl/oa/float-cmp) |
| gif | 0.14.2 | [index](https://github.com/rust-lang/crates.io-index/blob/master/3/g/gif) |
| image-webp | 0.2.4 | [index](https://github.com/rust-lang/crates.io-index/blob/master/im/ag/image-webp) |
| imagesize | 0.15.0 | [index](https://github.com/rust-lang/crates.io-index/blob/master/im/ag/imagesize) |
| kurbo | 0.13.1 | [index](https://github.com/rust-lang/crates.io-index/blob/master/ku/rb/kurbo) |
| pico-args | 0.5.0 | [index](https://github.com/rust-lang/crates.io-index/blob/master/pi/co/pico-args) |
| polycool | 0.4.0 | [index](https://github.com/rust-lang/crates.io-index/blob/master/po/ly/polycool) |
| quick-error | 2.0.1 | [index](https://github.com/rust-lang/crates.io-index/blob/master/qu/ic/quick-error) |
| rgb | 0.8.53 | [index](https://github.com/rust-lang/crates.io-index/blob/master/3/r/rgb) |
| roxmltree | 0.21.1 | [index](https://github.com/rust-lang/crates.io-index/blob/master/ro/xm/roxmltree) |
| simplecss | 0.2.2 | [index](https://github.com/rust-lang/crates.io-index/blob/master/si/mp/simplecss) |
| strict-num | 0.1.1 | [index](https://github.com/rust-lang/crates.io-index/blob/master/st/ri/strict-num) |
| weezl | 0.1.12 | [index](https://github.com/rust-lang/crates.io-index/blob/master/we/ez/weezl) |

The lock graph still needs the later authorized Cargo resolution/build gate.
Verified registry records and source-level API review do not imply that Cargo
accepted this graph or that the implementation compiled.

## Primary API sources

Retrieved 2026-10-13. Source and documentation retrieval only.

- <https://github.com/linebender/resvg/blob/v0.48.1/crates/resvg/src/lib.rs>
- <https://github.com/linebender/resvg/blob/v0.48.1/crates/resvg/src/image.rs>
- <https://github.com/linebender/resvg/blob/v0.48.1/crates/usvg/src/parser/mod.rs>
- <https://github.com/linebender/resvg/blob/v0.48.1/crates/usvg/src/parser/options.rs>
- <https://github.com/linebender/resvg/blob/v0.48.1/crates/usvg/src/parser/image.rs>
- <https://github.com/linebender/tiny-skia/blob/v0.12.0/src/pixmap.rs>
- <https://github.com/linebender/svgtypes/blob/v0.16.1/src/path.rs>
- <https://github.com/linebender/svgtypes/blob/v0.16.1/src/number.rs>
- <https://docs.rs/png/0.18.1/png/struct.Decoder.html>
- <https://docs.rs/png/0.18.1/png/struct.Reader.html>
- <https://docs.rs/png/0.18.1/png/struct.Encoder.html>
- <https://docs.rs/png/0.18.1/png/enum.Compression.html>
- <https://docs.rs/png/0.18.1/png/enum.Filter.html>
- <https://docs.rs/zune-jpeg/0.5.15/zune_jpeg/struct.JpegDecoder.html>
- <https://docs.rs/zune-core/0.5.3/zune_core/options/struct.DecoderOptions.html>
- <https://docs.rs/imagesize/0.15.0/imagesize/fn.blob_size.html>
- <https://docs.rs/imagesize/0.15.0/imagesize/struct.ImageSize.html>
