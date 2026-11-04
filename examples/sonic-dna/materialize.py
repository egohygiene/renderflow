#!/usr/bin/env python3
"""Explicit source preparation: write original 100 ms PCM silence, never inference.

Authored/unrun with #397. Refuses overwrite; no provider/model/media command.
"""
import argparse
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, help="New WAV path; parent must exist")
    arguments = parser.parse_args()
    path = Path(arguments.output)
    data = 1600 * 2
    wave = (b"RIFF" + (36 + data).to_bytes(4, "little") + b"WAVEfmt "
            + (16).to_bytes(4, "little") + (1).to_bytes(2, "little") + (1).to_bytes(2, "little")
            + (16000).to_bytes(4, "little") + (32000).to_bytes(4, "little")
            + (2).to_bytes(2, "little") + (16).to_bytes(2, "little")
            + b"data" + data.to_bytes(4, "little") + bytes(data))
    with path.open("xb") as stream:
        stream.write(wave)


if __name__ == "__main__":
    main()
