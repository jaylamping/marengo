#!/usr/bin/env python3
"""Forearm fork clearance to the upper arm and elbow RS02 across elbow travel, in arm coordinates (X 170.49)."""
import json
from scipy.spatial import cKDTree

exec(open("sweep.py").read().split("def main():")[0])
E = Path(r"J:\code\marengo\cad\exports\left_arm")
export = json.loads((HERE.parent / "cad_export.json").read_text())
raw = json.loads((HERE.parent / "component_boxes_mm.json").read_text())
boxes = {k.replace("parts/", "").replace(".SLDPRT", ""): v for k, v in raw.items() if k.startswith("parts/")}
comp = export["components"]


def mm(t):
    return t * 1000 if np.ptp(t.reshape(-1, 3), axis=0).max() < 5 else t


S = np.array([38.0, 0, 0])
ua = sample(mm(read_stl(HERE / "stl" / "UPPER_ARM.STL")), 1.0)
rs02 = sample(world("ELBOW_RS02", read_stl(HERE / "stl" / "ELBOW_RS02.STL"), comp, boxes), 1.0) - S
rs00 = sample(world("FOREARM_RS00", read_stl(HERE / "stl" / "FOREARM_RS00.STL"), comp, boxes), 1.5) - S
fork = sample(mm(read_stl(E / "marengo_forearm_left.stl")), 1.0)
O = np.array([0, 209.860668, 5.985875])
print("fork box", fork.min(0).round(1), fork.max(0).round(1))
print("dir test", (np.array([[0, -100.0, 0]]) @ rot(ELBOW_AXIS, 90).T).round(1))
keep = ua[:, 1] > 150
tu = cKDTree(ua[keep & (ua[:, 0] >= 0)])
t2 = cKDTree(rs02)
print("fork to RS02 at rest, away from the output face", round(float(t2.query(fork[fork[:, 0] < 192])[0].min()), 2))
rel = fork - O
th = np.degrees(np.arctan2(rel[:, 2], -rel[:, 1]))
rr = np.hypot(rel[:, 1], rel[:, 2])
finger = (fork[:, 0] > 135.9) & (fork[:, 0] < 140.2) & (rr > 34) & (rr < 42) & (th > 21) & (th < 63)
rest = fork[:, 0] > 147.0
print("finger pts", int(finger.sum()), "rest", int((rest & ~finger).sum()))
for e in [0, 2, 15, 30, 45, 60, 75, 90, 105, 120, 133, 135]:
    Rm = rot(ELBOW_AXIS, e)
    f = (fork - O) @ Rm.T + O
    r = (rs00 - O) @ Rm.T + O
    m = rest & ~finger
    a, _ = tu.query(f[m])
    g, _ = tu.query(f[finger])
    b, _ = tu.query(r)
    print(e, "fork to arm", round(float(a.min()), 1), "at", f[m][int(a.argmin())].round(0).tolist(), "finger to arm", round(float(g.min()), 1), "cheek face to plate", round(float(tu.query(f[(fork[:,0]>134)&(fork[:,0]<137)&(rr>29)])[0].min()),1), "RS00 to arm", round(float(b.min()), 1))
