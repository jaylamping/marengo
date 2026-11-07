"""Neck concept layout, option C at +10 mm: shared numbers and kinematics (no SolidWorks).

Torso frame, mm: X lateral (+X robot left), Y up, Z forward. h is height above the collar bottom (Y 529),
the z of the head frame in layout_study.py. Head X fwd = robot +Z, head Y left = robot +X.
"""
from math import asin, atan2, cos, degrees, hypot, pi, radians, sin

import numpy as np

COLLAR_Y = 529.0
RAIL_TOP_Y = 495.0
UJ_H = 80.0
PITCH_DOWN, PITCH_UP = 34.0, 25.0
ROLL_MAX, YAW_MAX = 20.0, 90.0

# STS3215 vendor STEP frame: X +-22.7, Y +-12.4, Z -19.4..20.2, shaft at X 12.5 along +Z. Lying flat, shaft along head Y.
SERVO_THICK = 24.8                # along Y up when lying flat (vendor STEP: 45.4 x 24.8 x 34, spline to 39.6)
GAP = 2.0                         # shelf between the stacked servos
SERVO_TOP_H = 1.0
SERVO_A_H = SERVO_TOP_H - SERVO_THICK / 2
SERVO_B_H = SERVO_A_H - SERVO_THICK - GAP
HORN_R = 19.0
SHAFT_FWD = 1.0                   # head X of the servo shaft: horn tip sits under the stud when level
STUD_FWD = -18.0                  # head X of the head-plate ball studs
STUD_SIDE = 15.0
HORN_PLANE = 20.2                 # |head Y| of the rod lower ball, in the horn arm (output face 18.7, arm 3 thick)
ROD_R = 2.0
BALL_R = 5.0


def y(h):
    return COLLAR_Y + h


def to_robot(p):
    """Head frame (x fwd, y left, h up, relative to the axis) to torso mm."""
    return np.array([p[1], y(p[2]), p[0]], float)


def Rx(a):  # head frame
    c, s = cos(a), sin(a)
    return np.array([[1, 0, 0], [0, c, -s], [0, s, c]])


def Ry(a):
    c, s = cos(a), sin(a)
    return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])


def Rz(a):
    c, s = cos(a), sin(a)
    return np.array([[c, -s, 0], [s, c, 0], [0, 0, 1]])


def head_rot(pitch_down, roll):
    """Yoke pin along head Y fixed to the post (pitch), roll pin on the cross. Nose down is positive."""
    return Ry(radians(pitch_down)) @ Rx(radians(roll))


UJ = np.array([0.0, 0.0, UJ_H])
SERVOS = {"A": dict(side=+1, h=SERVO_A_H), "B": dict(side=-1, h=SERVO_B_H)}


def stud0(side):
    return np.array([STUD_FWD, side * STUD_SIDE, UJ_H])


def tip(t, side, h):
    """Horn ball at horn angle t (rad, + = rear tip up), head frame, before yaw."""
    return np.array([SHAFT_FWD - HORN_R * cos(t), side * HORN_PLANE, h + HORN_R * sin(t)])


def rod_len(k):
    s = SERVOS[k]
    return float(np.linalg.norm(stud0(s["side"]) - tip(0.0, s["side"], s["h"])))


def solve_horn(k, pitch_down, roll):
    """Horn angle (rad) that keeps the rod length, nearest level, and the stud position."""
    s = SERVOS[k]
    st = UJ + head_rot(pitch_down, roll) @ (stud0(s["side"]) - UJ)
    L = rod_len(k)
    lo, hi = -pi / 2 + 0.01, pi / 2 - 0.01
    f = lambda t: np.linalg.norm(st - tip(t, s["side"], s["h"])) - L
    # rod runs up from the tip, so raising the tip shortens the distance: f falls monotonically here
    if f(lo) * f(hi) > 0:
        raise ValueError(f"no horn solution {k} {pitch_down} {roll}")
    for _ in range(60):
        mid = (lo + hi) / 2
        if f(lo) * f(mid) <= 0:
            hi = mid
        else:
            lo = mid
    return (lo + hi) / 2, st


def corners():
    for p in (PITCH_DOWN, -PITCH_UP):
        for r in (-ROLL_MAX, ROLL_MAX):
            yield p, r


def rod_pose(k, pitch_down, roll):
    t, st = solve_horn(k, pitch_down, roll)
    return t, tip(t, SERVOS[k]["side"], SERVOS[k]["h"]), st


if __name__ == "__main__":
    for k in SERVOS:
        print(k, "rod length", round(rod_len(k), 2), "shaft height Y", round(y(SERVOS[k]["h"]), 2))
    worst = {}
    for p in range(-int(PITCH_UP), int(PITCH_DOWN) + 1):
        for r in range(-int(ROLL_MAX), int(ROLL_MAX) + 1, 2):
            for k in SERVOS:
                t, a, b = rod_pose(k, p, r)
                d = b - a
                lean = degrees(atan2(hypot(d[0], d[1]), d[2]))
                if abs(degrees(t)) > abs(worst.get("horn", (0,))[0]):
                    worst["horn"] = (degrees(t), k, p, r)
                if lean > worst.get("lean", (0,))[0]:
                    worst["lean"] = (lean, k, p, r)
    print(worst)
