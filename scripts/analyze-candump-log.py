#!/usr/bin/env python3
"""Summarize a candump -t z log (frame count, duration, Hz per interface).

Usage:
  python scripts/analyze-candump-log.py /path/to/candump-latest.log
"""

from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

TS_RE = re.compile(r"^\(([0-9.]+)\)\s+(\S+)\s+")


def main() -> int:
    path = Path(sys.argv[1] if len(sys.argv) > 1 else "candump-latest.log")
    if not path.is_file():
        print(f"error: not found: {path}", file=sys.stderr)
        return 1

    times: list[float] = []
    ifaces: Counter[str] = Counter()
    for line in path.read_text(errors="replace").splitlines():
        m = TS_RE.match(line.strip())
        if not m:
            continue
        times.append(float(m.group(1)))
        ifaces[m.group(2)] += 1

    print(f"file: {path}")
    print(f"frames: {sum(ifaces.values())}")
    if not times:
        print("(no parsed frames)")
        return 0

    dur = times[-1] - times[0]
    print(f"duration_s: {dur:.3f}")
    if dur > 0:
        print(f"approx_hz_total: {sum(ifaces.values()) / dur:.1f}")
    for iface, count in sorted(ifaces.items()):
        hz = count / dur if dur > 0 else 0.0
        print(f"  {iface}: {count} frames ({hz:.1f} Hz)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
