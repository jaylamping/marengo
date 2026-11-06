"""Elbow pass 2: extend the outboard gusset to Z 4.486, closing the notch at the outboard cheek root."""
import sys
from sk import *
depth = float(sys.argv[1]) if len(sys.argv) > 1 else 30.5
sw = app()
e, w = L(), L()
d = sw.OpenDoc6(FK, 1, 1, '', e, w)
f = d.FeatureByName('GUSSET_OUT')
x = f.GetDefinition
print('depth', round(x.GetDepth(True) * 1000, 3))
x.SetDepth(True, depth / 1000)
print('modify', f.ModifyDefinition(x, d, NUL), 'rebuild', d.ForceRebuild3(False))
x = d.FeatureByName('GUSSET_OUT').GetDefinition
print('depth now', round(x.GetDepth(True) * 1000, 3))
for b in d.GetBodies2(0, True) or []:
    print(b.Name, [round(v * 1000, 2) for v in b.GetBodyBox()])
