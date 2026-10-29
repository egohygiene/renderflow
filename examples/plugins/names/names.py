"""Example local Renderflow plugin: exact supplied names to JSON and CSV.

Uses Python 3.9+ standard library only. Does not recognize arbitrary people,
read PDFs, perform OCR, access the network or modify source documents.
"""

import argparse
import bisect
import csv
import hashlib
import io
import json
from pathlib import Path

SCHEMA = "example.names/v1"
MAX_BYTES = 8 * 1024 * 1024
MAX_MATCHES = 10000


def read_bounded(path, limit=MAX_BYTES):
    """Read at most limit bytes and reject oversized input."""
    with Path(path).open("rb") as stream:
        content = stream.read(limit + 1)
    if len(content) > limit:
        raise ValueError("input exceeds byte limit")
    return content


def valid_name(value):
    """Accept bounded, visible names without control characters."""
    return (isinstance(value, str) and 0 < len(value) <= 128
            and value == value.strip() and all(character.isprintable() for character in value))


def word_character(character):
    """Use Unicode alphanumeric characters and underscore as word boundaries."""
    return character.isalnum() or character == "_"


def extract_names(content, config):
    """Count nonoverlapping, case-sensitive literal matches for each supplied name.

    Offsets are zero-based Unicode code points in decoded UTF-8, end-exclusive.
    Page/line/column values are one-based. A form-feed starts a new page.
    Overlap between different configured names is counted independently.
    """
    if not isinstance(config, dict) or set(config) - {"names", "max_matches"}:
        raise ValueError("unsupported name extraction configuration")
    names = config.get("names")
    if (not isinstance(names, list) or not 1 <= len(names) <= 100
            or not all(valid_name(name) for name in names) or len(set(names)) != len(names)):
        raise ValueError("names must contain 1 to 100 unique, bounded strings")
    maximum = config.get("max_matches", MAX_MATCHES)
    if type(maximum) is not int or not 1 <= maximum <= MAX_MATCHES:
        raise ValueError("max_matches must be an integer from 1 to 10000")
    if len(content) > MAX_BYTES:
        raise ValueError("input exceeds byte limit")
    text = content.decode("utf-8", errors="strict")
    page_starts = [0]
    line_starts = [0]
    for index, character in enumerate(text):
        if character == "\f":
            page_starts.append(index + 1)
            line_starts.append(index + 1)
        elif character == "\n":
            line_starts.append(index + 1)
    rows = []
    total = 0
    for name in names:
        occurrences = []
        start = 0
        while True:
            start = text.find(name, start)
            if start < 0:
                break
            end = start + len(name)
            if ((start == 0 or not word_character(text[start - 1]))
                    and (end == len(text) or not word_character(text[end]))):
                total += 1
                if total > maximum:
                    raise ValueError("match limit exceeded; no partial report is published")
                page = bisect.bisect_right(page_starts, start)
                line_index = bisect.bisect_right(line_starts, start) - 1
                first_line = bisect.bisect_left(line_starts, page_starts[page - 1])
                occurrences.append({
                    "page": page, "line": line_index - first_line + 1,
                    "column": start - line_starts[line_index] + 1,
                    "char_start": start, "char_end": end,
                })
            start = end
        rows.append({"name": name, "count": len(occurrences), "occurrences": occurrences})
    return {
        "schema": SCHEMA,
        "source_sha256": hashlib.sha256(content).hexdigest(),
        "matching": "literal_case_sensitive_unicode_word_boundaries",
        "names": rows,
    }


def report_csv(report, config):
    """Export bounded counts/pages; keep JSON for complete occurrence locations."""
    if config != {}:
        raise ValueError("CSV exporter accepts an empty configuration only")
    if not isinstance(report, dict) or set(report) != {"schema", "source_sha256", "matching", "names"}:
        raise ValueError("invalid names report")
    digest = report["source_sha256"]
    if (report["schema"] != SCHEMA or not isinstance(digest, str) or len(digest) != 64
            or any(character not in "0123456789abcdef" for character in digest)
            or report["matching"] != "literal_case_sensitive_unicode_word_boundaries"):
        raise ValueError("unsupported report schema or source identity")
    rows = report["names"]
    if not isinstance(rows, list) or not 1 <= len(rows) <= 100:
        raise ValueError("invalid report rows")
    stream = io.StringIO(newline="")
    writer = csv.writer(stream, lineterminator="\n")
    writer.writerow(["name", "count", "pages", "source_sha256"])
    seen = set()
    total = 0
    for row in rows:
        if not isinstance(row, dict) or set(row) != {"name", "count", "occurrences"}:
            raise ValueError("invalid name row")
        name, count, occurrences = row["name"], row["count"], row["occurrences"]
        if (not valid_name(name) or name in seen or type(count) is not int or count < 0
                or not isinstance(occurrences, list) or count != len(occurrences)):
            raise ValueError("invalid counts or duplicate name")
        seen.add(name)
        total += count
        if total > MAX_MATCHES:
            raise ValueError("report exceeds match limit")
        previous_end = 0
        for occurrence in occurrences:
            if (not isinstance(occurrence, dict)
                    or set(occurrence) != {"page", "line", "column", "char_start", "char_end"}
                    or any(type(value) is not int for value in occurrence.values())
                    or any(occurrence[key] < 1 for key in ["page", "line", "column"])
                    or occurrence["char_start"] < previous_end
                    or occurrence["char_end"] - occurrence["char_start"] != len(name)):
                raise ValueError("invalid occurrence locations")
            previous_end = occurrence["char_end"]
        # CSV readers can interpret these prefixes as spreadsheet formulas.
        safe_name = "'" + name if name.startswith(("=", "+", "-", "@")) else name
        pages = ";".join(str(page) for page in sorted({item["page"] for item in occurrences}))
        writer.writerow([safe_name, count, pages, digest])
    return stream.getvalue()


def main():
    """Read host-supplied files and create exactly one new output file."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=["names", "csv"])
    parser.add_argument("--input", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--config", required=True)
    args = parser.parse_args()
    try:
        content = read_bounded(args.input)
        config = json.loads(read_bounded(args.config, 64 * 1024))
        if args.mode == "names":
            output = json.dumps(extract_names(content, config), ensure_ascii=False, sort_keys=True, indent=2) + "\n"
        else:
            output = report_csv(json.loads(content), config)
        with Path(args.output).open("x", encoding="utf-8", newline="") as stream:
            stream.write(output)
    except (ValueError, OSError, UnicodeError):
        # Never echo a document, configured name, private path or raw decoder error.
        parser.exit(2, "name example failed: check configuration, input shape and limits\n")


if __name__ == "__main__":
    main()
