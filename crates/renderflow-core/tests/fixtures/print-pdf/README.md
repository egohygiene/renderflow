# Synthetic print pages

Four 100 × 100 pixel RGB images generated for tests with Pillow 12.3.0. The
red and blue pages are authored test data, with no photographic or publication
assets. PNG files are unprofiled, noninterlaced RGB; JPEG files are baseline
JFIF RGB without EXIF. The print fixture declares a 90 mm square trim with
5 mm bleed, yielding a 100 mm square media/image area. The ordered collection
tests also create a 44-page recipe by copying these two immutable byte patterns
into distinct root-relative source paths.

The optional real-provider test requires `RENDERFLOW_TEST_IMG2PDF` to name an
installed `img2pdf` executable. The default suite exercises preflight and
failure cases with synthetic shims and does not require that external tool.
