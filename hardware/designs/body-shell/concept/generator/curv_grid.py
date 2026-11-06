#!/usr/bin/env python3
"""Tightest convex radius on the dense outer grid (analytic surface, ~1 mm spacing), chest upper front, left half.
Usage: curv_grid.py grids_dense.json [tag] [fit radius]"""
import sys, json, numpy as np
from scipy.spatial import cKDTree
G = {g['tag']: g for g in json.load(open(sys.argv[1]))['grids']}
tag = sys.argv[2] if len(sys.argv) > 2 else 'chest upper front'; RAD = float(sys.argv[3]) if len(sys.argv) > 3 else 3.0
g = G[tag]; A = np.array(g['pts']).reshape(g['nz'] + 1, g['nt'] + 1, 3)
A = A[3:-3, 8:-8:2]
di, dj = np.gradient(A, axis=0), np.gradient(A, axis=1); N = np.cross(di, dj); N /= np.linalg.norm(N, axis=2, keepdims=True)
rad = A.copy(); rad[..., 0] -= 4; rad[..., 2] = 0
if np.sum(N * rad) < 0: N = -N
P, N = A.reshape(-1, 3), N.reshape(-1, 3); T = cKDTree(P)
sel = np.where((P[:, 1] > 60) & (P[:, 0] > 0))[0] if 'front' in tag else np.where(P[:, 1] > 60)[0]
out = []
for i in sel:
    p, n = P[i], N[i]; idx = T.query_ball_point(p, RAD)
    u = np.cross(n, [0, 0, 1.0]); u /= np.linalg.norm(u); v = np.cross(n, u)
    Q = P[idx] - p; x, y, h = Q @ u, Q @ v, Q @ n
    M = np.stack([x * x, x * y, y * y, x, y, np.ones_like(x)], 1); a, b, c = np.linalg.lstsq(M, h, rcond=None)[0][:3]
    kc = -np.linalg.eigvalsh([[2 * a, b], [b, 2 * c]]).min()
    if kc > 0: out.append((1 / kc, i))
out.sort(); seen = []
sock = [(r, i) for r, i in out if np.hypot(P[i, 0], P[i, 2] - 1234.9) < 120 and P[i, 1] > 110]
if sock: print(f'socket zone (r<120 about the pitch axis): min convex R {sock[0][0]:.2f} at {np.round(P[sock[0][1]], 1)}')
print(f'{tag}: min convex R {out[0][0]:.2f}')
for r, i in out:
    if r > 6 or len(seen) > 8: break
    p = P[i]
    if all(np.linalg.norm(p - q) > 12 for q in seen): seen.append(p); print(f'   R {r:5.2f} at x {p[0]:6.1f} y {p[1]:6.1f} z {p[2]:7.1f}')
