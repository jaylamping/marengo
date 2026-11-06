"""Elbow pass 2: solid interference across elbow travel, plus RS00 turned to the lower-arm yaw limits. Writes envelope/interference_p2.json."""
import json
import sys
from pathlib import PureWindowsPath as PW
import numpy as np
from swlib import *
from params import *
NUL = VARIANT(pythoncom.VT_DISPATCH, None)
m = 1e-3
R = 'J:/code/marengo/cad/exports/left_arm/render/'
L_ = 'J:/code/marengo/cad/parts/left_arm/'
ID = [1, 0, 0, 0, 1, 0, 0, 0, 1]
E2 = ([0, 0, -1, 0, 1, 0, 1, 0, 0], (AX + 22.7, EY, AZ))
F0 = ([1, 0, 0, 0, 0, -1, 0, 1, 0], (AX, EY - 94.2 - RS00_OFFSET, AZ))
A, AE = (ID, (FW, 0, 0)), (ID, (FW + 4, 0, 0))
ARM = [('UA', R + 'UA2_ARM.SLDPRT', A, 'u'), ('COVER', R + 'UA2_COVER.SLDPRT', A, 'u'), ('CARRIER', R + 'UA2_CARRIER.SLDPRT', A, 'u'),
       ('GOLD_EL', R + 'HW_GOLD_EL.SLDPRT', AE, 'u'), ('BLUE_EL', R + 'HW_BLUE_EL.SLDPRT', AE, 'u'),
       ('ELBOW', W / 'parts/ELBOW_RS02.SLDPRT', E2, 'u'), ('FORK', L_ + 'marengo_forearm_left.SLDPRT', A, 'f'), ('RS00', W / 'parts/FOREARM_RS00.SLDPRT', F0, 'f')]
PATH = 'J:/code/marengo/cad/exports/left_arm/render/SWEEP.SLDASM'
OUT = W / 'envelope' / 'interference_p2.json'


def rot(axis, deg):
    x, y, z = axis
    c = np.radians(deg)
    k = np.array([[0, -z, y], [z, 0, -x], [-y, x, 0]], float)
    return np.eye(3) + np.sin(c) * k + (1 - np.cos(c)) * k @ k


sw = app()
for _, p, _, _ in ARM:
    e, w = L(), L()
    sw.OpenDoc6(str(PW(p)), 1, 1, '', e, w)
a = sw.NewDocument(sw.GetUserPreferenceStringValue(9), 0, 0, 0)
e, w = L(), L()
a.Extension.SaveAs(str(PW(PATH)), 0, 1, NUL, e, w)
print('new', e.value)
comp = {}
for n, p, (rows, t), g in ARM:
    c = a.AddComponent5(str(PW(p)), 0, '', False, '', 0, 0, 0)
    comp[n] = (c, np.array(rows, float).reshape(3, 3), np.array(t, float), g)
for n, (c, _, _, _) in comp.items():
    print(n, c.Name2)
names = {c.Name2: n for n, (c, _, _, _) in comp.items()}
O = np.array([AX, EY, AZ])
Y = np.array([AX, 0, AZ])


def place(n, Rm, t):
    c = comp[n][0]
    q = c.Transform2
    q.ArrayData = VARIANT(pythoncom.VT_ARRAY | pythoncom.VT_R8, [*Rm.ravel().tolist(), t[0] * m, t[1] * m, t[2] * m, 1., 0., 0., 0.])
    c.Transform2 = q


def pose(elbow, yaw=0):
    Rt = rot((-1, 0, 0), elbow).T
    Ry = rot((0, 1, 0), yaw).T
    for n, (c, Rm, t, g) in comp.items():
        if n == 'RS00':
            Rm, t = Rm @ Ry, (t - Y) @ Ry + Y
        if g == 'f':
            Rn, tn = Rm @ Rt, (t - O) @ Rt + O
        else:
            Rn, tn = Rm, t
        place(n, Rn, tn)
    a.EditRebuild3


def check():
    im = a.InterferenceDetectionManager
    im.TreatCoincidenceAsInterference = False
    im.TreatSubAssembliesAsComponents = True
    im.IncludeMultibodyPartInterferences = False
    im.MakeInterferingPartsTransparent = False
    im.CreateFastenersFolder = False
    im.IgnoreHiddenBodies = False
    im.ShowIgnoredInterferences = False
    im.UseTransform = False
    n = im.GetInterferenceCount
    rows = []
    for x in (im.GetInterferences or []):
        cs = [names.get(c.Name2, c.Name2) for c in x.Components]
        rows.append([sorted(cs), round(x.Volume * 1e9, 2)])
    im.Done()
    return n, sorted(rows)


cases = [(e, 0) for e in list(range(0, 131, 5)) + [133]] + [(e, y) for e in (0, 90, 133) for y in (-60, 60)]
res = []
for e, y in cases:
    t = time.time()
    pose(e, y)
    n, rows = check()
    res.append({'elbow': e, 'yaw': y, 'n': n, 'rows': rows})
    print(e, y, n, rows, round(time.time() - t, 1), 's', flush=True)
    OUT.write_text(json.dumps(res, indent=0))
pose(0)
sw.CloseDoc(a.GetTitle)
print('docs', sw.GetDocumentCount)
