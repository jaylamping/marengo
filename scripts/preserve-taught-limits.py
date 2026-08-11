#!/usr/bin/env python3
"""Preserve Set Limits taught envelopes across install-pi rsync.

After Consul Set Limits Apply, Pi `/opt/marengo/config/{motors,control}.yaml`
and expand-only URDF hard bounds are the durable SoT (ADR 0012 / 0017).
`install-pi.sh` rsyncs the git checkout over those files; without a merge step,
the next deploy clobbers taught hard/soft and can shrink effective Davout hard.

Default: for each joint present in both previous and newly installed trees,
restore previous motors bench hard + control soft, and take the expand-only
union of URDF `<limit>` hard. Opt out with MARENGO_REPLACE_LIMITS=1.
"""

from __future__ import annotations

import argparse
import os
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

try:
    import yaml
except ImportError as exc:  # pragma: no cover
    print(f"preserve-taught-limits: PyYAML required: {exc}", file=sys.stderr)
    sys.exit(2)


def _load_yaml(path: Path) -> Any:
    with path.open(encoding="utf-8") as fh:
        return yaml.safe_load(fh)


def _motors_by_joint(doc: dict[str, Any]) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for entry in doc.get("motors") or []:
        joint = entry.get("joint")
        if isinstance(joint, str) and joint:
            out[joint] = entry
    return out


def preserve_motors_hard(
    previous: dict[str, Any], installed: dict[str, Any]
) -> list[str]:
    """Copy previous bench hard limits onto matching installed motors (in-memory)."""
    prev = _motors_by_joint(previous)
    restored: list[str] = []
    for entry in installed.get("motors") or []:
        joint = entry.get("joint")
        if not isinstance(joint, str) or joint not in prev:
            continue
        prev_bench = prev[joint].get("bench") or {}
        bench = entry.setdefault("bench", {})
        if "position_lower_rad" in prev_bench:
            bench["position_lower_rad"] = prev_bench["position_lower_rad"]
        if "position_upper_rad" in prev_bench:
            bench["position_upper_rad"] = prev_bench["position_upper_rad"]
        restored.append(joint)
    return restored


def preserve_control_soft(
    previous: dict[str, Any], installed: dict[str, Any]
) -> list[str]:
    """Copy previous soft bounds onto matching installed control joints (in-memory)."""
    prev_joints = ((previous.get("control") or {}).get("joints")) or {}
    inst_joints = ((installed.get("control") or {}).get("joints")) or {}
    restored: list[str] = []
    for joint, prev_entry in prev_joints.items():
        if joint not in inst_joints:
            continue
        inst_entry = inst_joints[joint]
        changed = False
        for key in ("position_soft_lower_rad", "position_soft_upper_rad"):
            if key in prev_entry and prev_entry[key] is not None:
                inst_entry[key] = prev_entry[key]
                changed = True
        if changed:
            restored.append(joint)
    return restored


_NUM = r"-?(?:\d+\.?\d*|\.\d+)(?:[eE][+-]?\d+)?"


def _patch_keyed_float(text: str, key: str, value: float) -> str:
    """Replace the first `key: <number>` occurrence in a YAML snippet."""
    pattern = re.compile(
        rf"^([ \t]*{re.escape(key)}:[ \t]*)({_NUM})([ \t]*(?:#.*)?)?$",
        re.MULTILINE,
    )
    return pattern.sub(rf"\g<1>{value}\g<3>", text, count=1)


def _list_item_span(text: str, anchor: int) -> tuple[int, int] | None:
    """Return [start, end) of the YAML list item (`- …`) containing offset `anchor`."""
    line_start = text.rfind("\n", 0, anchor) + 1
    item_start = None
    idx = line_start
    while idx >= 0:
        ls = text.rfind("\n", 0, idx) + 1 if idx > 0 else 0
        line = text[ls: text.find("\n", ls) if text.find("\n", ls) != -1 else len(text)]
        if re.match(r"^[ \t]*-[ \t]", line):
            item_start = ls
            break
        if ls == 0:
            break
        idx = ls - 1
    if item_start is None:
        return None
    # End at next list item with same-or-less indent, or EOF.
    item_line = text[item_start : text.find("\n", item_start)]
    indent_match = re.match(r"^([ \t]*)-", item_line)
    indent = indent_match.group(1) if indent_match else ""
    next_item = re.search(
        rf"\n{re.escape(indent)}-[ \t]",
        text[item_start + 1 :],
    )
    item_end = (
        item_start + 1 + next_item.start()
        if next_item
        else len(text)
    )
    return item_start, item_end


def patch_motors_hard_text(
    text: str, joint: str, lower: float, upper: float
) -> str | None:
    """Surgically restore bench hard limits for one joint; keep comments/layout."""
    joint_line = re.search(
        rf"^[ \t]*(?:-[ \t]+)?joint:[ \t]*{re.escape(joint)}[ \t]*(?:#.*)?$",
        text,
        re.MULTILINE,
    )
    if not joint_line:
        return None
    span = _list_item_span(text, joint_line.start())
    if span is None:
        return None
    start, end = span
    block = text[start:end]
    patched = _patch_keyed_float(block, "position_lower_rad", lower)
    patched = _patch_keyed_float(patched, "position_upper_rad", upper)
    if patched == block:
        return text
    return text[:start] + patched + text[end:]


def patch_control_soft_text(
    text: str, joint: str, soft_lower: float, soft_upper: float
) -> str | None:
    """Surgically restore soft limits for one joint under control.joints."""
    joint_re = re.compile(
        rf"(^[ \t]*{re.escape(joint)}:[ \t]*(?:#.*)?\n)"
        rf"(.*?)(?=^[ \t]{{4}}[A-Za-z_][A-Za-z0-9_]*:|\Z)",
        re.MULTILINE | re.DOTALL,
    )
    match = joint_re.search(text)
    if not match:
        return None
    block = match.group(0)
    patched = _patch_keyed_float(block, "position_soft_lower_rad", soft_lower)
    patched = _patch_keyed_float(patched, "position_soft_upper_rad", soft_upper)
    if patched == block:
        return text
    return text[: match.start()] + patched + text[match.end() :]


def _urdf_limits(root: ET.Element) -> dict[str, ET.Element]:
    out: dict[str, ET.Element] = {}
    for joint in root.findall("joint"):
        name = joint.get("name")
        lim = joint.find("limit")
        if name and lim is not None:
            out[name] = lim
    return out


def preserve_urdf_expand_only(previous_urdf: Path, installed_urdf: Path) -> list[str]:
    """Union previous + installed URDF hard (expand-only; never shrink)."""
    prev_tree = ET.parse(previous_urdf)
    inst_tree = ET.parse(installed_urdf)
    prev_lims = _urdf_limits(prev_tree.getroot())
    inst_lims = _urdf_limits(inst_tree.getroot())
    widened: list[str] = []
    for name, inst_lim in inst_lims.items():
        prev_lim = prev_lims.get(name)
        if prev_lim is None:
            continue
        try:
            prev_lo = float(prev_lim.get("lower", "0"))
            prev_hi = float(prev_lim.get("upper", "0"))
            inst_lo = float(inst_lim.get("lower", "0"))
            inst_hi = float(inst_lim.get("upper", "0"))
        except ValueError:
            continue
        new_lo = min(prev_lo, inst_lo)
        new_hi = max(prev_hi, inst_hi)
        if new_lo != inst_lo or new_hi != inst_hi:
            inst_lim.set("lower", f"{new_lo:.6g}")
            inst_lim.set("upper", f"{new_hi:.6g}")
            widened.append(name)
    if widened:
        inst_tree.write(installed_urdf, encoding="unicode", xml_declaration=True)
    return widened


def run_preserve(previous_root: Path, install_root: Path) -> int:
    motors_prev = previous_root / "config" / "motors.yaml"
    control_prev = previous_root / "config" / "control.yaml"
    urdf_prev = previous_root / "assets" / "urdf" / "marengo.urdf"
    motors_inst = install_root / "config" / "motors.yaml"
    control_inst = install_root / "config" / "control.yaml"
    urdf_inst = install_root / "assets" / "urdf" / "marengo.urdf"

    if not motors_prev.is_file() or not motors_inst.is_file():
        print("preserve-taught-limits: skip (motors.yaml missing on previous or install)")
        return 0

    prev_motors = _load_yaml(motors_prev)
    inst_motors = _load_yaml(motors_inst)
    hard_joints = preserve_motors_hard(prev_motors, inst_motors)
    if hard_joints:
        motors_text = motors_inst.read_text(encoding="utf-8")
        prev_by_joint = _motors_by_joint(prev_motors)
        for joint in hard_joints:
            bench = prev_by_joint[joint].get("bench") or {}
            lower = bench.get("position_lower_rad")
            upper = bench.get("position_upper_rad")
            if not isinstance(lower, (int, float)) or not isinstance(upper, (int, float)):
                continue
            patched = patch_motors_hard_text(motors_text, joint, float(lower), float(upper))
            if patched is None:
                print(
                    f"preserve-taught-limits: warning: could not patch motors hard for {joint}",
                    file=sys.stderr,
                )
                continue
            motors_text = patched
        motors_inst.write_text(motors_text, encoding="utf-8")
        print(
            "preserve-taught-limits: restored motors hard for "
            + ", ".join(hard_joints)
        )

    soft_joints: list[str] = []
    if control_prev.is_file() and control_inst.is_file():
        prev_control = _load_yaml(control_prev)
        inst_control = _load_yaml(control_inst)
        soft_joints = preserve_control_soft(prev_control, inst_control)
        if soft_joints:
            control_text = control_inst.read_text(encoding="utf-8")
            prev_soft = ((prev_control.get("control") or {}).get("joints")) or {}
            for joint in soft_joints:
                entry = prev_soft[joint]
                soft_lo = entry.get("position_soft_lower_rad")
                soft_hi = entry.get("position_soft_upper_rad")
                if not isinstance(soft_lo, (int, float)) or not isinstance(
                    soft_hi, (int, float)
                ):
                    continue
                patched = patch_control_soft_text(
                    control_text, joint, float(soft_lo), float(soft_hi)
                )
                if patched is None:
                    print(
                        f"preserve-taught-limits: warning: could not patch control soft for {joint}",
                        file=sys.stderr,
                    )
                    continue
                control_text = patched
            control_inst.write_text(control_text, encoding="utf-8")
            print(
                "preserve-taught-limits: restored control soft for "
                + ", ".join(soft_joints)
            )

    urdf_joints: list[str] = []
    if urdf_prev.is_file() and urdf_inst.is_file():
        urdf_joints = preserve_urdf_expand_only(urdf_prev, urdf_inst)
        if urdf_joints:
            print(
                "preserve-taught-limits: expand-merged URDF hard for "
                + ", ".join(urdf_joints)
            )

    if not hard_joints and not soft_joints and not urdf_joints:
        print("preserve-taught-limits: no overlapping taught limits to restore")
    return 0


def main(argv: list[str] | None = None) -> int:
    if os.environ.get("MARENGO_REPLACE_LIMITS", "0") == "1":
        print("preserve-taught-limits: skipped (MARENGO_REPLACE_LIMITS=1)")
        return 0

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--previous",
        type=Path,
        required=True,
        help="Backup tree with previous config/ and assets/urdf/",
    )
    parser.add_argument(
        "--install-root",
        type=Path,
        required=True,
        help="Newly installed /opt/marengo (or staging) root",
    )
    args = parser.parse_args(argv)
    return run_preserve(args.previous, args.install_root)


if __name__ == "__main__":
    sys.exit(main())
