#!/usr/bin/env python3
"""Clearance of the upper arm, carrier, elbow RS02, fork and RS00 to the shell, as built by the elbow compaction (arg: stl dir)."""
import json, sys
from pathlib import Path as _P
src = open("shell_gap_sock.py").read().split("PITCH = [i")[0]
exec(src)
D = sys.argv[1] if len(sys.argv) > 1 else "stl"
E = _P(r"J:\code\marengo\cad\exports\left_arm")
S = np.array([38.0, 0, 0])
mm = lambda t: t * 1000 if np.ptp(t.reshape(-1, 3), axis=0).max() < 5 else t
ua = sample(mm(read_stl(HERE / D / "UPPER_ARM.stl")), 2.0) + S
fk = sample(mm(read_stl((HERE / D / "FORK.stl") if D != "stl" else E / "marengo_forearm_left.stl")), 2.0) + S
rs = sample(world("ELBOW_RS02", read_stl(HERE / "stl" / "ELBOW_RS02.STL"), comp, boxes), 2.0) - (0 if D == "stl" else np.array([4.0, 0, 0]))
r0 = sample(world("FOREARM_RS00", read_stl(HERE / "stl" / "FOREARM_RS00.STL"), comp, boxes), 3.0)
lim = 0
print("ua", ua.min(0).round(1), ua.max(0).round(1), "fork", fk.min(0).round(1), fk.max(0).round(1))
res = {}
for sock in (76.0, 78.0):
    for roll in (0.0, -5.0):
        for e in (0, 45, 90, 133):
            Rm = rot(ELBOW_AXIS, e)
            f = np.vstack([fk, r0])
            f = (f - ELBOW_O) @ Rm.T + ELBOW_O
            cloud = np.vstack([ua, rs, f])[::2]
            worst = None
            bad = []
            for pitch in [i * 5.0 for i in range(-10, 37)]:
                best = None
                for yaw in range(-90, 91, 30):
                    g, w, h = gap(posed(yawed(cloud, yaw) if yaw else cloud, pitch, roll, 0.0), sock, 135.5)
                    if g is not None and (best is None or g < best[0]):
                        best = (round(g, 1), yaw, w)
                if best and (worst is None or best[0] < worst[0]):
                    worst = (best[0], pitch, best[1], best[2])
                if best and best[0] < 3:
                    bad.append(pitch)
            print("sock", sock, "roll", roll, "elbow", e, "min", worst, "under3 at pitch", (min(bad), max(bad)) if bad else None)
            res[f"{sock}_{roll}_{e}"] = {"min": worst, "under3": bad}
(HERE / f"shell_gaps_elbow_{D}.json").write_text(json.dumps(res, indent=1))

