# Ordered collection fixture

Two tiny HTML fragments in `.md` files exercise ordered, immutable local
artifact sources without depending on a publication provider. `aggregate.py`
is a deterministic test-only collection transform that writes a valid HTML
document. The integration tests copy these files to a temporary root, compute
the exact declared SHA-256 digests, and drive the public SDK and CLI.

`ordered_collection.rs` also generates a 44-member collection from a small
text template. It does not commit 44 binaries. This checks order and plan
identity at realistic page counts before PDF and EPUB exporters exist.

Run with:

```bash
cargo test --package renderflow --test ordered_collection --locked
cargo test --package renderflow-cli --test ordered_collection_cli --locked
```
