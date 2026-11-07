#!/usr/bin/env python3
"""Actuator test stand, RS03 set (first in Joey's test order RS03, RS02, RS00, RS04). CadQuery, mm.

Stand frame (all parts are built in it):
  X along the bench rail (2020, clamped to the bench), +X toward the scale anchor.
  Y along the actuator axis, +Y toward the lever (output side).
  Z up. Rail top face at Z = 0, rail occupies Y -10..10, Z -20..0.
  Actuator axis: X = 0, Z = AXIS_Z.

Vendor geometry (RobStride RS03-20261030.stp, CAN2.0 folder, commit b05095a; manual RS03 261013 section 1.1):
  output face is the front-most face; housing front face 2.5 mm behind it; 8x M4 (drawing: 8 deep) on D98 at
  +-22.5, +-67.5, +-112.5, +-157.5 deg; output 6x M4 (drawing: 6 deep) on D30.36 at 0, 60, ... deg;
  contact disc on the output face is D46 (the D46..D70 ring is left untouched).

Run: python build_stand.py            -> STEP + STL per part in ./out, assembly check against the vendor STEP if present
"""
import math
import sys
from pathlib import Path

import cadquery as cq

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"

# ---- shared stand dimensions -------------------------------------------------------------------------------------
AXIS_Z = 100.0          # actuator axis above the rail top; same for every actuator size so lever, anchor and lock are shared
FOOT_T = 10.0           # upright foot thickness on the rail
M5_CLR, M5_HEAD = 5.5, 9.5
M4_CLR, M4_HEAD = 4.5, 7.6
M8_CLR = 8.5
M5_NUT_AF, M5_NUT_T = 8.15, 4.3   # DIN 934 M5 nut is 8.0 AF x 4.0, plus print clearance
M8_HEAD_AF, M8_HEAD_T = 13.2, 5.6  # ISO 4017 M8 hex head is 13 AF x 5.3
LEVER_W = 20.1          # 2020 lever channel width (20.0 nominal + 0.1 print allowance, clamp takes up the rest)

# ---- RS03 interface ----------------------------------------------------------------------------------------------
RS03 = dict(
    name="rs03",
    housing_bc=98.0, housing_n=8, housing_start=22.5,   # 8x M4 on D98 at 22.5 + k*45
    bore=74.0,                                          # clears the D70 front boss
    output_bc=30.36, output_n=6,
    contact_d=44.0,                                     # stays inside the D46 output disc
    boss=2.5,                                           # output face is 2.5 proud of the housing face
    ear_r=52.0,                                         # ear lugs reach about R50.7 (connector bump); keep ribs outside 54
)

PLATE_T = 12.0
PLATE_BACK_Y = -2.0                 # housing front face sits here
PLATE_FRONT_Y = PLATE_BACK_Y + PLATE_T   # +10, flush with the rail's front face
OUT_FACE_Y = PLATE_BACK_Y + RS03["boss"]  # +0.5, actuator output face

# adapter stack along Y (from the output face forward)
HUB_Y0 = PLATE_FRONT_Y + 1.5        # 1.5 mm running gap to the upright face
FLOOR_Y1 = HUB_Y0 + 10.0            # channel floor
WALL_Y1 = FLOOR_Y1 + 10.0           # channel wall tops
LEVER_Y0, LEVER_Y1 = FLOOR_Y1, FLOOR_Y1 + 20.0
LEVER_YC = (LEVER_Y0 + LEVER_Y1) / 2
HUB_D = 72.0
STOP_X = -30.0                      # lever end butts here: hook radius = (lever end to M8 pin) - 30
CAP_T = 8.0
CLAMP_XZ = [(sx * 22.0, sz * 18.0) for sx in (-1, 1) for sz in (-1, 1)]

# load block and anchor
BLOCK_T = 15.0                      # block front plate thickness in front of the lever
PIN_Y = LEVER_Y1 + BLOCK_T + 8.0    # where the scale strap / hook sits on the M8 pin (about Y 64.5)


def bolt_circle(d, n, start_deg):
    return [((d / 2) * math.cos(math.radians(start_deg + 360 * k / n)), (d / 2) * math.sin(math.radians(start_deg + 360 * k / n))) for k in range(n)]


def y_cyl(d, y0, y1, x=0.0, z=0.0):
    """Cylinder along +Y from y0 to y1 centred on (x, z)."""
    return cq.Workplane("XZ", origin=(0, y0, 0)).center(x, z).circle(d / 2).extrude(-(y1 - y0))


def z_cyl(d, z0, z1, x=0.0, y=0.0):
    return cq.Workplane("XY", origin=(0, 0, z0)).center(x, y).circle(d / 2).extrude(z1 - z0)


def x_cyl(d, x0, x1, y=0.0, z=0.0):
    return cq.Workplane("YZ", origin=(x0, 0, 0)).center(y, z).circle(d / 2).extrude(x1 - x0)


def box(x0, x1, y0, y1, z0, z1):
    return cq.Workplane("XY").box(x1 - x0, y1 - y0, z1 - z0, centered=False).translate((x0, y0, z0))


def hex_y(af, y0, y1, x, z):
    return cq.Workplane("XZ", origin=(0, y0, 0)).center(x, z).polygon(6, af / math.cos(math.pi / 6)).extrude(-(y1 - y0))


def yz_profile(points, x0, x1):
    """Closed polygon in (Y, Z), extruded along X from x0 to x1."""
    return cq.Workplane("YZ", origin=(x0, 0, 0)).polyline(points).close().extrude(x1 - x0)


# ---- parts -------------------------------------------------------------------------------------------------------
def upright(a=RS03):
    """Vertical plate the actuator bolts to by its 8 housing M4s, foot bolted to the rail top with 4x M5 T-nuts.
    Printed front face down: bore and screw holes print as vertical holes, ribs stand up."""
    w = 60.0
    plate = (cq.Workplane("XZ", origin=(0, PLATE_FRONT_Y, 0))
             .polyline([(-w, FOOT_T), (w, FOOT_T), (w, AXIS_Z + 45), (w - 15, AXIS_Z + 60), (-w + 15, AXIS_Z + 60), (-w, AXIS_Z + 45)]).close()
             .extrude(PLATE_T))
    foot = box(-96, 96, -40, PLATE_FRONT_Y, 0, FOOT_T)
    body = plate.union(foot)
    for sx in (-1, 1):  # rear ribs, outside the housing ears
        rib = yz_profile([(PLATE_BACK_Y + 0.01, FOOT_T - 0.01), (-40, FOOT_T - 0.01), (-40, FOOT_T + 6), (PLATE_BACK_Y + 0.01, 72)], 0, 8)
        body = body.union(rib.translate((sx * 56 - 4, 0, 0)))  # X 52..60, inside the plate edge
    body = body.cut(y_cyl(a["bore"], PLATE_BACK_Y - 1, PLATE_FRONT_Y + 1, 0, AXIS_Z))
    for x, z in bolt_circle(a["housing_bc"], a["housing_n"], a["housing_start"]):
        body = body.cut(y_cyl(M4_CLR, PLATE_BACK_Y - 1, PLATE_FRONT_Y + 1, x, AXIS_Z + z))
        body = body.cut(y_cyl(M4_HEAD, PLATE_FRONT_Y - 3.0, PLATE_FRONT_Y + 1, x, AXIS_Z + z))  # M4x16: 9 grip, 7 in the ear
    for x in (-86, -70, 70, 86):  # foot to rail-top slot T-nuts
        body = body.cut(z_cyl(M5_CLR, -1, FOOT_T + 1, x, 0))
    return body


def adapter(a=RS03):
    """Output adapter: D44 spigot bolted to the output with 6x M4, hub with a channel the 2020 lever drops into,
    lever end stop, 4 captive M5 nuts for the clamp cap. Printed spigot face down. 100 % infill (part under load)."""
    spig = y_cyl(a["contact_d"], OUT_FACE_Y, HUB_Y0 + 0.01, 0, AXIS_Z)
    hub = y_cyl(HUB_D, HUB_Y0, WALL_Y1, 0, AXIS_Z)
    body = spig.union(hub)
    chan = box(STOP_X, 40, FLOOR_Y1, WALL_Y1 + 1, AXIS_Z - LEVER_W / 2, AXIS_Z + LEVER_W / 2)
    body = body.cut(chan)
    body = body.cut(y_cyl(16, OUT_FACE_Y - 1, OUT_FACE_Y + 1.0, 0, AXIS_Z))  # relief over the output centre features
    for x, z in bolt_circle(a["output_bc"], a["output_n"], 30.0):  # clocked 30 deg: two holes in the channel floor, four under the walls
        body = body.cut(y_cyl(M4_CLR, OUT_FACE_Y - 1, WALL_Y1 + 1, x, AXIS_Z + z))
        body = body.cut(y_cyl(M4_HEAD, OUT_FACE_Y + 10.0, WALL_Y1 + 1, x, AXIS_Z + z))  # M4x16: 10 grip, 6 in the output
    for x, z in CLAMP_XZ:
        body = body.cut(y_cyl(M5_CLR, HUB_Y0 - 1, WALL_Y1 + 1, x, AXIS_Z + z))
        body = body.cut(hex_y(M5_NUT_AF, HUB_Y0 - 1, HUB_Y0 + M5_NUT_T, x, AXIS_Z + z))
    return body


def clamp_cap():
    """Bar across the lever front, pads down onto the channel walls, 4x M5x30 into the adapter's captive nuts.
    Pads stop 0.5 short of the wall tops so the screws clamp the lever, not the walls."""
    y0 = LEVER_Y1
    bar = box(-30, 30, y0, y0 + CAP_T, AXIS_Z - 28, AXIS_Z + 28)
    for x, z in CLAMP_XZ:
        pad = box(x - 8, x + 8, WALL_Y1 + 0.5, y0 + 0.01, AXIS_Z + z - 7.5, AXIS_Z + z + 7.5)
        bar = bar.union(pad)
    bar = bar.cut(box(-31, 31, WALL_Y1 + 0.4, y0, AXIS_Z - LEVER_W / 2 - 0.2, AXIS_Z + LEVER_W / 2 + 0.2))  # lever passes under
    for x, z in CLAMP_XZ:
        bar = bar.cut(y_cyl(M5_CLR, WALL_Y1, y0 + CAP_T + 1, x, AXIS_Z + z))
    return bar


def load_block(r=250.0):
    """Clamps on the lever at radius r with 2x M5 into the lever's top and bottom slot T-nuts. A captive M8x60 hex bolt
    points forward (+Y) on the lever centreline: scale strap or weights hang on it. Built at the horizontal lever position."""
    zc = AXIS_Z
    front = box(r - 18, r + 18, LEVER_Y1, LEVER_Y1 + BLOCK_T, zc - 22, zc + 22)
    ears = box(r - 18, r + 18, LEVER_Y0 + 2, LEVER_Y1 + 0.01, zc + 10.1, zc + 22).union(
        box(r - 18, r + 18, LEVER_Y0 + 2, LEVER_Y1 + 0.01, zc - 22, zc - 10.1))
    body = front.union(ears)
    body = body.cut(y_cyl(M8_CLR, LEVER_Y1 - 1, LEVER_Y1 + BLOCK_T + 1, r, zc))
    body = body.cut(hex_y(M8_HEAD_AF, LEVER_Y1 - 1, LEVER_Y1 + M8_HEAD_T, r, zc))  # head pocket against the lever face
    for s in (-1, 1):
        z0, z1 = (zc + 9.9, zc + 23) if s > 0 else (zc - 23, zc - 9.9)
        body = body.cut(z_cyl(M5_CLR, z0, z1, r, LEVER_YC))
    return body.edges("|Z").fillet(2)


def anchor():
    """Scale anchor: slides along the rail to sit under the load pin. An M8x70 bolt goes through the arm end along X at
    Y = PIN_Y with a nut; the scale hook hangs on the 25 mm that sticks out. Printed on its X side, no supports."""
    yf = PIN_Y
    prof = [(-10, 0), (10, 0), (10, -20), (16, -20), (16, 0), (yf + 12, 0), (yf + 12, 38), (yf - 12, 38), (16, 18), (-10, 18)]
    body = yz_profile(prof, -20, 20)
    body = body.cut(x_cyl(M8_CLR, -21, 21, yf, 28))
    body = body.cut(z_cyl(M5_CLR, -1, 19, 0, 0))                       # rail top slot
    body = body.cut(y_cyl(M5_CLR, 9, 17, 0, -10))                      # rail front slot
    return body


def lock_post(r=150.0):
    """T3 output lock: holds the lever horizontal at radius r. U-cradle around the lever, M5 into the lever's front and
    back slot T-nuts; base on the rail top and front face like the anchor."""
    zl = AXIS_Z - 10  # lever bottom face
    y0, y1 = LEVER_Y0 - 5, LEVER_Y1 + 5
    prof = [(-10, 0), (10, 0), (10, -20), (16, -20), (16, 0), (y1, 0), (y1, zl + 18), (LEVER_Y1, zl + 18), (LEVER_Y1, zl),
            (LEVER_Y0, zl), (LEVER_Y0, zl + 18), (y0, zl + 18), (y0, 18), (-10, 18)]
    body = yz_profile(prof, r - 15, r + 15)
    win = box(r - 16, r + 16, y0 + 6, y1 - 6, 26, zl - 10).edges("|X").fillet(4)
    body = body.cut(win)
    body = body.cut(y_cyl(M5_CLR, y0 - 1, LEVER_Y0 + 0.01, r, AXIS_Z))   # into the lever back slot
    body = body.cut(y_cyl(M5_CLR, LEVER_Y1 - 0.01, y1 + 1, r, AXIS_Z))   # into the lever front slot
    body = body.cut(z_cyl(M5_CLR, -1, 19, r, 0))
    body = body.cut(y_cyl(M5_CLR, 9, 17, r, -10))
    return body


def lever(length=300.0):
    """Reference only (Joey's own 2020 offcut, not printed): end at STOP_X, horizontal, in the adapter channel."""
    return box(STOP_X, STOP_X + length, LEVER_Y0, LEVER_Y1, AXIS_Z - 10, AXIS_Z + 10)


def rail(length=420.0):
    return box(-110, -110 + length, -10, 10, -20, 0)


PARTS = {
    "marengo_teststand_upright_rs03": upright,
    "marengo_teststand_adapter_rs03": adapter,
    "marengo_teststand_clamp_cap": clamp_cap,
    "marengo_teststand_load_block": load_block,
    "marengo_teststand_anchor": anchor,
    "marengo_teststand_lock_post": lock_post,
}


def vendor_rs03():
    """RS03 vendor STEP placed in the stand frame: STEP z = 0 is the output face, +z goes back into the housing,
    STEP +y (connector side) up."""
    step = Path("/tmp/claude-0/rs/Product Literature/CAN2.0/RS03/RS03-20261030.stp")
    if not step.exists():
        return None
    m = cq.importers.importStep(str(step))
    # (x, y, z)_step -> (X, Y, Z) = (x, OUT_FACE_Y - z, AXIS_Z + y): rotate +90 about X then translate
    return m.rotate((0, 0, 0), (1, 0, 0), 90).translate((0, OUT_FACE_Y, AXIS_Z))


def common_volume(a, b):
    from OCP.BRepAlgoAPI import BRepAlgoAPI_Common
    sa = cq.Compound.makeCompound([v for v in a.vals() if isinstance(v, cq.Shape)]).wrapped
    sb = cq.Compound.makeCompound([v for v in b.vals() if isinstance(v, cq.Shape)]).wrapped
    op = BRepAlgoAPI_Common(sa, sb)
    op.Build()
    return cq.Shape.cast(op.Shape()).Volume() if op.IsDone() else float("nan")


ANCHOR_X = 200.0  # assembly check position: under the load pin with the lever about 37 deg up


def main():
    OUT.mkdir(exist_ok=True)
    solids = {}
    for name, fn in PARTS.items():
        s = fn()
        solids[name] = s
        cq.exporters.export(s, str(OUT / f"{name}.step"))
        cq.exporters.export(s, str(OUT / f"{name}.stl"), tolerance=0.01, angularTolerance=0.1)
        v = s.val()
        print(f"{name}: volume {v.Volume() / 1000:.1f} cm3, solids {len(s.solids().vals())}, bbox {[round(t, 1) for t in (v.BoundingBox().xlen, v.BoundingBox().ylen, v.BoundingBox().zlen)]}")
    ref = {"lever_2020x300": lever(), "rail_2020x420": rail()}
    act = vendor_rs03()
    if act is not None:
        ref["rs03_vendor"] = act
    for name, s in ref.items():
        cq.exporters.export(s, str(OUT / f"ref_{name}.stl"), tolerance=0.05, angularTolerance=0.2)
    # interference check, every pair
    allp = {**solids, **ref}
    allp["marengo_teststand_anchor"] = solids["marengo_teststand_anchor"].translate((ANCHOR_X, 0, 0))
    names = list(allp)
    bad = []
    for i in range(len(names)):
        for j in range(i + 1, len(names)):
            a, b = allp[names[i]], allp[names[j]]
            v = common_volume(a, b)
            if v > 0.5:
                bad.append((names[i], names[j], round(v, 1)))
    print("interference > 0.5 mm3:", bad or "none")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
