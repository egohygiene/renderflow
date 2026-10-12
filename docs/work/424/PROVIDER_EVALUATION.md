# Scripttool provider evaluation — Renderflow #424

Source review date: 2026-10-12. This record evaluates the optional external
`tool.scripttool` provider. It is not a native qualification report. No executable,
installer, converter, test, compiler, schema validator, formatter, linter, or CI
job was run for this checkpoint. No upstream binary or sample screenplay was
downloaded into the repository.

## Reviewed versions

| Component | Reviewed identity | Evidence |
| --- | --- | --- |
| scripttool current default-branch source | `ea8ed24020b6f6989e01df44eef64febdfa7bfb2`, committed 2025-08-09 | [Commit](https://github.com/rsdoiel/scripttool/commit/ea8ed24020b6f6989e01df44eef64febdfa7bfb2) |
| Latest published scripttool release observed | `v0.0.10`, published 2024-05-20; tag resolves to `70c880b18f1f209529e2431fa1c5c7fd1c2c316d` | [Release](https://github.com/rsdoiel/scripttool/releases/tag/v0.0.10), [tag reference](https://api.github.com/repos/rsdoiel/scripttool/git/ref/tags/v0.0.10) |
| FDX dependency | `v1.0.2`, `c76771533429bc857f3abd97e6cbcbd537eccab5` | [Dependency source](https://github.com/rsdoiel/fdx/tree/c76771533429bc857f3abd97e6cbcbd537eccab5) |
| Fountain dependency | `v1.0.1`, `c963739bb3e4c3a2e07b1ea60638d0817c6ca5b7` | [Dependency source](https://github.com/rsdoiel/fountain/tree/c963739bb3e4c3a2e07b1ea60638d0817c6ca5b7) |
| OSF dependency | `v0.0.7`, `2d5b6c66662669a644885b86d472a3427cfd8910` | [Dependency source](https://github.com/rsdoiel/osf/tree/2d5b6c66662669a644885b86d472a3427cfd8910) |

The reviewed [go.mod](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/go.mod)
declares Go 1.22.0 and the three dependency versions above. The release tag has
the same dependency declarations. Current source and the older release both
report version `0.0.10`; matching that version alone does not establish the
reviewed commit, build inputs, executable bytes, or behavior.

The reviewed [version constants](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/version.go)
also contain a release date of `2025-08-10` and abbreviated hash `b59480b`.
The actual `-version` branch in
[main.go](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/cmd/scripttool/main.go)
prints only the invoked executable basename, one space, `0.0.10`, and a newline.
Neither the date nor the hash appears in this version response. With the usual
executable name, the expected response is `scripttool 0.0.10`.

## Command contract

The authoritative dispatch is
[cli.go](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/cli.go),
with conversion implementations in
[scripttool.go](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/scripttool.go)
and wrappers in
[convertions.go](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/convertions.go).
The upstream documentation also lists these routes, but declaration of a verb
does not establish faithful conversion.

| Requested direction | Exact upstream verb | Source assessment |
| --- | --- | --- |
| FDX → Fountain | `fdx2fountain` | Authored, unqualified import candidate. Explicit opt-in and executable pin required; strict independent comparison of a body-only subset. |
| Fountain → FDX | `fountain2fdx` | Execution rejected. Adoption declined because of the incomplete, unqualified native root envelope described below. |
| OSF → Fountain | `osf2fountain` | Authored, unqualified import candidate with the same opt-in/pin requirements. Accept only explicit paragraph styles and independently compared body content. |
| Fountain → OSF | `fountain2osf` | Execution rejected: semantic paragraph kinds are written as bookmarks rather than styles. |
| Fade In → Fountain | `fadein2fountain` | Authored, unqualified import candidate with the same opt-in/pin and OSF restrictions, plus strict archive validation. |
| Fountain → Fade In | `fountain2fadein` | Execution rejected: packaging as a ZIP does not repair the OSF semantic-style defect. |
| Fountain → provider JSON | `fountain2json` | Execution rejected in this checkpoint. Distinct inspection/derivative candidate requiring its own validation and loss contract; it is not canonical screenplay JSON. |

The source-backed argument forms are:

```text
scripttool -version
scripttool VERB -i INPUT_PATH -o OUTPUT_PATH
```

`-i` and `-o` are the actual named input/output options; the reviewed source has
no `--input` or `--output` aliases. Positional input/output paths also exist.
Adapter code must use an argument vector, a staged input, and a fresh staged
output, rather than build a shell command or supply the user's original path.
The CLI opens an output with truncation before dispatching the verb. Fade In
import also reopens its input as a ZIP, so it requires an actual filename.

## Findings that constrain the adapter

### Native output identity and semantic paragraph styles

The Fountain-to-FDX wrapper creates a zero-valued `FinalDraft`, then calls
`FromFountain`; it does not call `NewFinalDraft`. The constructor supplies the
document type and version, but `FromFountain` does not. The resulting source path
therefore emits an FDX root with an omitted `DocumentType` and empty `Template`
and `Version` attributes. This is an incomplete, unqualified native envelope,
not proof that a particular vendor application rejects the output: no vendor
application was run. Renderflow declines this export route rather than guessing
defaults or rewriting the provider's output into a different native document.

See [FDX conversion](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/convertions.go),
[FDX model, constructor, and serializer](https://github.com/rsdoiel/fdx/blob/c76771533429bc857f3abd97e6cbcbd537eccab5/fdx.go),
and [FromFountain](https://github.com/rsdoiel/fdx/blob/c76771533429bc857f3abd97e6cbcbd537eccab5/fromfountain.go).

The OSF `FromFountain` implementation assigns each element's type name to the
paragraph bookmark. It does not create a paragraph style. The inverse OSF
formatter uses `style.basestylename` to distinguish action, character, dialogue,
and other paragraph types, and does not use the bookmark. The scripttool wrapper
also leaves the OSF document type empty. The Fade In writer packages this same
OSF XML as `document.xml`; the package does not restore missing semantics.

See [OSF FromFountain](https://github.com/rsdoiel/osf/blob/2d5b6c66662669a644885b86d472a3427cfd8910/fromfountain.go)
and [OSF model and formatter](https://github.com/rsdoiel/osf/blob/2d5b6c66662669a644885b86d472a3427cfd8910/osf.go).
These are source findings, not observed native failures.

### Import fidelity

FDX and OSF formatters flatten title-page paragraphs into unfielded text. OSF
`String()` does not render the separate `info` metadata structure. Layout,
revision, font, and unknown XML data are also not fully represented in the
Fountain result. A successful process exit is therefore insufficient evidence
of a faithful import. Source validation must reject unsupported structures rather
than silently remove them, and independent output parsing must compare the
accepted ordered semantic elements.

FDX scene-heading, character, and transition text is uppercased. Both native
formatters may add parentheses to parenthetical text. Accepting such a
transformation without recording it would misstate text preservation; strict
comparison should reject any changed accepted content. Multiple text fragments
may also acquire new line breaks, so this checkpoint's accepted subset must be
narrower than the set of XML documents upstream can parse.

### Fountain parsing

The pinned [Fountain parser](https://github.com/rsdoiel/fountain/blob/c963739bb3e4c3a2e07b1ea60638d0817c6ca5b7/fountain.go)
has several source-visible limitations:

- It begins in title-page mode and leaves it using scene/transition heuristics;
  a blank line alone does not provide a general title-page boundary.
- Action classification precedes transition classification, and its exclusion
  logic does not exclude transitions. Familiar transition lines can therefore
  be classified as action depending on context.
- Dialogue classification recognizes only a preceding character or
  parenthetical; a second dialogue line may become action.
- Forced element markers remain in element content in the conversion path.
- Scene recognition includes broad prefix and punctuation heuristics. A line
  resembling a scene heading can be classified as one without being an
  intentionally authored heading.

A conservative adapter should accept ordinary, independently understood
single-line scene/action/character/parenthetical/dialogue elements and reject
ambiguous or unsupported syntax. It must not equate the provider's parse with
the canonical model merely because JSON was produced.

The separate provider-JSON candidate remains rejected for execution in this
checkpoint. Its JSON root uses `TitlePage` and `Elements`. Elements contain numeric
`type`, optional `name`, and `content`; relevant type numbers are 1 for empty,
2 for title page, 3 for scene heading, 4 for action, 5 for character, 6 for
dialogue, 7 for parenthetical, and 8 for transition. Empty structural elements
may be ignored only when their content is actually whitespace. Unknown types
and contradictory names/content need explicit rejection. Provider JSON cannot
be relabeled as canonical screenplay JSON. These shape observations support a
future evaluation; they do not advertise a usable JSON conversion capability.

### Fade In archive handling

Upstream opens the archive and reads the first exact root entry named
`document.xml`. It does not establish a bounded, single-document archive
contract before that read. Renderflow must reject ambiguous or oversized
archives before provider execution and must not extract arbitrary archive
members into the output directory. Import does not imply preservation of
additional native archive content.

## Installation, distribution, and license evidence

The observed v0.0.10 release lists ZIP assets for Linux x86_64, aarch64, and
armv7l; macOS x86_64 and arm64; Windows x86_64 and arm64; and a separately named
Raspberry Pi OS ARM7 asset. The GitHub release response has no digest value for
these assets. This is a published-asset inventory, not verification of their
bytes, compatibility, signing, or correspondence to the reviewed HEAD.

Upstream [INSTALL.md](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/INSTALL.md)
describes experimental remote installers and a source-build path.
[Makefile](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/Makefile)
also invokes documentation/version-generation tools, including Pandoc and
`cmt`; it is not merely an isolated Go compile. This checkpoint does not run
those installers, add an automatic download, alter a devcontainer, or assert
that an installation procedure has been qualified. A later qualification must
record the acquired source/build or release artifact, executable hash, platform,
version response, and original fixture results together.

The repository's
[root LICENSE](https://github.com/rsdoiel/scripttool/blob/ea8ed24020b6f6989e01df44eef64febdfa7bfb2/LICENSE)
contains AGPL version 3 text, and the v0.0.10 release describes a change to AGPL
version 3 or later. Several reviewed source headers, including `main.go`,
`scripttool.go`, and `convertions.go`, still contain BSD 2-Clause notices. This
records the source discrepancy without resolving the licensing of a particular
build or distribution. Renderflow uses an optional external executable and does
not vendor/link scripttool code or redistribute its binaries in this checkpoint.
That process boundary is an implementation fact, not a legal compatibility
guarantee. Executable distribution review remains separate from user screenplay
rights and any future font or rendering dependencies.

## Qualification still required

The source-backed command contract and authored guards must later be exercised
against an identified provider binary on each claimed platform. Record positive
fixtures, semantic-difference failures, version/output bounds, malformed XML,
archive rejection, and cleanup on failure. Verify that unavailable or rejected
routes remain unavailable to callers and that no original input is overwritten.
Keep #424 open until the issue's acceptance conditions are satisfied or its
remaining upstream limitations are explicitly resolved.
