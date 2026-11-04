# #413 — ordered publication source checkpoint

November 4, 2026 (America/New_York). The maintainer authorized continuing the
oldest local feature gaps after API drafts #481/#482. This checkpoint is stacked
on #482; it does not merge those drafts or claim their runtime qualification.

## Source changes

- Public `publication::raster::prepare_composition_rgb` reuses the native PNG
  contract validator and existing PNG dependency. Input digest/plan, opaque
  alpha, dimensions/bytes and cooperative cancellation are checked. Explicit
  `RGB_POLICY` acknowledges device-color metadata loss. RGB sample values are
  retained; sRGB/density metadata and opaque alpha format are removed. Receipt
  schema records source/plan/output/sample identities, partial fidelity and
  review-required/unqualified state. No arbitrary color conversion is claimed.
- Existing comic harness now wires observed native page publications through
  that port into ordinary public SDK configs. Native fixed-layout EPUB uses
  front cover, interior order and back cover, with independent structural and
  embedded-image hash/order assertions. Dialogue remains in separate page
  transcripts; no fully accessible textual EPUB edition is claimed.
- Interior and two-cover PDF packets reuse img2pdf 0.6.3 and its existing
  independent image-stream/page inspector. They require a full/maximal tier and
  explicit existing `RENDERFLOW_TEST_IMG2PDF`; fast tier never probes external
  providers. Selected failures fail the case; unselected routes are labelled
  `not_attempted`. The two-cover packet uses the native `interior` role, with
  a separate fixture `cover_pages` grouping. No cover-wrap/spine provider,
  PDF/X/CMYK/ICC/retailer/printer acceptance or inherited approval is invented.
- Corpus 1.4.0 / pack 1.1.0 explicitly changes sampling from 72 to 127 DPI so
  the 100×140 mm canvas is exactly 500×700 pixels. Existing strict aspect checks
  are retained; no crop/stretch or hidden geometry adjustment is introduced.
  Recipe support source/normalizer pins and report/schema fields are additive.
  Actual outputs/baselines were not generated or rewritten.
- The maximal 44+2 case remains source/page planning and aggregate **recipes
  only**. It reports zero rendered/normalized publications and makes no claim
  to an aggregate SDK plan before raster inputs exist. The story still repeats
  two beats as finite stress data.

## Review and evidence

Six focused RGB regressions are authored/unrun: exact samples/preflight, alpha
refusal, required identity/policy, invalid plan/corrupt input, pre-cancel and
determinism/source immutability. Existing comic cases additionally author EPUB
membership/order and optional PDF assertions in the same harness. No duplicate
fixture framework, publication renderer or new dependency is added.

Source review covers public SDK/config types, strict aspect policy, input and
publication joins, native output validation, provider allowlists, fast-tier
effect boundaries, report fields, metadata loss and private mutable workspace
ownership. It is not compiler, test, schema or native evidence.

No tests, build/compiler/Cargo resolution, lint/format, schema/docs/OpenAPI
validator, corpus generator/harness, SDK/native/provider/font/media/model/
listener/container execution or CI ran. No rendered comic/EPUB/PDF, passing
report, approval, provider installation, release or consumer integration is
claimed. All source paths and receipts remain reviewable for the later sweep.

#413/#412 remain open for original pack/corpus/output acceptance; #429/#458/
#419 retain toolchain/repository/release qualification. #405/#409 gates remain
unchanged. No new issue is needed for this already-tracked scope.
