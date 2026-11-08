#!/usr/bin/env python3
"""pre-push: no pushed file of 100 MB or more, and the LFS budget holds.

Called by the shell hook from tools/cad/install_hooks.py with git's pre-push arguments
(remote name, URL) and stdin (one line per ref). The shell hook then runs git lfs pre-push.
"""

from __future__ import annotations

import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent))
sys.path.insert(0, str(HERE.parent.parent))
import check_file_sizes
import lfs_budget
from gitlfs import pre_push_rev_args


def main(argv: list[str]) -> int:
    remote = argv[0] if argv else "origin"
    failed = False
    for rev_args in pre_push_rev_args(sys.stdin.read(), remote):
        failed |= (
            check_file_sizes.report(
                check_file_sizes.pushed(rev_args), f"pushed ({' '.join(rev_args)})"
            )
            != 0
        )
        ok, msgs = lfs_budget.check_push(rev_args)
        print("\n".join(msgs))
        failed |= not ok
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
