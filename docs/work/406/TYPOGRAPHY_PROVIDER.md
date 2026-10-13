# Composition typography provider decision

Date: 2026-10-13. Issue: #406. Qualification: source review only.

## Decision

Use HarfRust **0.14.0** for glyph shaping and Skrifa **0.48.0** for unhinted
outline extraction and font metrics. Both use read-fonts **0.45.0**, so the
shaper and outline reader consume the same captured SFNT bytes without system
font discovery, native processes, or browser text shaping.

The composition profile admits static, single-face TrueType/OpenType fonts and
horizontal Latin text with authored line breaks. One declared font must cover
the entire text layer; fallback selection never splits a combining sequence
across fonts. The high-level composition policy admits scripts and locale
direction, while the low-level provider shapes each admitted line explicitly
as Latin, left to right, using the locale as the OpenType language.

SVG and HTML serialize the positioned glyph outlines used for measurement.
Separate semantic transcripts retain the actual text. This establishes a
shared measurement/output representation; it does not establish pixel equality
between rasterizers or replace later visual and print qualification.

## Why this pair

The initially considered RustyBuzz/ttf-parser pair is no longer an appropriate
new dependency. RustyBuzz's upstream repository is archived and explicitly
recommends HarfRust. Its upstream discussion also records the maintenance
transition. HarfRust uses the maintained Fontations parser, which Skrifa also
uses. No native HarfBuzz installation or host font registry is required.

The selected releases have Rust 1.85 as their declared minimum, below this
workspace's Rust 1.94 floor. The std feature is selected explicitly for both
direct dependencies. Skrifa's autohint_shaping default is disabled: this profile
draws unhinted outlines and does not execute font hinting programs.

## Exact API contract used

- `harfrust::Font::new(bytes.to_vec(), 0)` owns the captured byte copy;
  `ShaperFont::new(&font)` supplies default font-unit scaling.
- `Buffer::push_str` records UTF-8 byte offsets as clusters. Direction, Latin
  script, locale language and monotone grapheme clusters are explicit.
- `harfrust::shape(..., ShapeOptions::default())` supplies glyph IDs, advances,
  offsets and clusters. `allocation_successful()` is also checked because
  provider allocation/operation limits are a distinct failure signal.
- `skrifa::FontRef::new(bytes)` and `MetadataProvider` provide nominal cmap,
  global metrics and outline glyph access.
- `DrawSettings::unhinted(Size::unscaled(), LocationRef::default())` produces
  outline coordinates in font units. The outline pen records SVG commands and
  a conservative control-point hull from the same coordinates.
- Coordinate serialization converts the provider's f32 coordinates to f64,
  normalizes negative zero, and writes a round-tripping f64 decimal. Bounds use
  the identical values, avoiding a discrepancy between decimal SVG coordinates
  and measured values.
- The high-level layer layout scales by points per em, positions the shaped
  glyphs, flips the font's upward Y axis for page coordinates, and compares the
  resulting bounds with authored text boxes, clipping and safe geometry.

The control-point hull encloses Bezier curves. It can conservatively reject a
near-edge glyph whose actual curve would fit; it is not a claim of exact ink
extrema. Moving or sizing the layer is the explicit correction path. Automatic
wrapping, shrinking and unreviewed geometry rewriting are outside this profile.

## Refusals and bounded work

The low-level provider rejects WOFF/WOFF2, collections, variable-font tables,
color/bitmap glyph tables, missing required metrics/cmap/outline tables,
overlapping or duplicate table records and table ranges outside the captured
font. It parses the required metric/cmap tables, present top-level GSUB/GPOS/GDEF
and kern tables, and the requested glyph outlines.
This is not a complete font validator: unrelated or unused data is not fully
decoded.

Every nominal character, including spaces, needs a nonzero cmap glyph. Every
shaped glyph must remain inside the font's glyph range and have a valid UTF-8
cluster. An empty outline is allowed only for whitespace clusters; non-whitespace
outline failures block the candidate. Required metrics, positions and outline
coordinates must be finite and bounded. The caller records rejected candidates
and chooses only from the declared font order.

Resource limits in the provider are 16 MiB per captured font, 256 SFNT tables,
4,096 input characters and shaped glyphs per authored line, 4 MiB of path data
and 131,072 path commands per line, 8 MiB of provider scratch space per outline,
and absolute coordinate magnitude of 1,000,000 font units. HarfRust's internal
operation/allocation failure state is checked. The bounded pen stops growing
when a limit is hit and discards the result after drawing. These limits do not
claim wall-clock isolation for arbitrary hostile fonts. The composition layer
also applies aggregate limits and cancellation at its existing boundaries.

RTL, vertical layout, non-Latin script shaping, variation selection, color
glyph rendering, font embedding restrictions beyond the registry's recorded
license policy, rasterization and print qualification remain separate work.

## Dependency provenance and lock strategy

No Cargo resolution, fetch, build or tests were run. The lock additions were
authored from the official crates.io index's release metadata, checksums and
normal dependency requirements. Six new nodes were added:

| Package | Version | Purpose |
| --- | --- | --- |
| harfrust | 0.14.0 | Shaping |
| skrifa | 0.48.0 | Metrics and outlines |
| read-fonts | 0.45.0 | Shared font parser |
| font-types | 0.12.6 | OpenType primitive types |
| bytemuck | 1.25.2 | Provider byte representations |
| bytemuck_derive | 1.12.1 | Required derived byte traits |

The new nodes reuse the lock's existing bitflags 2.13.2, smallvec 1.15.1,
once_cell 1.21.4, yoke 0.8.2, proc-macro2, quote and syn 3.0.5. Optional ICU,
libm and spin provider features are not enabled. The lock graph still needs an
authorized Cargo resolution/build gate later; verified registry metadata is
not evidence that compilation or runtime qualification has passed.

## Primary sources

All retrieved 2026-10-13. Source retrieval only; no crate archive, runtime,
installer or font was downloaded for this decision.

Maintenance and architectural direction:

- <https://github.com/harfbuzz/rustybuzz>
- <https://github.com/harfbuzz/rustybuzz/issues/166>
- <https://github.com/harfbuzz/harfrust>

Pinned shaping source:

- <https://github.com/harfbuzz/harfrust/blob/0.14.0/harfrust/src/lib.rs>
- <https://github.com/harfbuzz/harfrust/blob/0.14.0/harfrust/src/buffer.rs>
- <https://github.com/harfbuzz/harfrust/blob/0.14.0/harfrust/src/shaper_font.rs>
- <https://github.com/harfbuzz/harfrust/blob/0.14.0/harfrust/src/shape.rs>
- <https://github.com/harfbuzz/harfrust/blob/0.14.0/harfrust/src/language.rs>
- <https://github.com/harfbuzz/harfrust/blob/0.14.0/harfrust/src/options.rs>

Pinned parser/outline source:

- <https://github.com/googlefonts/fontations/blob/read-fonts-v0.45.0/read-fonts/src/lib.rs>
- <https://github.com/googlefonts/fontations/blob/read-fonts-v0.45.0/read-fonts/src/table_provider.rs>
- <https://github.com/googlefonts/fontations/blob/read-fonts-v0.45.0/read-fonts/src/model/font.rs>
- <https://github.com/googlefonts/fontations/blob/read-fonts-v0.45.0/read-fonts/src/model/font/blob.rs>
- <https://github.com/googlefonts/fontations/blob/read-fonts-v0.45.0/read-fonts/src/model/font/source.rs>
- <https://github.com/googlefonts/fontations/blob/skrifa-v0.48.0/skrifa/src/outline/mod.rs>
- <https://github.com/googlefonts/fontations/blob/skrifa-v0.48.0/skrifa/src/outline/pen.rs>
- <https://github.com/googlefonts/fontations/blob/skrifa-v0.48.0/skrifa/src/instance.rs>
- <https://github.com/googlefonts/fontations/blob/skrifa-v0.48.0/skrifa/src/metrics.rs>

Official package version and dependency/checksum metadata:

- <https://github.com/rust-lang/crates.io-index/blob/master/ha/rf/harfrust>
- <https://github.com/rust-lang/crates.io-index/blob/master/sk/ri/skrifa>
- <https://github.com/rust-lang/crates.io-index/blob/master/re/ad/read-fonts>
- <https://github.com/rust-lang/crates.io-index/blob/master/fo/nt/font-types>
- <https://github.com/rust-lang/crates.io-index/blob/master/by/te/bytemuck>
- <https://github.com/rust-lang/crates.io-index/blob/master/by/te/bytemuck_derive>

