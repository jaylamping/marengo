"""Upper arm clamshell master: copy the db71b23b part, find what makes the cable window and the cover, and report bodies.
usage: python ua_master.py probe | python ua_master.py build FEAT1,FEAT2,...  (features to suppress)"""
import shutil, sys, os
from sk import *
sw = app()
sw.CloseAllDocuments(True)
SRC = r'J:\code\marengo\cad\parts\left_arm\marengo_upper_arm_left.SLDPRT'
DST = r'J:\code\marengo\cad\parts\left_arm\marengo_upper_arm_clam_master_left.SLDPRT'
EXP = r'C:\Users\joeyl\Documents\Codex\2026-10-29\task\LEFT_ARM_V1_LAYOUT_20261106\pkg\stl\ua'
os.makedirs(EXP, exist_ok=True)
shutil.copy(SRC, DST)
e, w = L(), L()
d = sw.OpenDoc6(DST, 1, 1, '', e, w)
print('open', e.value, flush=True)


def bodies():
    out = []
    for b in d.GetBodies2(0, True):
        bb = [round(v * 1e3, 1) for v in b.GetBodyBox()]
        out.append((b.Name, round(b.GetMassProperties(1190)[3] * 1e9 / 1000, 2), bb))
    return out


for b in bodies():
    print('body', b, flush=True)
if sys.argv[1] == 'build':
    for n in sys.argv[2].split(','):
        f = d.FeatureByName(n)
        if f is None:
            print('missing', n); continue
        d.ClearSelection2(True); f.Select2(False, 0)
        print('suppress', n, d.EditSuppress2, flush=True)
    d.ForceRebuild3(False)
    for b in bodies():
        print('after', b, flush=True)
    e, w = L(), L()
    print('save', d.Save3(1, e, w))
    d.Extension.SaveAs(EXP + r'\ua_master.STL', 0, 1, NUL, e, w)
sw.CloseDoc(d.GetTitle)
