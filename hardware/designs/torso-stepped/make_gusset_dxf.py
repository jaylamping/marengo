#!/usr/bin/env python3
"""Gusset plate outline as a DXF, in torso STEP coordinates (mm, X lateral, Y up). Left side; mirror X for the right."""
import math

POINTS = [(55, 285), (75, 285), (113, 335), (113, 425), (93, 425), (93, 373.62), (55, 373.62)]
HOLES = [(65, 297), (65, 317), (65, 337), (62, 363.62), (84, 363.62), (104, 363.62), (103, 385), (103, 400), (103, 415)]
R, HOLE_D = 8.0, 5.5


def unit(a, b):
    d = math.dist(a, b)
    return (b[0] - a[0]) / d, (b[1] - a[1]) / d


lines, arcs = [], []
n = len(POINTS)
ends = []
for i in range(n):
    p, a, b = POINTS[i], POINTS[i - 1], POINTS[(i + 1) % n]
    ua, ub = unit(p, a), unit(p, b)
    half = math.acos(ua[0] * ub[0] + ua[1] * ub[1]) / 2
    t = R / math.tan(half)
    s, e = (p[0] + ua[0] * t, p[1] + ua[1] * t), (p[0] + ub[0] * t, p[1] + ub[1] * t)
    bis = unit((0, 0), (ua[0] + ub[0], ua[1] + ub[1]))
    c = (p[0] + bis[0] * R / math.sin(half), p[1] + bis[1] * R / math.sin(half))
    a0, a1 = (math.degrees(math.atan2(v[1] - c[1], v[0] - c[0])) for v in (s, e))
    cross = (s[0] - c[0]) * (e[1] - c[1]) - (s[1] - c[1]) * (e[0] - c[0])
    arcs.append((c, R, a0, a1) if cross > 0 else (c, R, a1, a0))
    ends.append((s, e))
for i in range(n):
    lines.append((ends[i][1], ends[(i + 1) % n][0]))

out = ["0", "SECTION", "2", "ENTITIES"]
for a, b in lines:
    out += ["0", "LINE", "8", "OUTLINE", "10", a[0], "20", a[1], "11", b[0], "21", b[1]]
for c, r, a0, a1 in arcs:
    out += ["0", "ARC", "8", "OUTLINE", "10", c[0], "20", c[1], "40", r, "50", a0, "51", a1]
for x, y in HOLES:
    out += ["0", "CIRCLE", "8", "HOLES", "10", x, "20", y, "40", HOLE_D / 2]
out += ["0", "ENDSEC", "0", "EOF"]
open("gusset_plate_left.dxf", "w").write("\n".join(str(v) for v in out) + "\n")
print(len(lines), len(arcs), len(HOLES))
