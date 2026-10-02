"""Cargo's semantic dev/production/alias distinction, not raw TOML strings."""
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import audit


class DependencyEvidenceTests(unittest.TestCase):
    def test_dev_driver_is_valid_but_renamed_production_driver_is_not(self):
        for kind, count in [("dev", 0), (None, 1)]:
            with self.subTest(kind=kind):
                metadata = {"packages": [{"name": "berthier", "dependencies": [{"name": "robstride", "kind": kind, "rename": "bench_driver"}]}]}
                proc = subprocess.CompletedProcess([], 0, json.dumps(metadata), "")
                report = audit.Report(date="2026-10-02")
                with patch.object(audit.subprocess, "run", return_value=proc) as runner:
                    audit.check_davout_bypass(["crates/berthier/Cargo.toml"], report)
                self.assertEqual(len(report.findings), count)
                self.assertEqual(report.evidence[0]["check"], "berthier_dependencies")
                self.assertEqual(runner.call_args.args[0], ["cargo", "metadata", "--offline", "--locked", "--no-deps", "--format-version", "1"])

    def test_failed_metadata_does_not_mean_no_dependencies(self):
        report = audit.Report(date="2026-10-02")
        with patch.object(audit.subprocess, "run", return_value=subprocess.CompletedProcess([], 1, "", "fixture failed")):
            audit.capture_check(report, "dependencies", lambda: audit.check_davout_bypass(["crates/berthier/Cargo.toml"], report))
        self.assertFalse(report.clean)
        self.assertEqual(report.checks["dependencies"]["status"], "failed")


if __name__ == "__main__":
    unittest.main()
