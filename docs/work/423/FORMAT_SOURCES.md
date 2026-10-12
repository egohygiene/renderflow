# Renderflow #423 — screenplay format source record

Source inspection date: 2026-10-12. This record supports the format identities,
canonical-model boundaries, and original fixtures for #423. It does not qualify
an external converter. Native adapters remain separate work in #424.

## Primary sources and exact pins

| Source | Pin / reference | What was inspected |
| --- | --- | --- |
| Fountain syntax | [Official syntax page](https://fountain.io/syntax/), observed 2026-10-12; the page provides no immutable revision identifier | Title-page fields, scene headings, action, characters, parentheticals, dialogue, transitions, notes, dual dialogue, and formatting limits. |
| Fountain author implementation | [`nyousefi/Fountain` at `c09b65e9d4bf6cb1c672501f793b9b3f2becd452`](https://github.com/nyousefi/Fountain/blob/c09b65e9d4bf6cb1c672501f793b9b3f2becd452/README.markdown) | The format authors' model description and explicit absence of production revision/page-lock features from Fountain. No source code is copied. |
| OSF SDK | [`OpenScreenplayFormat/osf-sdk` at `72758e2e1276c34f8fb7383e2edfd8ba2d129872`](https://github.com/OpenScreenplayFormat/osf-sdk/tree/72758e2e1276c34f8fb7383e2edfd8ba2d129872) | [`v2/OSF-2.0.xml`](https://github.com/OpenScreenplayFormat/osf-sdk/blob/72758e2e1276c34f8fb7383e2edfd8ba2d129872/v2/OSF-2.0.xml) and [`v4/OSF-4.0.xml`](https://github.com/OpenScreenplayFormat/osf-sdk/blob/72758e2e1276c34f8fb7383e2edfd8ba2d129872/v4/OSF-4.0.xml): XML envelopes, paragraph/style/text structure, and version differences. |
| Candidate scripttool dependency set | [`rsdoiel/scripttool` at `ea8ed24020b6f6989e01df44eef64febdfa7bfb2`](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/go.mod) | Pins `fdx v1.0.2`, `fountain v1.0.1`, and `osf v0.0.7`. This is source provenance, not a tested installation. |
| FDX candidate implementation | [`rsdoiel/fdx` v1.0.2, commit `c76771533429bc857f3abd97e6cbcbd537eccab5`](https://github.com/rsdoiel/fdx/blob/c76771533429bc857f3abd97e6cbcbd537eccab5/fdx.go) | `FinalDraft`, `Content`, `Paragraph`, and `Text` structures; paragraph type names and text runs. This is the candidate implementation's source, not a complete Final Draft vendor specification. |
| FDX envelope example | [`scripttool/testdata/testplay-01a.fdx`](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/testdata/testplay-01a.fdx) | Root attributes and paragraph nesting only. The example's story text is not reused. |
| OSF/FadeIn candidate implementation | [`rsdoiel/osf` v0.0.7, commit `2d5b6c66662669a644885b86d472a3427cfd8910`](https://github.com/rsdoiel/osf/blob/2d5b6c66662669a644885b86d472a3427cfd8910/osf.go) | OSF 1.2/2.0 structures and `ReadFile`: a `.fadein` input is opened as ZIP and its exact `document.xml` member is read before XML parsing. |
| OSF older envelope examples | [`scripttool/testdata/OSF-2.0.xml`](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/testdata/OSF-2.0.xml) and [`sample-01.osf`](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/testdata/sample-01.osf) | Version `20` with `basestylename`; the sample also shows structural parentheticals without literal parentheses. Fixture text is independently authored. |

## Envelopes and limits

Fountain is plain screenplay text with optional authoring markup. The original
fixture uses only a title page and common body elements. Recognizing its suffix
does not establish which Fountain features a future adapter preserves.

FDX is represented here by an XML `FinalDraft` root with a `Content` element,
typed `Paragraph` children, and `Text` runs. The fixture is intentionally smaller
than a native application export. A paragraph type and its readable text do not
preserve every page-layout, revision, script-note, or application-specific field.

OSF uses a `document` root, `paragraphs/para`, paragraph `style`, and `text` runs
in the inspected examples. Version strings cannot be inferred solely from a
directory or filename: the pinned SDK file named `v2/OSF-2.0.xml` actually has
root `version="30"`; the scripttool copy has `version="20"`. The v4 SDK example
has `version="40"` and uses `basestyle` where older examples use
`basestylename`. The authored fixture deliberately follows the older
version-20 shape. Detection must not imply that a candidate parser handles all
OSF versions.

FadeIn has a separate ZIP envelope in the inspected candidate implementation.
The fixture wraps the same original OSF bytes in one `document.xml` member.
Neither arbitrary ZIP magic nor a `.fadein` suffix validates an application
document. Future input adapters need explicit archive bounds and member policy;
this checkpoint performs no archive extraction or conversion of user input.
Renaming raw XML to `.fadein` would not create the envelope this candidate reads.

No native-format-specific registered media type was established from these
sources. Any Renderflow-owned media-type identifier is an internal catalog
identity, not a claim of a vendor or IANA registration. XML/ZIP transport alone
is insufficient to choose a screenplay format.

## Preservation boundaries for the next adapter checkpoint

The canonical model can describe normalized screenplay structure without
promising byte-for-byte native round trips. Title fields, ordered body elements,
dialogue grouping, scene references, text marks, notes, and source/loss metadata
must retain their meaning explicitly. A source-specific field that is not
represented must be reported as unsupported or lost when importing/exporting;
successful deserialization is not proof of preservation.

Examples requiring separate qualification include native revision histories,
page and scene locks, layout measurements, custom element styles, embedded
resources, application metadata, note attachment positions, and dual-dialogue
layout. Fountain's own authors explicitly describe production revision and
page-lock features as outside its format. Canonical dual-dialogue grouping does
not itself reproduce a native application's columns or pagination.

The candidate OSF converter also has known source-level gaps:
[`fromfountain.go`](https://github.com/rsdoiel/osf/blob/2d5b6c66662669a644885b86d472a3427cfd8910/fromfountain.go)
leaves text-style splitting as a TODO and places imported element names into
`bookmark` fields. Its output must be inspected and qualified in #424; its
presence cannot be used to claim faithful semantic conversion today.

## Fixture provenance and licensing

All story content in `tests/fixtures/screenplay/` was newly authored for this
repository and uses the repository's MIT license. Format tokens and minimal
envelopes are drawn from the inspected sources; no upstream screenplay corpus,
implementation code, fonts, model data, or native binary export was vendored.
The synthetic `.fadein` ZIP is produced only from the original OSF fixture;
its exact authoring recipe is in the fixture README.

The source references do not license an external executable or application for
distribution. Candidate dependency licensing remains a separate concern from
fixture-content licensing; see the provider source record on the separate
[#429 draft PR branch](https://github.com/egohygiene/renderflow/blob/feat/renderflow-429-creative-toolchain/docs/work/429/PROVIDER_SOURCES.md).
That draft is not a dependency of this #423 checkpoint. No provider, native
application, model, or font was installed or executed for this checkpoint.

All fixture parsing, schema validation, builds, tests, native-app opening,
conversion round trips, formatting/linting, and CI remain deferred. These are
authored corpus and source-inspection records, not successful qualification
results.
