#!/usr/bin/env python3
"""Elbow stack only (Y under 255, arm hanging) to the shell at 5 degrees inward roll, pass 1 against pass 2."""
from pathlib import Path as _P
src = open("shell_gap_sock.py").read().split("PITCH = [i")[0]
exec(src)
E = _P(r"J:\code\marengo\cad\exports\left_arm")
S = np.array([38.0, 0, 0])
mm = lambda t: t * 1000 if np.ptp(t.reshape(-1, 3), axis=0).max() < 5 else t
rs0 = world("ELBOW_RS02", read_stl(HERE / "stl" / "ELBOW_RS02.STL"), comp, boxes)
for name, ua_p, fk_p, dx in (("pass 1", HERE / "stl_comp75" / "UPPER_ARM.stl", HERE / "stl_comp75" / "FORK.stl", -4.0),
                             ("pass 2", HERE / "stl" / "UPPER_ARM.stl", E / "marengo_forearm_left.stl", 0.0)):
    ua = sample(mm(read_stl(ua_p)), 1.5) + S
    fk = sample(mm(read_stl(fk_p)), 1.5) + S
    rs = sample(rs0, 1.5) + np.array([dx, 0, 0])
    cloud = np.vstack([ua, rs, fk])
    cloud = cloud[(cloud[:, 1] < 255) & (cloud[:, 1] > 165)]
    for sock in (78.0,):
        for roll in (0.0, -5.0):
            worst = None
            for pitch in [i * 5.0 for i in range(-10, 37)]:
                g, w, h = gap(posed(cloud, pitch, roll, 0.0), sock, 135.5)
                if g is not None and (worst is None or g < worst[0]):
                    worst = (round(g, 1), pitch, w)
            print(name, "sock", sock, "roll", roll, "min", worst)
