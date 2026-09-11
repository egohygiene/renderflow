# Lulu publication pack

Renderflow's Lulu pack converts a reviewed provider-neutral publication contract
and local artifacts into deterministic candidate evidence. It is a versioned
adapter, not an uploader: it performs no login, upload, proof order, ISBN
allocation, retailer submission, or publishing action.

## Model

The request pins `lulu-2026-09-11`. Renderflow refuses to silently substitute a
newer rule pack. The report records that pack, its observation date and source
URLs, the publication-spec digest, every declared artifact and template digest,
and `upload_performed: false`.

Eligibility is independent for `print_direct`, `lulu_bookstore`,
`global_distribution`, `pdf_ebook`, `epub_distribution`, `amazon`, `ingram`, and
`barnes_and_noble`. A pass for one channel never implies a pass for another.
Uninspected or missing required evidence produces `unknown` or `ineligible`, not
an optimistic pass.

## Print evidence

Print requests declare the product, page count, trim geometry, interior PDF, and
channel-specific one-piece covers. Bookstore and distribution covers each pair
with a current, user-supplied Lulu template; Renderflow records its digest and
never calculates or guesses a spine. Distribution also requires reviewed proof
and contact-sheet evidence.

The pinned checks cover PDF envelopes and sizes, product exclusions, the Amazon
color-page minimum, 80# paper and hardcover exclusions, spine text below 100
pages, ISBN checksum, metadata lengths, proof approval, and explicit reviewed
attestations for fonts, layers, vectors, resolution, page order, blanks, trim
marks, barcode identity/geometry/clearance, title and copyright ordering, and
metadata consistency.

The synthetic request at
`examples/lulu/44-page-color-amazon-failure.yaml` deliberately demonstrates that
a 44-page color comic is rejected for Amazon while Ingram and Barnes & Noble are
reported independently.

## EPUB evidence

EPUB distribution requires an EPUB 3.3 candidate, an undashed 13-digit ebook
ISBN, a separate flat cover, English-language metadata supported by the pinned
profile, structural inspection, and explicit content attestations. Run with
`--epubcheck` for local EPUBCheck v5 evidence. A missing executable is honestly
reported as `unknown`, which keeps the candidate from upload-ready status.

Accessibility claims use native package metadata as preliminary evidence. They
remain unknown when that evidence is sparse; attach an Ace by DAISY report to
the release evidence before making a reviewed accessibility claim. Fixed-layout
retailer compatibility is also an explicit review decision.

## Portable operation

The rule evaluator, PDF-envelope checks, EPUB structural inspection, digesting,
and report generation are native and offline on Linux, macOS, Windows, and
Termux/proot-style environments. EPUBCheck is optional. Its absence does not
break unrelated print or PDF-ebook channels.

## Adding another provider

Keep provider changes outside `renderflow.publication/v1`. Add a dated rule pack,
a provider request/report schema, a virtual tool capability, and an adapter entry
in `adapter-packs.yaml`. Preserve independent channel decisions, exact source
observations, artifact digests, fail-closed unknowns, and the no-side-effects
boundary.

## Pinned official references

- [Mandatory print distribution requirements](https://help.lulu.com/en/support/solutions/articles/64000255462-mandatory-print-book-distribution-requirements)
- [How to create a print book](https://help.lulu.com/en/support/solutions/articles/64000255486-how-to-create-a-print-book)
- [Global distribution print exclusions](https://help.lulu.com/en/support/solutions/articles/64000267552-global-distribution-print-exclusions)
- [Mandatory ebook distribution requirements](https://help.lulu.com/en/support/solutions/articles/64000255463-mandatory-ebook-distribution-requirements)
- [Publishing an ebook for global distribution](https://help.lulu.com/en/support/solutions/articles/64000255592-publishing-an-ebook-for-global-distribution)
- [Accessible ebooks](https://help.lulu.com/en/support/solutions/articles/64000308545-learn-more-about-accessible-ebooks)
