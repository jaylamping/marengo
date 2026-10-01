"""Real shell entrypoint with owned scanner/DB fixtures and both report formats."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
from datetime import datetime, timedelta, timezone

ROOT = Path(__file__).resolve().parents[2]


class ClassificationTests(unittest.TestCase):
    def test_report_classifications_and_supported_command(self):
        cases = [(0, 0, 0, 0, "clean"), (1, 2, 0, 0, "vulnerabilities"),
                 (0, 0, 1, 0, "maintenance-warnings"), (0, 0, 0, 8, "stale-database"),
                 (2, 0, 0, 0, "error"), (1, 0, 0, 0, "error")]
        for code, count, warnings, age, expected in cases:
            with self.subTest(expected=expected, code=code), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                scripts = root / "scripts/daily-audit"
                scripts.mkdir(parents=True)
                shutil.copyfile(ROOT / "scripts/daily-audit/run.sh", scripts / "run.sh")
                (scripts / "audit.py").write_text('import pathlib,json,datetime\np=pathlib.Path("var/log/daily-audit")/datetime.datetime.now(datetime.timezone.utc).date().isoformat();p.mkdir(parents=True,exist_ok=True);(p/"report.json").write_text(json.dumps({"findings":[],"clean":True}));(p/"report.md").write_text("fixture\\n")\n')
                db = root / "advisory-db"
                db.mkdir()
                bin_dir = root / "bin"
                bin_dir.mkdir()
                stamp = (datetime.now(timezone.utc) - timedelta(days=age)).isoformat()
                payload = {"database": {"last-updated": stamp, "last-commit": "a" * 40},
                           "vulnerabilities": {"count": count, "found": bool(count), "list": [{}] * count},
                           "warnings": {"unmaintained": [{}] * warnings}}
                cargo = bin_dir / "cargo"
                cargo.write_text('#!/usr/bin/python3\nimport json,sys,pathlib\npathlib.Path("args.json").write_text(json.dumps(sys.argv[1:]))\nprint(' + repr(json.dumps(payload)) + ')\nprint("fixture stderr",file=sys.stderr)\nsys.exit(' + str(code) + ')\n')
                cargo.chmod(0o755)
                scanner = bin_dir / "cargo-audit"
                scanner.write_text('#!/bin/sh\nexit 0\n')
                scanner.chmod(0o755)
                proc = subprocess.run(["sh", str(scripts / "run.sh")], cwd=root,
                    env=dict(os.environ, PATH=f"{bin_dir}:/usr/bin:/bin", MARENGO_ADVISORY_DB=str(db)),
                    capture_output=True, text=True)
                self.assertEqual(proc.returncode, 0, proc.stderr)
                out = next((root / "var/log/daily-audit").iterdir())
                result = json.loads((out / "cargo-audit-result.json").read_text())
                report = json.loads((out / "report.json").read_text())
                self.assertEqual(result["status"], expected)
                self.assertEqual(result["exit_code"], code)
                self.assertEqual(report["clean"], expected == "clean")
                self.assertIn(expected, (out / "report.md").read_text())
                self.assertEqual((out / "cargo-audit.stderr.log").read_text(), "fixture stderr\n")
                self.assertEqual(json.loads((root / "args.json").read_text()),
                    ["audit", "--no-fetch", "--no-yanked", "--json", "--db", str(db)])


if __name__ == "__main__":
    unittest.main()
