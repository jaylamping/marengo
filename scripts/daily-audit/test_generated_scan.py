"""Independent mismatch/error controls for isolated generation verification."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from generated_scan import verify_generated


class GeneratedScanTests(unittest.TestCase):
    def run_case(self, current=b"generated", checksum=None, failure=False, missing=False, deleted=False):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            consul = root / "consul"
            target = consul / "src/gen/marengo/v1/marengo_pb.ts"
            target.parent.mkdir(parents=True)
            target.write_bytes(current)
            expected = b"generated"
            digest = checksum if checksum is not None else hashlib.sha256(expected).hexdigest()
            (consul / "src/gen/.checksum").write_text(digest)
            packages = {}
            for name in ["@bufbuild/buf", "@bufbuild/protoc-gen-es"]:
                packages[f"node_modules/{name}"] = {"version": "fixture"}
                directory = consul / "node_modules" / name
                directory.mkdir(parents=True)
                if not missing:
                    (directory / "package.json").write_text(json.dumps({"version": "fixture"}))
            (consul / "package-lock.json").write_text(json.dumps({"packages": packages}))
            def generate(command, **kwargs):
                template = json.loads(command[command.index("--template") + 1])
                generated = Path(template["plugins"][0]["out"]) / "marengo/v1/marengo_pb.ts"
                self.assertNotEqual(generated, target)
                generated.parent.mkdir(parents=True)
                generated.write_bytes(expected)
                return subprocess.CompletedProcess(command, 1 if failure else 0, "", "failed fixture" if failure else "")
            with patch("generated_scan.subprocess.run", side_effect=generate):
                result = verify_generated(root, ["consul/src/gen/marengo/v1/marengo_pb.ts"] + (["consul/src/gen/deleted_pb.ts"] if deleted else []))
            self.assertEqual(target.read_bytes(), current)
            self.assertEqual((consul / "src/gen/.checksum").read_text(), digest)
            return result

    def test_matching_deleted_output_is_not_manual_edit(self):
        self.assertEqual(self.run_case(deleted=True)["status"], "verified")

    def test_manual_artifact_edit_is_mismatch(self):
        result = self.run_case(current=b"manual edit")
        self.assertEqual(result["status"], "mismatch")
        self.assertEqual(result["mismatches"], ["consul/src/gen/marengo/v1/marengo_pb.ts"])

    def test_manual_checksum_is_mismatch(self):
        result = self.run_case(checksum="0" * 64)
        self.assertEqual(result["status"], "mismatch")
        self.assertIn("consul/src/gen/.checksum", result["mismatches"])

    def test_failed_generator_is_not_verified(self):
        self.assertEqual(self.run_case(failure=True)["status"], "failed")

    def test_unavailable_generator_is_unknown(self):
        self.assertEqual(self.run_case(missing=True)["status"], "unknown")

    def test_malformed_checksum_is_unknown(self):
        self.assertEqual(self.run_case(checksum="not a checksum")["status"], "unknown")


if __name__ == "__main__":
    unittest.main()
