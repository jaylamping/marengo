#!/usr/bin/env python3
"""Proposed elbow jumper band, an annulus around the Ø84 carrier inboard of the fork cheek: distance to the moving fork and RS00, and to the shell."""
from pathlib import Path as _P
src = open("shell_gap_sock.py").read().split("PITCH = [i")[0]
exec(src)
from scipy.spatial import cKDTree
E = _P(r"J:\code\marengo\cad\exports\left_arm")
S = np.array([38.0, 0, 0])
mm = lambda t: t * 1000 if np.ptp(t.reshape(-1, 3), axis=0).max() < 5 else t
ua = sample(mm(read_stl(HERE / "stl" / "UPPER_ARM.stl")), 1.5) + S
fk = sample(mm(read_stl(E / "marengo_forearm_left.stl")), 1.5) + S
r0 = sample(world("FOREARM_RS00", read_stl(HERE / "stl" / "FOREARM_RS00.STL"), comp, boxes), 2.0)
rs = sample(world("ELBOW_RS02", read_stl(HERE / "stl" / "ELBOW_RS02.STL"), comp, boxes), 2.0)
print("min X arm coords", (ua[:, 0].min() - 38).round(2), (fk[:, 0].min() - 38).round(2), (rs[:, 0].min() - 38).round(2))


def band(x0, x1, r0_, r1_):
    th = np.radians(np.arange(0, 360, 3.0))
    pts = []
    for x in np.arange(x0, x1 + .01, 1.0):
        for r in np.arange(r0_, r1_ + .01, 1.0):
            pts.append(np.c_[np.full(th.size, x + 38), EY_ + r * np.cos(th), AZ_ + r * np.sin(th)])
    return np.vstack(pts)


EY_, AZ_ = ELBOW_O[1], ELBOW_O[2]
for name, x0, x1, r0_, r1_ in (("as proposed, X 126.7 to 131.2", 126.7, 131.2, 42.5, 50.0), ("9 mm flat wide face along X, X 121.7 to 131.2", 121.7, 131.2, 42.5, 50.0), ("Ø8 round, X 122.7 to 131.2", 122.7, 131.2, 42.5, 50.0)):
    b = band(x0, x1, r0_, r1_)
    t = cKDTree(b)
    dmin = None
    for e in list(range(0, 131, 5)) + [133]:
        Rm = rot(ELBOW_AXIS, e)
        f = (np.vstack([fk, r0]) - ELBOW_O) @ Rm.T + ELBOW_O
        d = float(t.query(f)[0].min())
        dmin = d if dmin is None else min(dmin, d)
    du = float(t.query(ua)[0].min())
    worst = None
    for roll in (0.0, -5.0):
        for pitch in [i * 5.0 for i in range(-10, 37)]:
            g, w, h = gap(posed(b, pitch, roll, 0.0), 78.0, 135.5)
            if g is not None and roll == -5.0 and (worst is None or g < worst[0]):
                worst = (round(g, 1), pitch)
    print(name, "| moving fork and RS00, min", round(dmin, 1), "| upper arm", round(du, 1), "| shell at 5 deg, sock 78", worst)
