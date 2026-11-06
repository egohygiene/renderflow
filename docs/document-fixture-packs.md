# Document fixture packs

The original **Lantern Map** comic extends the existing
[golden corpus](conformance-corpus.md), its v1 schemas and Rust integration
harness. Corpus 1.4.0 has 31 source fixtures and an additive `packs` convention;
there is no second runner or product publication model. This checkpoint is
**authored, uncompiled and unrun**. No rendered comic, passing report, visual
baseline or provider acceptance has been produced.

## First pack

[`pack.lantern-map`](../tests/fixtures/golden-artifacts/v1/packs/lantern-map/README.md)
contains an original two-character story, simple geometric artwork and explicit
text/artwork reading relationships. Pip is an orange circle courier; Vale is a
blue square mapmaker. They find a route home by sharing a lantern. Text and
geometry are dedicated to CC0; unchanged DejaVu Sans and its full independent
license are reused from the existing composition fixture. No private comic,
customer data, generated service image or actual approval record is copied.

| Case | Tier | Source model | Selected exercise |
| --- | --- | --- | --- |
| `minimal` | `fast` | Two interior pages plus front/back cover pages | Five per-page candidates, explicit RGB preparation, ordered fixed-layout EPUB, inspections and negative gates; PDF packets opt in separately |
| `publication-44` | `maximal` | 44 ordered interior pages and a separate two-page cover group | Source materialization and planning only; zero rendered publications |

Both cases use one pinned geometric recipe, seed zero, 100 × 140 mm canvas and
127 DPI preview settings, yielding exact 500 × 700 pixel canvases. This explicit
recipe/version change avoids rounding-induced aspect drift when the ordered
publication routes place images without crop/stretch. The larger case cycles two story beats as numbered
synthetic layout stress; it is not 44 unique narrative pages. Each page retains
one textless two-panel SVG plate, separate English content and stable semantic
IDs. All page manifests share one captured font registry/license in the package.
Synthetic `reviewed` flags exercise the content gate; every generated candidate
still requires human review and has no publication approval.

## Manifest convention

The `packs` array in the canonical `corpus.json` defines
`renderflow.document-fixture-pack/v1` inside the existing golden schema:

- Stable pack/version/document-type and source fixture IDs.
- Exact recipe source and supporting publication/normalization source, Cargo
  lock, font/registry/license and CC0 rights pins.
- Selected native profile, required/optional capabilities and unavailable mappings.
- Case tier, fixed parameters, source/file/output bounds and cooperative time budget.
- Expected per-page role, format, media type, optional schema and assertion identity.
- Named positive/negative cases. Scenarios reference the pack/case and original
  source fixture; every case must have exactly one scenario.

The story uses additive `encoding: file`: its small repository-relative source
is pinned and confined, then copied into the ordinary fixture directory.
UTF-8/hex fixtures retain their existing meaning. Materialization creates
editable composition/content JSON, textless SVG, ordinary `renderflow/v2` JSON
configurations and a `publication-expectations.json` fixture declaration.
That declaration records desired interior/cover grouping; it is not a new
Renderflow aggregation contract.

The recipe lives in
`crates/renderflow-core/tests/common/golden_comic.rs`, called by the existing
`golden_conformance` test. Its hash and the lockfile/assets are checked before
generation. Changing any pin, story, expectation or recipe needs an explicit
manifest review and corpus version update. Ordinary execution never rewrites
source declarations or approved baselines.

The current generator and schema admit this one bounded comic subset. To add a
different document type, extend this same `packs` definition and register its
recipe in the same harness, rather than copying the harness or using an
unimplemented comic option. Start with one small case and named residual gaps.

## What authored cases inspect

The minimal case uses public frozen composition planning/execution APIs and
checks an ordinary generated build configuration through the shared planner.
Each successful page must have exactly five actual native publication joins:

| Candidate | Independent fixture assertions |
| --- | --- |
| SVG | Measured path and stable layer identity markers |
| HTML | Content security policy, selectable transcript/content IDs and embedded visual structure |
| Transcript JSON | Schema, page/locale/review identity, exact text order/content IDs and artwork description |
| PNG | Bounded PNG decoding, expected canvas pixels, RGBA8 and opaque alpha |
| PDF | Strict bounded parsing, one page/image, expected MediaBox/pixel geometry, RGB space and matching same-settings PNG pixel samples |

Shared file verification checks published size/hash/locator joins; the receipt's
review/fidelity state remains unchanged. A warm-cache rerun checks terminal hits
and byte identity. A separate cold rerun of the first page checks candidate-byte
reproducibility without the shared cache. Source snapshots must remain unchanged.
These finite assertions do not establish visual quality, accessibility standard
conformance, calibrated color, PDF/X, provider acceptance or cross-platform fidelity.

Fourteen named negatives cover stale/unreviewed content, wrong source digest,
missing artwork/font, unsupported schema, page order/count/geometry, missing
accessibility declarations, cancellation before publication, unavailable ORA
with a healthy transcript sibling, and missing/corrupt files after native success.
Content/layout branch codes use production diagnostics. Loader refusals currently
check bounded existing error messages, not invented typed error codes. Order,
count, geometry and alt requirements use **fixture expectation** diagnostics;
they do not claim a new production collection validator. The parent #412 keeps
uncovered provider/version/process/font/script/failure obligations visible.

## Ordered publication candidates

Pack 1.1.0 wires existing public SDK operations after each native page run.
The selected PNG is read through the shared observed-publication reader, which
checks its native target membership, confined locator, size and digest. The
public `publication::raster::prepare_composition_rgb` port validates that exact
RGBA output against its raster plan, refuses non-opaque pixels, and preserves
RGB samples. It requires the explicit policy
`assumed_srgb_samples_to_device_rgb_no_conversion`.

The new PNG is device RGB with no embedded color/density metadata. Its receipt
binds source/plan/output/sample hashes, dimensions, source DPI, provider profile
and declared losses. This is a **partial-fidelity preparation**, not color
conversion, ICC/CMYK proofing or print approval. Original layered/RGBA sources
and transcripts remain separate and unchanged. The
[RGB evidence schema](../schemas/renderflow-publication-rgb-v1.schema.json)
records that boundary.

The native fixed-layout EPUB route receives front cover, ordered interior pages
and back cover. Existing structural inspection plus exact embedded-image hashes
checks order and membership. It does not establish EPUBCheck/retailer acceptance
or a fully accessible text edition: dialogue remains in the companion page
transcripts and each EPUB image has a page description.

Two PDF packets reuse the existing img2pdf 0.6.3 exact ordered-image route:
interior pages and front/back cover pages. The latter uses the native
`interior` target role and is explicitly labelled `cover_pages` in fixture
evidence. It is a two-page packet, not a spine/wrap/spread exporter or a
printer-certified cover. The existing PDF inspector checks page geometry,
order and exact image-stream identities before publication.

PDF execution requires `full`/`maximal` tier **and** explicit
`RENDERFLOW_TEST_IMG2PDF=/path/to/img2pdf-0.6.3`. Fast tier never probes that
provider even when the variable is present. Unselected PDF branches report
`not_attempted`, not success or observed absence. A selected provider failure
fails the fixture. No tool is installed/downloaded automatically. Native EPUB
execution is local; EPUBCheck is not invoked by this recipe.

`publication-plan.json` records grouping and prerequisites. After an actual
minimal run, `publication-results.json`, `rgb-preparation.json` and report
`publication_outputs` distinguish observed candidates from unattempted routes.
The larger case remains page-source planning and publication **recipes only**:
no RGB files, aggregate SDK plan or output is invented. Cover-wrap/PDF-X/CMYK,
provider/platform/runtime and human publication acceptance remain outstanding.

## Deferred execution

After the maintainer starts the testing sweep, the existing harness can retain
its generated **source packages** at a new, explicitly selected directory:

```bash
RENDERFLOW_CONFORMANCE_TIER="fast" \
RENDERFLOW_GOLDEN_PACK_SOURCES="/tmp/lantern-map-sources" \
RENDERFLOW_CONFORMANCE_REPORT="/tmp/lantern-map-report.json" \
cargo test --package renderflow --test golden_conformance --locked
```

This command is an **unrun example**. The source parent must exist and the
destination must be new. The maximal tier additionally materializes/plans the
44+2 case; it can also probe the harness's existing optional providers. Fast
remains Rust-only and makes no external tool probe. Retained sources contain
ordinary per-page build configurations for later explicit CLI use; native test
outputs stay temporary. Do not confuse retained input recipes with observed
output artifacts.

Each case allows at most 256 source files and 2 MiB source bytes; the minimal
case allows 32 MiB primary output-bundle bytes and the larger plan allows zero.
Additional cache/rerun working bytes have a finite aggregate guard. Cooperative
checks run between pages/phases, with 120 seconds per case and the enclosing
corpus deadline. These are authored guards, not measured memory/performance or
an operating-system sandbox. Existing CI/sharding/memory-budget qualification
remains under #412; no workflow change or CI execution is included.

See [#413 handoff](work/413/HANDOFF.md) for remaining acceptance. Source-only
delivery does not close #412 or #413.
