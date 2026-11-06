# #456 checkpoint 3 — exact review and export

This follows draft #486 (candidates) and #485 (explicit runtime mapping), stacked
after the existing #481–#484 source drafts. All remain drafts for maintainer sync;
no merge, closure, release or model execution is part of this checkpoint.

The reusable `publication::summary::review` module records accept/revise/reject
decisions against the candidate ID **and exact file SHA-256**. Acceptance and
revision require an explicit semantic-review declaration. Revised statements
retain original evidence IDs and the same structural/reference checks. Rejection
records no selected content and cannot export. Review imports the original bytes
as an immutable candidate subject and writes a separate review artifact; it does
not change candidate state or canonical publication metadata. The reviewer
reference is operator-supplied, not authenticated human authority. Scope is only
the accepted synopsis text, with publication approval explicitly false.

Review/export reobserve bounded exact CAS bytes through the existing confined,
no-symlink local file observer. A deserialized Artifact locator cannot grant an
arbitrary file read. Review/content/subject hashes are checked again before
export. Existing output directories are refused. Local payloads are observed
before the completion receipt is written last; failed writes can leave an
unaccepted partial directory. No post, network publisher or overwrite exists.

Export writes accepted plain text, a JSON companion containing citations,
uncertainty/omissions/coverage/processing identities, the review record and a
receipt. Raw source quotes stay in the private candidate review bundle, not the
export. Plain-text losses are declared. The CLI has four thin subcommands:
`ai summary plan`, `generate`, `review`, `export`. Disabled planning has no provider
effects; all prepared-model and source/privacy permission gates remain explicit.

The original synthetic CC0 comic-text example includes a pinned recipe with AI
permission off, human evaluation criteria and a **not_run** reporting template.
Its quality/hardware/resource fields are null, not fake receipts or passing
scores. The guide supplies future commands only. No inference or human acceptance
has been performed. A source-review correction uses the existing native
DigestAlgorithm enum rather than a string comparison in artifact observation.

Ten new meaningful cases are authored/unrun: immutable separate acceptance;
exact-file whitespace/ID binding; revised-content review/citations; rejection;
review/hash tampering; self-approval/unknown fields; export evidence/receipt;
existing-output/cancellation protection; CAS corruption/review-bundle mismatch;
and Unix symlink confinement. Together with the earlier checkpoints there are
29 new cases authored, with no execution result claimed.

No compiler/build/Cargo resolution, tests, product lint/format/schema/docs
validators, repository generators/harnesses, SDK/native/provider/font/media/model/
listener/container execution, install/download, new dependency/lock change or CI
dispatch/polling occurred. Manual source review and Git tree equality are source
transport evidence only. Issue #456 stays open for its original complete criteria,
Flow #80 shared-profile compatibility and measured model/hardware results. Existing
#405/#409 and #429/#458/#419 gates remain unchanged. No new issue is required.
