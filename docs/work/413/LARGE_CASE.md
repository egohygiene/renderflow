# #413 / #412 — full-size comic source checkpoint

November 6, 2026 (America/New_York). The maintainer authorized three separate
draft checkpoints: the large comic, bounded API uploads, then streamed retrieval
and native schema coverage. This is the first, based on merged #487/main
`ed9de9c4fbf5adc08909529b6d663c39e5fc90d0`. No merge or next-batch release occurs.

The same existing golden harness now has an `opt_in_execute` exercise for the
44-interior/separate-two-cover recipe. Maximal selection alone still plans it;
only maximal plus the exact `RENDERFLOW_TEST_COMIC_LARGE=1` selects the larger
render. `0` or absence preserves planning, and other supplied values refuse the
case. Fast tier cannot enable it. The selection is frozen once per case and
reported separately from the declared exercise. An unselected larger render
does not count as observed native-output coverage.

When later executed, the existing composition SDK renders each of the 46 pages,
then verifies five native publications, exact transcript/content/order,
geometry and same-settings PNG/PDF pixels. The existing RGB preparation port
captures each observed PNG. Complete native/preparation sets are required before
aggregates. The existing fixed-layout EPUB uses front cover, 44 ordered interior
pages and back cover. Existing ordered PDF routes use 44 interior pages and a
separate two-page front/back packet. They still require explicit full/maximal
img2pdf selection. No wrapper renderer, generator, harness or dependency is added.

Aggregate receipts distinguish expected page count from observed count and
recipe-only/not-attempted/observed-candidate status. EPUB asserts native count,
order and exact embedded image identities. PDF adds an independent existing
inspector pass against source-measured page boxes, pixels and ordered image
stream hashes, joined to the observed publication hash. Failure cannot become
a passing provider receipt. Cover pages are not a spine/wrap or printer exporter.

Large source bounds stay at 256 files/2 MiB; primary output is bounded at 128 MiB,
with the existing separate finite working/cache allowance. Cooperative byte/file/
time checks run between page, preparation and aggregate phases. Both case and
enclosing corpus retain the 120-second deadline. A slow or oversize future run
must fail that bound; no observed performance or hard OS memory/quota promise
is invented. Existing source immutability, first-page warm/cold identity and
small-case negatives remain in the same harness. Corpus 1.5.0 / pack 1.2.0 pins
the changed recipe sources; raw fixture/golden output bytes are not rewritten.

Five new cases are authored/unrun: exact large flag/tier; 44/2/46 grouping/order;
plan-only absence of aggregate outputs; missing capture-set refusal; finite
large output budget. This checkpoint ran no compiler/build/Cargo resolution,
tests, lint/format/schema/docs validators, generator/harness, SDK/native/provider/
font/media/model/listener/container execution or CI. No output, passing report,
visual/native/EPUB/printer approval, release, upload or publication is claimed.

#413/#412 remain open for all original observed-output/corpus/qualification
acceptance. #429/#458/#419 retain toolchain/repository/release evidence. #405/#409
gates are unchanged. The following API checkpoints stay in existing #420.
