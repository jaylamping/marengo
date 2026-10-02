"""Aging, failed scans, evidence refusal, recurrence, and exclusive writes."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
from defect_ledger import accept_resolution, record_findings

FINDING = {"severity": "critical", "category": "safety", "file": "fixture.rs", "rule": "motor boundary", "message": "driver path at line 3", "commit": ""}


class DefectLedgerTests(unittest.TestCase):
    def test_aging_out_never_closes_and_bound_resolution_can_reopen(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "defects.json"
            row = record_findings(path, [FINDING], "2026-01-01")[0]
            later = record_findings(path, [], "2026-10-02")
            self.assertEqual(later[0]["id"], row["id"])
            self.assertEqual(later[0]["state"], "open")
            evidence = path.with_name("proof.json")
            receipt = {"finding_id": row["id"], "observed_evidence_sha256": row["observed_evidence_sha256"], "result": "passed", "repair_commit": "a" * 40, "validation_command": "fixture boundary regression"}
            evidence.write_text(json.dumps(receipt))
            digest = hashlib.sha256(evidence.read_bytes()).hexdigest()
            before = path.read_bytes()
            with self.assertRaises(ValueError):
                accept_resolution(path, row["id"], evidence, "0" * 64)
            self.assertEqual(path.read_bytes(), before)
            accept_resolution(path, row["id"], evidence, digest)
            self.assertEqual(record_findings(path, [], "2026-10-03"), [])
            self.assertEqual(record_findings(path, [FINDING], "2026-10-04")[0]["state"], "open")

    def test_foreign_or_failed_proof_cannot_resolve(self):
        for kind in ["foreign", "failed", "missing-validation"]:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "defects.json"
                row = record_findings(path, [FINDING], "2026-01-01")[0]
                receipt = {"finding_id": row["id"] if kind != "foreign" else "foreign", "observed_evidence_sha256": row["id"], "result": "failed" if kind == "failed" else "passed", "repair_commit": "a" * 40, "validation_command": "" if kind == "missing-validation" else "fixture"}
                evidence = path.with_name("proof.json")
                evidence.write_text(json.dumps(receipt))
                before = path.read_bytes()
                with self.assertRaises(ValueError):
                    accept_resolution(path, row["id"], evidence, hashlib.sha256(evidence.read_bytes()).hexdigest())
                self.assertEqual(before, path.read_bytes())

    def test_manual_state_edit_without_proof_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "defects.json"
            row = record_findings(path, [FINDING], "2026-01-01")[0]
            payload = json.loads(path.read_text())
            payload["findings"][row["id"]]["state"] = "resolved"
            path.write_text(json.dumps(payload))
            before = path.read_bytes()
            with self.assertRaises(ValueError):
                record_findings(path, [], "2026-10-02")
            self.assertEqual(before, path.read_bytes())

    def test_competing_writer_is_refused_without_data_loss(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "defects.json"
            record_findings(path, [FINDING], "2026-01-01")
            before = path.read_bytes()
            path.with_suffix(".json.lock").touch()
            with self.assertRaises(FileExistsError):
                record_findings(path, [], "2026-10-02")
            self.assertEqual(before, path.read_bytes())


if __name__ == "__main__":
    unittest.main()
