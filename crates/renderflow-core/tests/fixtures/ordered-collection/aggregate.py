"""Synthetic deterministic collection transform; no publication exporter."""

from pathlib import Path
import sys


def main() -> None:
    output = Path(sys.argv[1])
    members = [Path(value) for value in sys.argv[2:]]
    if not members:
        raise SystemExit("at least one member required")
    output.write_bytes(
        b"<!doctype html>\n<html><body>\n"
        + b"\n".join(member.read_bytes() for member in members)
        + b"\n</body></html>\n"
    )


if __name__ == "__main__":
    main()
