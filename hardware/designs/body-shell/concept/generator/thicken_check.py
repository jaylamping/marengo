#!/usr/bin/env python3
"""Will a 2.4 mm inward thicken work? Offset the dense outer grid inward along its normals and find quads whose
orientation flips (the offset folds over itself). Usage: thicken_check.py grids_dense.json [wall]"""
import json, sys, numpy as np
src = sys.argv[1]; T = float(sys.argv[2]) if len(sys.argv) > 2 else 2.4
G = {g['tag']: g for g in json.load(open(src))['grids']}
for tag in ('chest upper front', 'chest upper back', 'chest lower front', 'chest lower back (service hatch)'):
    g = G[tag]; A = np.array(g['pts']).reshape(g['nz'] + 1, g['nt'] + 1, 3)
    di, dj = np.gradient(A, axis=0), np.gradient(A, axis=1); n = np.cross(di, dj)
    n /= np.linalg.norm(n, axis=2, keepdims=True) + 1e-12
    rad = A.copy(); rad[..., 0] -= 4; rad[..., 2] = 0
    if np.sum(n * rad) < 0: n = -n
    I = A - T * n
    def qn(P):
        return np.cross(P[1:, :-1] - P[:-1, :-1], P[:-1, 1:] - P[:-1, :-1])
    o, i = qn(A), qn(I)
    flip = np.sum(o * i, axis=2) < 0
    k = np.argwhere(flip)
    pts = A[k[:, 0], k[:, 1]] if len(k) else np.zeros((0, 3))
    print(f'{tag:34s} folds {len(k):5d}', '' if not len(k) else f'z {pts[:,2].min():.0f}-{pts[:,2].max():.0f}, |y| {np.abs(pts[:,1]).min():.0f}-{np.abs(pts[:,1]).max():.0f}, x {pts[:,0].min():.0f}-{pts[:,0].max():.0f}')
