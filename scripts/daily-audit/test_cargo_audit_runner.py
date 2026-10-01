"""Public shell-runner contract: a scanner usage error is never an advisory."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]


class RunnerErrorTests(unittest.TestCase):
    def test_cli_usage_error_is_not_reported_as_vulnerability(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            scripts = root / "scripts/daily-audit"
            scripts.mkdir(parents=True)
            shutil.copyfile(ROOT / "scripts/daily-audit/run.sh", scripts / "run.sh")
            (scripts / "audit.py").write_text("# fixture handled by interpreter shim\n")
            bin_dir = root / "bin"
            bin_dir.mkdir()
            # Only replace deterministic audit production for this dependency scan fixture.
            shim = '#!/usr/bin/python3\nimport sys,json,datetime,pathlib,subprocess\nif len(sys.argv)>1 and sys.argv[1].endswith("/audit.py"):\n p=pathlib.Path.cwd()/"var/log/daily-audit"/datetime.datetime.now(datetime.timezone.utc).date().isoformat();p.mkdir(parents=True,exist_ok=True);(p/"report.json").write_text(json.dumps({"findings":[],"clean":True}));(p/"report.md").write_text("fixture\\n")\nelse:\n sys.exit(subprocess.call(["/usr/bin/python3",*sys.argv[1:]]))\n'
            (bin_dir / "python3").write_text(shim)
            (bin_dir / "cargo-audit").write_text('#!/bin/sh\nexit 0\n')
            (bin_dir / "cargo").write_text('#!/bin/sh\necho "unexpected argument: scanner operational fixture" >&2\nexit 2\n')
            for path in bin_dir.iterdir():
                path.chmod(0o755)
            environment = dict(os.environ, PATH=f"{bin_dir}:/usr/bin:/bin")
            result = subprocess.run(["sh", str(scripts / "run.sh")], cwd=root, env=environment, capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            report_path = next((root / "var/log/daily-audit").glob("*/report.json"))
            report = json.loads(report_path.read_text())
            self.assertFalse(any("advisories" in f["message"] for f in report["findings"]), "scanner argument failure is not a vulnerability finding")
            self.assertEqual(report["cargo_audit"]["status"], "error")


if __name__ == "__main__":
    unittest.main()
