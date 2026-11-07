"""Fit check of the snap hooks against the cradle sides, on the exported meshes at 0.1 mm voxels.

Seats each hook, then walks it out along X to show what it meets on the way in. Also sweeps the hook boxes
through parts.json out past the frame side.
"""
import json
from pathlib import Path

import numpy as np
import trimesh

HERE = Path(__file__).parent
PITCH = 0.1
PARTS = json.loads((HERE.parent / "generator" / "parts.json").read_text())["parts"]
LO, HI = np.array([-58.0, 339.0, -19.0]), np.array([-28.0, 351.0, 9.0])  # right side window


def fill(mesh, lo, hi):
    xs = np.arange(lo[0] + PITCH / 2, hi[0], PITCH)
    ys = np.arange(lo[1] + PITCH / 2, hi[1], PITCH)
    zs = np.arange(lo[2] + PITCH / 2, hi[2], PITCH)
    hits = {}
    for tri in mesh.triangles + np.array([1e-4, 0, 2e-4]):
        a, b, c = tri[:, [0, 2]]
        det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
        if abs(det) < 1e-12:
            continue
        ix = np.where((xs >= tri[:, 0].min()) & (xs <= tri[:, 0].max()))[0]
        iz = np.where((zs >= tri[:, 2].min()) & (zs <= tri[:, 2].max()))[0]
        for i in ix:
            for k in iz:
                u = ((b[1] - c[1]) * (xs[i] - c[0]) + (c[0] - b[0]) * (zs[k] - c[1])) / det
                v = ((c[1] - a[1]) * (xs[i] - c[0]) + (a[0] - c[0]) * (zs[k] - c[1])) / det
                if u >= 0 and v >= 0 and u + v <= 1:
                    hits.setdefault((i, k), []).append(u * tri[0, 1] + v * tri[1, 1] + (1 - u - v) * tri[2, 1])
    out = np.zeros((len(xs), len(ys), len(zs)), bool)
    for (i, k), h in hits.items():
        h = sorted(h)
        for y0, y1 in zip(h[::2], h[1::2]):
            out[i, (ys > y0) & (ys < y1), k] = True
    return out


def main():
    strap = trimesh.load(HERE / "marengo_torso_cradle_top_strap.stl")
    print("strap bounds", np.round(strap.bounds, 3).tolist(), "watertight", strap.is_watertight, "volume", round(strap.volume, 1))
    for s, label in ((-1, "right"), (1, "left")):
        side = trimesh.load(HERE / f"marengo_torso_cradle_side_{label}.stl")
        hook = strap.copy()
        keep = hook.triangles_center[:, 0] * s > 4
        hook.update_faces(keep)
        hook.remove_unreferenced_vertices()
        if s > 0:
            for m in (side, hook):
                m.apply_scale([-1, 1, 1])
        print(label, "hook volume", round(abs(hook.volume), 1), "watertight", hook.is_watertight)
        S, H = fill(side, LO, HI), fill(hook, LO, HI)
        print(" seated overlap voxels", (S & H).sum(), f"= {(S & H).sum() * PITCH**3:.2f} mm3")
        for dx in (0.1, 0.2):
            print(f" shifted in {dx}:", (S & np.roll(H, int(dx / PITCH), 0)).sum(), f" out {dx}:", (S & np.roll(H, -int(dx / PITCH), 0)).sum())
        for dz in (-0.7, -0.5, -0.3, 0.3, 0.5, 0.7):  # the slot ends are the Z stops, 0.5 per end
            print(f" shifted Z {dz}:", (S & np.roll(H, int(dz / PITCH), 2)).sum())
        # Walk out along X: which Y band hits the side at each step. The barb should be all that does.
        yy = np.arange(LO[1] + PITCH / 2, HI[1], PITCH)
        for dx in (0.3, 0.6, 1, 2, 3.5, 4.5, 8):
            n = int(dx / PITCH)
            hit = S[:-n] & H[n:] if n else S & H
            where = np.argwhere(hit)
            if len(where):
                print(f" out {dx}: {len(where)} vox, Y {yy[where[:, 1].min()]:.1f} to {yy[where[:, 1].max()]:.1f}")
            else:
                print(f" out {dx}: clear")
    # Insertion path: hook boxes from the seat out to |X| 130, including the barb height
    for s, x0 in ((-1, 5), (1, 25)):
        a, b = sorted((s * x0, s * 170))
        for p in PARTS:
            bs = [(p["lo"], p["hi"])] if p["type"] == "box" else [tuple(q) for q in p.get("boxes", [])]
            if p["type"] == "cyl":
                i = "xyz".index(p["axis"][-1])
                lo = [v - p["r"] for v in p["c"]]
                hi = [v + p["r"] for v in p["c"]]
                lo[i], hi[i] = p["c"][i] - p["h"] / 2, p["c"][i] + p["h"] / 2
                bs = [(lo, hi)]
            for lo, hi in bs:
                if lo[0] < b and hi[0] > a and lo[1] < 350 and hi[1] > 340.5 and lo[2] < 5 and hi[2] > -15 and not p["id"].startswith(("packrail", "cradle_strap")):
                    print("path", label if s > 0 else "right", p["id"], p["group"], lo, hi)


if __name__ == "__main__":
    main()
