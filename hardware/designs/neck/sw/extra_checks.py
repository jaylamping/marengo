"""Mid-range interference poses plus the bellows at neutral, and the swept numbers for LAYOUT_SW.md.
Run: python extra_checks.py <solidworks session pid file>
"""
import itertools
import json
import sys
from pathlib import Path

import numpy as np

import assemble_check as A
import neck_geom as g

sys.path.insert(0, r"J:\code\marengo\hardware\tools\solidworks")
from sw_instance import attach

inst = attach(int(open(sys.argv[1]).read().strip()))
L = A.Layout(inst.sw)
L.open()
res = {}
L.pose()
rows = L.check()
res["neutral_with_bellows"] = [x for x in rows if set(x[0]) not in A.JOINTS]
L.show("bellows", False)
for p, r in [(g.PITCH_DOWN, 0), (-g.PITCH_UP, 0), (0, g.ROLL_MAX), (0, -g.ROLL_MAX), (17, 10), (17, -10), (-12, 10), (-12, -10)]:
    L.pose(pitch=p, roll=r)
    res[f"p{p}_r{r}"] = [x for x in L.check() if set(x[0]) not in A.JOINTS]
L.pose()
L.show("bellows", True)
inst.sw.CloseAllDocuments(True)

horn, low, top_r, rail_r = 0, 1e9, 0, 0
GRID = list(itertools.product(np.linspace(-g.PITCH_UP, g.PITCH_DOWN, 13), np.linspace(-g.ROLL_MAX, g.ROLL_MAX, 9)))
for p, r in GRID:
    for k in g.SERVOS:
        t, a, b = g.rod_pose(k, p, r)
        horn = max(horn, abs(np.degrees(t)))
        low = min(low, a[2] - 6)
        rail_r = max(rail_r, np.hypot(a[0], a[1]) + g.BALL_R)
        top_r = max(top_r, np.hypot(b[0], b[1]) + g.BALL_R)
res["max_horn_deg"] = round(horn, 1)
res["lowest_link_Y"] = round(g.y(low), 1)
res["link_radius_at_horn"] = round(rail_r, 1)
res["link_radius_at_head"] = round(top_r, 1)
res["cage_sweep_r"] = 45.0
Path(__file__).with_name("extra_checks.json").write_text(json.dumps(res, indent=1))
print(json.dumps(res, indent=1))
