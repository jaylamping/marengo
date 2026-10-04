#!/usr/bin/env python3
"""Rank cargo test binaries by run time from a captured `cargo test` log.

  cargo test --workspace 2>&1 | tee /tmp/test-run.log
  python3 rank_test_binaries.py /tmp/test-run.log [--top 25]

cargo runs test binaries one after another, so the total run time is the sum of these
rows and one slow binary delays everything behind it. Each row: seconds, test count,
binary (unittests/integration file/doc-tests). Compile and link time is not in the log;
measure it separately with `cargo test --workspace --no-run`.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

PREFIX = r"^\s*(?:\d+\.\d+\s+)?\s*"  # optional elapsed-seconds prefix from the timing recipe
RUNNING = re.compile(PREFIX + r"Running (?P<what>.+?)(?: \((?P<path>[^)]+)\))?\s*$")
STAMP = re.compile(r"^\s*(\d+\.\d+)\s")
DOCTESTS = re.compile(PREFIX + r"Doc-tests (?P<crate>\S+)")
RESULT = re.compile(
    r"test result: (?P<status>\w+)\. (?P<passed>\d+) passed; (?P<failed>\d+) failed; "
    r"(?P<ignored>\d+) ignored;.*finished in (?P<secs>[\d.]+)s"
)


def crate_of(path: str | None) -> str:
    if not path:
        return "?"
    name = Path(path).name
    return re.sub(r"-[0-9a-f]{16}$", "", name)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("log", type=Path)
    ap.add_argument("--top", type=int, default=25)
    args = ap.parse_args()

    rows: list[tuple[float, int, str]] = []
    overhead: list[tuple[float, str]] = []
    current: str | None = None
    started: float | None = None
    for line in args.log.read_text(errors="replace").splitlines():
        stamp = STAMP.match(line)
        now = float(stamp[1]) if stamp else None
        if m := RUNNING.match(line):
            current = f"{crate_of(m['path'])} :: {m['what']}"
            started = now
        elif m := DOCTESTS.match(line):
            current = f"{m['crate']} :: doc-tests"
            started = now
        elif (m := RESULT.search(line)) and current:
            tests = int(m["passed"]) + int(m["failed"])
            rows.append((float(m["secs"]), tests, current))
            if started is not None and now is not None and not current.endswith("doc-tests"):
                overhead.append((now - started - float(m["secs"]), current))
            current = None

    if not rows:
        sys.exit("no `test result:` lines found; capture stdout and stderr (2>&1)")
    total = sum(r[0] for r in rows)
    print(f"{len(rows)} binaries, {sum(r[1] for r in rows)} tests, {total:.1f} s of test execution (sequential)")
    print(f"{'secs':>8} {'share':>6} {'tests':>6}  binary")
    for secs, tests, name in sorted(rows, reverse=True)[: args.top]:
        print(f"{secs:8.2f} {secs / total:6.1%} {tests:6d}  {name}")
    if overhead:
        spent = sum(o[0] for o in overhead)
        print(f"\nlaunch overhead (wall time outside test execution, timestamped log): {spent:.1f} s "
              f"over {len(overhead)} binaries, mean {spent / len(overhead):.2f} s")
        for secs, name in sorted(overhead, reverse=True)[:5]:
            print(f"{secs:8.2f}  {name}")


if __name__ == "__main__":
    main()
