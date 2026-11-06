#!/usr/bin/env python3
"""Elbow cross-section through the axis, plane Z 5.986, before and after compaction, from the STL exports. Writes ELBOW_SECTION.png."""
import json
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle
from pathlib import Path as _P

exec(open("sweep.py").read().split("def main():")[0])
E = _P(r"J:\code\marengo\cad\exports\left_arm")
export = json.loads((HERE.parent / "cad_export.json").read_text())
raw = json.loads((HERE.parent / "component_boxes_mm.json").read_text())
boxes = {k.replace("parts/", "").replace(".SLDPRT", ""): v for k, v in raw.items() if k.startswith("parts/")}
S = np.array([38.0, 0, 0])
mm = lambda t: t * 1000 if np.ptp(t.reshape(-1, 3), axis=0).max() < 5 else t
Z = 5.986
RS = world("ELBOW_RS02", read_stl(HERE / "stl" / "ELBOW_RS02.STL"), export["components"], boxes) - S
XS = np.arange(116, 212, 0.2)
YS = np.arange(140, 262, 0.2)


def section(tris):
    """Even-odd fill of the slice of a closed mesh by Z = 5.986 on the X, Y grid."""
    t = tris.reshape(-1, 3, 3)
    z = t[:, :, 2] - Z - 1e-5
    keep = (z.min(1) < 0) & (z.max(1) > 0)
    t, z = t[keep], z[keep]
    segs = []
    for a, b in ((0, 1), (1, 2), (2, 0)):
        c = z[:, a] * z[:, b] < 0
        s = z[c, a] / (z[c, a] - z[c, b])
        p = t[c, a] + (t[c, b] - t[c, a]) * s[:, None]
        segs.append((np.where(c)[0], p[:, :2]))
    idx = np.concatenate([s[0] for s in segs])
    pts = np.concatenate([s[1] for s in segs])
    o = np.argsort(idx, kind="stable")
    idx, pts = idx[o], pts[o]
    pairs = pts[:-1:1][(idx[1:] == idx[:-1])], pts[1:][(idx[1:] == idx[:-1])]
    A, B = pairs
    img = np.zeros((len(YS), len(XS)), bool)
    for i, y in enumerate(YS):
        m = (A[:, 1] - y) * (B[:, 1] - y) < 0
        x = np.sort(A[m, 0] + (y - A[m, 1]) / (B[m, 1] - A[m, 1]) * (B[m, 0] - A[m, 0]))
        for j in range(0, len(x) - 1, 2):
            img[i, (XS >= x[j]) & (XS <= x[j + 1])] = True
    return img


def draw(ax, ua, fk, title, br, ring=(145.7, 147.7), rs_dx=-4.0):
    rs = (RS.reshape(-1, 3) + np.array([rs_dx, 0, 0])).reshape(RS.shape)
    for img, col, z in ((section(ua), "#d8dce2", 2), (section(fk), "#f2c89a", 2), (section(rs), "#6b7078", 1)):
        rgba = np.zeros((*img.shape, 4))
        rgba[img] = matplotlib.colors.to_rgba(col)
        ax.imshow(rgba, origin="lower", extent=(XS[0], XS[-1], YS[0], YS[-1]), zorder=z, interpolation="nearest")
    cy = 209.861
    for sgn in (1, -1):
        y0 = cy + sgn * 20 if sgn > 0 else cy - 26
        ax.add_patch(Rectangle((br, y0), 7, 6, fc="#2666e6", ec="k", lw=.6, zorder=4))
        y1 = cy + 33 if sgn > 0 else cy - 42
        ax.add_patch(Rectangle((ring[0], y1), 2, 9, fc="#d9a821", ec="k", lw=.6, zorder=4))
    ax.axvline(170.49, color="#b03020", lw=1, ls="--", zorder=5)
    ax.axhline(cy, color="#555", lw=.6, ls=":", zorder=5)
    ax.set_title(title, fontsize=13, loc="left")
    ax.set_xlim(116, 212)
    ax.set_ylim(140, 262)
    ax.set_aspect("equal")
    ax.set_xlabel("X, arm coordinates, mm (inboard left, outboard right)")
    ax.grid(alpha=.15, zorder=0)


fig, axs = plt.subplots(1, 3, figsize=(22, 9.4), dpi=200)
draw(axs[0], mm(read_stl(HERE / "stl_wide" / "UPPER_ARM.stl")), mm(read_stl(HERE / "stl_wide" / "FORK.stl")),
     "Before: 78.5 mm, X 122.7 to 201.2, bearing beside the fork", 124.7)
draw(axs[1], mm(read_stl(HERE / "stl_comp75" / "UPPER_ARM.stl")), mm(read_stl(HERE / "stl_comp75" / "FORK.stl")),
     "Pass 1: 75.5 mm, X 127.7 to 203.2, bearing nested in the carrier", 139.0)
draw(axs[2], mm(read_stl(HERE / "stl" / "UPPER_ARM.stl")), mm(read_stl(E / "marengo_forearm_left.stl")),
     "Pass 2: 75.5 mm, X 131.7 to 207.2, stack 4 mm outboard", 143.0, (149.7, 151.7), 0.0)
axs[0].set_ylabel("Y, mm")
fig.suptitle("Elbow section through the axis, plane Z 5.986. Gray RS02, white upper arm and carrier, tan fork, gold steel ring, blue 6808. Red dashes: arm axis. DRAFT, unreviewed.", fontsize=12)
fig.tight_layout(rect=[0, 0.01, 1, 0.95])
fig.savefig(HERE.parent / "views" / "sheets" / "elbow_compaction_study.png")
