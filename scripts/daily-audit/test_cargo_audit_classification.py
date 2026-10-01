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
        cases = [(0, 0, 0, 0, "clean", "normal"), (1, 2, 0, 0, "vulnerabilities", "normal"),
                 (0, 0, 1, 0, "maintenance-warnings", "normal"),
                 (0, 0, 0, 8, "stale-database", "normal"),
                 (0, 0, 0, -1, "stale-database", "normal"),
                 (2, 0, 0, 0, "error", "normal"), (1, 0, 0, 0, "error", "normal"),
                 (0, 0, 0, 0, "error", "malformed"),
                 (0, 0, 0, 0, "error", "unknown-provenance"),
                 (0, 0, 0, 0, "missing-database", "missing-db"),
                 (2, 0, 0, 0, "missing-database", "missing-db"),
                 (0, 0, 0, 0, "error", "ancestor-db"),
                 (0, 0, 0, 0, "error", "timeout"),
                 (0, 0, 0, 0, "error", "provenance-timeout"),
                 (0, 0, 0, 0, "unavailable", "unavailable")]
        for code, count, warnings, age, expected, scenario in cases:
            with self.subTest(expected=expected, code=code), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                scripts = root / "scripts/daily-audit"
                scripts.mkdir(parents=True)
                shutil.copyfile(ROOT / "scripts/daily-audit/run.sh", scripts / "run.sh")
                (scripts / "audit.py").write_text('import pathlib,json,datetime\np=pathlib.Path("var/log/daily-audit")/datetime.datetime.now(datetime.timezone.utc).date().isoformat();p.mkdir(parents=True,exist_ok=True);(p/"report.json").write_text(json.dumps({"findings":[],"clean":True}));(p/"report.md").write_text("fixture\\n")\n')
                db = root / "advisory-db"
                if scenario != "missing-db":
                    db.mkdir()
                bin_dir = root / "bin"
                bin_dir.mkdir()
                stamp = (datetime.now(timezone.utc) - timedelta(days=age)).isoformat()
                payload = {"database": {"last-updated": stamp, "last-commit": "a" * 40},
                           "vulnerabilities": {"count": count, "found": bool(count), "list": [{}] * count},
                           "warnings": {"unmaintained": [{}] * warnings}}
                if scenario in {"unknown-provenance", "ancestor-db", "provenance-timeout"}:
                    payload["database"]["last-updated"] = None
                if scenario == "ancestor-db":
                    subprocess.run(["git", "init", "-q", str(root)], check=True)
                    (root / "tracked").write_text("unrelated parent fixture")
                    subprocess.run(["git", "-C", str(root), "add", "tracked"], check=True)
                    subprocess.run(["git", "-C", str(root), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "commit", "-qm", "fixture"], check=True)
                    # Ignore fixture artifacts so only repository identity distinguishes this case.
                    (root / ".git/info/exclude").write_text("*\n")
                if scenario == "timeout":
                    (root / "sitecustomize.py").write_text('import subprocess\noriginal=subprocess.run\ndef run(cmd,*args,**kwargs):\n if cmd[:2]==["cargo","audit"]: kwargs["timeout"]=0.5\n return original(cmd,*args,**kwargs)\nsubprocess.run=run\n')
                if scenario == "provenance-timeout":
                    (root / "sitecustomize.py").write_text('import subprocess\noriginal=subprocess.run\ndef run(cmd,*args,**kwargs):\n if cmd[0]=="git": raise subprocess.TimeoutExpired(cmd,10,output=b"git partial",stderr=b"git error")\n return original(cmd,*args,**kwargs)\nsubprocess.run=run\n')
                encoded = "not json" if scenario == "malformed" else json.dumps(payload)
                cargo = bin_dir / "cargo"
                cargo.write_text('#!/usr/bin/python3\nimport json,sys,pathlib\npathlib.Path("args.json").write_text(json.dumps(sys.argv[1:]))\nprint(' + repr(encoded) + ')\nprint("fixture stderr",file=sys.stderr)\nsys.exit(' + str(code) + ')\n')
                if scenario == "timeout":
                    cargo.write_text(cargo.read_text().replace('sys.exit(', 'import time\nsys.stdout.flush();sys.stderr.flush();time.sleep(2)\nsys.exit('))
                cargo.chmod(0o755)
                scanner = bin_dir / "cargo-audit"
                scanner.write_text('#!/bin/sh\nexit 0\n')
                scanner.chmod(0o755)
                if scenario == "unavailable":
                    scanner.unlink()
                proc = subprocess.run(["sh", str(scripts / "run.sh")], cwd=root,
                    env=dict(os.environ, PATH=f"{bin_dir}:/usr/bin:/bin", MARENGO_ADVISORY_DB=str(db), PYTHONPATH=str(root)),
                    capture_output=True, text=True)
                self.assertEqual(proc.returncode, 0, proc.stderr)
                out = next((root / "var/log/daily-audit").iterdir())
                result = json.loads((out / "cargo-audit-result.json").read_text())
                report = json.loads((out / "report.json").read_text())
                self.assertEqual(result["status"], expected)
                self.assertEqual(result["exit_code"], None if scenario in {"unavailable", "timeout"} else code)
                if scenario == "timeout":
                    self.assertEqual(result["termination"], "timeout")
                    self.assertIn("vulnerabilities", (out / "cargo-audit.stdout.json").read_text())
                if scenario == "provenance-timeout":
                    self.assertEqual(result["timeout_operation"], "database-provenance")
                    self.assertEqual(result["provenance_partial_stdout"], "git partial")
                    self.assertEqual(json.loads((out / "cargo-audit.stdout.json").read_text()), payload)
                self.assertEqual(report["clean"], expected == "clean")
                self.assertIn(expected, (out / "report.md").read_text())
                self.assertEqual((out / "cargo-audit.stderr.log").read_text(), "" if scenario == "unavailable" else "fixture stderr\n")
                if scenario == "unavailable":
                    self.assertFalse((root / "args.json").exists())
                    continue
                self.assertEqual(json.loads((root / "args.json").read_text()),
                    ["audit", "--no-fetch", "--no-yanked", "--json", "--db", str(db)])


if __name__ == "__main__":
    unittest.main()
