#!/usr/bin/env python3
"""Keep Git LFS inside GitHub Free (10 GiB storage, every pushed version counts).

Works from git history alone (pointer files), so it runs without LFS content.

Usage:
    python tools/cad/lfs_budget.py report              storage used and new LFS bytes per month
    python tools/cad/lfs_budget.py push [--remote R] REV...   check what these commits would add to R (origin)
                                                       (e.g. origin/main..HEAD); used by pre-push
Limits: cad/manifests/lfs-budget.json. "push" fails (exit 1) when the push adds more than
pushMaxBytes, takes this month past monthMaxBytes, or takes storage past storageMaxBytes,
unless CAD_BUDGET_OK=1 is set (Joey's OK for that push). Exit 2 = bad usage.
"""

from __future__ import annotations

import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from gitlfs import REPO_ROOT, blob_sizes, changed_blobs, git

BUDGET_PATH = REPO_ROOT / "cad" / "manifests" / "lfs-budget.json"
GB = 1e9


def lfs_history(
    rev_args: list[str], root: Path = REPO_ROOT
) -> dict[str, tuple[int, str]]:
    """LFS oid -> (size, YYYY-MM it first appeared) for objects added by the commits rev-list selects."""
    out = git(
        "log",
        "--format=>%ct",
        "--raw",
        "-m",
        "--no-abbrev",
        "--no-renames",
        "--diff-filter=AM",
        *rev_args,
        root=root,
    )
    first_seen: dict[str, float] = {}
    when = 0.0
    for line in out.decode(errors="replace").splitlines():
        if line.startswith(">"):
            when = float(line[1:])
        elif line.startswith(":"):
            blob = line.split()[3]
            if blob not in first_seen or when < first_seen[blob]:
                first_seen[blob] = when
    sizes = blob_sizes(list(first_seen), root=root)
    result: dict[str, tuple[int, str]] = {}
    for blob, (_, ptr) in sizes.items():
        if not ptr:
            continue
        oid, size = ptr
        month = time.strftime("%Y-%m", time.gmtime(first_seen[blob]))
        if oid not in result or month < result[oid][1]:
            result[oid] = (size, month)
    return result


def new_objects(
    rev_args: list[str], stored: dict[str, tuple[int, str]], root: Path = REPO_ROOT
) -> dict[str, int]:
    """LFS oid -> size for objects the commits add that the remotes don't already store."""
    blobs = [b for b, _ in changed_blobs(rev_args, root=root)]
    pushed = {ptr[0]: ptr[1] for _, ptr in blob_sizes(blobs, root=root).values() if ptr}
    return {oid: size for oid, size in pushed.items() if oid not in stored}


def report(root: Path = REPO_ROOT) -> int:
    hist = lfs_history(["--all"], root=root)
    total = sum(s for s, _ in hist.values())
    per_month: dict[str, int] = {}
    for size, month in hist.values():
        per_month[month] = per_month.get(month, 0) + size
    print(
        f"LFS storage in history (all refs): {total / GB:.2f} GB in {len(hist)} objects"
    )
    for month in sorted(per_month):
        print(f"  {month}: +{per_month[month] / GB:.3f} GB")
    return 0


def check_push(
    rev_args: list[str],
    root: Path = REPO_ROOT,
    budget: dict | None = None,
    remote: str = "origin",
) -> tuple[bool, list[str]]:
    """Only objects the TARGET remote already has count as stored (another remote's copy doesn't)."""
    budget = budget or json.loads(BUDGET_PATH.read_text(encoding="utf-8"))
    stored = lfs_history([f"--remotes={remote}"], root=root)
    new = new_objects(rev_args, stored, root=root)
    adding = sum(new.values())
    total = sum(s for s, _ in stored.values())
    month = time.strftime("%Y-%m", time.gmtime())
    this_month = sum(s for s, m in stored.values() if m == month)
    msgs = [
        (
            f"LFS: this push adds {adding / 1e6:.1f} MB in {len(new)} new object(s); "
            f"{month} so far {this_month / 1e6:.0f} MB; stored {total / GB:.2f} GB"
        )
    ]
    ok = True
    if adding > budget["pushMaxBytes"]:
        ok = False
        msgs.append(f"over the per-push limit of {budget['pushMaxBytes'] / 1e6:.0f} MB")
    if this_month + adding > budget["monthMaxBytes"]:
        ok = False
        msgs.append(
            f"takes {month} past {budget['monthMaxBytes'] / 1e6:.0f} MB of new LFS data"
        )
    if total + adding > budget["storageMaxBytes"]:
        ok = False
        msgs.append(
            f"takes storage past {budget['storageMaxBytes'] / GB:.0f} GB (GitHub Free quota is 10 GiB)"
        )
    elif total + adding > budget["storageWarnBytes"]:
        msgs.append(
            f"warning: storage past {budget['storageWarnBytes'] / GB:.0f} GB; plan a cleanup or paid data pack"
        )
    if not ok:
        if os.environ.get("CAD_BUDGET_OK") == "1":
            msgs.append("CAD_BUDGET_OK=1 set: pushing anyway")
            ok = True
        else:
            msgs.append(
                "Commit CAD at gates, not every save; squash interim saves before pushing. "
                "With Joey's OK, push again with CAD_BUDGET_OK=1."
            )
    return ok, msgs


def main(argv: list[str]) -> int:
    if argv == ["report"]:
        return report()
    if len(argv) >= 2 and argv[0] == "push":
        args = argv[1:]
        remote = "origin"
        if args[:1] == ["--remote"] and len(args) >= 3:
            remote, args = args[1], args[2:]
        ok, msgs = check_push(args, remote=remote)
        print("\n".join(msgs))
        return 0 if ok else 1
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
