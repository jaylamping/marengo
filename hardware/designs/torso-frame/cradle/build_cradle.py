"""Build the pack cradle parts in a dedicated SolidWorks instance, in torso coordinates, mm.

X lateral (+X robot left), Y up, Z forward. Right side is -X; the left side is the same build with s = +1.
Run: python build_cradle.py <solidworks session pid file>
"""
import sys
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

    def extrude(self, a, b, name, cut=False, merge=True):
        """Close the open sketch and extrude along the plane normal from coordinate a to b (a < b)."""
        self.sm.AddToDB = False
        t0, off, flip_off = (START_OFFSET, abs(a) * MM, a < 0) if a else (START_PLANE, 0.0, False)
        d = (b - a) * MM
        tail = (BLIND, BLIND, d, 0.0, False, False, False, False, 0.0, 0.0, False, False, False, False)
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
    p.sketch("front")
    p.triangle([(s * 51.5, 349.62), (s * 55, 349.62), (s * 51.5, 346.12)])
    p.extrude(5.2, 37.5, "Top_root")  # forward of the strap slot, the lips run through here
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
    # Strap slot, open at the rear edge, and the detent pocket for the strap lip
    box(47, 53, 345.5, 348.9, -51, 5.2, "Strap_slot", cut=True)
    a, b = bx(s, 50.7, 52.5)
    p.sketch("right")
    p.circle(5, 342.5, 4.4)
    p.extrude(a, b, "Strap_detent", cut=True)
    p.finish(f"marengo_torso_cradle_side_{label}", f"Pack cradle side, {label}, rev A")


def build_strap(sw):
    p = Part(sw, CAD / "marengo_torso_cradle_top_strap.SLDPRT")
    p.box((-51.7, 345.5, -15), (51.7, 348.5, 5), "Strap")  # 0.2 per side over the 103 across the webs, 0.5 over the pack
    for s, n in ((-1, "R"), (1, "L")):
        a, b = bx(s, 51.7, 54.2)
        p.box((a, 340.5, -15), (b, 348.5, 5), f"Lip_{n}")
        a, b = bx(s, 51.2, 51.7)
        p.sketch("right")
        p.circle(5, 342.5, 4)
        p.extrude(a, b, f"Detent_{n}")
    p.finish("marengo_torso_cradle_top_strap", "Pack cradle top strap, rev A")


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
