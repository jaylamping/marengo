"""Scanner failures never imply a clean complete daily report."""
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import audit


class CompletenessTests(unittest.TestCase):
    def test_failed_check_is_visible_and_nonclean(self):
        report = audit.Report(date="2026-10-01")
        def fail():
            raise RuntimeError("scanner fixture failed")
        audit.capture_check(report, "fixture", fail)
        self.assertFalse(report.clean)
        self.assertEqual(report.checks["fixture"]["status"], "failed")

    def test_uncertain_syntax_cannot_be_complete(self):
        report = audit.Report(date="2026-10-01")
        audit.capture_check(report, "fixture", lambda: report.add(audit.Finding("warn", "scan", "fixture", "parse", "Unknown")))
        self.assertEqual(report.checks["fixture"]["status"], "unknown")

    def test_failed_inventory_writes_incomplete_report_and_nonzero(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with patch.object(audit, "ROOT", root), patch.object(audit, "git_changed_files", side_effect=RuntimeError("inventory failed")), patch.object(audit, "check_ci_status"), patch.object(audit, "check_stale_safety_prs"):
                self.assertEqual(audit.main(), 1)
            report_path = next(root.rglob("report.json"))
            payload = json.loads(report_path.read_text())
            self.assertFalse(payload["clean"])
            self.assertEqual(payload["completeness"], "unknown")
            self.assertEqual(payload["checks"]["git_inventory"]["status"], "failed")
            self.assertEqual(payload["checks"]["check_davout_bypass"]["status"], "unknown")


if __name__ == "__main__":
    unittest.main()
