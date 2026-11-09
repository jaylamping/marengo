#!/usr/bin/env python3
"""pre-commit: no staged file of 100 MB or more; commits touching CAD respect the layout and the writer lock.

Installed by tools/cad/install_hooks.py. Skips the CAD checks when nothing staged is CAD.
"""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path, PurePosixPath

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
from gitlfs import REPO_ROOT, git

CAD_EXT = {
    ".sldprt",
    ".sldasm",
    ".slddrw",
    ".step",
    ".stp",
    ".stl",
    ".3mf",
    ".gcode",
    ".x_t",
    ".igs",
    ".iges",
}


def touches_cad(paths: list[str]) -> bool:
    return any(
        p.startswith("cad/") or PurePosixPath(p).suffix.lower() in CAD_EXT
        for p in paths
    )


def run(*args: str) -> int:
    return subprocess.run(
        [sys.executable, *args], cwd=REPO_ROOT, check=False
    ).returncode


def main() -> int:
    staged = [
        p
        for p in git("diff", "--cached", "--name-only", "-z").decode().split("\0")
        if p
    ]
    failed = run(str(REPO_ROOT / "tools" / "check_file_sizes.py"), "--staged") != 0
    if touches_cad(staged):
        sys.path.insert(0, str(HERE.parent))
        import cad_lock

        ok, msg = cad_lock.check(os.environ.get("CAD_WRITER"))
        if not ok:
            print(f"pre-commit: {msg}")
            failed = True
        else:
            cad_lock.refresh(
                os.environ.get("CAD_WRITER", "")
            )  # keeps a held lock alive; no-op otherwise
        # --repo checks the index (what is being committed), not untracked files in the tree
        failed |= (
            run(str(HERE.parent / "check_cad_layout.py"), "--repo", "--quiet") != 0
        )
    if failed:
        print("pre-commit: fix the above (see cad/README.md 'What goes in git').")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
