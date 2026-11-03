# Presentations from reviewed content

The `presentation-slidev-v1` standalone publication profile consumes
`renderflow.presentation/v1` JSON. The public `presentation::plan` and
`presentation::build` functions and `renderflow presentation` commands share the
same implementation. The content model is independent of Slidev. The current
adapter targets a prepared **@slidev/cli 53.0.0** installation; it never invokes
npm, npx, a package installer, a theme installer, an AI model, or a remote font
service during publication.

This is authored, unqualified source. The interface is not yet admitted to the
general `renderflow/v2` derivative forest. Ordinary graph planning is unchanged.
Keep that distinction when integrating a consumer; a standalone result is not a
released Flow provider contract.

## Author a deck

Start from the [synthetic example](../../examples/presentation/README.md). A deck
contains an explicit ID/title/language, original-source repository/revision/path
and digest, author-supplied freshness label, rights statement, declared review
state, theme, transition policy and ordered slides. Review/rights/source claims
are retained declarations. Renderflow does not fetch the source repository,
verify legal rights, infer freshness or grant approval.

Slides have stable IDs, titles, optional section labels, literal-text paragraphs
and bullets, citations, local images and optional speaker notes. The initial
themes `paper` and `night` are original bundled CSS; their bytes are hashed.
The implemented geometry is 16:9 with no transition effects. Arbitrary raw
Markdown, Vue, JavaScript, videos, embedded web pages, remote images, animations
and user themes are outside this bounded first model.

Literal content is escaped into generated HTML inside Vue `v-pre` sections.
Supplied text does not become template expressions or components. Citations
retain explicit HTTP(S) links in slide content and the readable companion.
Images require source-relative local PNG/JPEG paths, exact digests, alt text and
rights descriptions; captions are optional. Source/asset bytes are captured and
bound during planning/build. The reusable source package rewrites image paths
to its captured media and retains original-source lineage.

Speaker notes are **excluded from exported source by default**. Select
`--include-speaker-notes` only when they belong in the downloadable source.
Interactive web output always requests Slidev's `--without-notes`. A model
inspection prints the original supplied model, including notes, to stdout;
inspect it privately. This is distinct from the public packaging policy.

## Inspect, plan, build

```bash
renderflow presentation inspect --input "deck.presentation.json"
renderflow presentation plan --input "deck.presentation.json" --output-dir "release" --target source,web,pdf --provider "provider.local.json"
renderflow presentation build --input "deck.presentation.json" --output-dir "release" --target source,web,pdf --provider "provider.local.json" --allow-unqualified
```

Targets are explicit, unique and ordered. The default `source` target needs no
provider or unqualified opt-in. `plan` reads the model/assets and configuration
without launching providers or creating outputs; configured pins are not
observed tool availability. Native web/PDF builds require the explicit opt-in
while qualification remains outstanding. All commands emit JSON on stdout.

Outputs include a deterministic source ZIP when selected, a static web ZIP when
selected, a PDF when selected, a readable HTML transcript and `bundle.json`.
The transcript supplies a linear heading order, source/freshness metadata,
descriptive images, citations and literal text. It complements the interactive
viewer; it is not a certified accessibility result. No upload or publication
occurs. Output paths and hashes in the manifest describe actual packaged files.

## Prepare local tools separately

Supply a private `renderflow.slidev-provider/v1` configuration based on the
[template](../../examples/presentation/provider.template.json):

- An absolute regular-file Node executable, SHA-256 and exact version line.
  Slidev 53 requires Node >=22.12.0; the configuration pins the chosen version.
- An absolute npm-style project root containing
  `node_modules/@slidev/cli` 53.0.0 and all prepared dependencies. The profile
  observes its bounded regular-file tree before and after execution using the
  existing package-fingerprint service. Symlinked pnpm-style trees are outside
  this initial snapshot contract. Prepare any dependency lockfile in that root
  before computing its identity.
- For PDF, an explicitly installed browser executable, SHA-256 and exact version
  line. Its shared libraries/runtime closure remain independently unqualified.
- A 1–1800-second timeout per rendering command. Version probes use five
  seconds. These are phase limits, not an aggregate hard deadline or disk/RSS
  quota.

```bash
renderflow presentation fingerprint --installation-root "/absolute/prepared/project"
```

The installation-tree algorithm is
`renderflow.package-files-json-sha256/v1`: sorted relative file name, size and
digest records; root `.git` and `node_modules/.bin` are excluded. Limits are
512 MiB total, 64 MiB per file, 40,000 entries, depth 32 and the existing
120-second cooperative file budget. A hash identifies supplied bytes, not a
trusted package publisher or proof that all dependencies are present.

Generated projects select no third-party Slidev theme, disable remote font
providers and use system font fallbacks. The staged project links the prepared
`node_modules`; Vite's cache is redirected to a temporary project-local folder.
That link requires supported filesystem permissions, including Windows symlink
privileges where applicable. There is no claimed qualified platform yet.

The adapter runs exact argv through the shared process service with cancellation,
timeouts and bounded diagnostics. Network denial is requested but is not an OS
sandbox. PDF rendering starts Slidev's temporary loopback server for the pinned
browser. Run qualified publication in a host that permits that local connection
while denying external traffic. No source hash can establish that a trusted
installed program makes no network calls; observe this during qualification.

## Inspect the resulting artifacts

Web publication requires an observed HTML index and a bounded, regular-file,
symlink-free output tree; it is packaged with the readable companion. Its
relative asset base and hash router are intended for static hosting, including
a product subdirectory. Browser routing, keyboard controls and site embedding
still need actual native/browser qualification.

PDF is independently parsed with the existing PDF library. It must have one
page per source slide and finite positive landscape 16:9 MediaBoxes. These checks
do not prove clipping-free layout, selectable text, correct fonts, link fidelity,
PDF/UA, or print acceptance. Review the result visually and inspect citations.
The original source/transcript remain the reading/editing alternatives.

Input is bounded to 4 MiB, 300 slides, 64 paragraph/list blocks per slide and
128 image references. Images are at most 16 MiB each, 64 MiB in aggregate and
100 million declared pixels. Packaged outputs have a 256 MiB aggregate bound;
web trees have 4,096 files and depth 24. Native running scratch space and child
RSS are not hard bounded by those final-output checks. Planning and building
are separate fresh captures; the API does not execute a previously serialized
plan or certify a hostile concurrently modified filesystem.

Publication writes a new destination and completion receipt last. Failure or
cancellation never overwrites an existing release; a publication I/O failure
can leave partial files without `bundle.json`. Treat those as unaccepted. Keep
prior accepted bundles for rollback. Deployment is a separate human-selected
host action: upload/extract a reviewed web ZIP into a new versioned directory,
retain the transcript, switch its site link, and restore the prior link on
rollback. No adapter deploys or mutates a remote site.

## Evidence and remaining acceptance

The model, adapter, schema, two deck examples and regression cases are authored.
The product-source example is a **review draft**, not a fabricated human-reviewed
product asset. No builds/tests, schema/docs validation, source packaging, native
Slidev/browser/PDF execution or CI ran. Exact API/CLI parity, deterministic
assembly, two-run native behavior, offline behavior, citations, accessible
reading order/contrast, reduced-motion handling, layout and platform behavior
remain under #350, with the baseline/release gates under #458/#419.

Adapter arguments and no-theme/font configuration were reviewed against
[Slidev v53.0.0 CLI source](https://github.com/slidevjs/slidev/blob/v53.0.0/packages/slidev/node/cli.ts),
[option resolution](https://github.com/slidevjs/slidev/blob/v53.0.0/packages/slidev/node/options.ts)
and [parser configuration](https://github.com/slidevjs/slidev/blob/v53.0.0/packages/parser/src/config.ts).
Source inspection is not provider execution evidence.
