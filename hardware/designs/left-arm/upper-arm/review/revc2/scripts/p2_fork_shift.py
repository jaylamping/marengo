"""Elbow pass 2: shift the fork stack 4 mm outboard, tube stays on the arm axis."""
from sk import *
DX = 4
PLANES = ['P_CHEEK_IN', 'P_RETAINER', 'P_CHEEK_FACE', 'P_STUB', 'P_OUT_CHEEK']
SKETCHES = ['LOFT_B', 'FORK_GAP_SK', 'INNER_B', 'GUSSET_SK', 'GUSSET_OUT_SK', 'GUSSET_IN_SK']
sw = app()
e, w = L(), L()
d = sw.OpenDoc6(FK, 1, 1, '', e, w)
print('open', e.value, w.value, d.GetPathName)
for n in PLANES:
    f = d.FeatureByName(n)
    x = f.GetDefinition
    before = x.Distance
    x.Distance = before + DX / 1000
    print(n, round(before * 1000, 3), '->', f.ModifyDefinition(x, d, NUL), round(x.Distance * 1000, 3))
for n in SKETCHES:
    move(d, n, DX)
print('rebuild', d.ForceRebuild3(False))
b = d.GetBodies2(0, True) or []
print('bodies', [[round(v * 1000, 2) for v in s.GetBodyBox()] for s in b])
print('dirty', d.GetSaveFlag)
