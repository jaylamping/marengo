"""Build the pack cradle parts in a dedicated SolidWorks instance, in torso coordinates, mm.

X lateral (+X robot left), Y up, Z forward. Right side is -X; the left side is the same build with s = +1.
Run: python build_cradle.py <solidworks session pid file>
"""
import sys
from math import pi
from pathlib import Path

import pythoncom
import win32com.client as wc

sys.path.insert(0, r"J:\code\marengo\hardware\tools\solidworks")
from sw_instance import session

CAD = Path(r"J:\code\marengo\cad\torso")
OUT = Path(r"J:\code\marengo\hardware\designs\torso-frame\cradle")
NULL = wc.VARIANT(pythoncom.VT_DISPATCH, None)
PLANES = {"front": "Front Plane", "top": "Top Plane", "right": "Right Plane"}
MM = 1e-3
BLIND, START_PLANE, START_OFFSET = 0, 0, 3
LEAD_GAP = ((-5, 345, -16), (25, 349, 6))
STRAP_NOTE = "Pack cradle top strap, rev B fit-check, two snap hooks, lead gap X -5 to 25"
SLOT = (-15.5, 5.5, 345.5, 348.9, 0.4)  # Z, Y and corner radius: hook Z -15 to 5 plus 0.5 per end, R0.4 still clears its corners
STOP = 49  # |X| where the full 20 mm bar starts again: 1.2 mm past the tongue tip, inside the 3.5 mm web
SLIT = -9  # rear of the bar: tongue Z -15 to -9, 2 mm slit to the bar at Z -7, so the 21.7 mm bridge cannot fuse to the tongue


class Part:
    def __init__(self, sw, path):
        self.sw, self.path = sw, str(path)
        self.m = sw.NewDocument(sw.GetUserPreferenceStringValue(8), 0, 0, 0)
        self.m.Extension.SetUserPreferenceInteger(261, 0, 1)  # swUnitSystem = MMGS
        self.fm = self.m.FeatureManager
        self.sm = self.m.SketchManager
        self.m.SaveAs3(self.path, 0, 1)

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

    def extrude(self, a, b, name, cut=False, merge=True, draft=None):
        """Close the open sketch and extrude along the plane normal from coordinate a to b (a < b).

        draft = (degrees, outward) tapers a boss from its start face.
        """
        self.sm.AddToDB = False
        t0, off, flip_off = (START_OFFSET, abs(a) * MM, a < 0) if a else (START_PLANE, 0.0, False)
        d = (b - a) * MM
        deg, outward = draft or (0, False)
        tail = (BLIND, BLIND, d, 0.0, bool(deg), False, outward, False, deg * pi / 180, 0.0, False, False, False, False)
        if cut:  # a cut runs against the sketch normal unless Dir is set
            f = self.fm.FeatureCut4(True, False, True, *tail, False, True, True, False, False, False, t0, off, flip_off, False)
        else:
            f = self.fm.FeatureExtrusion2(True, False, False, *tail, merge, True, True, t0, off, flip_off)
        if f is None:
            raise RuntimeError(f"feature failed: {name}")
        f.Name = name
        self.m.ClearSelection2(True)
        self.save()  # rule 5: save after each step
        print(name, round(self.m.Extension.CreateMassProperty.Volume * 1e9), [round(v * 1e3, 2) for v in self.m.GetPartBox(True)], flush=True)

    def box(self, lo, hi, name, cut=False):
        self.sketch("front")
        self.sm.CreateCornerRectangle(lo[0] * MM, lo[1] * MM, 0, hi[0] * MM, hi[1] * MM, 0)
        self.extrude(lo[2], hi[2], name, cut)

    def circle(self, x, y, d):
        self.sm.CreateCircleByRadius(x * MM, y * MM, 0, d / 2 * MM)

    def stadium(self, x, y, w, h):
        """Slot centred on (x, y), overall w x h, rounded on its two short ends."""
        sm, r = self.sm, min(w, h) / 2
        p = lambda u, v: (u * MM, v * MM, 0)
        if w >= h:
            dx = w / 2 - r
            sm.CreateLine(*p(x - dx, y + r), *p(x + dx, y + r))
            sm.CreateLine(*p(x + dx, y - r), *p(x - dx, y - r))
            sm.CreateArc(*p(x + dx, y), *p(x + dx, y - r), *p(x + dx, y + r), 1)
            sm.CreateArc(*p(x - dx, y), *p(x - dx, y + r), *p(x - dx, y - r), 1)
        else:
            dy = h / 2 - r
            sm.CreateLine(*p(x - r, y - dy), *p(x - r, y + dy))
            sm.CreateLine(*p(x + r, y + dy), *p(x + r, y - dy))
            sm.CreateArc(*p(x, y + dy), *p(x + r, y + dy), *p(x - r, y + dy), 1)
            sm.CreateArc(*p(x, y - dy), *p(x - r, y - dy), *p(x + r, y - dy), 1)

    def rounded_rect(self, u0, u1, v0, v1, r):
        """Rectangle u0 to u1, v0 to v1 with corner radius r, for a sketch already open."""
        sm = self.sm
        p = lambda u, v: (u * MM, v * MM, 0)
        sm.CreateLine(*p(u0 + r, v0), *p(u1 - r, v0))
        sm.CreateArc(*p(u1 - r, v0 + r), *p(u1 - r, v0), *p(u1, v0 + r), 1)
        sm.CreateLine(*p(u1, v0 + r), *p(u1, v1 - r))
        sm.CreateArc(*p(u1 - r, v1 - r), *p(u1, v1 - r), *p(u1 - r, v1), 1)
        sm.CreateLine(*p(u1 - r, v1), *p(u0 + r, v1))
        sm.CreateArc(*p(u0 + r, v1 - r), *p(u0 + r, v1), *p(u0, v1 - r), 1)
        sm.CreateLine(*p(u0, v1 - r), *p(u0, v0 + r))
        sm.CreateArc(*p(u0 + r, v0 + r), *p(u0, v0 + r), *p(u0 + r, v0), 1)

    def triangle(self, pts):
        for (u0, v0), (u1, v1) in zip(pts, pts[1:] + pts[:1]):
            self.sm.CreateLine(u0 * MM, v0 * MM, 0, u1 * MM, v1 * MM, 0)

    def finish(self, stem, note):
        self.m.EditRebuild3
        props = self.m.Extension.CustomPropertyManager("")
        props.Add3("Material", 30, "PETG (1.27 g/cm3)", 1)
        props.Add3("Description", 30, note, 1)
        self.m.ShowNamedView2("*Isometric", 7)
        self.m.ViewZoomtofit2()
        self.save()
        for ext in ("step", "stl"):
            print(stem, ext, self.m.SaveAs3(str(OUT / f"{stem}.{ext}"), 0, 2 | 1), flush=True)  # silent, copy
        mp = self.m.Extension.CreateMassProperty
        print(stem, "bodies", len(self.m.GetBodies2(0, True)), "volume_mm3", round(mp.Volume * 1e9, 1),
              "area_mm2", round(mp.SurfaceArea * 1e6, 1), "box", [round(v * 1e3, 3) for v in self.m.GetPartBox(True)], flush=True)
        self.sw.CloseDoc(self.m.GetTitle)


def bx(s, x0, x1):
    """Sorted X span for a feature given as |X| from x0 to x1 on side s."""
    a, b = s * x0, s * x1
    return min(a, b), max(a, b)


def strap_slot(p, s):
    """Closed slot through the web. Its ends stop the hook in Z. Right Plane sketch x = -Z."""
    z0, z1, y0, y1, r = SLOT
    a, b = bx(s, 47, 53)
    p.sketch("right")
    p.rounded_rect(-z1, -z0, y0, y1, r)
    p.extrude(a, b, "Strap_slot", cut=True)


def top_root(p, s, z0, z1, name):
    p.sketch("front")
    p.triangle([(s * 51.5, 349.62), (s * 55, 349.62), (s * 51.5, 346.12)])
    p.extrude(z0, z1, name)


def top_root_rear(p, s, z0):
    """Web to top foot chamfer behind the slot. Its forward end is 45 deg so the print, front edge down, needs no support under it."""
    top_root(p, s, z0, SLOT[0], "Top_root_rear")
    a, b = bx(s, 51.5, 55.5)
    p.sketch("right")
    p.triangle([(-SLOT[0], 346.12), (-SLOT[0], 349.62), (3.5 - SLOT[0], 349.62)])
    p.extrude(a, b, "Top_root_rear_end", cut=True)


def build_side(sw, s, label):
    p = Part(sw, CAD / f"marengo_torso_cradle_side_{label}.SLDPRT")

    def box(x0, x1, y0, y1, z0, z1, name, cut=False):
        a, b = bx(s, x0, x1)
        p.box((a, y0, z0), (b, y1, z1), name, cut)

    box(48, 51.5, 175, 353.62, -50, 37.5, "Web")  # 0.5 per side of slide clearance to the 95 mm pack
    box(51.5, 55, 175, 179, -44, 37.5, "Bottom_foot_strip")
    box(55, 75, 175, 179, -21.5, 21.5, "Bottom_foot_tongue")  # corner bracket legs fill Z +-22 to +-50 on this rail
    box(51.5, 75, 349.62, 353.62, -44, 37.5, "Top_foot")
    box(40, 48, 179, 183, -45, 34, "Ledge")
    box(42, 48, 179, 345, 34, 37.5, "Front_lip")  # down to Y 179: at Y 183 it would meet the ledge on a bare edge
    for y, n in ((200, "Lo"), (330, "Hi")):
        box(48, 55.5, y - 5, y + 5, 29.5, 37.5, f"Carrier_pad_{n}")
    # 3.5 x 3.5 root chamfers, web to feet; the bottom one stays on the strip, clear of the bracket legs
    p.sketch("front")
    p.triangle([(s * 51.5, 179), (s * 55, 179), (s * 51.5, 182.5)])
    p.extrude(-44, 37.5, "Bottom_root")
    top_root(p, s, SLOT[1], 37.5, "Top_root")  # stops either side of the strap slot, the lips run through there
    top_root_rear(p, s, -44)
    # Thumbscrew boss, Right Plane sketch x = -Z. M4 x 16 head seats on the boss face, tip 4.5 mm past the centred pack side.
    a, b = bx(s, 51.5, 59)
    p.sketch("right")
    p.circle(5, 252.6, 16)
    p.extrude(a, b, "Thumbscrew_boss")
    a, b = bx(s, 47, 61)
    p.sketch("right")
    p.circle(5, 252.6, 4.5)
    p.extrude(a, b, "Thumbscrew_hole", cut=True)
    # M5 slots, Top Plane sketch y = -Z
    for y0, y1, n, zs in ((170, 185, "Bottom", (-12, 12)), (345.7, 358, "Top", (-30, 20))):
        p.sketch("top")
        for z in zs:
            p.stadium(s * 65, -z, 5.5, 10)
        p.extrude(y0, y1, f"M5_slots_{n}", cut=True)
    # Lightening windows, 6 mm clear of the boss, feet, pads and lip
    a, b = bx(s, 47, 52.5)
    p.sketch("right")
    p.stadium(10, 214, 62, 44)
    p.stadium(10, 301, 62, 62)
    p.extrude(a, b, "Windows", cut=True)
    # Carrier pad inserts, M3 x 5, 4.2 pilot, 5.5 deep
    p.sketch("front")
    p.circle(s * 51.75, 200, 4.2)
    p.circle(s * 51.75, 330, 4.2)
    p.extrude(32, 37.5, "M3_insert_holes", cut=True)
    strap_slot(p, s)
    p.finish(f"marengo_torso_cradle_side_{label}", f"Pack cradle side, {label}, rev B fit-check")


def build_strap(sw):
    p = Part(sw, CAD / "marengo_torso_cradle_top_strap.SLDPRT")
    p.box((-51.7, 345.5, -15), (51.7, 348.5, 5), "Strap")  # 0.2 per side over the 103 across the webs, 0.5 over the pack
    for s, n in ((-1, "R"), (1, "L")):
        a, b = bx(s, 51.7, 54.2)
        p.box((a, 340.5, -15), (b, 348.5, 5), f"Lip_{n}")
    p.box(*LEAD_GAP, "Lead_gap", cut=True)  # 30 mm over the pack lead path, leaves two separate hooks
    for s, n in ((-1, "R"), (1, "L")):
        # Snap tongue, rear 6 mm of the bar, rooted at |X| 30, 1.6 thick, free end at the web inner face
        def cut(x0, x1, y0, y1, z0, z1, name):
            a, b = bx(s, x0, x1)
            p.box((a, y0, z0), (b, y1, z1), name, cut=True)

        cut(30, 47.8, 345, 346.9, -16, SLIT, f"Tongue_relief_{n}")
        cut(30, 47.8, 345, 349, SLIT, -7, f"Tongue_slit_{n}")
        cut(47.8, STOP, 345, 349, -16, -7, f"Tongue_end_{n}")  # leaves the bar full width in the web, so both slot ends stop it in Z
        p.sketch("front")
        p.triangle([(s * 47.8, 348.5), (s * 47.8, 350), (s * 46.8, 350), (s * 42.8, 348.5)])  # catch face on the web side
        p.extrude(-15, SLIT, f"Barb_{n}")
    p.finish("marengo_torso_cradle_top_strap", STRAP_NOTE)


if __name__ == "__main__":
    inst = session(sys.argv[1])
    print("pid", inst.pid, "no_auto_recover", inst.no_auto_recover, flush=True)
    inst.sw.CloseAllDocuments(True)
    only = sys.argv[2] if len(sys.argv) > 2 else "all"
    if only in ("all", "right"):
        build_side(inst.sw, -1, "right")
    if only in ("all", "left"):
        build_side(inst.sw, 1, "left")
    if only in ("all", "strap"):
        build_strap(inst.sw)
