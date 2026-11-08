#!/usr/bin/env python3
"""Record where an export in cad/exports/ came from, in cad/manifests/exports.json.

    python tools/cad/record_export.py EXPORT SOURCE [--config NAME] [--settings TEXT]

EXPORT and SOURCE are paths under cad/ (relative to cad/ or the repo, or absolute), e.g.
    python tools/cad/record_export.py exports/shoulder/marengo_shoulder_pitch_bracket_left.step \\
        shoulder/marengo_shoulder_pitch_bracket_left.SLDPRT --config Default --settings "STEP AP214"
Run it right after exporting. The layout checker then flags the export as stale when the
source changes, and as an error when the export changes without a new record.
Hashes are of the real content (for an LFS pointer, its oid), so it works in any clone.
"""

from __future__ import annotations

import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gitlfs import REPO_ROOT, content_sha256

CAD_ROOT = REPO_ROOT / "cad"
EXPORTS_PATH = CAD_ROOT / "manifests" / "exports.json"


def cad_rel(arg: str, cad_root: Path = CAD_ROOT) -> str:
    p = Path(arg)
    if not p.is_absolute():
        p = (cad_root.parent / p) if p.parts and p.parts[0] == "cad" else (cad_root / p)
    return p.resolve().relative_to(cad_root.resolve()).as_posix()


def record(
    export: str,
    source: str,
    config: str = "",
    settings: str = "",
    cad_root: Path = CAD_ROOT,
    manifest: Path = EXPORTS_PATH,
) -> dict:
    exp, src = cad_rel(export, cad_root), cad_rel(source, cad_root)
    if not exp.startswith("exports/"):
        raise SystemExit(f"{exp}: exports live under cad/exports/<region>/")
    for rel in (exp, src):
        if not (cad_root / rel).is_file():
            raise SystemExit(f"not found: cad/{rel}")
    data = json.loads(manifest.read_text(encoding="utf-8"))
    entry = {
        "export": exp,
        "source": src,
        "config": config,
        "settings": settings,
        "sourceSha256": content_sha256(cad_root / src),
        "exportSha256": content_sha256(cad_root / exp),
        "recorded": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    entries = [e for e in data.get("exports", []) if e["export"].lower() != exp.lower()]
    entries.append(entry)
    data["exports"] = sorted(entries, key=lambda e: e["export"].lower())
    with manifest.open("w", encoding="utf-8", newline="\n") as fh:
        json.dump(data, fh, indent=2)
        fh.write("\n")
    return entry


def main(argv: list[str]) -> int:
    opts = {"--config": "", "--settings": ""}
    pos = []
    i = 0
    while i < len(argv):
        if argv[i] in opts and i + 1 < len(argv):
            opts[argv[i]] = argv[i + 1]
            i += 2
        else:
            pos.append(argv[i])
            i += 1
    if len(pos) != 2 or any(p.startswith("--") for p in pos):
        print(__doc__)
        return 2
    entry = record(pos[0], pos[1], opts["--config"], opts["--settings"])
    print(f"recorded cad/{entry['export']} <- cad/{entry['source']}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
