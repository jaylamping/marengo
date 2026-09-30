#!/usr/bin/env python3
"""Unit tests for preserve-taught-limits.py (stdlib unittest)."""

from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path

import yaml

_SCRIPT = Path(__file__).resolve().parent / "preserve-taught-limits.py"
_SPEC = importlib.util.spec_from_file_location("preserve_taught_limits", _SCRIPT)
assert _SPEC and _SPEC.loader
preserve_taught_limits = importlib.util.module_from_spec(_SPEC)
sys.modules["preserve_taught_limits"] = preserve_taught_limits
_SPEC.loader.exec_module(preserve_taught_limits)

preserve_control_soft = preserve_taught_limits.preserve_control_soft
preserve_motors_hard = preserve_taught_limits.preserve_motors_hard
preserve_urdf_expand_only = preserve_taught_limits.preserve_urdf_expand_only
run_preserve = preserve_taught_limits.run_preserve


class PreserveTaughtLimitsTests(unittest.TestCase):
    def test_motors_hard_restored_for_overlap_only(self) -> None:
        previous = {
            "motors": [
                {
                    "joint": "right_shoulder_roll",
                    "bench": {
                        "position_lower_rad": -0.1,
                        "position_upper_rad": 0.25,
                    },
                },
                {
                    "joint": "gone_joint",
                    "bench": {
                        "position_lower_rad": -1.0,
                        "position_upper_rad": 1.0,
                    },
                },
            ]
        }
        installed = {
            "motors": [
                {
                    "joint": "right_shoulder_roll",
                    "bench": {
                        "position_lower_rad": -0.05,
                        "position_upper_rad": 2.5,
                        "torque_limit_nm": 5.0,
                    },
                },
                {
                    "joint": "right_lower_arm_yaw",
                    "bench": {
                        "position_lower_rad": -1.5,
                        "position_upper_rad": 1.5,
                    },
                },
            ]
        }
        restored = preserve_motors_hard(previous, installed)
        self.assertEqual(restored, ["right_shoulder_roll"])
        roll = installed["motors"][0]["bench"]
        self.assertEqual(roll["position_lower_rad"], -0.1)
        self.assertEqual(roll["position_upper_rad"], 0.25)
        self.assertEqual(roll["torque_limit_nm"], 5.0)
        yaw = installed["motors"][1]["bench"]
        self.assertEqual(yaw["position_upper_rad"], 1.5)

    def test_control_soft_restored(self) -> None:
        previous = {
            "control": {
                "joints": {
                    "right_shoulder_roll": {
                        "position_soft_lower_rad": -0.07,
                        "position_soft_upper_rad": 0.22,
                    }
                }
            }
        }
        installed = {
            "control": {
                "joints": {
                    "right_shoulder_roll": {
                        "position_soft_lower_rad": -0.02,
                        "position_soft_upper_rad": 2.4,
                        "impedance": {"kp": 18.0},
                    }
                }
            }
        }
        restored = preserve_control_soft(previous, installed)
        self.assertEqual(restored, ["right_shoulder_roll"])
        entry = installed["control"]["joints"]["right_shoulder_roll"]
        self.assertEqual(entry["position_soft_lower_rad"], -0.07)
        self.assertEqual(entry["position_soft_upper_rad"], 0.22)
        self.assertEqual(entry["impedance"]["kp"], 18.0)

    def test_urdf_expand_only_union(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            prev = root / "prev.urdf"
            inst = root / "inst.urdf"
            prev.write_text(
                """<?xml version="1.0"?>
<robot name="t">
  <joint name="right_shoulder_roll" type="revolute">
    <limit lower="-0.1" upper="3.14" effort="60" velocity="2"/>
  </joint>
</robot>
""",
                encoding="utf-8",
            )
            inst.write_text(
                """<?xml version="1.0"?>
<robot name="t">
  <joint name="right_shoulder_roll" type="revolute">
    <limit lower="-0.05" upper="2.5" effort="60" velocity="2"/>
  </joint>
</robot>
""",
                encoding="utf-8",
            )
            widened = preserve_urdf_expand_only(prev, inst)
            self.assertEqual(widened, ["right_shoulder_roll"])
            lim = ET.parse(inst).getroot().find("joint/limit")
            assert lim is not None
            self.assertAlmostEqual(float(lim.get("lower")), -0.1)
            self.assertAlmostEqual(float(lim.get("upper")), 3.14)

    def test_patch_motors_keeps_comments(self) -> None:
        text = """# header
motors:
  - joint: right_shoulder_roll
    # keep me
    bench:
      position_lower_rad: -0.05
      position_upper_rad: 2.5
      torque_limit_nm: 5.0
"""
        patched = preserve_taught_limits.patch_motors_hard_text(
            text, "right_shoulder_roll", -0.099, 0.248
        )
        assert patched is not None
        self.assertIn("# header", patched)
        self.assertIn("# keep me", patched)
        self.assertIn("position_lower_rad: -0.099", patched)
        self.assertIn("position_upper_rad: 0.248", patched)
        self.assertIn("torque_limit_nm: 5.0", patched)

    def test_run_preserve_end_to_end(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            prev = root / "previous"
            inst = root / "install"
            for base in (prev, inst):
                (base / "config").mkdir(parents=True)
                (base / "assets" / "urdf").mkdir(parents=True)
            (prev / "config" / "motors.yaml").write_text(
                yaml.safe_dump(
                    {
                        "motors": [
                            {
                                "joint": "right_shoulder_roll",
                                "bench": {
                                    "position_lower_rad": -0.099,
                                    "position_upper_rad": 0.248,
                                },
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            (inst / "config" / "motors.yaml").write_text(
                yaml.safe_dump(
                    {
                        "motors": [
                            {
                                "joint": "right_shoulder_roll",
                                "bench": {
                                    "position_lower_rad": -0.05,
                                    "position_upper_rad": 2.58,
                                },
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            (prev / "config" / "control.yaml").write_text(
                yaml.safe_dump(
                    {
                        "control": {
                            "joints": {
                                "right_shoulder_roll": {
                                    "position_soft_lower_rad": -0.072,
                                    "position_soft_upper_rad": 0.221,
                                }
                            }
                        }
                    }
                ),
                encoding="utf-8",
            )
            (inst / "config" / "control.yaml").write_text(
                yaml.safe_dump(
                    {
                        "control": {
                            "joints": {
                                "right_shoulder_roll": {
                                    "position_soft_lower_rad": -0.02,
                                    "position_soft_upper_rad": 2.5,
                                }
                            }
                        }
                    }
                ),
                encoding="utf-8",
            )
            urdf = """<?xml version="1.0"?>
<robot name="t">
  <joint name="right_shoulder_roll" type="revolute">
    <limit lower="{lo}" upper="{hi}" effort="60" velocity="2"/>
  </joint>
</robot>
"""
            (prev / "assets" / "urdf" / "marengo.urdf").write_text(
                urdf.format(lo="-0.099415", hi="3.14159"), encoding="utf-8"
            )
            (inst / "assets" / "urdf" / "marengo.urdf").write_text(
                urdf.format(lo="-0.05", hi="2.5"), encoding="utf-8"
            )

            self.assertEqual(run_preserve(prev, inst), 0)
            motors = yaml.safe_load(
                (inst / "config" / "motors.yaml").read_text(encoding="utf-8")
            )
            bench = motors["motors"][0]["bench"]
            self.assertEqual(bench["position_lower_rad"], -0.099)
            self.assertEqual(bench["position_upper_rad"], 0.248)
            control = yaml.safe_load(
                (inst / "config" / "control.yaml").read_text(encoding="utf-8")
            )
            soft = control["control"]["joints"]["right_shoulder_roll"]
            self.assertEqual(soft["position_soft_lower_rad"], -0.072)
            self.assertEqual(soft["position_soft_upper_rad"], 0.221)
            lim = (
                ET.parse(inst / "assets" / "urdf" / "marengo.urdf")
                .getroot()
                .find("joint/limit")
            )
            assert lim is not None
            self.assertAlmostEqual(float(lim.get("lower")), -0.099415)
            self.assertAlmostEqual(float(lim.get("upper")), 3.14159)


if __name__ == "__main__":
    unittest.main()
