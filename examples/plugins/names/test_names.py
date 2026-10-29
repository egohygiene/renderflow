"""Authored, UNRUN regressions for the local names example (#466)."""

import csv
import io
import unittest

from names import extract_names, report_csv


class NamesTests(unittest.TestCase):
    def test_literal_boundaries_unicode_locations_and_pages(self):
        content = "Adaline Ada ada\nLin\fÉva met Ada.".encode("utf-8")
        report = extract_names(content, {"names": ["Ada", "Lin", "Éva", "Unknown"]})
        self.assertEqual([row["count"] for row in report["names"]], [2, 1, 1, 0])
        self.assertEqual(report["names"][2]["occurrences"][0], {
            "page": 2, "line": 1, "column": 1, "char_start": 20, "char_end": 23,
        })
        self.assertEqual(report, extract_names(content, {"names": ["Ada", "Lin", "Éva", "Unknown"]}))

    def test_invalid_configuration_limits_and_non_utf8_refuse(self):
        for config in [{}, {"names": ["Ada", "Ada"]}, {"names": [""]},
                       {"names": ["Ada"], "unknown": True},
                       {"names": ["Ada"], "max_matches": True},
                       {"names": ["Ada"], "max_matches": 1}]:
            with self.subTest(config=config), self.assertRaises(ValueError):
                extract_names(b"Ada Ada", config)
        with self.assertRaises(UnicodeError):
            extract_names(b"\xff", {"names": ["Ada"]})

    def test_csv_counts_formula_neutralization_and_shape_refusal(self):
        report = extract_names(b"Ada Ada =cmd", {"names": ["Ada", "=cmd", "Lin"]})
        rows = list(csv.DictReader(io.StringIO(report_csv(report, {}))))
        self.assertEqual([(row["name"], row["count"], row["pages"]) for row in rows],
                         [("Ada", "2", "1"), ("'=cmd", "1", "1"), ("Lin", "0", "")])
        self.assertTrue(all(row["source_sha256"] == report["source_sha256"] for row in rows))
        report["names"][0]["count"] = 500
        with self.assertRaises(ValueError):
            report_csv(report, {})


if __name__ == "__main__":
    unittest.main()
