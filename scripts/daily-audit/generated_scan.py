"""Regenerate proto TypeScript into owned output; never overwrite audited artifacts."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile


def verify_generated(root: Path, changed: list[str]) -> dict:
    consul = root / "consul"
    lock = json.loads((consul / "package-lock.json").read_text())
    packages = lock["packages"]
    for package in ("@bufbuild/buf", "@bufbuild/protoc-gen-es"):
        installed = consul / "node_modules" / package / "package.json"
        if not installed.is_file() or json.loads(installed.read_text())["version"] != packages[f"node_modules/{package}"]["version"]:
            return {"status": "unknown", "detail": f"locked generator unavailable: {package}"}
    buf = consul / "node_modules/.bin/buf"
    plugin = consul / "node_modules/.bin/protoc-gen-es"
    with tempfile.TemporaryDirectory(prefix="marengo-audit-codegen-") as directory:
        output = Path(directory) / "generated"
        template = {"version": "v2", "plugins": [{"local": str(plugin), "out": str(output), "opt": ["target=ts"]}]}
        generated = subprocess.run([str(buf), "generate", "--template", json.dumps(template), str(root / "proto")],
                                   cwd=consul, capture_output=True, text=True, timeout=60)
        if generated.returncode:
            return {"status": "failed", "detail": generated.stderr, "exit_code": generated.returncode}
        target = output / "marengo/v1/marengo_pb.ts"
        if not target.is_file():
            return {"status": "failed", "detail": "generator omitted expected output"}
        actual_hash = hashlib.sha256(target.read_bytes()).hexdigest()
        expected_hash = (consul / "src/gen/.checksum").read_text().strip()
        if not re.fullmatch(r"[0-9a-f]{64}", expected_hash):
            return {"status": "unknown", "detail": "invalid/missing committed checksum"}
        mismatches = []
        if expected_hash != actual_hash:
            mismatches.append("consul/src/gen/.checksum")
        paths = set(changed)
        paths.update(str(path.relative_to(root)) for path in (consul / "src/gen").rglob("*.ts"))
        for path in sorted(paths):
            if not path.startswith("consul/src/gen/") or path.endswith(".checksum"):
                continue
            relative = Path(path).relative_to("consul/src/gen")
            if ".." in relative.parts:
                return {"status": "unknown", "detail": "invalid generated path"}
            current = root / path
            expected = output / relative
            if not current.exists() and not expected.exists():
                continue
            if not current.is_file() or not expected.is_file() or current.read_bytes() != expected.read_bytes():
                mismatches.append(path)
        return {"status": "mismatch" if mismatches else "verified", "detail": "locked regeneration compared",
                "mismatches": mismatches, "generated_sha256": actual_hash,
                "generator_versions": {package: packages[f"node_modules/{package}"]["version"] for package in ("@bufbuild/buf", "@bufbuild/protoc-gen-es")}}
