#!/usr/bin/env python3
"""Fail if any file that git tracks (or would add) is 100 MB or larger.

Joey's rule (2026-11-08): the repo holds the latest version of every CAD file,
but no single file may be 100 MB or more. GitHub rejects plain files of 100 MiB
and LFS storage is metered, so the limit applies to LFS files too.

Usage:
    python tools/check_file_sizes.py [--limit-mb N]

Checks tracked files plus untracked files that are not ignored (what `git add -A`
would pick up). For an LFS pointer the real size is read from the pointer, so the
check also works in a checkout without the LFS content (CI).
Exit code 0 = all files under the limit, 1 = at least one file at or over it.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

DEFAULT_LIMIT_MB = 100
LFS_HEADER = b"version https://git-lfs.github.com/spec/v1"


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def candidate_files(root: Path) -> list[str]:
    # safe.directory: CI runs the checkout as a different user inside Docker.
    out = subprocess.run(
        ["git", "-c", f"safe.directory={root}", "-C", str(root),
         "ls-files", "-z", "--cached", "--others", "--exclude-standard"],
        capture_output=True,
        check=True,
    )
    return sorted({p for p in out.stdout.decode("utf-8").split("\0") if p})


def real_size(path: Path) -> int:
    size = path.stat().st_size
    if size < 1024:
        with path.open("rb") as fh:
            head = fh.read(1024)
        if head.startswith(LFS_HEADER):
            for line in head.splitlines():
                if line.startswith(b"size "):
                    return int(line.split()[1])
    return size


def main(argv: list[str]) -> int:
    limit_mb = DEFAULT_LIMIT_MB
    if argv[:1] == ["--limit-mb"] and len(argv) == 2:
        limit_mb = int(argv[1])
    elif argv:
        print(__doc__)
        return 2
    limit = limit_mb * 1000 * 1000
    root = repo_root()
    too_big = []
    for rel in candidate_files(root):
        path = root / rel
        if not path.is_file():
            continue  # deleted in the working tree, or a submodule
        size = real_size(path)
        if size >= limit:
            too_big.append((size, rel))
    for size, rel in sorted(too_big, reverse=True):
        print(f"TOO BIG {size / 1e6:8.1f} MB  {rel}")
    if too_big:
        print(
            f"\n{len(too_big)} file(s) at or over {limit_mb} MB. Keep them out of git: "
            "move to _wip/, _sim/ or _archive/ (gitignored), or simplify/split the file."
        )
        return 1
    print(f"OK: no tracked or addable file is {limit_mb} MB or larger")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
