#!/usr/bin/env python3
"""Clearance from the 2020 two-box frame (and gusset plates) to the inside of the shell, row by row.
Shell inner = concept outer section buffered in by the 2.4 mm wall. Frame boxes from torso-stepped/README.md.
Negative = the frame pokes through the shell wall."""
import json, numpy as np
from shapely.geometry import Polygon, box
from shapely.ops import unary_union
G = {g["tag"]: g for g in json.load(open("out/grids.json"))["grids"]}
ZS = 740 + 424.9 + 70; F0, F1 = 740 + 155 + 70, 740 + 495 + 70
ZB, ZG = ZS - 71.24, ZS - 139.86
def frame(z):
    s = []
    if F0 <= z <= ZB: s.append(box(-70, -75, 70, 75))
    if ZB <= z <= F1: s.append(box(-70, -113, 70, 113))
    if ZG <= z <= ZS - 10:                      # gusset plates on the front/rear faces, 3 mm, |Y| 55..113 (tapered)
        yo = 75 + 38 * min(1, max(0, (z - ZG) / 50.0))
        for sx in (1, -1): s.append(box(min(sx * 70, sx * 73), -yo, max(sx * 70, sx * 73), yo))
    return unary_union(s) if s else None
def rows(front, back):
    A = np.array(G[front]["pts"]).reshape(G[front]["nz"] + 1, -1, 3); B = np.array(G[back]["pts"]).reshape(G[back]["nz"] + 1, -1, 3)
    for i in range(A.shape[0]):
        ring = np.vstack([A[i, :, :2], B[i, :, :2]])
        yield A[i, 0, 2], Polygon(ring).buffer(0)
out = []
for f, b in (("chest lower front", "chest lower back (service hatch)"), ("chest upper front", "chest upper back")):
    for z, P in rows(f, b):
        fr = frame(z)
        if fr is None or P.is_empty: continue
        inner = P.buffer(-2.4)
        from shapely.geometry import Point
        if fr.within(inner): c = inner.exterior.distance(fr)
        else:
            geoms = getattr(fr, 'geoms', [fr]); vs = [Point(v) for g in geoms for v in g.exterior.coords]
            c = -max(inner.exterior.distance(v) for v in vs if not inner.contains(v))
        # where: nearest frame corner
        out.append((round(z, 1), round(c, 1)))
out.sort()
worst = sorted(out, key=lambda t: t[1])[:8]
print("worst rows (z, clearance mm):", worst)
bands = {}
for z, c in out:
    k = int((z - F0) // 20) * 20
    bands[k] = min(bands.get(k, 99), c)
print("min clearance per 20 mm band above frame bottom:", bands)
json.dump(dict(rows=out, worst=worst), open("out/frame_clearance.json", "w"))
