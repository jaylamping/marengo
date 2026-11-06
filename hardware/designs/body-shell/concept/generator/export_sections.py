#!/usr/bin/env python3
"""Outer-surface section curves for SolidWorks lofts: one CSV per chest panel, robot frame mm (X fwd, Y left, Z up from floor).
Rows: section, x, y, z. Each section is a constant-Z curve; loft through them, then thicken 2.4 mm inward.

Input: out/grids_dense.json from `node dump.js "dense=1" out/grids_dense.json` (8x the columns, so the sternum groove is
resolved). Each row is resampled evenly by arc length. Also reports the tightest convex radius in each panel, because
SolidWorks cannot thicken 2.4 mm inward where the outer surface is tighter than that (pilot 2026-11-06)."""
import json, pathlib, numpy as np
OUT = pathlib.Path('../../manufacturing/sections'); OUT.mkdir(parents=True, exist_ok=True)
G = {g['tag']: g for g in json.load(open('out/grids_dense.json'))['grids']}
def grid(t):
    g = G[t]; return np.array(g['pts']).reshape(g['nz'] + 1, g['nt'] + 1, 3)
UF, UB, LF, LB = grid('chest upper front'), grid('chest upper back'), grid('chest lower front'), grid('chest lower back (service hatch)')

def resample(row, n):
    d = np.r_[0, np.cumsum(np.linalg.norm(np.diff(row, axis=0), axis=1))]
    s = np.linspace(0, d[-1], n)
    return np.stack([np.interp(s, d, row[:, k]) for k in range(3)], 1)

def min_radius(curves, axis_x=4.0):
    """Smallest convex radius of curvature over a set of polylines (convex = bending toward the torso axis)."""
    best = (1e9, None)
    for c in curves:
        a, b, e = c[:-2], c[1:-1], c[2:]
        ab, be, ae = np.linalg.norm(b - a, axis=1), np.linalg.norm(e - b, axis=1), np.linalg.norm(e - a, axis=1)
        cross = np.cross(b - a, e - b); area2 = np.linalg.norm(cross, axis=1)
        ok = area2 > 1e-9
        R = np.full(len(b), np.inf); R[ok] = ab[ok] * be[ok] * ae[ok] / (2 * area2[ok])
        # convex if the chord midpoint lies toward the axis from b (surface bulges outward)
        mid = (a + e) / 2; rad = b.copy(); rad[:, 0] -= axis_x; rad[:, 2] = 0
        conv = np.sum((mid - b) * rad, axis=1) < 0
        R[~conv] = np.inf
        i = int(np.argmin(R))
        if R[i] < best[0]: best = (float(R[i]), [round(float(v), 1) for v in b[i]])
    return best

report = {}
def dump(name, A, nsec, npt=121):
    ii = np.unique(np.linspace(0, A.shape[0] - 1, nsec).round().astype(int))
    secs = [resample(A[i], npt) for i in ii]
    with open(OUT / f'{name}.csv', 'w') as f:
        f.write('section,x,y,z\n')
        for k, S in enumerate(secs):
            for p in S: f.write(f'{k},{p[0]:.3f},{p[1]:.3f},{p[2]:.3f}\n')
    cols = [np.array([S[j] for S in secs]) for j in range(npt)]
    report[name] = dict(sections=len(secs), points=npt, min_convex_radius_rows=min_radius(secs), min_convex_radius_cols=min_radius(cols))

n = UF.shape[1] - 1; m = LF.shape[1] - 1
for nm, A, ns in (('upper_front_L', UF[:, : n // 2 + 1], 80), ('upper_front_R', UF[:, n // 2:], 80),
                  ('upper_back_L', UB[:, n // 2:], 80), ('upper_back_R', UB[:, : n // 2 + 1], 80),
                  ('lower_front_L', LF[:, : m // 2 + 1], 40), ('lower_front_R', LF[:, m // 2:], 40),
                  ('lower_back_L', LB[:, m // 2:], 40), ('lower_back_R', LB[:, : m // 2 + 1], 40)):
    dump(nm, A, ns)
json.dump(report, open(OUT / 'sections_report.json', 'w'), indent=1)
for k, v in report.items(): print(k, v['sections'], 'rows R', v['min_convex_radius_rows'], 'cols R', v['min_convex_radius_cols'])
