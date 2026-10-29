"""Write the original geometric PNG used by the synthetic editorial article.

This authoring recipe uses only the Python standard library. Ordinary Renderflow
builds read the checked-in PNG and do not run this script.
"""

from pathlib import Path
import struct
import zlib


def chunk(kind: bytes, payload: bytes) -> bytes:
    """Encode one PNG chunk with its CRC."""
    return struct.pack(">I", len(payload)) + kind + payload + struct.pack(">I", zlib.crc32(kind + payload))


def main() -> None:
    """Create a fixed 480 by 240 RGB illustration beside this recipe."""
    width, height = 480, 240
    rows = bytearray()
    for y in range(height):
        rows.append(0)
        for x in range(width):
            color = (237, 243, 233)
            if y > 180 or 145 < x < 170:
                color = (222, 207, 174)
            if any(left <= x < left + 100 and 95 <= y < 167 for left in (35, 190, 345)):
                color = (51, 110, 84)
            if (x - 390) ** 2 + (y - 45) ** 2 < 24 ** 2:
                color = (226, 174, 66)
            rows.extend(color)
    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(rows), level=9)) + chunk(b"IEND", b"")
    output = Path(__file__).parent / "assets" / "garden.png"
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(png)


if __name__ == "__main__":
    main()
