#!/usr/bin/env python3
"""Outer-surface section curves for SolidWorks lofts: one CSV per chest panel, robot frame mm (X fwd, Y left, Z up from floor).
Rows: section index, x, y, z. Each section is a constant-Z curve; loft through them, then thicken 2.4 mm inward."""
import json, pathlib, numpy as np
OUT = pathlib.Path('../../manufacturing/sections'); OUT.mkdir(parents=True, exist_ok=True)
G = {g['tag']: g for g in json.load(open('out/grids.json'))['grids']}
def grid(t):
    g = G[t]; return np.array(g['pts']).reshape(g['nz'] + 1, g['nt'] + 1, 3)
UF, UB, LF, LB = grid('chest upper front'), grid('chest upper back'), grid('chest lower front'), grid('chest lower back (service hatch)')
def dump(name, A, nsec=24, npt=41):
    ii = np.unique(np.linspace(0, A.shape[0] - 1, nsec).round().astype(int)); jj = np.unique(np.linspace(0, A.shape[1] - 1, npt).round().astype(int))
    with open(OUT / f'{name}.csv', 'w') as f:
        f.write('section,x,y,z\n')
        for k, i in enumerate(ii):
            for j in jj: f.write(f'{k},{A[i, j, 0]:.3f},{A[i, j, 1]:.3f},{A[i, j, 2]:.3f}\n')
n = UF.shape[1] - 1; m = LF.shape[1] - 1
dump('upper_front_L', UF[:, : n // 2 + 1], 40); dump('upper_front_R', UF[:, n // 2:], 40)
dump('upper_back_L', UB[:, n // 2:], 40); dump('upper_back_R', UB[:, : n // 2 + 1], 40)
dump('lower_front_L', LF[:, : m // 2 + 1]); dump('lower_front_R', LF[:, m // 2:])
dump('lower_back_L', LB[:, m // 2:]); dump('lower_back_R', LB[:, : m // 2 + 1])
print(sorted(p.name for p in OUT.iterdir()))
