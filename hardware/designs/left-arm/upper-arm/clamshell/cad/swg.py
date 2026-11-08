"""Generic SolidWorks feature helpers in model (arm) coordinates, mm. Sketch points are mapped through the active sketch's
ModelToSketchTransform, so plane orientation and flips never matter."""
import math
import numpy as np
import pythoncom
from win32com.client import VARIANT
from sk import *

m = 1e-3
BASE = {'X': 'Right Plane', 'Y': 'Top Plane', 'Z': 'Front Plane'}
MID, BLIND, THRU_ALL = 6, 0, 1


def vol(d):
    return sum(b.GetMassProperties(1190)[3] for b in (d.GetBodies2(0, True) or [])) * 1e9


def plane(d, axis, off, name):
    """Plane normal to arm axis 'X'/'Y'/'Z' at coordinate off (mm)."""
    d.ClearSelection2(True); d.FeatureByName(BASE[axis]).Select2(False, 0)
    if abs(off) < 1e-9:
        return d.FeatureByName(BASE[axis])
    p = d.FeatureManager.InsertRefPlane(8 + (256 if off < 0 else 0), abs(off) * m, 0, 0, 0, 0)
    if p is None:
        raise SystemExit('plane failed ' + name)
    p.Name = name
    return p


class Sk:
    """with Sk(d, plane, name) as s: s.poly([...3D pts...]); s.circle(c3d, r)"""

    def __init__(self, d, pl, name):
        self.d, self.pl, self.name = d, pl, name

    def __enter__(self):
        d = self.d
        d.ClearSelection2(True); self.pl.Select2(False, 0)
        d.SketchManager.InsertSketch(True)
        self.T = d.GetActiveSketch2.ModelToSketchTransform.ArrayData
        d.SketchManager.AddToDB = True
        return self

    def p(self, q):
        T = self.T; x, y, z = q[0] * m, q[1] * m, q[2] * m
        return x * T[0] + y * T[3] + z * T[6] + T[9], x * T[1] + y * T[4] + z * T[7] + T[10]

    def poly(self, pts, closed=True):
        P = [self.p(q) for q in pts]
        for a, b in zip(P, (P[1:] + P[:1]) if closed else P[1:]):
            self.d.SketchManager.CreateLine(a[0], a[1], 0, b[0], b[1], 0)

    def circle(self, c, r):
        a = self.p(c)
        self.d.SketchManager.CreateCircleByRadius(a[0], a[1], 0, r * m)

    def arc(self, c, p0, p1, direction):
        a, b, e = self.p(c), self.p(p0), self.p(p1)
        self.d.SketchManager.CreateArc(a[0], a[1], 0, b[0], b[1], 0, e[0], e[1], 0, direction)

    def __exit__(self, *a):
        self.d.SketchManager.AddToDB = False
        self.d.InsertSketch2(True)
        s = self.d.FeatureByPositionReverse(0); s.Name = self.name + '_SK'
        self.feat = s


_APP = []


def sw_app():
    if not _APP:
        _APP.append(app())
    return _APP[0]


def feat(d, sk, name, depth, cut, mode=BLIND, want=None, lo=1.0, flip_side=False):
    """Extrude (cut=False) or cut a sketch. depth > 0 one way; tries both directions and keeps the one whose
    feature faces lie on the 'want' side: want = (axis_index, sign) relative to the sketch plane coordinate.
    mode MID = mid-plane (depth total), THRU_ALL = through all both sides."""
    fm = d.FeatureManager
    tries = (False,) if mode in (MID,) else (False, True)
    for rev in tries:
        v0 = vol(d)
        d.ClearSelection2(True); sk.Select2(False, 0)
        if cut:
            f = fm.FeatureCut4(True, flip_side, rev, mode, 0, abs(depth) * m, abs(depth) * m, False, False, False, False, 0, 0, False, False, False, False, False, False, True, False, False, False, 0, 0, False, False)
        else:
            f = fm.FeatureExtrusion3(True, False, rev, mode, 0, abs(depth) * m, abs(depth) * m, False, False, False, False, 0, 0, False, False, False, False, True, False, True, 0, 0, False)
            if f is None:   # auto-select merge fails on some multi-body states; feature scope without auto-select works
                d.ClearSelection2(True); sk.Select2(False, 0)
                f = fm.FeatureExtrusion3(True, False, rev, mode, 0, abs(depth) * m, abs(depth) * m, False, False, False, False, 0, 0, False, False, False, False, True, True, False, 0, 0, False)
        if f is None:
            print(f'   {name} rev={rev}: no feature', flush=True)
            continue
        d.ForceRebuild3(False)
        dv = (v0 - vol(d)) if cut else (vol(d) - v0)
        print(f'   {name} rev={rev}: dv {dv:.1f}', flush=True)
        ok = dv > lo
        if ok and want is not None:
            ax, sg, c0 = want
            vals = []
            for fc in (f.GetFaces or []):
                b = fc.GetBox; vals += [b[ax] * 1e3, b[ax + 3] * 1e3]
            ok = bool(vals) and (sum(vals) / len(vals) - c0) * sg > 0
        if ok:
            f.Name = name
            print(f'  {name}: {"cut" if cut else "added"} {dv:.1f} mm3', flush=True)
            return f
        d.EditUndo2(1)
    raise SystemExit(name + ' failed')


def revolve(d, sk, name, axis_line_pts, cut, angle=360.0, mid=False, lo=1.0):
    """Revolve a closed sketch that also contains a centreline (added by caller through Sk.centerline)."""
    v0 = vol(d)
    d.ClearSelection2(True); sk.Select2(False, 0)
    f = d.FeatureManager.FeatureRevolve2(True, True, False, cut, False, False, 6 if mid else 0, 0, math.radians(angle), 0, False, False, 0, 0, 0, 0, 0, True, False, True)
    d.ForceRebuild3(False)
    dv = (v0 - vol(d)) if cut else (vol(d) - v0)
    if f is None or dv < lo:
        raise SystemExit(name + ' revolve failed')
    f.Name = name
    print(f'  {name}: {"cut" if cut else "added"} {dv:.1f} mm3', flush=True)
    return f


def centerline(s, a, b):
    p, q = s.p(a), s.p(b)
    s.d.SketchManager.CreateCenterLine(p[0], p[1], 0, q[0], q[1], 0)


def bodies(d):
    return [(b.Name, round(b.GetMassProperties(1190)[3] * 1e6, 2), [round(v * 1e3, 1) for v in b.GetBodyBox()]) for b in d.GetBodies2(0, True)]


def delete_body(d, name):
    for b in d.GetBodies2(0, True):
        if b.Name == name:
            d.ClearSelection2(True)
            print('  select body', d.Extension.SelectByID2(name, 'SOLIDBODY', 0, 0, 0, False, 0, NUL, 0), flush=True)
            f = d.FeatureManager.InsertDeleteBody2(False)
            d.ForceRebuild3(False)
            return f
    raise SystemExit('no body ' + name)
