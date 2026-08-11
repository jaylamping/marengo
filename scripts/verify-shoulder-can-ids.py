#!/usr/bin/env python3
"""Verify right-bench shoulder CAN IDs: pitch=1, roll=2.

Rerun after any motors.yaml / inventory / docs remapping. Exit 0 only when
the master SoT and the critical joint↔id assertions match hardware.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

# (path, must_match_patterns, must_not_match_patterns)
CHECKS: list[tuple[Path, list[str], list[str]]] = [
    (
        ROOT / "config" / "motors.yaml",
        [
            r"^  - joint:\s*right_shoulder_pitch\b(?:(?!^  - joint:)[\s\S])*?^\s+device_id:\s*1\b",
            r"^  - joint:\s*right_shoulder_pitch\b(?:(?!^  - joint:)[\s\S])*?^\s+recv_can_id:\s*0x241\b",
            r"^  - joint:\s*right_shoulder_roll\b(?:(?!^  - joint:)[\s\S])*?^\s+device_id:\s*2\b",
            r"^  - joint:\s*right_shoulder_roll\b(?:(?!^  - joint:)[\s\S])*?^\s+recv_can_id:\s*0x242\b",
        ],
        [
            r"^  - joint:\s*right_shoulder_roll\b(?:(?!^  - joint:)[\s\S])*?^\s+device_id:\s*1\b",
            r"^  - joint:\s*right_shoulder_pitch\b(?:(?!^  - joint:)[\s\S])*?^\s+device_id:\s*2\b",
        ],
    ),
    (
        ROOT / "consul" / "src" / "data" / "robot-inventory.ts",
        [
            r"right_shoulder_roll'[^)]*'rs03',\s*2\)",
            r"right_shoulder_pitch'[^)]*'rs03',\s*1\)",
        ],
        [
            r"right_shoulder_roll'[^)]*'rs03',\s*1\)",
            r"right_shoulder_pitch'[^)]*'rs03',\s*2\)",
        ],
    ),
    (
        ROOT / "AGENTS.md",
        [r"pitch CAN id 1,.*roll CAN id 2"],
        [r"roll CAN id 1,.*pitch CAN id 2"],
    ),
]


def main() -> int:
    failures: list[str] = []
    for path, must, must_not in CHECKS:
        if not path.is_file():
            failures.append(f"missing file: {path}")
            continue
        text = path.read_text(encoding="utf-8")
        for pat in must:
            if not re.search(pat, text, re.MULTILINE):
                failures.append(f"{path.relative_to(ROOT)}: missing /{pat}/")
        for pat in must_not:
            if re.search(pat, text, re.MULTILINE):
                failures.append(f"{path.relative_to(ROOT)}: stale /{pat}/")

    motors = (ROOT / "config" / "motors.yaml").read_text(encoding="utf-8")
    # Direction stays with the joint, not the CAN id.
    if not re.search(
        r"^  - joint:\s*right_shoulder_pitch\b(?:(?!^  - joint:)[\s\S])*?^\s+direction:\s*-1\b",
        motors,
        re.MULTILINE,
    ):
        failures.append("config/motors.yaml: pitch direction must remain -1")
    if not re.search(
        r"^  - joint:\s*right_shoulder_roll\b(?:(?!^  - joint:)[\s\S])*?^\s+direction:\s*1\b",
        motors,
        re.MULTILINE,
    ):
        failures.append("config/motors.yaml: roll direction must remain 1")

    if failures:
        print("shoulder CAN id verify FAILED:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("shoulder CAN id verify OK: pitch=1 (0x241), roll=2 (0x242)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
