"""Mesh gate and support-on-visible-face check for the upper-arm clamshell parts in their print orientations."""
import sys, json, numpy as np, trimesh
sys.path[:0] = ['../_repo/hardware/tools', '../_repo/hardware/designs/left-arm/prints']
import mesh_check
from mesh_whitelist import WHITELIST
from support_check import check
U = 'stl/ua/'
UP = {'ua_upper': [0, 0, 1], 'ua_lower': [0, 0, -1], 'ua_ring': [1, 0, 0], 'ua_hub': [0, -1, 0], 'ua_loop_clip': [0, -0.70710678, -0.70710678]}
ref = trimesh.util.concatenate([trimesh.load(U + n + '.STL', force='mesh') for n in ('ua_upper', 'ua_lower', 'ua_ring', 'ua_hub')] + [trimesh.load('stl/UA2_CARRIER.STL', force='mesh')])
res = {}
for n in sys.argv[1:] or UP:
    m = trimesh.load(U + n + '.STL', force='mesh')
    wl = WHITELIST.get(n, {})
    ok, r = mesh_check.check(m, n, bodies=wl.get('bodies', 1), corners=wl.get('corners', ()), thin=wl.get('thin', ()))
    open(f'ua_gate/{n}_meshcheck.txt', 'w').write(r)
    s, outer, inner = check(m, UP[n], ref=ref if n != 'ua_loop_clip' else None)
    c = m.triangles_center[outer]
    s['outer_where'] = [[round(float(v), 1) for v in c.min(0)], [round(float(v), 1) for v in c.max(0)]] if len(c) else []
    res[n] = dict(mesh='PASS' if ok else 'FAIL', **s)
    print(n, res[n], flush=True)
    if not ok:
        print(r)
json.dump(res, open('ua_gate/summary.json', 'w'), indent=1)
