#!/usr/bin/env python3
"""Generate left_arm.urdf from the SolidWorks skeleton export.

Inputs: cad_export.json (frames and mass properties read from the saved CAD)
and mass_overrides.json (kg, weighed or supplied; null uses density below).
Run with --check to validate the output.
"""
import argparse
import json
import xml.etree.ElementTree as ET
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
PRINT_DENSITY = 1190.0  # kg/m3, effective 100%-infill PETG print, from weighed P1 and P2 (see MEASURED_MASSES.md)

# STEP (X lateral, Y up, Z depth) to robot frame (REP-103: X forward, Y left, Z up).
STEP_TO_ROBOT = np.array([[0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]])

# Axis sign gives the same anatomical motion on both sides: +pitch raises forward,
# +roll abducts, +yaw rotates outward, +elbow flexes. The right arm negates roll and yaws.
JOINTS = [
    ("shoulder_pitch", "F_L_SHOULDER_PITCH", (0, -1, 0), (-0.87, 3.14), 60, 20),
    ("shoulder_roll", "F_L_SHOULDER_ROLL", (1, 0, 0), (-0.26, 3.14), 60, 20),
    ("upper_arm_yaw", "F_L_UPPER_ARM_YAW", (0, 0, 1), (-1.57, 1.57), 17, 44),
    ("elbow_pitch", "F_L_ELBOW_PITCH", (0, -1, 0), (0.0, 2.321), 17, 44),
    ("lower_arm_yaw", "F_L_LOWER_ARM_YAW", (0, 0, 1), (-1.05, 1.05), 14, 50),
]
LINKS = ["shoulder_pitch_link", "shoulder_roll_link", "upper_arm_link", "forearm_link", "hand_link"]

# Each whole actuator is carried on the stator side until rotor mass is known.
LINK_PARTS = {
    "shoulder_pitch_link": ("P1_L01", "ROLL_RS03"),
    "shoulder_roll_link": ("SPACER", "P2_L04B", "YAW_RS02"),
    "upper_arm_link": ("ELBOW_RS02", "marengo_upper_arm_left"),
    "forearm_link": ("FOREARM_RS00",),
    "hand_link": (),
}
PLACEHOLDER_MASS = 0.001


def robot_origin(frames, name):
    """Frame origin in the robot frame, relative to F_TORSO."""
    delta = np.array(frames[name]["origin"]) - np.array(frames["F_TORSO"]["origin"])
    return STEP_TO_ROBOT @ delta


def part_in_robot(part, mass, torso):
    """Return COM (robot frame, from F_TORSO) and inertia about that COM, scaled to mass."""
    to_step = np.array(part["rows"]).T
    moi = np.array(part["moi_com"]).reshape(3, 3) * (mass / part["sw_mass"])
    rotation = STEP_TO_ROBOT @ to_step
    com = STEP_TO_ROBOT @ (to_step @ np.array(part["com"]) + np.array(part["t"]) - torso)
    return com, rotation @ moi @ rotation.T


def link_inertial(names, parts, overrides, torso, origin):
    """Combine parts into mass, COM and inertia in the link frame."""
    items = []
    for name in names:
        part = parts[name]
        mass = overrides.get(name)
        if mass is None:
            mass = part["volume"] * PRINT_DENSITY
        com, moi = part_in_robot(part, mass, torso)
        items.append((mass, com - origin, moi))
    if not items:
        return PLACEHOLDER_MASS, np.zeros(3), np.eye(3) * 1e-7
    total = sum(m for m, _, _ in items)
    center = sum(m * c for m, c, _ in items) / total
    inertia = np.zeros((3, 3))
    for mass, com, moi in items:
        r = com - center
        inertia += moi + mass * ((r @ r) * np.eye(3) - np.outer(r, r))
    return total, center, inertia


def vec(values):
    return " ".join(f"{v:.6f}" for v in values)


def build(export, overrides):
    frames, parts = export["frames"], export["components"]
    torso = np.array(frames["F_TORSO"]["origin"])
    robot = ET.Element("robot", name="marengo_left_arm")
    ET.SubElement(robot, "link", name="torso_link")
    parent, parent_origin = "torso_link", np.zeros(3)
    for (joint, frame, axis, limit, effort, velocity), link in zip(JOINTS, LINKS):
        origin = robot_origin(frames, frame)
        element = ET.SubElement(robot, "link", name=f"left_{link}")
        mass, com, inertia = link_inertial(LINK_PARTS[link], parts, overrides, torso, origin)
        inertial = ET.SubElement(element, "inertial")
        ET.SubElement(inertial, "origin", xyz=vec(com), rpy="0 0 0")
        ET.SubElement(inertial, "mass", value=f"{mass:.6f}")
        ET.SubElement(
            inertial, "inertia",
            ixx=f"{inertia[0, 0]:.9f}", ixy=f"{inertia[0, 1]:.9f}", ixz=f"{inertia[0, 2]:.9f}",
            iyy=f"{inertia[1, 1]:.9f}", iyz=f"{inertia[1, 2]:.9f}", izz=f"{inertia[2, 2]:.9f}",
        )
        node = ET.SubElement(robot, "joint", name=f"left_{joint}", type="revolute")
        ET.SubElement(node, "parent", link=parent)
        ET.SubElement(node, "child", link=f"left_{link}")
        ET.SubElement(node, "origin", xyz=vec(origin - parent_origin), rpy="0 0 0")
        ET.SubElement(node, "axis", xyz=" ".join(str(a) for a in axis))
        ET.SubElement(node, "limit", lower=str(limit[0]), upper=str(limit[1]),
                      effort=str(effort), velocity=str(velocity))
        parent, parent_origin = f"left_{link}", origin
    for name, frame in (("left_wrist", "F_L_HAND"), ("left_hand_tip", "F_L_HAND_TIP")):
        ET.SubElement(robot, "link", name=name)
        node = ET.SubElement(robot, "joint", name=f"{name}_fixed", type="fixed")
        ET.SubElement(node, "parent", link="left_hand_link")
        ET.SubElement(node, "child", link=name)
        ET.SubElement(node, "origin", xyz=vec(robot_origin(frames, frame) - parent_origin), rpy="0 0 0")
    ET.indent(robot)
    return robot


def rotation(axis, angle):
    x, y, z = axis
    k = np.array([[0, -z, y], [z, 0, -x], [-y, x, 0]], dtype=float)
    return np.eye(3) + np.sin(angle) * k + (1 - np.cos(angle)) * k @ k


def forward(robot, q):
    """Wrist and tip positions in the robot frame, read back from the written URDF."""
    pose_r, pose_t = np.eye(3), np.zeros(3)
    points = {}
    for joint in robot.findall("joint"):
        origin = np.array([float(v) for v in joint.find("origin").get("xyz").split()])
        if joint.get("type") == "revolute":
            pose_t = pose_t + pose_r @ origin
            axis = [float(v) for v in joint.find("axis").get("xyz").split()]
            pose_r = pose_r @ rotation(axis, q.get(joint.get("name"), 0.0))
        else:
            points[joint.find("child").get("link")] = pose_t + pose_r @ origin
    return points


def check_actuators(export):
    """Each saved actuator axis must pass through its skeleton frame, independent of placement code."""
    frames, parts = export["frames"], export["components"]
    # part, local point on the output axis (m), joint frame, output normal in STEP, local output sign
    cases = [
        ("YAW_RS02", (-0.07349, 0.006, -0.105), "F_L_UPPER_ARM_YAW", (0, -1, 0), -1),
        ("ELBOW_RS02", (0, 0, 0), "F_L_ELBOW_PITCH", (1, 0, 0), 1),
        ("FOREARM_RS00", (0, 0, -0.0054), "F_L_LOWER_ARM_YAW", (0, -1, 0), -1),
    ]
    for name, local, frame, normal, sign in cases:
        part = parts[name]
        rows = np.array(part["rows"])
        point = rows.T @ np.array(local) + np.array(part["t"])
        offset = point - np.array(frames[frame]["origin"])
        off_axis = offset - np.array(normal) * (offset @ np.array(normal))
        assert np.linalg.norm(off_axis) < 1e-5, f"{name} axis misses {frame}"
        assert np.allclose(rows[2] * sign, normal, atol=1e-6), f"{name} output direction"


def check(path, export):
    robot = ET.parse(path).getroot()
    frames = export["frames"]
    zero = forward(robot, {})
    assert np.allclose(zero["left_wrist"], robot_origin(frames, "F_L_HAND"), atol=1e-6), "wrist"
    assert np.allclose(zero["left_hand_tip"], robot_origin(frames, "F_L_HAND_TIP"), atol=1e-6), "tip"
    check_actuators(export)
    half = np.pi / 2
    raised = forward(robot, {"left_shoulder_pitch": half})["left_wrist"]
    assert raised[0] > zero["left_wrist"][0] + 0.3, "+pitch must raise forward"
    abducted = forward(robot, {"left_shoulder_roll": half})["left_wrist"]
    assert abducted[1] > zero["left_wrist"][1] + 0.3, "+roll must abduct"
    flexed = forward(robot, {"left_elbow_pitch": half})["left_wrist"]
    assert flexed[0] > zero["left_wrist"][0] + 0.15, "+elbow must flex forward"
    mass = sum(float(e.get("value")) for e in robot.iter("mass"))
    print(f"check passed: wrist {zero['left_wrist'].round(4)}, tip {zero['left_hand_tip'].round(4)}, mass {mass:.3f} kg")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    export = json.loads((HERE / "cad_export.json").read_text(encoding="utf8"))
    overrides = json.loads((HERE / "mass_overrides.json").read_text(encoding="utf8"))
    path = HERE / "left_arm.urdf"
    if not args.check:
        ET.ElementTree(build(export, overrides)).write(path, encoding="unicode")
    check(path, export)


if __name__ == "__main__":
    main()
