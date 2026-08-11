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

PathChecks = tuple[Path, list[str], list[str]]

FILE_CHECKS: list[PathChecks] = [
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

JOINT_DIRECTION: dict[str, int] = {
    "right_shoulder_pitch": -1,
    "right_shoulder_roll": 1,
}


def joint_field_pattern(joint: str, field: str, value: str) -> str:
    return (
        rf"^  - joint:\s*{re.escape(joint)}\b"
        rf"(?:(?!^  - joint:)[\s\S])*?^\s+{re.escape(field)}:\s*{re.escape(value)}\b"
    )


def check_joint_directions(motors_text: str) -> list[str]:
    failures: list[str] = []
    for joint, direction in JOINT_DIRECTION.items():
        if not re.search(
            joint_field_pattern(joint, "direction", str(direction)),
            motors_text,
            re.MULTILINE,
        ):
            failures.append(
                f"config/motors.yaml: {joint} direction must remain {direction}"
            )
    return failures


def main() -> int:
    failures: list[str] = []
    for path, must, must_not in FILE_CHECKS:
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
    failures.extend(check_joint_directions(motors))

    if failures:
        print("shoulder CAN id verify FAILED:")
        for f in failures:
            print(f"  - {f}")
        return 1
    print("shoulder CAN id verify OK: pitch=1 (0x241), roll=2 (0x242)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
