#!/usr/bin/env python3
"""Actuator test stand: per-size upright + output adapter (RS03, RS02, RS00 so far; Joey's order RS03, RS02, RS00, RS04) and shared fixtures. CadQuery, mm.

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

# ---- actuator interfaces (vendor STEP, RobStride Product_Information CAN2.0, commit b05095a) --------------------------
# housing: screws into the housing front face (the face the output protrudes from), angles in the STEP XY frame, which maps
# straight onto the stand's XZ (X = x, Z = AXIS_Z + y). grip = screw length under the head inside the upright.
RS03 = dict(
    name="rs03", step="RS03/RS03-20261030.stp", step_c=(0.0, 0.0), step_out_z=0.0,
    housing_bc=98.0, housing_ang=[22.5 + 45 * k for k in range(8)],   # 8x M4, 8 deep (drawing)
    h_clr=M4_CLR, h_head=M4_HEAD, h_grip=9.0,                           # M4x16: 9 grip, 7 in the housing
    bore=74.0,                                          # clears the D70 front boss
    output_bc=30.36, output_n=6,                        # 6x M4, 6 deep
    contact_d=44.0,                                     # stays inside the D46 output disc
    boss=2.5,                                           # output face is 2.5 proud of the housing face
    w=60.0, rib_x=52.0,                                 # plate half-width; ribs X 52..60 clear the ears (R52)
)
RS02 = dict(
    name="rs02", step="RS02/RS02-20261030.stp", step_c=(54.084, 40.164), step_out_z=-27.5,
    housing_bc=73.0, housing_ang=[-6.1 + 40 * k for k in range(9)],    # 9x M3 on D73 (drawing: 9-M3, 8 deep)
    h_clr=3.4, h_head=6.2, h_grip=8.0,                                  # M3x14: 8 grip, 6 in the housing
    bore=48.0,                                          # clears the D43 output flange
    output_bc=24.0, output_n=6,                         # 6x M4 on D24 (drawing: 7 min; STEP 6)
    contact_d=41.0,                                     # bears on the raised D29.7 output pad; the D43 flange is 0.5 behind it
    boss=4.0,                                           # output face is 4.0 proud of the housing face
    cb_slot=True,                                       # D24 output circle puts counterbores across the channel walls
    w=48.0, rib_x=40.0,                                 # housing is D78.5; ribs stay below Z 72
)
RS00 = dict(
    name="rs00", step="RS00/RS00-20261030.stp", step_c=(0.0, 0.0), step_out_z=-6.0,
    housing_bc=50.0, housing_ang=[60.0 * k for k in range(6)],         # 6x M3 on D50 (drawing: 6-M3; STEP 5 deep)
    h_clr=3.4, h_head=6.2, h_grip=8.0,                                  # M3x12: 8 grip, 4 in the housing
    bore=39.0,                                          # clears the D35 output
    output_bc=27.0, output_n=6,                         # 6x M3 on D27 (drawing: 5 deep blind)
    o_clr=3.4, o_head=6.2,                              # M3x14: 10 grip, 4 in the output
    cb_slot=True,                                       # D27 circle leaves a 0.35 mm sliver to the channel wall otherwise
    contact_d=32.0, spigot_d=37.0,                      # D32 pad stays inside the D34.6 output face; D37 clears the D39 bore
    boss=1.0,                                           # output face is 1.0 proud of the housing face
    w=38.0, rib_x=30.0,                                 # housing is D57
)
RS04 = dict(  # STEP output faces +z, so it is turned 180 deg about y first: stand angle = 180 - STEP angle (STEP +y stays up)
    name="rs04", step="RS04/RS04-20261030.stp", step_c=(0.0, 0.0), step_out_z=25.5, step_flip=True,
    housing_bc=106.0, housing_ang=[159.0 - 36 * k for k in range(10)],  # 10x M4 on D106, 7 deep (drawing: 5 min)
    h_clr=M4_CLR, h_head=M4_HEAD, h_grip=9.0,                           # M4x16: 9 grip, 7 in the housing
    bore=56.0,                                          # clears the D49.6 output disc
    output_bc=36.0, out_ang=[15, 75, 105, 135, -15, -45, -105, -135, -165],   # 9x M5 on D36 (drawing: 6.5 min; STEP models 3.5)
    o_clr=M5_CLR, o_head=M5_HEAD, o_grip=10.5,          # M5x16: 10.5 grip, 5.5 in the output
    pins=[45, 165, -75], pin_r=17.9, pin_hole=6.4, pin_depth=3.5,   # 3x D6 dowels stand 2.7 proud of the output face
    cb_slot=True,
    contact_d=48.0,                                     # stays inside the D49.6 output disc
    boss=0.5,                                           # output face is 0.5 proud of the housing face
    w=66.0, rib_x=58.0,                                 # housing is D120; ribs stay below the housing at Z 72
    foot_x=102.0, foot_holes=(76.0, 92.0),              # foot holes move out past the wider plate
)
ACTUATORS = {"rs03": RS03, "rs02": RS02, "rs00": RS00, "rs04": RS04}

PLATE_T = 12.0
PLATE_BACK_Y = -2.0                 # housing front face sits here
PLATE_FRONT_Y = PLATE_BACK_Y + PLATE_T   # +10, flush with the rail's front face


def out_face_y(a):
    return PLATE_BACK_Y + a["boss"]


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
    w = a["w"]
    top = a["housing_bc"] / 2 + 11.0  # RS03: 60
    plate = (cq.Workplane("XZ", origin=(0, PLATE_FRONT_Y, 0))
             .polyline([(-w, FOOT_T), (w, FOOT_T), (w, AXIS_Z + top - 15), (w - 15, AXIS_Z + top), (-w + 15, AXIS_Z + top), (-w, AXIS_Z + top - 15)]).close()
             .extrude(PLATE_T))
    fx = a.get("foot_x", 96.0)
    foot = box(-fx, fx, -40, PLATE_FRONT_Y, 0, FOOT_T)
    body = plate.union(foot)
    for sx in (-1, 1):  # rear ribs, outside the housing ears
        rib = yz_profile([(PLATE_BACK_Y + 0.01, FOOT_T - 0.01), (-40, FOOT_T - 0.01), (-40, FOOT_T + 6), (PLATE_BACK_Y + 0.01, 72)], 0, 8)
        body = body.union(rib.translate((sx * (a["rib_x"] + 4) - 4, 0, 0)))  # 8 thick, inside the plate edge
    body = body.cut(y_cyl(a["bore"], PLATE_BACK_Y - 1, PLATE_FRONT_Y + 1, 0, AXIS_Z))
    r = a["housing_bc"] / 2
    for ang in a["housing_ang"]:
        x, z = r * math.cos(math.radians(ang)), r * math.sin(math.radians(ang))
        body = body.cut(y_cyl(a["h_clr"], PLATE_BACK_Y - 1, PLATE_FRONT_Y + 1, x, AXIS_Z + z))
        body = body.cut(y_cyl(a["h_head"], PLATE_BACK_Y + a["h_grip"], PLATE_FRONT_Y + 1, x, AXIS_Z + z))
    h1, h2 = a.get("foot_holes", (70.0, 86.0))
    for x in (-h2, -h1, h1, h2):  # foot to rail-top slot T-nuts
        body = body.cut(z_cyl(M5_CLR, -1, FOOT_T + 1, x, 0))
    return body


def adapter(a=RS03):
    """Output adapter: D44 spigot bolted to the output with 6x M4, hub with a channel the 2020 lever drops into,
    lever end stop, 4 captive M5 nuts for the clamp cap. Printed spigot face down. 100 % infill (part under load)."""
    of = out_face_y(a)
    spig = y_cyl(a["contact_d"], of, HUB_Y0 + 0.01, 0, AXIS_Z)
    if "spigot_d" in a:  # contact pad 1 mm, then a fatter spigot so the screw counterbores stay inside it
        spig = spig.union(y_cyl(a["spigot_d"], of + 1.0, HUB_Y0 + 0.01, 0, AXIS_Z))
    hub = y_cyl(HUB_D, HUB_Y0, WALL_Y1, 0, AXIS_Z)
    body = spig.union(hub)
    chan = box(STOP_X, 40, FLOOR_Y1, WALL_Y1 + 1, AXIS_Z - LEVER_W / 2, AXIS_Z + LEVER_W / 2)
    body = body.cut(chan)
    body = body.cut(y_cyl(min(16.0, a["output_bc"] - 14), of - 1, of + 1.0, 0, AXIS_Z))  # relief over the output centre features
    if "out_ang" in a:
        holes = [((a["output_bc"] / 2) * math.cos(math.radians(t)), (a["output_bc"] / 2) * math.sin(math.radians(t))) for t in a["out_ang"]]
    else:
        holes = bolt_circle(a["output_bc"], a["output_n"], 30.0)  # clocked 30 deg: two holes in the channel floor, four under the walls
    clr, head, grip = a.get("o_clr", M4_CLR), a.get("o_head", M4_HEAD), a.get("o_grip", 10.0)
    for x, z in holes:
        body = body.cut(y_cyl(clr, of - 1, WALL_Y1 + 1, x, AXIS_Z + z))
        body = body.cut(y_cyl(head, of + grip, WALL_Y1 + 1, x, AXIS_Z + z))
        if a.get("cb_slot") and abs(z) > 0.1:  # run the counterbore into the channel as a slot: no knife cusp where it meets the wall
            body = body.cut(box(x - head / 2, x + head / 2, of + grip, WALL_Y1 + 1, AXIS_Z + min(0, z), AXIS_Z + max(0, z)))  # M4x16: 10 grip, 6 in the output
    for i in range(len(holes)):  # neighbouring counterbores that would leave a sliver between them merge into one slot
        for j in range(i + 1, len(holes)):
            (x1, z1), (x2, z2) = holes[i], holes[j]
            d = math.hypot(x2 - x1, z2 - z1)
            if d < head + 1.2:
                body = body.cut(cq.Workplane("XZ", origin=(0, of + grip, 0)).center((x1 + x2) / 2, AXIS_Z + (z1 + z2) / 2)
                                .slot2D(d + head, head, math.degrees(math.atan2(z2 - z1, x2 - x1))).extrude(-(WALL_Y1 + 1 - of - grip)))
    for t in a.get("pins", []):  # sockets for the output's dowel pins: they take the torque in shear
        x, z = a["pin_r"] * math.cos(math.radians(t)), a["pin_r"] * math.sin(math.radians(t))
        body = body.cut(y_cyl(a["pin_hole"], of - 1, of + a["pin_depth"], x, AXIS_Z + z))
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
    **{f"marengo_teststand_upright_{k}": (lambda a=a: upright(a)) for k, a in ACTUATORS.items()},
    **{f"marengo_teststand_adapter_{k}": (lambda a=a: adapter(a)) for k, a in ACTUATORS.items()},
    "marengo_teststand_clamp_cap": clamp_cap,
    "marengo_teststand_load_block": load_block,
    "marengo_teststand_anchor": anchor,
    "marengo_teststand_lock_post": lock_post,
}


def vendor(a):
    """Vendor STEP placed in the stand frame: output face on out_face_y, axis on (0, AXIS_Z), STEP +y up.
    (x, y, z)_step -> (X, Y, Z) = (x - cx, out_face_y - (z - z_out), AXIS_Z + y - cy)."""
    step = Path("/tmp/claude-0/rs/Product Literature/CAN2.0") / a["step"]
    if not step.exists():
        return None
    cx, cy = a["step_c"]
    m = cq.importers.importStep(str(step)).translate((-cx, -cy, -a["step_out_z"]))
    if a.get("step_flip"):  # output faces +z in this STEP: half turn about y, so x -> -x and the output points along +Y
        m = m.rotate((0, 0, 0), (0, 1, 0), 180)
    return m.rotate((0, 0, 0), (1, 0, 0), 90).translate((0, out_face_y(a), AXIS_Z))


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
    for name, sh in ref.items():
        cq.exporters.export(sh, str(OUT / f"ref_{name}.stl"), tolerance=0.05, angularTolerance=0.2)
    shared = {n: sh for n, sh in solids.items() if not any(n.endswith("_" + k) for k in ACTUATORS)}
    shared["marengo_teststand_anchor"] = shared["marengo_teststand_anchor"].translate((ANCHOR_X, 0, 0))
    bad = []
    for k, a in ACTUATORS.items():  # one assembly per actuator size: its upright + adapter + vendor model + shared parts
        allp = {**shared, **ref, f"upright_{k}": solids[f"marengo_teststand_upright_{k}"], f"adapter_{k}": solids[f"marengo_teststand_adapter_{k}"]}
        act = vendor(a)
        if act is not None:
            allp[f"{k}_vendor"] = act
            cq.exporters.export(act, str(OUT / f"ref_{k}_vendor.stl"), tolerance=0.05, angularTolerance=0.2)
        names = list(allp)
        for i in range(len(names)):
            for j in range(i + 1, len(names)):
                v = common_volume(allp[names[i]], allp[names[j]])
                if v > 0.5:
                    bad.append((k, names[i], names[j], round(v, 1)))
    print("interference > 0.5 mm3:", bad or "none")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
