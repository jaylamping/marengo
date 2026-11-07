"""Build the neck concept parts in this session's SolidWorks instance. Torso frame, mm (see neck_geom.py).

Most parts are modelled where they sit at the neutral pose, so the assembly places them with an identity
transform. horn, rod and pinion are modelled about their own axes.
Run through run_all.py, or: python build_parts.py <solidworks session pid file> [part ...]
"""
import sys
from math import acos, atan2, cos, hypot, pi, radians, sin, sqrt, tan
from pathlib import Path

import pythoncom
import win32com.client as wc

import neck_geom as g

sys.path.insert(0, r"J:\code\marengo\hardware\tools\solidworks")
from sw_instance import attach

CAD = Path(r"J:\code\marengo\cad\neck")
NULL = wc.VARIANT(pythoncom.VT_DISPATCH, None)
PLANES = {"front": "Front Plane", "top": "Top Plane", "right": "Right Plane"}
MM = 1e-3
BLIND, START_PLANE, START_OFFSET = 0, 0, 3
Y = g.y


class Part:
    def __init__(self, sw, name, process="print", material="PETG"):
        self.sw, self.path = sw, str(CAD / f"marengo_neck_{name}.SLDPRT")
        self.m = sw.NewDocument(sw.GetUserPreferenceStringValue(8), 0, 0, 0)
        self.m.Extension.SetUserPreferenceInteger(261, 0, 1)  # swUnitSystem = MMGS
        self.fm, self.sm = self.m.FeatureManager, self.m.SketchManager
        self.m.SaveAs3(self.path, 0, 1)
        pm = self.m.Extension.CustomPropertyManager("")
        for k, v in dict(process=process, material=material, revision="concept", owner="Joey").items():
            pm.Add3(k, 30, v, 1)

    def save(self):
        err = wc.VARIANT(pythoncom.VT_BYREF | pythoncom.VT_I4, 0)
        warn = wc.VARIANT(pythoncom.VT_BYREF | pythoncom.VT_I4, 0)
        if not self.m.Save3(1, err, warn):
            raise RuntimeError(f"save failed {err.value}")

    def sketch(self, plane):
        self.m.ClearSelection2(True)
        if not self.m.Extension.SelectByID2(PLANES[plane], "PLANE", 0, 0, 0, False, 0, NULL, 0):
            raise RuntimeError(plane)
        self.sm.InsertSketch(True)
        self.sm.AddToDB = True
        self.plane = plane

    def extrude(self, a, b, name, cut=False, merge=True):
        """Close the sketch and extrude along the plane normal from coordinate a to b (a < b)."""
        self.sm.AddToDB = False
        t0, off, flip_off = (START_OFFSET, abs(a) * MM, a < 0) if a else (START_PLANE, 0.0, False)
        tail = (BLIND, BLIND, (b - a) * MM, 0.0, False, False, False, False, 0.0, 0.0, False, False, False, False)
        if cut:  # a cut runs against the sketch normal unless Dir is set
            f = self.fm.FeatureCut4(True, False, True, *tail, False, True, True, False, False, False, t0, off, flip_off, False)
        else:
            f = self.fm.FeatureExtrusion2(True, False, False, *tail, merge, True, True, t0, off, flip_off)
        if f is None:
            raise RuntimeError(f"feature failed: {name}")
        f.Name = name
        self.m.ClearSelection2(True)
        self.save()  # rule 5: save after each step
        print(" ", name, round(self.m.Extension.CreateMassProperty.Volume * 1e9), [round(v * 1e3, 1) for v in self.m.GetPartBox(True)], flush=True)

    # Top Plane sketch coordinates are (X, -Z); these helpers take torso X, Z
    def circle(self, x, z, r):
        self.sm.CreateCircleByRadius(x * MM, -z * MM, 0, r * MM)

    def rect(self, x0, x1, z0, z1):
        self.sm.CreateCornerRectangle(x0 * MM, -z0 * MM, 0, x1 * MM, -z1 * MM, 0)

    def poly(self, pts):
        for (x0, z0), (x1, z1) in zip(pts, pts[1:] + pts[:1]):
            self.sm.CreateLine(x0 * MM, -z0 * MM, 0, x1 * MM, -z1 * MM, 0)

    def ring(self, ri, ro, y0, y1, name, cut=False):
        self.sketch("top")
        self.circle(0, 0, ro)
        if ri:
            self.circle(0, 0, ri)
        self.extrude(y0, y1, name, cut)

    def box(self, lo, hi, name, cut=False):
        self.sketch("top")
        self.rect(lo[0], hi[0], lo[2], hi[2])
        self.extrude(lo[1], hi[1], name, cut)

    def done(self):
        self.save()
        self.sw.CloseDoc(self.m.GetTitle)
        print(Path(self.path).name, flush=True)


def involute_gear(teeth, m=1.0, backlash=0.1, cx=0.0, step=None):
    """Closed outline of a 20 deg spur gear, 5 points a flank, rotated so a tooth sits at angle step."""
    a = radians(20)
    rp, rb, ra, rf = m * teeth / 2, m * teeth / 2 * cos(a), m * teeth / 2 + m, m * teeth / 2 - 1.25 * m
    inv = lambda p: tan(p) - p
    half = pi / (2 * teeth) - backlash / (2 * rp * 2) + inv(a)   # half tooth angle at the base circle
    r_lo = max(rb, rf)
    rs = [r_lo + (ra - r_lo) * i / 4 for i in range(5)]
    flank = [(r, half - inv(acos(rb / r))) for r in rs]
    pts = []
    for k in range(teeth):
        c = 2 * pi * k / teeth + (step or 0)
        one = [(rf, c - half)] + [(r, c - t) for r, t in flank] + [(r, c + t) for r, t in reversed(flank)] + [(rf, c + half)]
        pts += [(cx + r * cos(t), r * sin(t)) for r, t in one]
    return pts


def rails(sw):
    p = Part(sw, "ref_rails", "purchase", "6063-T5")
    for s in (-1, 1):
        p.box((-93, 475, min(s * 50, s * 70)), (93, 495, max(s * 50, s * 70)), f"rail_{'front' if s > 0 else 'rear'}")
    p.done()


def collar(sw):
    p = Part(sw, "ref_collar")
    p.ring(52, 75, Y(0), Y(8), "collar_lower")
    p.ring(52, 68, Y(8), Y(14), "collar_upper")
    p.done()


def bearing(sw):
    p = Part(sw, "bearing_61814", "purchase", "steel")
    p.ring(35, 38.6, Y(4), Y(14), "inner_race")
    p.ring(41.4, 45, Y(4), Y(14), "outer_race")
    p.ring(38.6, 41.4, Y(5), Y(13), "seal")
    p.done()


def tower(sw):
    p = Part(sw, "tower")
    # foot on the two top rails, plus the strip that carries the yaw servo pod
    p.box((-42, 495, -68), (42, 499, 68), "foot")
    p.box((42, 495, -41), (72, 499, 16), "foot_pod")
    p.ring(49, 60, 499, Y(0), "wall")
    p.ring(49, 52, Y(0), Y(3), "neck")
    p.ring(43.5, 52, Y(3), Y(4), "lip")
    p.ring(45, 52, Y(4), Y(14), "bearing_seat")
    # pod walls round the yaw servo
    p.box((44, 495, -41), (72, Y(-9.2), -38), "pod_rear")
    p.box((44, 495, 13), (72, Y(-9.2), 16), "pod_front")
    p.box((69.5, 495, -41), (72, Y(-9.2), 16), "pod_side")
    p.sketch("top")
    p.rect(44, 72, -41, 16)
    p.circle(56, 0, 11)
    p.extrude(Y(-12.2), Y(-9.2), "pod_shelf")
    p.ring(0, 49, 495, Y(-9.5), "bore", cut=True)
    p.box((43, 495, -38), (69.5, Y(-12.2), 13), "servo_pocket", cut=True)
    p.box((40, Y(-9.2), -16), (75, Y(0.8), 16), "pinion_window", cut=True)
    p.sketch("top")
    for x in (-30, 30):
        for z in (-60, 60):
            p.circle(x, z, 2.75)
    p.extrude(495, 499, "m5_clearance", cut=True)
    p.done()


def tube(sw):
    p = Part(sw, "yaw_tube")
    # head X is torso Z. D plate carries the post; the rods pass behind it
    p.ring(0, 30.5, Y(34), Y(40), "post_plate")
    p.box((-31, Y(34), -31), (31, Y(40), -6), "rod_clearance", cut=True)
    p.sketch("top")
    for x in (-13, 13):
        p.circle(x, 13, 5)
    p.extrude(Y(34), Y(40), "cable_holes", cut=True)
    p.ring(30, 35, Y(4), Y(14), "journal")
    p.ring(30, 39, Y(14), Y(17), "flange")
    p.ring(30, 33, Y(17), Y(48), "sleeve")
    p.ring(0, 7, Y(40), Y(62), "post")
    p.box((-14, Y(60), -6), (14, Y(64), 6), "yoke_base")
    for s in (-1, 1):
        p.box((min(s * 11, s * 14), Y(64), -6), (max(s * 11, s * 14), Y(86), 6), f"yoke_ear_{s + 1}")
    p.sketch("right")
    p.sm.CreateCircleByRadius(0, Y(g.UJ_H) * MM, 0, 3.1 * MM)
    p.extrude(-15, 15, "pitch_pin_hole", cut=True)
    p.done()


def cage(sw):
    p = Part(sw, "servo_cage")
    floor_lo = g.SERVO_B_H - g.SERVO_THICK / 2
    p.ring(0, 42.6, Y(floor_lo - 3), Y(floor_lo), "floor")
    p.ring(41, 42.6, Y(floor_lo), Y(1.5), "wall")
    p.ring(33, 42.6, Y(1.5), Y(3.6), "top_plate")
    p.ring(33, 39, Y(3.6), Y(4), "race_seat")
    # shelf between the servos, ahead of the rods
    p.sketch("top")
    half = sqrt(41.5 ** 2 - 8 ** 2)
    p.sm.CreateArc(0, 0, 0, -half * MM, 8 * MM, 0, half * MM, 8 * MM, 0, 1)   # sketch y = -Z: arc runs round the front
    p.sm.CreateLine(-half * MM, 8 * MM, 0, half * MM, 8 * MM, 0)
    p.extrude(Y(g.SERVO_A_H - g.SERVO_THICK / 2 - g.GAP), Y(g.SERVO_A_H - g.SERVO_THICK / 2), "shelf")
    p.sketch("top")
    p.poly(involute_gear(88))
    p.circle(0, 0, 41.5)
    p.extrude(Y(-8), Y(-2), "ring_gear")
    p.done()


def pinion(sw):
    p = Part(sw, "yaw_pinion")
    p.sketch("top")
    p.poly(involute_gear(24, step=pi + pi / 24))   # a gap faces the ring tooth at +X when set at X 56
    p.circle(0, 0, 4.6)
    p.extrude(Y(-8), Y(-2), "pinion")
    p.done()


def horn(sw):
    """Pivot at the origin, level arm toward -X, thickness along Z."""
    p = Part(sw, "horn")
    p.sketch("front")
    r = 10.0
    p.sm.CreateCircleByRadius(0, 0, 0, r * MM)
    p.sm.CreateCircleByRadius(0, 0, 0, 4.6 * MM)
    p.extrude(-1.5, 1.5, "hub")
    p.sketch("front")
    p.sm.CreateCornerRectangle(-g.HORN_R * MM, -4 * MM, 0, -9 * MM, 4 * MM, 0)
    p.extrude(-1.5, 1.5, "arm")
    p.done()


def rod(sw, k):
    """Lower ball at the origin, upper ball at +Y rod length."""
    L = g.rod_len(k)
    p = Part(sw, f"rod_{k.lower()}", "purchase", "steel")
    p.sketch("top")
    p.circle(0, 0, g.ROD_R)
    p.extrude(0, L, "rod")
    for name, c in (("link_low", 0.0), ("link_high", L)):
        p.sketch("top")
        p.circle(0, 0, g.BALL_R)
        p.extrude(c - 6, c + 6, name)
    p.done()


def cross(sw):
    p = Part(sw, "ujoint_cross", "purchase", "steel")
    uy = Y(g.UJ_H)
    p.box((-8, uy - 8, -8), (8, uy + 8, 8), "block")
    p.sketch("right")
    p.sm.CreateCircleByRadius(0, uy * MM, 0, 3 * MM)
    p.extrude(-14.5, 14.5, "pitch_pin")
    p.sketch("front")
    p.sm.CreateCircleByRadius(0, uy * MM, 0, 3 * MM)
    p.extrude(-14.5, 14.5, "roll_pin")
    p.done()


def plate(sw):
    p = Part(sw, "head_plate")
    uy = Y(g.UJ_H)
    p.ring(0, 30, Y(96), Y(100), "plate")
    for s in (-1, 1):
        p.box((-6, Y(74), min(s * 11, s * 14)), (6, Y(96), max(s * 11, s * 14)), f"roll_ear_{s + 1}")
        c = s * g.STUD_SIDE
        p.box((c - 4, Y(86.2), g.STUD_FWD - 5), (c + 4, Y(96), g.STUD_FWD + 5), f"stud_lug_{s + 1}")
    p.sketch("front")
    p.sm.CreateCircleByRadius(0, uy * MM, 0, 3.1 * MM)
    p.extrude(-15, 15, "roll_pin_hole", cut=True)
    p.done()


def bellows(sw):
    p = Part(sw, "bellows", "print", "TPU")
    p.ring(33.5, 36, Y(17), Y(66), "wall")
    for h in (18, 27, 36, 45):
        p.ring(33.5, 41, Y(h), Y(h + 4), f"rib_{h}")
    p.done()


BUILD = dict(rails=rails, collar=collar, bearing=bearing, tower=tower, tube=tube, cage=cage, pinion=pinion,
             horn=horn, rod_a=lambda s: rod(s, "A"), rod_b=lambda s: rod(s, "B"), cross=cross, plate=plate, bellows=bellows)

if __name__ == "__main__":
    inst = attach(int(open(sys.argv[1]).read().strip()))
    print("pid", inst.pid, flush=True)
    for name in (sys.argv[2:] or BUILD):
        print(name, flush=True)
        BUILD[name](inst.sw)
