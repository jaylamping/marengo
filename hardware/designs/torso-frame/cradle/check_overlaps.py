"""Overlap and clearance check of the cradle boxes against generator/parts.json (exact box and cylinder maths)."""
import json
import math
from pathlib import Path

HERE = Path(__file__).parent
PARTS = json.loads((HERE.parent / "generator" / "parts.json").read_text())["parts"]

# Right side (-X) solids, mirrored for left below. lo, hi in torso coordinates, mm. Holes are ignored, so this errs high.
RIGHT = {
    "web": ((-51.5, 175, -50), (-48, 353.62, 37.5)),
    "bottom_foot_strip": ((-55, 175, -44), (-51.5, 179, 37.5)),
    "bottom_foot_tongue": ((-75, 175, -21.5), (-55, 179, 21.5)),
    "bottom_root": ((-55, 179, -44), (-51.5, 182.5, 37.5)),
    "top_root": ((-55, 346.12, 5.2), (-51.5, 349.62, 37.5)),
    "top_foot": ((-75, 349.62, -44), (-51.5, 353.62, 37.5)),
    "ledge": ((-48, 179, -45), (-40, 183, 34)),
    "front_lip": ((-48, 179, 34), (-42, 345, 37.5)),
    "boss": ((-59, 244.6, -13), (-51.5, 260.6, 3)),
    "thumbscrew_head": ((-65.5, 243.6, -14), (-59, 261.6, 4)),
    "pad_lo": ((-55.5, 195, 29.5), (-48, 205, 37.5)),
    "pad_hi": ((-55.5, 325, 29.5), (-48, 335, 37.5)),
}
ALL = dict(RIGHT)
ALL.update({"L_" + k: ((-hi[0], lo[1], lo[2]), (-lo[0], hi[1], hi[2])) for k, (lo, hi) in RIGHT.items()})
ALL["strap"] = ((-51.7, 345.5, -15), (51.7, 348.5, 5))
ALL["strap_lip_r"] = ((-54.2, 340.5, -15), (-51.7, 348.5, 5))
ALL["strap_lip_l"] = ((51.7, 340.5, -15), (54.2, 348.5, 5))


def boxes_of(p):
    """Bounding boxes of one parts.json entry."""
    if p["type"] == "box":
        return [(p["lo"], p["hi"])]
    if p["type"] == "multibox":
        return [tuple(b) for b in p["boxes"]]
    if p["type"] == "cyl":
        i = "xyz".index(p["axis"][-1])
        lo = [v - p["r"] for v in p["c"]]
        hi = [v + p["r"] for v in p["c"]]
        lo[i], hi[i] = p["c"][i] - p["h"] / 2, p["c"][i] + p["h"] / 2
        return [(lo, hi)]
    if p["type"] == "poly":
        xs, ys = zip(*p["pts"])
        z0 = p["z0"]
        z1 = z0 + p.get("t", 3)
        return [([min(xs), min(ys), min(z0, z1)], [max(xs), max(ys), max(z0, z1)])]
    return []


def gap(a, b):
    """Per-axis signed gaps: negative on all three means overlap by that much."""
    return [max(a[0][i] - b[1][i], b[0][i] - a[1][i]) for i in range(3)]


rows = []
for name, box in ALL.items():
    for p in PARTS:
        for b in boxes_of(p):
            g = gap(box, b)
            if max(g) < 0:
                rows.append(("OVERLAP", name, p["id"], p["group"], [round(-v, 2) for v in g]))
            elif max(g) == 0 and sum(v == 0 for v in g) == 1:
                rows.append(("touch", name, p["id"], p["group"], [round(v, 2) for v in g]))
            elif 0 < max(g) < 2 and all(v < 2 for v in g):
                rows.append(("near", name, p["id"], p["group"], [round(v, 2) for v in g]))

for r in rows:
    print(*r)
print("skipped", [p["id"] for p in PARTS if not boxes_of(p)])

# Same question on the exported meshes, at the voxel pitch of mass_estimate.py, so holes and windows count.
import numpy as np  # noqa: E402
import trimesh  # noqa: E402

from mass_estimate import PITCH, fill  # noqa: E402

for path in sorted(HERE.glob("*.stl")):
    mesh = trimesh.load(path)
    grid = fill(mesh)
    lo = mesh.bounds[0]
    pts = (np.argwhere(grid) + 0.5) * PITCH + lo
    for p in PARTS:
        for b in boxes_of(p):
            n = np.all((pts > np.array(b[0]) + 1e-6) & (pts < np.array(b[1]) - 1e-6), axis=1).sum()
            if n:
                print("MESH", path.stem, p["id"], p["group"], f"{n * PITCH**3:.0f} mm3")
