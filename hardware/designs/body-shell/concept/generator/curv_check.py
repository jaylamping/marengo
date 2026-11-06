#!/usr/bin/env python3
"""Principal curvature of the outer surface from the SolidWorks section CSVs (what the loft actually goes through).
Local quadric fit over neighbours within RAD mm; reports the tightest CONVEX radius (surface bulging outward, which is
what blocks an inward thicken) and every point under LIMIT. Usage: curv_check.py panel.csv [limit=4] [rad=4]"""
import sys, numpy as np
from scipy.spatial import cKDTree
f = sys.argv[1]; LIMIT = float(sys.argv[2]) if len(sys.argv) > 2 else 4.0; RAD = float(sys.argv[3]) if len(sys.argv) > 3 else 4.0
D = np.loadtxt(f, delimiter=',', skiprows=1); S = D[:, 0].astype(int); P = D[:, 1:]
ns = S.max() + 1; npt = len(P) // ns; A = P.reshape(ns, npt, 3)
di, dj = np.gradient(A, axis=0), np.gradient(A, axis=1); N = np.cross(di, dj); N /= np.linalg.norm(N, axis=2, keepdims=True)
c = A.reshape(-1, 3).mean(0); rad = A - c; rad[..., 2] = 0     # outward ~ away from the torso's vertical axis
if np.sum(N * rad) < 0: N = -N
inner = np.zeros(A.shape[:2], bool); inner[2:-2, 3:-3] = True   # one-sided fits at the panel edges are meaningless
P, N, inner = A.reshape(-1, 3), N.reshape(-1, 3), inner.reshape(-1)
# densify along both directions so the fit has neighbours in the coarse (section) direction too
T = cKDTree(P); out = []
for i, (p, n) in enumerate(zip(P, N)):
    if not inner[i]: continue
    idx = T.query_ball_point(p, RAD)
    if len(idx) < 8: continue
    u = np.cross(n, [0, 0, 1.0]); u = u if np.linalg.norm(u) > 0.1 else np.cross(n, [1.0, 0, 0]); u /= np.linalg.norm(u); v = np.cross(n, u)
    Q = P[idx] - p; x, y, h = Q @ u, Q @ v, Q @ n
    M = np.stack([x * x, x * y, y * y, x, y, np.ones_like(x)], 1)
    try: a, b, cc, *_ = np.linalg.lstsq(M, h, rcond=None)[0]
    except Exception: continue
    k = np.linalg.eigvalsh([[2 * a, b], [b, 2 * cc]])   # h curving toward -n (inward) = convex -> negative k
    kc = -k.min()
    if kc > 0: out.append((1 / kc, i))
out.sort(); r0 = out[0][0]
print(f'{f.split("/")[-1]}: min convex R {r0:.2f} mm at {np.round(P[out[0][1]], 1)}  ({sum(r < LIMIT for r, _ in out)} pts < {LIMIT})')
seen = []
for r, i in out:
    if r >= LIMIT: break
    p = P[i]
    if all(np.linalg.norm(p - q) > 15 for q in seen): seen.append(p); print(f'   R {r:5.2f} at x {p[0]:6.1f} y {p[1]:6.1f} z {p[2]:7.1f}')
