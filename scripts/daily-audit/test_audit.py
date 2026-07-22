#!/usr/bin/env python3
"""Unit tests for daily audit helpers."""

from __future__ import annotations

import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from audit import (
    UNWRAP_RE,
    check_adr_staleness,
    check_proto_checksum,
    production_rust_lines,
    strip_rust_tests,
    Finding,
    Report,
)


class AuditRegexTests(unittest.TestCase):
    def test_unwrap_or_not_matched(self) -> None:
        src = "let x = foo.unwrap_or(0);\nlet y = bar.unwrap_or_else(|| 0);"
        self.assertIsNone(UNWRAP_RE.search(src))

    def test_unwrap_matched(self) -> None:
        src = "let x = foo.unwrap();"
        self.assertIsNotNone(UNWRAP_RE.search(src))

    def test_expect_matched(self) -> None:
        src = 'let x = foo.expect("msg");'
        self.assertIsNotNone(UNWRAP_RE.search(src))

    def test_test_module_stripped(self) -> None:
        src = """
pub fn ok() {}

#[cfg(test)]
mod tests {
    fn t() {
        let _ = 1_u32.try_into().unwrap();
    }
}
"""
        prod = production_rust_lines(src)
        self.assertIsNone(UNWRAP_RE.search(prod))

    def test_comment_robstride_ignored_in_strip(self) -> None:
        src = "// Robstride only streams status after MIT frames\nlet x = 1;"
        prod = production_rust_lines(src)
        self.assertNotIn("Robstride", prod)


class StripRustTests(unittest.TestCase):
    def test_strip_rust_tests_removes_block(self) -> None:
        src = "line1\n#[cfg(test)]\nmod tests {\n    fn t() {}\n}\nline2"
        out = strip_rust_tests(src)
        self.assertIn("line1", out)
        self.assertIn("line2", out)
        self.assertNotIn("mod tests", out)


class ProtoChecksumTests(unittest.TestCase):
    def test_proto_change_without_checksum_warns(self) -> None:
        report = Report(date="2026-06-16")
        check_proto_checksum(["proto/marengo/v1/marengo.proto"], report)
        self.assertFalse(report.clean)
        self.assertEqual(len(report.findings), 1)

    def test_proto_and_checksum_change_is_clean(self) -> None:
        report = Report(date="2026-06-16")
        check_proto_checksum(
            ["proto/marengo/v1/marengo.proto", "consul/src/gen/.checksum"],
            report,
        )
        self.assertTrue(report.clean)


class AdrStalenessTests(unittest.TestCase):
    def test_davout_without_safety_md_warns(self) -> None:
        report = Report(date="2026-07-21")
        check_adr_staleness(["crates/davout/src/active_reporting.rs"], report)
        self.assertFalse(report.clean)
        self.assertEqual(len(report.findings), 1)
        self.assertIn("docs/safety.md", report.findings[0].rule)

    def test_davout_with_safety_md_is_clean(self) -> None:
        report = Report(date="2026-07-21")
        check_adr_staleness(
            [
                "crates/davout/src/active_reporting.rs",
                "docs/safety.md",
            ],
            report,
        )
        self.assertTrue(report.clean)

    def test_robstride_with_hardware_adr_is_clean(self) -> None:
        report = Report(date="2026-07-21")
        check_adr_staleness(
            [
                "crates/robstride/src/bus.rs",
                "hardware/docs/decisions/0002-robstride-protocol.md",
            ],
            report,
        )
        self.assertTrue(report.clean)

    def test_unrelated_decision_doc_does_not_clear_davout(self) -> None:
        report = Report(date="2026-07-21")
        check_adr_staleness(
            [
                "crates/davout/src/lib.rs",
                "docs/decisions/0001-protobuf-wire-types.md",
            ],
            report,
        )
        self.assertFalse(report.clean)


if __name__ == "__main__":
    unittest.main()
