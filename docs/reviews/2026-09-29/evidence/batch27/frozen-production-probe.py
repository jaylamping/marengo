"""The historical review Berthier source's test driver references are not a production bypass."""
from pathlib import Path
import sys
import tempfile
from unittest.mock import patch
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import audit


class ProductionAuditTests(unittest.TestCase):
    def test_actual_berthier_test_driver_does_not_become_critical(self):
        path = "crates/berthier/src/loop.rs"
        source = (audit.ROOT / "docs/reviews/2026-09-29/evidence/batch27/original-berthier-loop.rs").read_text()
        self.assertIn("#[cfg(test)]", source)
        self.assertIn("robstride", source)
        report = audit.Report(date="2026-10-01")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            file = root / path
            file.parent.mkdir(parents=True)
            file.write_text(source)
            with patch.object(audit, "ROOT", root):
                audit.check_davout_bypass([path], report)
        self.assertFalse(any(f.category == "safety" and f.severity == "critical" for f in report.findings),
                         "test-only driver references are not a production motor bypass")


if __name__ == "__main__":
    unittest.main()
