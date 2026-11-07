"""Checks that need no SolidWorks: horn and rod agree, gears mesh, and the U-joint parts clear each other as boxes."""
import itertools
import sys
from math import pi

import numpy as np
from shapely.affinity import rotate, translate
from shapely.geometry import Polygon

import assemble_check as A
import build_parts as B
import neck_geom as g

ok = True
# horn tip and rod foot coincide; rod head lands on the moved stud
for p, r in list(g.corners()) + [(0, 0)]:
    q = A.pose_all(pitch=p, roll=r)
    for k in "ab":
        R, t = q["horn_" + k]
        tip = R @ np.array([-g.HORN_R, 0, 0]) + t
        ok &= bool(np.linalg.norm(tip - q["rod_" + k][1]) < 1e-6)
        R2, t2 = q["rod_" + k]
        top = R2 @ np.array([0, g.rod_len(k.upper()), 0]) + t2
        Rp, tp = q["plate"]
        stud = Rp @ g.to_robot(g.stud0(g.SERVOS[k.upper()]["side"])) + tp
        ok &= bool(np.linalg.norm(top - stud) < 1e-6)
print("horn and rod ends agree:", ok)

# gears: (X, Z) angles as in the parts; the ring turns by -yaw in that plane, the pinion by +yaw * 88/24
ring, pin = Polygon(B.involute_gear(88)), Polygon(B.involute_gear(24, step=pi + pi / 24))
print("gear outlines valid:", ring.is_valid, pin.is_valid, "segments", len(ring.exterior.coords), len(pin.exterior.coords))
worst = 0
for yaw in np.arange(-90, 90.01, 0.37):
    a = rotate(ring, -yaw, origin=(0, 0))
    b = translate(rotate(pin, yaw * 88 / 24, origin=(0, 0)), 56, 0)
    worst = max(worst, a.intersection(b).area)
print("largest gear overlap mm2 over yaw:", round(worst, 4))


def obb(lo, hi, R=np.eye(3), t=np.zeros(3)):
    lo, hi = np.array(lo, float), np.array(hi, float)
    return R @ ((lo + hi) / 2) + t, R, (hi - lo) / 2


def hit(a, b):
    """Separating-axis test between two oriented boxes."""
    c = b[0] - a[0]
    axes = [a[1][:, i] for i in range(3)] + [b[1][:, i] for i in range(3)]
    axes += [np.cross(u, v) for u in axes[:3] for v in axes[3:]]
    for n in axes:
        m = np.linalg.norm(n)
        if m < 1e-9:
            continue
        n = n / m
        ra = sum(abs(n @ a[1][:, i]) * a[2][i] for i in range(3))
        rb = sum(abs(n @ b[1][:, i]) * b[2][i] for i in range(3))
        if abs(n @ c) > ra + rb:
            return False
    return True


Y = g.y
GRID = list(itertools.product(np.linspace(-g.PITCH_UP, g.PITCH_DOWN, 13), np.linspace(-g.ROLL_MAX, g.ROLL_MAX, 9)))
fixed = {"yoke_base": obb((-14, Y(60), -6), (14, Y(64), 6)), "yoke_L": obb((11, Y(64), -6), (14, Y(86), 6)), "yoke_R": obb((-14, Y(64), -6), (-11, Y(86), 6)),
         "post": obb((-7, Y(40), -7), (7, Y(62), 7))}
plate = {"ear_f": ((-6, Y(74), 11), (6, Y(96), 14)), "ear_r": ((-6, Y(74), -14), (6, Y(96), -11)), "disc": ((-21, Y(96), -21), (21, Y(100), 21)),
         "lug_L": ((11, Y(86.2), -23), (19, Y(96), -13)), "lug_R": ((-19, Y(86.2), -23), (-11, Y(96), -13))}
block = ((-8, Y(72), -8), (8, Y(88), 8))
bad = set()
for p, r in GRID:
    q = A.pose_all(pitch=p, roll=r)
    Rp, tp = q["plate"]
    Rc, tc = q["cross"]
    moved = {n: obb(*b, Rp, tp) for n, b in plate.items()}
    cb = obb(*block, Rc, tc)
    bad |= {(f, n) for f in fixed for n, b in moved.items() if hit(fixed[f], b)}
    bad |= {(f, "block") for f in ("yoke_base", "post") if hit(fixed[f], cb)}
    bad |= {("block", n) for n, b in moved.items() if hit(cb, b)}
print("U-joint box hits over the range:", sorted(bad) or "none")

# rods inside the sleeve, and rod B past the upper servo
worst_r, worst_s = 0, 1e9
for p, r in GRID:
    for k in g.SERVOS:
        t, a, b = g.rod_pose(k, p, r)
        for f in np.linspace(0, 1, 200):
            c = a + (b - a) * f
            if 17 <= c[2] <= 48:
                worst_r = max(worst_r, np.hypot(c[0], c[1]) + g.ROD_R)
            if k == "B" and -24 <= c[2] <= 1:   # servo A body: x from -9.2, |y| up to 17
                worst_s = min(worst_s, np.hypot(max(-9.2 - c[0], 0), max(abs(c[1]) - 17, 0)) - g.ROD_R)
print("rod outer radius inside the sleeve (bore 30):", round(worst_r, 2))
print("rod B clearance to servo A body:", round(worst_s, 2))

# bellows length, flange h 17 to head h 66 at r 36; ball-link angle out of the plane normal to the stud
Lmin, Lmax, mis = 1e9, 0, 0
for p, r in g.corners():
    Rh = g.head_rot(p, r)
    for th in np.linspace(0, 2 * pi, 72, endpoint=False):
        foot = np.array([36 * np.cos(th), 36 * np.sin(th), 17.0])
        top = g.UJ + Rh @ (np.array([foot[0], foot[1], 66.0]) - g.UJ)
        L = np.linalg.norm(top - foot)
        Lmin, Lmax = min(Lmin, L), max(Lmax, L)
    for k in g.SERVOS:
        t, a, b = g.rod_pose(k, p, r)
        d = (b - a) / np.linalg.norm(b - a)
        mis = max(mis, abs(np.degrees(np.arcsin(d @ (Rh @ [0, 1, 0])))), abs(np.degrees(np.arcsin(d[1]))))
print(f"bellows length 49 neutral, {Lmin:.1f} to {Lmax:.1f} over the corners")
print(f"worst ball-link angle out of the stud plane (stud along head Y): {mis:.1f} deg")
sys.exit(0 if ok and not bad and worst < 1e-3 else 1)
