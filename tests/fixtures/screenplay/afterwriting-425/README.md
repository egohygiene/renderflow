# Afterwriting checkpoint fixtures

`strict.fountain` is an original, synthetic, printable ASCII screenplay body.
It is not an upstream afterwriting example, a native rendering result, or a
qualification fixture obtained from a screenplay application. It intentionally
uses only the independently parsed body subset adopted in #424 and #425.

`expected-analysis.json` is the manually authored Renderflow core analysis for
that source. Its source digest and byte length identify the exact fixture. The
report is not afterwriting's internal statistics or CLI output. No afterwriting
run was used to create it.

Expected semantic sequence, using zero-based element indices:

| Index | Kind | Text |
| --- | --- | --- |
| 0 | Scene heading | INT. STUDIO - DAY |
| 1 | Action | A lamp glows. |
| 2 | Character | ADA |
| 3 | Parenthetical | quietly |
| 4 | Dialogue | Keep the light on. |
| 5 | Character | BOB |
| 6 | Dialogue | I can see it. |
| 7 | Transition | CUT TO: |
| 8 | Scene heading | EXT. ROOF - NIGHT |
| 9 | Action | Rain crosses the empty roof. |
| 10 | Character | ADA |
| 11 | Dialogue | Wait here, please. |
| 12 | Scene heading | INT. STUDIO - DAY |
| 13 | Action | The lamp remains on. |
| 14 | Character | ADA |
| 15 | Dialogue | We are home. |
| 16 | Transition | FADE OUT. |

The three scene headings remain three occurrences; this report does not infer
physical locations. Exact cue `ADA` occurs three times and owns three dialogue
elements totaling ten ASCII whitespace-delimited tokens. Exact cue `BOB` occurs
once and owns one dialogue element totaling four such tokens. Action contributes
3 + 5 + 4 = 12 tokens. Parenthetical text does not contribute to dialogue tokens.

No tests, schema validators, compiler, afterwriting process, or PDF renderer were
run while authoring these fixtures. Native PDF acceptance, visual inspection,
reproducibility, and runtime qualification remain deferred.
