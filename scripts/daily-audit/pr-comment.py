#!/usr/bin/env python3
"""Comment on PRs that overlap daily-audit flagged files."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


def main() -> int:
    report_path = Path(sys.argv[1])
    report = json.loads(report_path.read_text(encoding="utf-8"))
    flagged = {
        f["file"]
        for f in report.get("findings", [])
        if f.get("severity") in ("warn", "critical")
    }
    if not flagged:
        return 0
    proc = subprocess.run(
        ["gh", "pr", "list", "--json", "number,files", "--limit", "20"],
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        return 0
    prs = json.loads(proc.stdout or "[]")
    for pr in prs:
        num = pr.get("number")
        for f in pr.get("files") or []:
            path = f.get("path", "")
            if path in flagged:
                subprocess.run(
                    [
                        "gh",
                        "pr",
                        "comment",
                        str(num),
                        "--body",
                        f"Daily audit flagged `{path}` — see open `daily-audit` issue.",
                    ],
                    check=False,
                )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
