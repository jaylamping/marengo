"""Public generated-code audit contract using an owned deterministic generator."""
import hashlib
import json
from pathlib import Path
import os
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
import audit


class GeneratedAuditProbe(unittest.TestCase):
    def test_legitimate_regeneration_is_not_a_critical_hand_edit(self):
        module = audit
        baseline = os.environ.get("MARENGO_AUDIT_PROBE_BASELINE")
        if baseline:
            code = subprocess.check_output(["git", "show", f"{baseline}:scripts/daily-audit/audit.py"], cwd=audit.ROOT, text=True)
            module = types.ModuleType("owned_original_audit")
            module.__file__ = audit.__file__
            sys.modules[module.__name__] = module
            exec(compile(code, module.__file__, "exec"), module.__dict__)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            consul = root / "consul"
            artifact = b"// deterministic generated fixture\nexport const robot = 1;\n"
            target = consul / "src/gen/marengo/v1/marengo_pb.ts"
            target.parent.mkdir(parents=True)
            target.write_bytes(artifact)
            (consul / "src/gen/.checksum").write_text(hashlib.sha256(artifact).hexdigest())
            packages = {}
            for package, version in [("@bufbuild/buf", "1.69.0"), ("@bufbuild/protoc-gen-es", "2.12.0")]:
                path = consul / "node_modules" / package
                path.mkdir(parents=True)
                (path / "package.json").write_text(json.dumps({"version": version}))
                packages[f"node_modules/{package}"] = {"version": version}
            (consul / "package-lock.json").write_text(json.dumps({"packages": packages}))
            bin_dir = consul / "node_modules/.bin"
            bin_dir.mkdir()
            buf = bin_dir / "buf"
            buf.write_text('#!/usr/bin/python3\nimport json,sys,pathlib\nout=pathlib.Path(json.loads(sys.argv[sys.argv.index("--template")+1])["plugins"][0]["out"])/"marengo/v1/marengo_pb.ts"\nout.parent.mkdir(parents=True);out.write_bytes(' + repr(artifact) + ')\n')
            buf.chmod(0o755)
            report = module.Report(date="2026-10-01")
            with patch.object(module, "ROOT", root):
                module.check_gen_handedit(["proto/marengo/v1/marengo.proto", "consul/src/gen/marengo/v1/marengo_pb.ts", "consul/src/gen/.checksum"], report)
            self.assertFalse(any(f.severity == "critical" for f in report.findings), "matching locked regeneration must not be called a hand edit")
            self.assertTrue(report.clean)


if __name__ == "__main__":
    unittest.main()
