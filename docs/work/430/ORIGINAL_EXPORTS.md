# #430 — original screenplay and transcript terminal exports

Implementation checkpoint on `feat/renderflow-430-original-exports`, based on
`0427749a2cccececae2d69aa9fa0a02779603509`. This design precedes the source edits.
It continues #430 with the existing #424 screenplay and #427 transcript owners.
The sprint's execution deferral in [HANDOFF.md](HANDOFF.md) remains in force.

## Intent and boundary

An original Fountain artifact may request independent canonical screenplay JSON,
FDX, Fade In, OSF and Trelby artifacts. `execution.screenplay_body` is an explicit
opt-in policy with required `allow_unqualified: true`. No executable, network,
recovery heuristic or sibling repository is involved. Each edge consumes the
immutable original Fountain directly; it cannot turn an imported or recovered
derivative into an approved original. PDF keeps its existing separate policy.

Reuse Renderflow's independently authored native body writers and readers, not
Scripttool's rejected reverse-export directions. Extract the shared codec below
the recovery wrapper; preserve recovery's verified projection, losses and review
requirements. Do not strip arbitrary metadata to make an original export fit.
The body profile admits scene heading, action, character, dialogue,
parenthetical and transition elements. Unsupported structure, titles, dual
dialogue, unknown extensions, losses and unrepresentable text fail closed.
Canonical JSON preserves the parsed bounded semantic model, not native layout.

## Trelby

Trelby (not “Trilby”) uses `.trelby`, a native coded text format distinct from
Fountain. Independently implement only the version 3 minimal body envelope from
the published upstream format at release 2.4.16.2, commit
`86079b162e6514754ce0a2f3ea323eaca81146e7`:

- UTF-8 BOM, `#Version 3` and `#Start-Script ` header lines.
- Complete, single-line records with final-line markers and the six body kinds.
- Literal enclosing parentheses around native parenthetical payloads.
- Renderflow's existing byte, element and text bounds; reject controls and
  Unicode line separators which upstream Python `splitlines` would split.

Upstream writes no `#End-Script` marker. Arbitrary GUI configuration, wrapping,
title pages and unsupported kinds are outside this reader's bounded profile.
No upstream implementation is copied or vendored. Upstream source inspection
does not qualify GUI opening, Unicode rendering/editing, pagination or layout.
All native exports keep `runtime_qualified: false` until observed qualification.

Primary format reference:
<https://github.com/trelby/trelby/blob/86079b162e6514754ce0a2f3ea323eaca81146e7/trelby/fileformat.txt>.

## Artifact and trust contract

Exports retain the exact original source hash, a retained canonical artifact
reference and hash, output hash, body profile and unqualified status. Publication
must re-read bounded payloads and reconstruct the expected result from the
original, independently of the generic validator switch. Reject changed bytes,
record identity, lineage, metadata, policy or canonical references. Disable
native export cache reuse until qualified. Conversion does not grant approval.
The auxiliary canonical record participates in inventory and storage/artifact
budgets. It remains in the local artifact store unless explicitly selected as a
`screenplay_json` target; native-only bundles do not promise portable companion
materialization. Preserve that store, or select the canonical target and retain
the original source and run evidence when moving outputs.

## Transcript text terminal

Register canonical timed transcript → UTF-8 text under the existing subtitle
policy, including explicit speech descendants. Preserve the canonical reference,
source lineage, language/provenance evidence and export losses. Text deliberately
drops timing, cue identifiers and structured metadata; use partial fidelity and
lower preservation in planning. Keep text terminal: no reverse edge, inferred
timing, implicit recognition, or parsing generic text as timed JSON. Strict speech
retention and publication evidence must accept this terminal without implying
original-video alignment.

## Qualification checkpoint

Author regression cases for body semantics and unsupported input, Trelby framing
and Unicode boundaries, immutable source/evidence tampering, policy isolation,
terminal planning and transcript loss preservation. Source review and the pure
capability documentation generator are permitted. Tests, compilation, formatting,
schema validation, provider execution and CI remain unexecuted. Record that status
in the draft PR and handoff; #430 remains open.
