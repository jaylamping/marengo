"""Assemble the neck concept, run interference across the range corners, render, export.

The assembly is pose-driven: components are placed by transform from neck_geom.py, not mated, because this is a
concept layout. Bellows is flexible, so it is hidden for the corner checks and checked by numbers in LAYOUT_SW.md.
Run through run_all.py, or: python assemble_check.py <solidworks session pid file> [build|check|render|export ...]
"""
import json
import sys
import time
from pathlib import Path

import numpy as np
import pythoncom
import win32com.client as wc
from PIL import Image

import neck_geom as g

sys.path.insert(0, r"J:\code\marengo\hardware\tools\solidworks")
from sw_instance import attach

CAD = Path(r"J:\code\marengo\cad\neck")
VENDOR = Path(r"J:\code\marengo\cad\vendor\feetech\sts3215\vendor_feetech_sts3215_vendor.SLDPRT")
OUT = Path(__file__).parent
ASM = CAD / "marengo_neck_layout.SLDASM"
NUL = wc.VARIANT(pythoncom.VT_DISPATCH, None)
ORANGE, BLUE, TEAL, DARK, STEEL, GREY, WHITE = (.95, .5, .12), (.2, .42, .78), (.1, .64, .6), (.13, .14, .16), (.7, .72, .75), (.55, .57, .6), (.92, .92, .92)

H2R = np.array([[0, 1, 0], [0, 0, 1], [1, 0, 0]], float)  # head frame (x fwd, y left, up) to torso axes


def Yaw(deg):
    c, s = np.cos(np.radians(deg)), np.sin(np.radians(deg))
    return np.array([[c, 0, s], [0, 1, 0], [-s, 0, c]])


def about(R, c):
    """Rotate about point c: p' = R (p - c) + c."""
    return R, np.asarray(c, float) - R @ np.asarray(c, float)


def head_about(Rh, h_pt):
    """A head-frame rotation about a head-frame point, for a part modelled in torso coordinates."""
    return about(H2R @ Rh @ H2R.T, g.to_robot(h_pt))


def servo_placement(k):
    s = g.SERVOS[k]
    # STEP axes: X long (shaft at +12.5), Z shaft. Body runs forward of the shaft, so STEP -X is head +X.
    ex, ez = np.array([-1, 0, 0.]), np.array([0, s["side"], 0.])
    ey = np.cross(ez, ex)
    Rh = np.column_stack([ex, ey, ez])
    shaft = np.array([g.SHAFT_FWD, 0, s["h"]])
    R = H2R @ Rh
    return R, g.to_robot(shaft) - R @ np.array([12.5, 0, 0.])


def yaw_servo_placement():
    # shaft up at torso (56, Y, 0), body runs back from the shaft; spline top sits just under the pinion
    R = np.column_stack([[0, 0, 1.], [1, 0, 0.], [0, 1, 0.]])
    top_h = -9.0
    return R, np.array([56, g.y(top_h - 20.2), 0.]) - R @ np.array([12.5, 0, 0.])


def horn_placement(k, t):
    """Horn is modelled in X-Y with its arm toward -X. Head x/up map to part X/Y, part Z to -head y."""
    s = g.SERVOS[k]
    c, sn = np.cos(-t), np.sin(-t)
    Rlocal = np.array([[c, -sn, 0], [sn, c, 0], [0, 0, 1.]])
    Rbase = np.column_stack([[1, 0, 0.], [0, 0, 1.], [0, -1, 0.]])  # part axes in head frame
    Rh = Rbase @ Rlocal
    return H2R @ Rh, g.to_robot([g.SHAFT_FWD, s["side"] * g.HORN_PLANE, s["h"]])


def rod_placement(a, b):
    """Part +Y along a->b (torso), origin at a."""
    d = (b - a) / np.linalg.norm(b - a)
    y0 = np.array([0, 1, 0.])
    v, c = np.cross(y0, d), y0 @ d
    k = np.array([[0, -v[2], v[1]], [v[2], 0, -v[0]], [-v[1], v[0], 0]])
    return np.eye(3) + k + k @ k / (1 + c), a


def pose_all(pitch=0.0, roll=0.0, yaw=0.0):
    Rp = g.head_rot(pitch, 0.0)
    Rh = g.head_rot(pitch, roll)
    Y = Yaw(yaw)
    out = {}

    def rot(name, R, t):   # apply yaw about the neck axis (X = Z = 0) to anything on the turntable
        out[name] = (Y @ R, Y @ t)

    rot("tube", np.eye(3), np.zeros(3))
    rot("cage", np.eye(3), np.zeros(3))
    rot("bellows", np.eye(3), np.zeros(3))
    rot("cross", *head_about(Rp, g.UJ))
    rot("plate", *head_about(Rh, g.UJ))
    for k in g.SERVOS:
        t, a, b = g.rod_pose(k, pitch, roll)
        rot(f"servo_{k.lower()}", *servo_placement(k))
        rot(f"horn_{k.lower()}", *horn_placement(k, t))
        rot(f"rod_{k.lower()}", *rod_placement(g.to_robot(a), g.to_robot(b)))
    out["pinion"] = (Yaw(-yaw * 88 / 24), np.array([56.0, 0, 0]))   # modelled on its own axis
    return out


FIXED = {"rails": ("ref_rails", GREY), "collar": ("ref_collar", WHITE), "tower": ("tower", TEAL), "bearing": ("bearing_61814", BLUE)}
MOVING = {"tube": ("yaw_tube", TEAL), "cage": ("servo_cage", TEAL), "pinion": ("yaw_pinion", TEAL), "cross": ("ujoint_cross", STEEL), "plate": ("head_plate", TEAL),
          "bellows": ("bellows", DARK), "horn_a": ("horn", STEEL), "horn_b": ("horn", STEEL), "rod_a": ("rod_a", STEEL), "rod_b": ("rod_b", STEEL)}
SERVOS = ("servo_a", "servo_b", "servo_yaw")
# the ball joints and press fits touch by design
JOINTS = [{"rod_a", "horn_a"}, {"rod_a", "plate"}, {"rod_b", "horn_b"}, {"rod_b", "plate"}, {"cross", "tube"}, {"cross", "plate"}]


class Layout:
    def __init__(self, sw):
        self.sw, self.c = sw, {}
        self.err = wc.VARIANT(pythoncom.VT_BYREF | pythoncom.VT_I4, 0)
        self.warn = wc.VARIANT(pythoncom.VT_BYREF | pythoncom.VT_I4, 0)

    def part_path(self, stem):
        return CAD / f"marengo_neck_{stem}.SLDPRT"

    def paths(self):
        d = {n: self.part_path(s) for n, (s, _) in {**FIXED, **MOVING}.items()}
        d.update({s: VENDOR for s in SERVOS})
        return d

    def new(self):
        sw = self.sw
        for p in set(self.paths().values()):
            sw.OpenDoc6(str(p), 1, 1, "", self.err, self.warn)
        self.a = sw.NewDocument(sw.GetUserPreferenceStringValue(9), 0, 0, 0)
        self.a.Extension.SaveAs(str(ASM), 0, 1, NUL, self.err, self.warn)
        self.a = sw.ActivateDoc3(Path(ASM).name, False, 0, self.err)
        for n, p in self.paths().items():
            c = self.a.AddComponent5(str(p), 0, "", False, "", 0, 0, 0)
            if c is None:
                raise RuntimeError(n)
            self.c[n] = c
            c.Select4(False, NUL, False)
            self.a.UnfixComponent()   # the first component comes in fixed
        for doc in set(self.paths().values()):
            sw.CloseDoc(doc.name)
        self.link()
        self.save()

    def open(self):
        self.a = self.sw.OpenDoc6(str(ASM), 2, 1, "", self.err, self.warn)
        self.a = self.sw.ActivateDoc3(ASM.name, False, 0, self.err)
        comps = list(self.a.GetComponents(True))
        pool = {}
        for c in comps:
            pool.setdefault(Path(c.GetPathName).name.lower(), []).append(c)
        for n, p in self.paths().items():
            self.c[n] = pool[Path(p).name.lower()].pop(0)
        self.link()

    def link(self):
        self.names = {c.Name2: n for n, c in self.c.items()}

    def save(self):
        self.a.Save3(1, self.err, self.warn)

    def place(self, n, R, t):
        c = self.c[n]
        q = c.Transform2
        q.ArrayData = wc.VARIANT(pythoncom.VT_ARRAY | pythoncom.VT_R8, [*np.asarray(R, float).T.ravel().tolist(), *(np.asarray(t, float) * 1e-3).tolist(), 1., 0., 0., 0.])
        c.Transform2 = q

    def pose(self, **kw):
        for n in FIXED:
            self.place(n, np.eye(3), np.zeros(3))
        self.place("servo_yaw", *yaw_servo_placement())
        for n, (R, t) in pose_all(**kw).items():
            self.place(n, R, t)
        self.a.GraphicsRedraw2()

    def colour(self):
        for n, (_, rgb) in {**FIXED, **MOVING, **{s: (None, ORANGE) for s in SERVOS}}.items():
            self.tint(n, rgb)

    def tint(self, n, rgb, clear=0.0):
        self.c[n].MaterialPropertyValues = wc.VARIANT(pythoncom.VT_ARRAY | pythoncom.VT_R8, [*rgb, 1, 1, .5, .4, clear, 0])

    def show(self, n, on):
        self.c[n].Visible = 1 if on else 0

    def check(self):
        im = self.a.InterferenceDetectionManager
        im.TreatCoincidenceAsInterference = False
        im.TreatSubAssembliesAsComponents = True
        im.IncludeMultibodyPartInterferences = False
        im.MakeInterferingPartsTransparent = False
        im.CreateFastenersFolder = False
        im.IgnoreHiddenBodies = True
        im.ShowIgnoredInterferences = False
        im.UseTransform = False
        im.GetInterferenceCount
        rows = []
        for x in (im.GetInterferences or []):
            cs = sorted(self.names.get(c.Name2, c.Name2) for c in x.Components)
            rows.append([cs, round(x.Volume * 1e9, 2)])
        im.Done()
        return sorted(rows)

    def shot(self, name, view=None, size=(2400, 1800), orient=None):
        if orient is not None:
            self.a.ActiveView.Orientation3 = orient
        else:
            self.a.ShowNamedView2(view, -1)
        self.a.ViewZoomtofit2()
        self.a.ClearSelection2(True)
        bmp = CAD.parent / "_wip" / "neck" / f"{name}.bmp"
        if not self.a.SaveBMP(str(bmp), *size):
            raise RuntimeError(name)
        Image.open(bmp).convert("RGB").save(OUT / "renders" / f"{name}.png")
        print(name, flush=True)


def do_check(L):
    L.show("bellows", False)
    res = []
    cases = [(0, 0, 0)] + [(p, r, y) for p, r in g.corners() for y in (-90, 0, 90)]
    for p, r, y in cases:
        t0 = time.time()
        L.pose(pitch=p, roll=r, yaw=y)
        rows = L.check()
        hits = [x for x in rows if set(x[0]) not in JOINTS]
        res.append(dict(pitch=p, roll=r, yaw=y, hits=hits, joints=[x for x in rows if set(x[0]) in JOINTS]))
        print(p, r, y, "hits", hits, round(time.time() - t0, 1), "s", flush=True)
        (OUT / "interference.json").write_text(json.dumps(res, indent=0))
    L.pose()
    L.show("bellows", True)


def do_render(L):
    (OUT / "renders").mkdir(exist_ok=True)
    L.colour()
    L.pose()
    L.show("rails", True)
    L.shot("NECK_SW_ISO", "*Isometric")
    L.shot("NECK_SW_FRONT", "*Front")
    L.shot("NECK_SW_SIDE", "*Right")
    # x-ray: everything round the mechanism goes glass
    for n, rgb, c in (("bellows", DARK, .86), ("tower", TEAL, .8), ("collar", WHITE, .86), ("cage", TEAL, .75), ("tube", TEAL, .7)):
        L.tint(n, rgb, c)
    L.shot("NECK_SW_XRAY_ISO", "*Isometric")
    L.shot("NECK_SW_XRAY_SIDE", "*Right")
    L.pose(pitch=g.PITCH_DOWN, roll=g.ROLL_MAX, yaw=35)
    L.shot("NECK_SW_XRAY_POSE", "*Isometric")
    L.colour()
    # exploded, by offsets along Y
    off = dict(plate=120, cross=95, bellows=70, rod_a=0, rod_b=0, tube=35, collar=20, bearing=5, tower=-10, horn_a=-25, horn_b=-25, servo_a=-25, servo_b=-25, cage=-25, pinion=-10, servo_yaw=-40)
    L.pose()
    for n, dy in off.items():
        R, t = _current(n)
        L.place(n, R, t + np.array([0, 1.6 * dy, 0]))
    L.show("rod_a", False)
    L.show("rod_b", False)
    L.shot("NECK_SW_EXPLODED", "*Isometric")
    L.show("rod_a", True)
    L.show("rod_b", True)
    L.pose()


def _current(n):
    if n in FIXED:
        return np.eye(3), np.zeros(3)
    if n == "servo_yaw":
        return yaw_servo_placement()
    return pose_all()[n]


def main():
    inst = attach(int(open(sys.argv[1]).read().strip()))
    print("pid", inst.pid, flush=True)
    steps = sys.argv[2:] or ["build", "check", "render", "export"]
    L = Layout(inst.sw)
    if "build" in steps:
        L.new()
        L.colour()
        L.pose()
        L.save()
    else:
        L.open()
    if "check" in steps:
        do_check(L)
    if "render" in steps:
        do_render(L)
    L.save()
    if "export" in steps:
        L.a.Extension.SaveAs(str(OUT / "marengo_neck_layout.step"), 0, 1, NUL, L.err, L.warn)
        print("step", L.err.value, flush=True)
    inst.sw.CloseAllDocuments(True)


if __name__ == "__main__":
    main()
