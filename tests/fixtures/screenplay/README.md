# Original synthetic screenplay fixtures

SPDX-License-Identifier: MIT

These tiny examples were authored for Renderflow #423. All story text is
original Renderflow fixture material under the repository's MIT license.
No upstream screenplay, user manuscript, native application export, font,
or rendered page is included.

| Fixture | Authored purpose |
| --- | --- |
| `minimal.fountain` | Plain UTF-8 screenplay text with a title page, six body elements, and Unicode dialogue. |
| `minimal.fdx` | Minimal Final Draft XML envelope with those six body elements. |
| `minimal.osf` | Minimal OSF-style XML envelope with those six body elements, using the inspected version-20 shape. |
| `minimal.fadein` | ZIP envelope with exactly one entry, `document.xml`, whose bytes are the authored `minimal.osf`. |
| `minimal.screenplay.json` | Separately authored canonical expansion of the same narrative, with explicit character references, dual dialogue, a scene number, a note, and namespaced extensions. |

The XML fixtures deliberately omit application settings, title-page layout,
pagination, revisions, native scene/character identifiers, and embedded
resources. Only the Fountain fixture has a title page. The OSF parenthetical
stores `quietly` as its content, while Fountain and FDX use `(quietly)`; this
matches the inspected source examples, and a future adapter must make its
normalization explicit. These files are not asserted to be equivalent full
documents or to pass a native application's import requirements.

The canonical JSON fixture was written directly against the #423 model. Its
source digest identifies `minimal.fountain` as an associated narrative seed;
the source extension explicitly records `manually_authored_expansion` and
`conversion_performed: false`. The additional ORI dialogue, dual-dialogue
group, note, and scene number are absent from the native-format fixtures.
The digest is not evidence of a verified conversion, a native round trip, or
equivalence to the seed. Its empty `losses` array means no historical conversion
losses were declared, not that an adapter proved lossless conversion.

The external-format fixtures establish an original corpus and recognizable
envelopes for future #424 work. They are not evidence of a parser, converter,
native-app round trip, or successful scripttool invocation. Source provenance,
version differences, and representation limits are recorded in
[FORMAT_SOURCES.md](../../../docs/work/423/FORMAT_SOURCES.md).

## FadeIn fixture recipe

The binary fixture was authored with Python's standard ZIP writer using the
recipe below, run from the repository root. This only packages the original
fixture bytes; it does not execute Renderflow, validate XML, or invoke a
screenwriting application. An uncompressed member avoids compressor-version
variations; the entry timestamp, creator platform, and permissions are fixed.

```python
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_STORED

fixture_root = Path("tests/fixtures/screenplay")
entry = ZipInfo("document.xml", date_time=(1980, 1, 1, 0, 0, 0))
entry.compress_type = ZIP_STORED
entry.create_system = 3
entry.external_attr = 0o100644 << 16
with ZipFile(fixture_root / "minimal.fadein", mode="w", compression=ZIP_STORED) as archive:
    archive.writestr(entry, (fixture_root / "minimal.osf").read_bytes())
```

Tests, schema checks, builds, linting, and native conversion qualification are
deferred for this checkpoint. The fixture recipe is authoring, not validation.
