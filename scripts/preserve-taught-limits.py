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
import copy
import os
import re
import shutil
import subprocess
import sys
import time
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


def _patch_keyed_float(text: str, key: str, value: float) -> tuple[str, bool]:
    """Replace the first `key: <number>` occurrence in a YAML snippet.

    Returns the text and whether the key was found and replaced.
    """
    pattern = re.compile(
        rf"^([ \t]*{re.escape(key)}:[ \t]*)({_NUM})([ \t]*(?:#.*)?)?$",
        re.MULTILINE,
    )
    patched, count = pattern.subn(rf"\g<1>{value}\g<3>", text, count=1)
    return patched, count == 1


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
    """Surgically restore bench hard limits for one joint; keep comments/layout.

    None when the joint or either bound is missing from the installed text.
    """
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
    patched, has_lower = _patch_keyed_float(block, "position_lower_rad", lower)
    patched, has_upper = _patch_keyed_float(patched, "position_upper_rad", upper)
    if not (has_lower and has_upper):
        return None
    return text[:start] + patched + text[end:]


def patch_control_soft_text(
    text: str, joint: str, soft_lower: float, soft_upper: float
) -> str | None:
    """Surgically restore soft limits for one joint under control.joints.

    None when the joint or either bound is missing from the installed text.
    """
    joint_re = re.compile(
        rf"(^[ \t]*{re.escape(joint)}:[ \t]*(?:#.*)?\n)"
        rf"(.*?)(?=^[ \t]{{4}}[A-Za-z_][A-Za-z0-9_]*:|\Z)",
        re.MULTILINE | re.DOTALL,
    )
    match = joint_re.search(text)
    if not match:
        return None
    block = match.group(0)
    patched, has_lower = _patch_keyed_float(block, "position_soft_lower_rad", soft_lower)
    patched, has_upper = _patch_keyed_float(patched, "position_soft_upper_rad", soft_upper)
    if not (has_lower and has_upper):
        return None
    return text[: match.start()] + patched + text[match.end() :]


def _urdf_limits(root: ET.Element) -> dict[str, ET.Element]:
    out: dict[str, ET.Element] = {}
    for joint in root.findall("joint"):
        name = joint.get("name")
        lim = joint.find("limit")
        if name and lim is not None:
            out[name] = lim
    return out


_XML_TAG = re.compile(r"<(/?)([A-Za-z_][\w:.-]*)\b[^>]*?(/?)>")


def _top_level_joint_body(masked: str, joint: str) -> tuple[int, int] | None:
    """(body start, body end) of the `<joint name=joint>` that is a direct child of `<robot>`.

    Same joints `_urdf_limits` reads (`root.findall("joint")`): a `<joint>` nested in a
    `<transmission>` or `<gazebo>` block is skipped. `<?...?>` and `<!...>` never match the tag
    pattern, so they do not change the depth.
    """
    name = re.compile(rf"\bname\s*=\s*([\"']){re.escape(joint)}\1")
    depth = 0
    body_start: int | None = None
    for tag in _XML_TAG.finditer(masked):
        closing, kind, self_closing = tag.groups()
        if closing:
            depth -= 1
            if body_start is not None and depth == 1 and kind == "joint":
                return body_start, tag.start()
            continue
        if body_start is None and depth == 1 and kind == "joint" and name.search(tag.group(0)):
            if self_closing:
                return None
            body_start = tag.end()
        if not self_closing:
            depth += 1
    return None


def patch_urdf_limit_text(text: str, joint: str, lower: float, upper: float) -> str | None:
    """Surgically set one joint's URDF `<limit lower upper>`; keep comments/layout.

    None when the joint, its `<limit>` or either attribute is missing from the text.
    """
    # Search with comments blanked (same offsets) so a commented-out tag never matches.
    masked = re.sub(r"<!--.*?-->", lambda m: " " * len(m.group(0)), text, flags=re.DOTALL)
    body = _top_level_joint_body(masked, joint)
    if body is None:
        return None
    body_start, body_end = body
    limit = re.search(r"<limit\b[^>]*>", masked[body_start:body_end])
    if not limit:
        return None
    start = body_start + limit.start()
    end = body_start + limit.end()
    tag = text[start:end]
    for key, value in (("lower", lower), ("upper", upper)):
        tag, count = re.subn(
            rf"(\b{key}\s*=\s*)([\"'])[^\"']*\2",
            lambda m, v=value: f"{m.group(1)}{m.group(2)}{v:.6g}{m.group(2)}",
            tag,
            count=1,
        )
        if count != 1:
            return None
    return text[:start] + tag + text[end:]


def preserve_urdf_expand_only(previous_urdf: Path, installed_urdf: Path) -> list[str]:
    """Union previous + installed URDF hard (expand-only; never shrink).

    Only the widened `lower`/`upper` attributes change: the rest of the installed file,
    comments and XML declaration included, stays byte-for-byte as git has it.
    """
    prev_lims = _urdf_limits(ET.parse(previous_urdf).getroot())
    inst_lims = _urdf_limits(ET.parse(installed_urdf).getroot())
    text = installed_urdf.read_text(encoding="utf-8")
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
            patched = patch_urdf_limit_text(text, name, new_lo, new_hi)
            if patched is None:
                print(
                    f"preserve-taught-limits: warning: could not expand-merge URDF hard for "
                    f"{name}; git's values stand",
                    file=sys.stderr,
                )
                continue
            text = patched
            widened.append(name)
    if widened:
        installed_urdf.write_text(text, encoding="utf-8")
    return widened


URDF_PATH = "assets/urdf/marengo.urdf"
_REV = re.compile(r"[0-9a-f]{7,40}")


def _urdf_without_limit_bounds(text: str) -> str:
    """Canonical URDF text with `<limit>` lower/upper removed (whitespace ignored)."""
    root = ET.fromstring(text)
    for lim in root.iter("limit"):
        lim.attrib.pop("lower", None)
        lim.attrib.pop("upper", None)
    return ET.canonicalize(ET.tostring(root, encoding="unicode"), strip_text=True)


def deployed_urdf_text(repo: Path | None, previous_root: Path) -> str | None:
    """The URDF of the previously deployed revision, or None when unknown.

    `previous_root/.deploy-rev` is the backed-up `/opt/marengo/.deploy-rev`
    ("<sha> <timestamp>"); the URDF is read from `repo` (the staging checkout)
    at that commit. A missing file, a malformed or ambiguous revision, a
    commit absent from the checkout or no URDF at that commit are all unknown.
    """
    if repo is None:
        return None
    try:
        fields = (previous_root / ".deploy-rev").read_text(encoding="utf-8").split()
    except OSError:
        return None
    if not fields or not _REV.fullmatch(fields[0]):
        return None
    # The checkout belongs to the deploy user while install runs as root.
    git = ["git", "-c", f"safe.directory={repo}", "-C", str(repo)]
    try:
        commit = subprocess.run(
            [*git, "rev-parse", "--verify", "--quiet", f"{fields[0]}^{{commit}}"],
            capture_output=True, text=True, check=False,
        )
        if commit.returncode != 0:
            return None
        shown = subprocess.run(
            [*git, "show", f"{commit.stdout.strip()}:{URDF_PATH}"],
            capture_output=True, text=True, check=False,
        )
    except OSError:
        return None
    return shown.stdout if shown.returncode == 0 else None


def keep_replaced_urdf(
    previous_urdf: Path,
    installed_urdf: Path,
    install_root: Path,
    deployed_urdf: str | None = None,
) -> Path | None:
    """Keep and name a live URDF this deploy replaced beyond its limits (EDGE-03).

    Only `<limit>` bounds survive a deploy (the union below). A URDF activated
    in the Hardware UI (new inertials, origins) is otherwise silently replaced
    by git's. `deployed_urdf` is the URDF of the previously deployed revision:
    when the Pi's copy still matches it, the difference is git's own change
    since that deploy and nothing is kept. When it is unknown the Pi's copy is
    kept with a neutral note, since a git change and an activation look alike.
    """
    pi = _urdf_without_limit_bounds(previous_urdf.read_text(encoding="utf-8"))
    if pi == _urdf_without_limit_bounds(installed_urdf.read_text(encoding="utf-8")):
        return None
    deployed = None
    if deployed_urdf is not None:
        try:
            deployed = _urdf_without_limit_bounds(deployed_urdf)
        except ET.ParseError:
            deployed = None
    if deployed is not None and deployed == pi:
        print("preserve-taught-limits: git changed the URDF since the last deploy; installed git's")
        return None
    keep_dir = install_root / "var" / "replaced-urdf"
    keep_dir.mkdir(parents=True, exist_ok=True)
    kept = keep_dir / f"{previous_urdf.name}.{time.strftime('%Y%m%dT%H%M%SZ', time.gmtime())}"
    shutil.copy2(previous_urdf, kept)
    if deployed is not None:
        print(
            "preserve-taught-limits: WARNING: the Pi's URDF differed from the last "
            "deployed revision beyond <limit> bounds (for example a URDF activated in "
            "the Hardware UI). This deploy installed git's URDF; the Pi's copy is kept "
            f"at {kept}. Commit its changes to assets/urdf/ to keep them.",
            file=sys.stderr,
        )
    else:
        print(
            "preserve-taught-limits: note: the Pi's URDF differed from git beyond "
            "<limit> bounds and the previously deployed revision is unknown, so this "
            "may be git's own change or a URDF activated on the Pi. This deploy "
            f"installed git's URDF; the Pi's previous copy is kept at {kept}. Diff it "
            "against assets/urdf/marengo.urdf before acting on it.",
            file=sys.stderr,
        )
    return kept


def _narrowed(
    kind: str, joint: str, git: tuple[float, float], pi: tuple[float, float]
) -> str | None:
    """Describe a git range that is tighter than the Pi's on either side."""
    if git[0] > pi[0] or git[1] < pi[1]:
        return (
            f"{kind} {joint}: git [{git[0]:g}, {git[1]:g}] is tighter than the Pi's "
            f"[{pi[0]:g}, {pi[1]:g}]; the Pi keeps its wider range"
        )
    return None


def _bounds(entry: dict[str, Any], lower: str, upper: str) -> tuple[float, float] | None:
    lo, hi = entry.get(lower), entry.get(upper)
    if isinstance(lo, (int, float)) and isinstance(hi, (int, float)):
        return float(lo), float(hi)
    return None


def _urdf_bounds(path: Path) -> dict[str, tuple[float, float]]:
    out: dict[str, tuple[float, float]] = {}
    for name, lim in _urdf_limits(ET.parse(path).getroot()).items():
        try:
            out[name] = (float(lim.get("lower", "0")), float(lim.get("upper", "0")))
        except ValueError:
            continue
    return out


def report_narrowed(narrowed: list[str]) -> None:
    """Loudly list git limits this deploy did not apply (EDGE-02)."""
    if not narrowed:
        return
    print(
        "preserve-taught-limits: WARNING: git narrows limits that this deploy did "
        "NOT apply:",
        file=sys.stderr,
    )
    for line in narrowed:
        print(f"  {line}", file=sys.stderr)
    print(
        "  To apply git's limits, redeploy with MARENGO_REPLACE_LIMITS=1 "
        "(this also drops limits taught on the Pi).",
        file=sys.stderr,
    )


def run_preserve(previous_root: Path, install_root: Path, repo: Path | None = None) -> int:
    """`repo` is the staging checkout, used to read the previously deployed URDF."""
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
    narrowed: list[str] = []
    # Restored = actually written into the installed text, never just overlapping.
    hard_joints: list[str] = []
    prev_by_joint = _motors_by_joint(prev_motors)
    # preserve_motors_hard overwrites inst_motors in memory: keep git's copy.
    git_by_joint = copy.deepcopy(_motors_by_joint(inst_motors))
    motors_text = motors_inst.read_text(encoding="utf-8")
    for joint in preserve_motors_hard(prev_motors, inst_motors):
        pi = _bounds(
            prev_by_joint[joint].get("bench") or {}, "position_lower_rad", "position_upper_rad"
        )
        if pi is None:
            continue
        patched = patch_motors_hard_text(motors_text, joint, pi[0], pi[1])
        if patched is None:
            print(
                f"preserve-taught-limits: warning: could not restore motors hard for {joint}; "
                "git's values stand",
                file=sys.stderr,
            )
            continue
        motors_text = patched
        hard_joints.append(joint)
        git = _bounds(
            git_by_joint[joint].get("bench") or {}, "position_lower_rad", "position_upper_rad"
        )
        if git is not None:
            line = _narrowed("motors hard", joint, git, pi)
            if line:
                narrowed.append(line)
    if hard_joints:
        motors_inst.write_text(motors_text, encoding="utf-8")
        print(
            "preserve-taught-limits: restored motors hard for "
            + ", ".join(hard_joints)
        )

    soft_joints: list[str] = []
    if control_prev.is_file() and control_inst.is_file():
        prev_control = _load_yaml(control_prev)
        inst_control = _load_yaml(control_inst)
        prev_soft = ((prev_control.get("control") or {}).get("joints")) or {}
        git_soft = ((inst_control.get("control") or {}).get("joints")) or {}
        git_soft = copy.deepcopy(git_soft)
        control_text = control_inst.read_text(encoding="utf-8")
        for joint in preserve_control_soft(prev_control, inst_control):
            keys = ("position_soft_lower_rad", "position_soft_upper_rad")
            pi = _bounds(prev_soft[joint], *keys)
            if pi is None:
                continue
            patched = patch_control_soft_text(control_text, joint, pi[0], pi[1])
            if patched is None:
                print(
                    f"preserve-taught-limits: warning: could not restore control soft for "
                    f"{joint}; git's values stand",
                    file=sys.stderr,
                )
                continue
            control_text = patched
            soft_joints.append(joint)
            git = _bounds(git_soft.get(joint, {}), *keys)
            if git is not None:
                line = _narrowed("control soft", joint, git, pi)
                if line:
                    narrowed.append(line)
        if soft_joints:
            control_inst.write_text(control_text, encoding="utf-8")
            print(
                "preserve-taught-limits: restored control soft for "
                + ", ".join(soft_joints)
            )

    urdf_joints: list[str] = []
    if urdf_prev.is_file() and urdf_inst.is_file():
        keep_replaced_urdf(
            urdf_prev, urdf_inst, install_root, deployed_urdf_text(repo, previous_root)
        )
        git_urdf = _urdf_bounds(urdf_inst)
        pi_urdf = _urdf_bounds(urdf_prev)
        urdf_joints = preserve_urdf_expand_only(urdf_prev, urdf_inst)
        for joint in urdf_joints:
            if joint in git_urdf and joint in pi_urdf:
                line = _narrowed("URDF hard", joint, git_urdf[joint], pi_urdf[joint])
                if line:
                    narrowed.append(line)
        if urdf_joints:
            print(
                "preserve-taught-limits: expand-merged URDF hard for "
                + ", ".join(urdf_joints)
            )

    report_narrowed(narrowed)
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
        help="Backup tree with previous config/, assets/urdf/ and .deploy-rev",
    )
    parser.add_argument(
        "--install-root",
        type=Path,
        required=True,
        help="Newly installed /opt/marengo (or staging) root",
    )
    parser.add_argument(
        "--repo",
        type=Path,
        default=Path(__file__).resolve().parent.parent,
        help="Git checkout holding the previously deployed revision (default: this bundle)",
    )
    args = parser.parse_args(argv)
    return run_preserve(args.previous, args.install_root, args.repo)


if __name__ == "__main__":
    sys.exit(main())
