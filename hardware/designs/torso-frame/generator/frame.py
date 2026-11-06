#!/usr/bin/env python3
"""Parametric model of the Marengo internal torso frame, rev A (two-box stepped 2020 frame).

Writes parts.json (geometry for the viewer), bom.json and cutlist.md. Coordinates are the torso
STEP frame in mm: X lateral (robot left +), Y up from the pelvis bottom, Z forward. Robot frame:
robot X = torso Z, robot Y = torso X, robot Z = torso Y + 810 (shell generator convention).
All numbers come from hardware/designs/torso-stepped/README.md (arm thread) and
hardware/designs/body-shell/concept/generator/mounts.json (shell thread) unless marked here.
"""
import json, math, re

# ---- frame parameters -------------------------------------------------------------------------
E = 20.0                 # 2020 extrusion
Y_BOT = 155.0            # bottom of the frame (pelvis 115 + 40 mm waist bay). Rev A geometry: the 13S2P pack (Joey 2026-11-06 06:13Z) fits without changing it
# 13S2P 21700 pack (hardware/electrical/power/custom-21700-pack.md): 4 x 7 grid, cells along Z. Box 95 x 162 x 79.
PK_W, PK_H, PK_D = 95.0, 162.0, 79.0
PK_Z1 = 34.0             # pack front face Z; rear face at -45, 25 mm inside the frame rear face
PSU_T = 24.0             # power and bus board, standing between the pack and the front rail plane (Z 34 to 58)
Y_STEP_LO = 353.62       # underside of the shared step rails
Y_STEP_HI = Y_STEP_LO + E
Y_TOP = 495.0            # top of the frame
Y_SHOULDER = 424.86      # pitch axis
HX_LO = 75.0             # lower box outer half-width
HX_UP = 113.0            # upper box outer half-width
HZ = 70.0                # depth half (both tiers)
HX_SHELF = 75.0          # shelf rails outer X (same line as the lower posts)
G_T = 3.0                # gusset thickness, 6061-T6
RS03_D, RS03_L = 99.0, 54.1
RS03_REAR, RS03_FACE, RS03_OUT = 78.4, 132.5, 135.0
G_PER_MM = 0.5           # 2020 mass, g/mm (assumption)

parts = []
def add(**k):
    k.setdefault('explode', [0, 0, 0]); k.setdefault('balloon', False); k.setdefault('opacity', 1.0)
    parts.append(k); return k
def box(id, item, name, lo, hi, color, group, explode, **k):
    return add(id=id, item=item, name=name, type='box', lo=list(lo), hi=list(hi), color=color, group=group, explode=list(explode), **k)
def cyl(id, item, name, axis, c, r, h, color, group, explode, **k):
    return add(id=id, item=item, name=name, type='cyl', axis=axis, c=list(c), r=r, h=h, color=color, group=group, explode=list(explode), **k)

COL = dict(alu_keep='#8c9096', alu_cut='#c8a24a', alu_new='#3f8fd2', gusset='#d9d9de', bracket='#5a5e66',
           motor='#6f7378', printed='#1f9c8f', shelf='#9a7bd1', pi='#3aa655', pdb='#b05fa0', hard='#f08a24',
           tpu='#2f3136', collar='#f2f2f0', adapter='#1f9c8f')
EXT = {}  # cut list accumulator: (item) -> dict

def ext(id, item, name, lo, hi, source, explode, qtykey, balloon=False):
    L = max(hi[i] - lo[i] for i in range(3))
    col = {'keep': COL['alu_keep'], 'cut': COL['alu_cut'], 'new': COL['alu_new']}[source]
    box(id, item, name, lo, hi, col, 'extrusion', explode, balloon=balloon, length=L, source=source,
        material='2020 6063-T5', mass_g=round(L * G_PER_MM))
    d = EXT.setdefault(item, dict(item=item, name=name, length=L, source=source, qty=0))
    d['qty'] += 1

# ---- lower box ------------------------------------------------------------------------------
first = True
for sx in (1, -1):
    for sz in (1, -1):
        ext(f'post_lo_{sx}_{sz}', 1, 'Post, lower box, 198.6 (cut)', (sx * (HX_LO - E) if sx > 0 else -HX_LO, Y_BOT, sz * (HZ - E) if sz > 0 else -HZ),
            (HX_LO if sx > 0 else -(HX_LO - E), Y_STEP_LO, HZ if sz > 0 else -(HZ - E)), 'cut',
            (0, -70, 0), 'post_lo', balloon=first); first = False
first = True
for sz in (1, -1):
    ext(f'rail_bot_{sz}', 2, 'Rail, bottom front/rear, 110', (-(HX_LO - E), Y_BOT, sz * (HZ - E) if sz > 0 else -HZ),
        (HX_LO - E, Y_BOT + E, HZ if sz > 0 else -(HZ - E)), 'keep', (0, -140, 0), 'rail_bot', balloon=first); first = False
first = True
for sx in (1, -1):
    ext(f'rail_botside_{sx}', 3, 'Rail, bottom side, 100', (sx * (HX_LO - E) if sx > 0 else -HX_LO, Y_BOT, -(HZ - E)),
        (HX_LO if sx > 0 else -(HX_LO - E), Y_BOT + E, HZ - E), 'keep', (0, -140, 0), 'rail_botside', balloon=first); first = False
# ---- step tier -------------------------------------------------------------------------------
first = True
for sz in (1, -1):
    ext(f'rail_shared_{sz}', 4, 'Rail, shared step, front/rear, 226 (new)', (-HX_UP, Y_STEP_LO, sz * (HZ - E) if sz > 0 else -HZ),
        (HX_UP, Y_STEP_HI, HZ if sz > 0 else -(HZ - E)), 'new', (0, 0, 0), 'rail_shared', balloon=first); first = False
first = True
for sx in (1, -1):
    ext(f'rail_stepside_{sx}', 5, 'Rail, step side, 100', (sx * (HX_UP - E) if sx > 0 else -HX_UP, Y_STEP_LO, -(HZ - E)),
        (HX_UP if sx > 0 else -(HX_UP - E), Y_STEP_HI, HZ - E), 'keep', (0, 0, 0), 'rail_stepside', balloon=first); first = False
first = True
for sx in (1, -1):
    ext(f'rail_shelf_{sx}', 6, 'Rail, shelf, 100 (new)', (sx * (HX_SHELF - E) if sx > 0 else -HX_SHELF, Y_STEP_LO, -(HZ - E)),
        (HX_SHELF if sx > 0 else -(HX_SHELF - E), Y_STEP_HI, HZ - E), 'new', (0, 0, 0), 'rail_shelf', balloon=first); first = False
# ---- upper box -------------------------------------------------------------------------------
first = True
for sx in (1, -1):
    for sz in (1, -1):
        ext(f'post_up_{sx}_{sz}', 7, 'Post, upper box, 121.4 (cut)', (sx * (HX_UP - E) if sx > 0 else -HX_UP, Y_STEP_HI, sz * (HZ - E) if sz > 0 else -HZ),
            (HX_UP if sx > 0 else -(HX_UP - E), Y_TOP, HZ if sz > 0 else -(HZ - E)), 'cut', (0, 70, 0), 'post_up', balloon=first); first = False
first = True
for sz in (1, -1):
    ext(f'rail_top_{sz}', 8, 'Rail, top front/rear, 186 (new)', (-(HX_UP - E), Y_TOP - E, sz * (HZ - E) if sz > 0 else -HZ),
        (HX_UP - E, Y_TOP, HZ if sz > 0 else -(HZ - E)), 'new', (0, 140, 0), 'rail_top', balloon=first); first = False
first = True
for sx in (1, -1):
    ext(f'rail_topside_{sx}', 9, 'Rail, top side, 100', (sx * (HX_UP - E) if sx > 0 else -HX_UP, Y_TOP - E, -(HZ - E)),
        (HX_UP if sx > 0 else -(HX_UP - E), Y_TOP, HZ - E), 'keep', (0, 140, 0), 'rail_topside', balloon=first); first = False

# ---- gusset plates, 3 mm 6061-T6, front and rear faces of each step corner -------------------
GUSSET_PTS = [(55, 285), (75, 285), (113, 335), (113, 425), (93, 425), (93, Y_STEP_HI), (55, Y_STEP_HI)]
GUSSET_R = [8, 8, 15, 8, 8, 8, 8]   # R15 at the X 113 / Y 335 corner (297 MPa peak in the arm thread FEA)
GUSSET_HOLES = [(65, 297), (65, 317), (65, 337), (65, 363.62), (84, 363.62), (103, 363.62), (103, 385), (103, 400), (103, 415)]
first = True
for sx in (1, -1):
    for sz in (1, -1):
        pts = [(sx * x, y) for x, y in GUSSET_PTS]
        add(id=f'gusset_{sx}_{sz}', item=10, name='Gusset plate, 3 mm 6061-T6', type='poly', pts=pts, radii=GUSSET_R,
            holes=[(sx * x, y, 2.75) for x, y in GUSSET_HOLES], z0=HZ if sz > 0 else -HZ - G_T, t=G_T,
            color=COL['gusset'], group='gusset', explode=[0, 0, sz * 90], balloon=first, material='6061-T6 sheet, 3 mm',
            mass_g=40, note='4 off (about 40 g each, 5.0e3 mm2 x 3 mm x 2.70); the left-front plate is the drawing, mirror X for the right. 9 x 5.5 mm holes for M5 T-nuts.')
        first = False

# ---- 2028 corner brackets --------------------------------------------------------------------
BR_L, BR_W, BR_T = 28.0, 20.0, 4.0
def bracket(id, corner, nA, nB, extent_axis, extent, explode, balloon=False, leg=None):
    """L bracket at corner point; legs run along +nA and +nB (unit axis vectors as (axis, sign))."""
    legs = []; BR_L = leg or 28.0
    for (ax, sg), (bx, bs) in ((nA, nB), (nB, nA)):
        lo = [0, 0, 0]; hi = [0, 0, 0]
        for i in range(3):
            if i == ax: lo[i], hi[i] = (corner[i], corner[i] + BR_L) if sg > 0 else (corner[i] - BR_L, corner[i])
            elif i == bx: lo[i], hi[i] = (corner[i], corner[i] + BR_T) if bs > 0 else (corner[i] - BR_T, corner[i])
            else: lo[i], hi[i] = corner[i] - extent / 2, corner[i] + extent / 2
        legs.append((lo, hi))
    short = BR_L < 28
    add(id=id, item=34 if short else 11, name='Corner bracket, 2020 (20 mm legs), top inner-X corners, clears the pitch bracket wing' if short else 'Corner bracket, 2028, with 2 x M5 T-nuts',
        type='multibox', boxes=legs, color=COL['bracket'], group='bracket', explode=list(explode), balloon=balloon, material='Al bracket + M5 T-nuts', mass_g=14 if short else 18)

nb = 0
def BX(sx, sz, y, dy, which, explode, leg=None):
    """post corner brackets: which='x' = bracket between post X-inward face and a front/rear rail; 'z' = post Z-inward face and a side rail."""
    global nb
    hx = HX_LO if y <= Y_STEP_LO else HX_UP   # gate review: at Y_STEP_LO the bracket is on the LOWER post (was HX_UP: 38 mm off)
    cx = sx * (hx - E); cz = sz * (HZ - E)   # inner faces of the post
    if which == 'x':
        bracket(f'br{nb}', (cx, y, sz * (HZ - E / 2)), (0, -sx), (1, dy), 2, E, explode, balloon=(nb == 0 or leg is not None and nb == 16), leg=leg)
    else:
        bracket(f'br{nb}', (sx * (hx - E / 2), y, cz), (2, -sz), (1, dy), 0, E, explode, balloon=False)
    nb += 1
for sx in (1, -1):
    for sz in (1, -1):
        BX(sx, sz, Y_BOT + E, +1, 'x', (0, -140, 0)); BX(sx, sz, Y_BOT + E, +1, 'z', (0, -140, 0))      # bottom corners (8)
        BX(sx, sz, Y_STEP_LO, -1, 'x', (0, -70, 0))                                                                 # lower post top to shared rail (4)
        BX(sx, sz, Y_STEP_HI, +1, 'x', (0, 70, 0))                                                                  # upper post bottom to shared rail (4)
        BX(sx, sz, Y_TOP - E, -1, 'x', (0, 140, 0), leg=20)          # top corners (4). Gate 2 / arm thread 11:47Z: the top 'z' brackets sat inside the as-built pitch bracket (4440 mm3 each); deleted, the pitch bracket carries that corner
# step-tier horizontal rails to the shared rails: shelf rails (4) and step side rails (4)
for sx in (1, -1):
    for sz in (1, -1):
        pass   # Joey 16:39Z: these 8 joints are END-TAPPED (M5 x 35 through the gusset and shared rail into the tapped rail end), see the gusset bolts below. The 8 brackets sat in the shelf plate and its cable slots.
N_BRACKETS = nb

# ---- fasteners: M5 button heads + T-nuts on every bracket, gusset and hardpoint ---------------------
FAST = {'m5': 0, 'tnut': 0, 'm4': 0}
def screw(id, item, name, axis, head_pt, sign, explode, balloon=False, dia=5, head_d=8.5, head_h=4.5, length=10, color='#d8c56a'):
    """Button head screw: head sits on head_pt, shank goes along axis in direction sign (into the part)."""
    a = 'xyz'.index(axis); c = list(head_pt)
    ch = c.copy(); ch[a] += -sign * head_h / 2
    cs = c.copy(); cs[a] += sign * length / 2
    cyl(id + '_h', item, name, axis, ch, head_d / 2, head_h, color, 'fastener', explode, balloon=balloon, mass_g=2, material='M%d button head' % dia)
    cyl(id + '_s', item, name, axis, cs, dia / 2, length, color, 'fastener', explode, mass_g=0, material='')
def tnut(id, item, name, axis, pt, sign, explode, slot_axis):
    """T-nut 10 x 6 x 10 (standard 2020 slide-in / drop-in) in the slot behind face point pt (face normal = axis*sign). Gate review: was 20 long."""
    a = 'xyz'.index(axis); sa = 'xyz'.index(slot_axis)
    lo = [pt[k] - 5 for k in range(3)]; hi = [pt[k] + 5 for k in range(3)]
    lo[a], hi[a] = (pt[a] + sign * 1.5, pt[a] + sign * 7.5) if sign > 0 else (pt[a] + sign * 7.5, pt[a] + sign * 1.5)
    lo[sa], hi[sa] = pt[sa] - 5, pt[sa] + 5
    box(id, item, name, lo, hi, '#8a8d93', 'fastener', explode, mass_g=3, material='M5 T-nut')
def bolt_bracket(p, idx):
    """Two M5 + T-nuts for an L bracket (multibox): one through each leg into the member behind it."""
    for li, (lo, hi) in enumerate(p['boxes']):
        # the leg's thin axis is the bracket thickness axis; the screw goes along it, from the open side into the member
        t_ax = min(range(3), key=lambda k: hi[k] - lo[k]); L = max(range(3), key=lambda k: hi[k] - lo[k])
        cen = [(lo[k] + hi[k]) / 2 for k in range(3)]
        # the member is on the side of the leg that is away from the bracket corner: the corner lies at the start of the long leg direction
        other = p['boxes'][1 - li]; ocen = [(other[0][k] + other[1][k]) / 2 for k in range(3)]
        sign = 1 if ocen[t_ax] < cen[t_ax] else -1      # member is opposite to the other leg
        head = cen.copy(); head[t_ax] = lo[t_ax] if sign > 0 else hi[t_ax]   # gate 2: head on the leg's open face (was on the member side, inside the leg)
        head[L] += 2.5 if cen[L] > ocen[L] else -2.5   # gate 2: the two heads met in the inside corner; holes 2.5 mm further out along the leg
        FAST['m5'] += 1; FAST['tnut'] += 1
        screw(f"f{idx}_{li}", 35, 'M5 x 10 button head + T-nut (brackets)', 'xyz'[t_ax], head, sign, p['explode'], balloon=(idx == 0 and li == 0))
        slot_ax = [k for k in range(3) if k not in (t_ax, L)][0]
        tn = head.copy(); tn[t_ax] = hi[t_ax] if sign > 0 else lo[t_ax]   # gate 2: nut behind the member face (was 4 mm shallow)
        tnut(f"t{idx}_{li}", 35, 'M5 T-nut', 'xyz'[t_ax], tn, sign, p['explode'], 'xyz'[L] if abs(cen[L]) < 1e9 else 'xyz'[slot_ax])
for idx, p in enumerate([q for q in parts if q['group'] == 'bracket']):
    bolt_bracket(p, idx)
# gusset bolts: 9 per plate, screws along Z into the frame, heads on the plate outer face
for p in [q for q in parts if q['group'] == 'gusset']:
    sz = 1 if p['z0'] > 0 else -1
    for k, (x, y, r) in enumerate(p['holes']):
        head = [x, y, p['z0'] + (p['t'] if sz > 0 else 0)]
        if abs(y - 363.62) < 0.1 and abs(abs(x) - 84) > 1:   # end-tap holes at the shelf rail (X 65) and step side rail (X 103) ends
            screw(f"g{p['id']}_{k}", 41, 'M5 x 35 socket head, END-TAP: through the gusset and the shared rail into the M5-tapped core of the shelf / step side rail end (8; Joey 16:34Z)', 'z', head, -sz, p['explode'], balloon=(p['id'] == 'gusset_1_1' and abs(x) < 70), length=35, head_h=5, color='#b0b4ba')
            FAST['m5_endtap'] = FAST.get('m5_endtap', 0) + 1
            continue
        screw(f"g{p['id']}_{k}", 36, 'M5 x 10 button head + T-nut (gussets, 36)', 'z', head, -sz, p['explode'], balloon=(k == 0 and p['id'] == 'gusset_1_1'))
        tnut(f"gt{p['id']}_{k}", 36, 'M5 T-nut', 'z', [x, y, p['z0'] if sz > 0 else p['z0'] + p['t']], -sz, p['explode'], 'y' if abs(y - 363.62) > 1 else 'x')
        FAST['m5'] += 1; FAST['tnut'] += 1

# ---- shoulder pitch actuators and brackets -----------------------------------------------------
for sx in (1, -1):
    cyl(f'rs03_boss_{sx}', 12, 'RS03 rear boss, D80 with two connector flats', 'x', (sx * (RS03_REAR + 6.8), Y_SHOULDER, 0), 40, 13.6, COL['motor'], 'actuator', (sx * 150, 70, 0))
    cyl(f'rs03_pitch_{sx}', 12, 'RS03, shoulder pitch (existing)', 'x', (sx * (92 + (RS03_FACE - 92) / 2), Y_SHOULDER, 0), RS03_D / 2, RS03_FACE - 92,
        COL['motor'], 'actuator', (sx * 150, 70, 0), balloon=(sx > 0), mass_g=880, material='RobStride RS03')
    cyl(f'rs03_pitch_out_{sx}', 12, 'RS03 output flange', 'x', (sx * (RS03_FACE + (RS03_OUT - RS03_FACE) / 2), Y_SHOULDER, 0), 40, RS03_OUT - RS03_FACE,
        '#9a9ea4', 'actuator', (sx * 150, 70, 0))
    xin, xout = sorted((sx * RS03_REAR, sx * HX_UP))
    box(f'pitch_bracket_{sx}', 13, 'Shoulder pitch bracket, PETG (existing part, moved out 38 mm; envelope from the arm thread CAD check)', (xin, Y_STEP_HI, -50), (xout, Y_TOP, 50),
        COL['printed'], 'actuator', (sx * 80, 70, 0), balloon=(sx > 0), mass_g=120, material='PETG print', opacity=0.85)
    xw0, xw1 = sorted((sx * RS03_REAR, sx * (HX_UP - E)))
    box(f'pitch_wing_{sx}', 13, 'pitch bracket wing', (xw0, 400, -69), (xw1, 450, 69), COL['printed'], 'actuator', (sx * 80, 70, 0), mass_g=0, material='PETG print', opacity=0.85)
    # real slots from the original torso STEP (arm thread 2026-11-06 06:09Z): 4 per bracket, bolts along X through 4 mm ears at X 89-93,
    # onto the X 93 member faces: step side rail (Y 363.69, Z 0), top side rail (Y 485, Z 0), front and rear upper posts (slot Y 416.6-432.8, Z +-60).
    # Bolt size not in CAD (slots fit M5/M6): M5 assumed, checked on the built robot.
    for k, (yy, zz) in enumerate(((363.69, 0), (485.0, 0), (424.7, 60), (424.7, -60))):
        FAST['m5'] += 1; FAST['tnut'] += 1
        screw(f'pb{sx}_{k}', 39, 'M5 x 10 button head + DIN 125 washer + T-nut, pitch bracket ears (slots 6.2-6.4 wide, arm thread) to the step side rail, top side rail and upper posts (8; real slot positions, bolt size assumed; gate 2: x12 bottomed in the slot)', 'x', [sx * 89, yy, zz], sx, (sx * 80, 70, 0), balloon=(sx > 0 and k == 0), length=10)
        tnut(f'pbt{sx}_{k}', 39, 'M5 T-nut', 'x', [sx * 93, yy, zz], sx, (sx * 80, 70, 0), 'z' if zz == 0 else 'y')   # gate 2: these nuts were counted but not drawn
    for k in range(8):
        ang = math.radians(k * 45 + 22.5); FAST['m4'] += 1
        screw(f'c{sx}_{k}', 38, 'M4 x 10 button head, collar on the RS03 outboard holes (16)', 'x', [sx * (RS03_OUT + 3), Y_SHOULDER + 49 * math.cos(ang), 49 * math.sin(ang)], -sx, (sx * 200, 70, 0), balloon=(k == 0 and sx > 0), dia=4, head_d=7.5, head_h=4, length=10, color='#cfae4a')
    cyl(f'collar_{sx}', 25, 'Shell collar half-pair on the 8 outboard M4 holes (shell part)', 'x', (sx * (RS03_OUT + 1), Y_SHOULDER, 0), 60, 2,
        COL['collar'], 'shellpart', (sx * 200, 70, 0), balloon=(sx > 0), mass_g=15, material='PETG print', opacity=0.9)

# ---- compute shelf, Pi, PDB, IMU -------------------------------------------------------------
box('shelf', 14, 'Compute shelf plate (existing: X +-55 between the shelf rails, Y 354.6 to 372.6; arm thread CAD check)', (-55, 354.6, -50), (55, 372.6, 50), COL['shelf'], 'electronics',
    (0, 40, 0), balloon=True, mass_g=60, material='existing part')
box('pi5', 15, 'Raspberry Pi 5 + 2-CH CAN HAT (existing)', (-37.5, 376, -71.8), (19.7, 397, 13.2), COL['pi'], 'electronics',
    (0, 70, 0), balloon=True, mass_g=110, material='electronics', note='Position from the arm thread CAD check; ports face the rear (-Z) and reach Z -71.8')
# power and peripheral board (board thread, main 0b6a6ef, hardware/electrical/pdb/PLAN.md s.5-6): two PCBs coplanar on one carrier, 6 x M3 at X +-46, Y 194/265/336.
# Rear face moved from Z 34 to 37.5: 3 mm air plus a 0.5 mm insulating sheet over the pack lid (board thread ask 1). Front face Z 61.5, 8.5 mm behind the frame front face (Z 70; the front rails are only at Y 155-175 and 353.6-373.6, so the board sees open space between the posts).
PDB_Z0 = PK_Z1 + 3.5
box('pdb', 16, 'Power and logic boards on one carrier (BMS onboard): power board ~100 x 95 (13S BMS, fuse, precharge, contactor, motor ports, DC-DC at Y 250-290 in the intake jet) on 6 mm standoffs', (-50, 245, PDB_Z0 + 10), (50, 340, PDB_Z0 + PSU_T), COL['pdb'], 'electronics',
    (0, 0, 120), balloon=True, mass_g=180, material='PCB + parts, 100 x 95 x 18', opacity=0.7,
    note='Board thread plan main 0b6a6ef. Top edge: pack XT90 + 16-pin balance (rear face), both arm ports, USB to the Pi, fan leads. Sides: E-stop loop, charge port, sensors.')
box('pdb_logic', 16, 'Logic / peripheral board ~100 x 55 below the power board, same carrier', (-50, 190, PDB_Z0 + 10), (50, 243, PDB_Z0 + PSU_T), COL['pdb'], 'electronics', (0, 0, 120), mass_g=70, material='PCB + parts, 100 x 55 x 18', opacity=0.7,
    note='Bottom edge: waist and leg ports, brake resistor lead.')
box('pdb_sheet', 16, 'Insulating sheet 0.5 mm (Nomex or FR4) over the pack lid, between the tray rails, 3 mm air gap to the carrier', (-42, 190, PK_Z1), (42, 340, PK_Z1 + 0.5), '#e8e2c0', 'electronics', (0, 0, 90), mass_g=5, material='Nomex 410 0.5 mm', opacity=0.8)
box('pdb_carrier', 16, 'Board carrier, printed, screwed to the front ends of the pack tray rails (gate 2: it had no attachment); holds the two PCBs coplanar on 6 mm M3 standoffs', (-55, 190, PDB_Z0), (55, 340, PDB_Z0 + 4), COL['printed'], 'electronics', (0, 0, 100), mass_g=60, material='PETG print 100 x 150 x 4', opacity=0.8)
for k, (xx, yy) in enumerate(((46, 194), (-46, 194), (46, 265), (-46, 265), (46, 336), (-46, 336))):
    # gate 2: the screws floated 10 mm above the carrier. Now: M3 x 6 through the 1.6 mm PCB into a 6 mm F-F standoff on the carrier, second M3 x 6 from the carrier rear
    screw(f'pdb_m3_{k}', 16, 'M3 x 6 (2 per standoff) + 6 mm F-F standoff, board to carrier (6 standoffs, 6 mm keep-out pads at X +-46, Y 194/265/336)', 'z', [xx, yy, PDB_Z0 + 11.6], -1, (0, 0, 150), balloon=(k == 0), dia=3, head_d=5.5, head_h=1.7, length=6, color='#c9b86a')
    cyl(f'pdb_so_{k}', 16, 'M3 F-F standoff 6 mm', 'z', (xx, yy, PDB_Z0 + 7), 2.75, 6, '#c9b86a', 'fastener', (0, 0, 130), mass_g=1, material='brass')
    FAST['m3'] = FAST.get('m3', 0) + 2
# brake resistor (board thread ask 3): 50 W aluminium-clad, 25 x 50 x 8, on the inner face of the front bottom rail, under the logic board's bottom edge, in the intake downdraft
# gate 2 + board thread a50269c3: Arcol HS50 real size 72.5 over the tags (49.1 body) x 29.7 x 14.8, must bolt flat to aluminium.
# The -X side rail spot (25 x 50 x 8) cannot take it (it would run into the pack and tray rail). New spot: flat on the inner (-Z) face of the
# FRONT bottom rail, long axis along X, |X| <= 36.3 of 55 available (room for a bigger part at the leg stage), Y 155-184.7, Z 35.2-50:
# behind the logic board's bottom edge (Y 190), in front of the pack (Z <= 34), in the lower intake's air.
# board thread 11:30Z: rev A takes an Arcol HS25 (28.0 wide, 51.0 over the tags, 27.3 body, 14.6 tall, holes 18.3 x 19.8; 10 ohm fine for the arm stage).
# On a 2 mm plate that hangs 1 mm below the frame bottom, the body spans Y 154-182, 1 mm clear of the pack bottom (Y 183). The HS50 (29.7 wide) overlapped the pack by 1.7 mm.
box('brake_plate', 16, 'Brake resistor adapter plate, 2 mm 6061, 70 x 30, on the front bottom rail inner face (2 x M5 drop-in T-nuts), hangs 1 mm below the frame bottom (hip stage: keep Y 154-155 free at |X| <= 35)', (-35, 154, 48), (35, 184, 50), '#b8bcc4', 'electronics', (0, -40, 60), mass_g=12, material='2 mm 6061')
box('brake_res', 16, 'Brake resistor Arcol HS25 10 ohm (51.0 over the tags x 28.0 x 14.6) flat on the adapter plate, 4 x M3 on 18.3 x 19.8; 1 mm clear of the pack bottom; leads to the logic board bottom edge (LP connectors at |X| > 36 or above Y 190); the leg stage takes a bigger part in the rail reserve', (-25.5, 154, 33.4), (25.5, 182, 48), '#9a9ea6', 'electronics', (0, -40, 60), balloon=True, mass_g=35, material='Arcol HS25, 4 x M3')
# harness trunks: Pi/PDB to each shoulder RS03 and down to the waist
for sx in (1, -1):
    # gate 2: was X +-42..78 and ran through the Jetson (X +-51.5). Now stops at |X| 52 and turns behind the Jetson (Z -58, 12 mm of air behind its rear face)
    cyl(f'harness_shoulder_{sx}', 27, 'Harness trunks: shoulder RS03 connector runs (right-angle exit inboard from the rear-boss flat, flat A assumed), then behind the Jetson to the centre', 'x', (sx * 65, 452, -27), 5, 26, '#d8b84a', 'harness', (0, 70, 0), balloon=(sx > 0), mass_g=40, material='silicone 14 AWG + CAN pair', opacity=0.95)
# side band trunk below
box('harness_pack', 27, 'Cable path reserved: pack power (10 AWG pair) and sense harness from the HP top edge over the pack top edge to the XT90S and sense board at the pack rear top corner (8.6 mm under the shared rail)', (-5, 345, -45), (25, 353.6, 37.5), '#d8b84a', 'harness', (0, 0, 0), balloon=True, mass_g=30, material='reserved path', opacity=0.6)
# Joey 16:34Z: cable passage = intentional oval slots in the printed shelf (PC-case style grommet slots), as on the built shelf.
# Front-edge slots sit straight above the HP top corners (arm terminals inboard of |X| 44) and the side bands; 4 mm land to the shelf front edge.
SHELF_SLOTS = [('shelf_slot_L', -42, -22), ('shelf_slot_C', -10, 10), ('shelf_slot_R', 22, 42)]   # corners stop at |X| 42, clear of the shelf-rail bracket heads
for sid, x0, x1 in SHELF_SLOTS:
    box(sid, 27, 'Oval cable slot in the printed shelf, 20 x 12, Z 34-46 (Joey 16:34Z): corners = arm pigtails, 12 V and fan leads; centre = USB, CAN and Pi stub 12 V', (x0, 354.6, 34), (x1, 372.6, 46), '#d8b84a', 'harness', (0, 0, 0), mass_g=0, material='slot (no part)', opacity=0.6)
for sx in (1, -1):
    box(f'harness_side_{sx}', 27, 'Cable trunk, side band X +-50..55, Z 42..50 (5 x 8 mm per side, tight: board thread to confirm bundle size): arm pigtails up to the proposed shelf corner notch, waist/aux/leg/charge pigtails down past the LP board to inline connectors below Y 190. Gate 2: replaces the front-corner path at Z 62-70, which ran through the front rails and 8 corner brackets', (min(sx*50, sx*55), 160, 42), (max(sx*50, sx*55), 353.6, 50), '#d8b84a', 'harness', (sx*40, 0, 0), balloon=(sx>0), mass_g=15, material='reserved path', opacity=0.6)
cyl('harness_shoulder_x', 27, 'Shoulder harness cross run behind the Jetson (Z -58), joins both shoulder runs to the step bus', 'x', (0, 452, -58), 5, 104, '#d8b84a', 'harness', (0, 70, 0), mass_g=20, material='silicone 14 AWG + CAN pair', opacity=0.95)
box('imu_old', 15, 'BNO085 IMU (existing)', (30, Y_STEP_HI + 3, -10), (50, Y_STEP_HI + 9, 10), '#8fd36b', 'electronics', (0, 70, 0), mass_g=5, material='electronics')

# ---- waist interface ---------------------------------------------------------------------------
box('waist_adapter', 18, 'Waist yaw adapter plate, PETG (existing; bolts to the bottom rectangle, pattern unverified)', (-HX_LO, Y_BOT - 6, -HZ), (HX_LO, Y_BOT, HZ),
    COL['adapter'], 'waist', (0, -190, 0), balloon=True, mass_g=120, material='PETG print', opacity=0.9,
    note='Placeholder 150 x 140 x 6 plate under the bottom rectangle. Joey to confirm how it bolts on.')
cyl('rs03_waist', 17, 'RS03, waist yaw (existing; 57 + 6 mm adapter in a 40 mm bay, 23 mm short: hip stage item)', 'y', (0, Y_BOT - 6 - 28.5, 0), RS03_D / 2, 57,
    COL['motor'], 'waist', (0, -250, 0), balloon=True, mass_g=880, material='RobStride RS03')

# ---- shell hardpoints (torso coords; from mounts.json: torso X = robot Y, torso Y = robot Z - 810, torso Z = robot X) ----
HP = {
 'H1': dict(item=19, face='+Z', pts=[(60, 485, 70), (-60, 485, 70)], carries='U1 upper front, top edge'),
 'H2': dict(item=20, face='-Z', pts=[(85, 485, -70), (-85, 485, -70)], carries='U2 upper back top edge, D1 catch, spine pads'),
 'H3': dict(item=21, face='+-Z', pts=[(35, 364, 70), (-35, 364, 70), (35, 364, -70), (-35, 364, -70)], carries='U1/U2 bottom, U3/U4 top, D1 frame (Y 374-492), rear: spine rail lip for the D2 magnets (Y 355-372), spine pads'),
 'H4': dict(item=22, face='+-Z', pts=[(40, Y_BOT + 10, 70), (-40, Y_BOT + 10, 70), (40, Y_BOT + 10, -70), (-40, Y_BOT + 10, -70)], carries='U3/U4 bottom, D2 battery door bottom hinge (D2 |X| <= 60, Y 175-352), spine pads'),
 'H5': dict(item=23, face='+-X', pts=[(75, 255, 60), (75, 255, -60), (-75, 255, 60), (-75, 255, -60)], carries='U3/U4 sides, V1 grilles'),
 'H6': dict(item=24, face='+-X', pts=[(75, Y_BOT + 10, 35), (75, Y_BOT + 10, -35), (-75, Y_BOT + 10, 35), (-75, Y_BOT + 10, -35)], carries='abdomen band carrier (PROPOSED: outer side faces of the bottom side rails, was the under-face)'),
}
for hid, h in HP.items():
    for i, (x, y, z) in enumerate(h['pts']):
        if abs(z) >= HZ - 0.1: n = (0, 0, 1 if z > 0 else -1)
        else: n = (1 if x > 0 else -1, 0, 0)
        w = 30; t = 12; hgt = 20
        lo = [x - w / 2, y - hgt / 2, z - w / 2]; hi = [x + w / 2, y + hgt / 2, z + w / 2]
        for a in range(3):
            if n[a] > 0: lo[a] = [x, y, z][a]; hi[a] = [x, y, z][a] + t
            if n[a] < 0: hi[a] = [x, y, z][a]; lo[a] = [x, y, z][a] - t
        tier = 0
        FAST['m5'] += 1; FAST['tnut'] += 1
        hp = [x, y, z]; hp['xyz'.index('xyz'[[k for k in range(3) if n[k]][0]])] += (t if n[[k for k in range(3) if n[k]][0]] > 0 else -t)
        screw(f'hf_{hid}_{i}', 37, 'M5 x 18 button head + T-nut (shell hardpoints; gate 2: x20 bottomed in the slot before clamping 12 mm of PETG, x16 left ~2 mm of thread)', 'xyz'[[k for k in range(3) if n[k]][0]], hp, -[v for v in n if v][0], (n[0] * 40, 0, n[2] * 40), balloon=(hid == 'H1' and i == 0), length=18)
        ax = 'xyz'[[k for k in range(3) if n[k]][0]]
        tnut(f'hft_{hid}_{i}', 37, 'M5 T-nut', ax, [x, y, z], -[v for v in n if v][0], (n[0] * 40, 0, n[2] * 40), 'x' if ax == 'z' else ('z' if hid == 'H6' else 'y'))   # gate 2: counted, not drawn
        box(f'{hid}_{i}', h['item'], f'Shell hardpoint bracket {hid}, PETG with M3 inserts, on M5 T-nut', lo, hi, COL['hard'], 'hardpoint',
            (n[0] * 40, 0, n[2] * 40), balloon=True, tag=hid, mass_g=8, material='PETG print', carries=h['carries'])

# spine rail lip: bridges the two rear H3 brackets; the D2 battery door's magnets close onto it (shell thread, main 761c62b). Stays below Y 374 (Pi ports).
box('spine_rail', 21, 'Spine rail lip on the H3 rear brackets, |X| <= 52 (gate 2: |X| <= 60 hit the rear gussets and their screw heads), Y 355-372, 12 mm proud of the frame rear face: magnet catch for the D2 battery door (shell thread, main 761c62b)', (-52, 355, -82), (52, 372, -70), COL['hard'], 'hardpoint',
    (0, 0, -40), balloon=True, tag='H3 rail', mass_g=12, material='PETG print, part of H3 rear', carries='D2 door magnets (3), joins the two H3 rear brackets')
# ---- packaging envelopes for Joey's component list (2026-11-06): proposed positions, not designed parts ----
PRO = '#26b3c9'
box('jetson', 29, 'Jetson Orin Nano dev kit envelope 103 x 90.5 x 45 including its fan, second shelf above the Pi, ports to the rear', (-51.5, 410, -45.5), (51.5, 455, 45), PRO, 'proposed', (0, 110, 0), balloon=True, mass_g=250, material='proposed', opacity=0.75,
    note='Fits the upper bay between the RS03 rears (156.8 wide); 20 mm left above the cooler under the top rails.')
box('cradle_strap', 40, 'Cradle top strap, PETG ~8 g, two hooks (30 mm gap at X -5..25 for the pack power leads, pending SolidWorks pass), slides into rear-open slots in the webs after the pack, detent', (-54.2, 345.5, -15), (-5, 348.5, 5), COL['printed'], 'electronics', (0, 60, 0), mass_g=4, material='PETG print', opacity=0.8)
box('cradle_strap_b', 40, 'cradle strap hook', (25, 345.5, -15), (54.2, 348.5, 5), COL['printed'], 'electronics', (0, 60, 0), mass_g=9, material='PETG print', opacity=0.8)
box('fan_exhaust_1', 30, 'Fans 40 x 40 x 10: 2 exhaust (upper rear), 2 intake (lower front). Exhaust PROVISIONAL: in the D1 door plane behind the frame rear face (needs a 12 mm shell gap); the 2026-11-06 reviews showed 13 mm of a fan inside the bay would blow into the top rail', (-55, 430, -80), (-15, 470, -70), PRO, 'proposed', (0, 150, 0), balloon=True, mass_g=20, material='proposed', opacity=0.75)
box('fan_exhaust_2', 30, 'Exhaust fan', (15, 430, -80), (55, 470, -70), PRO, 'proposed', (0, 150, 0), mass_g=20, material='proposed', opacity=0.75)
box('fan_intake_hp', 30, 'Intake fan 40 x 40 x 10 in the front panel grille, Y 264-304, on the E-stop banks and the lower FET bank of the power board (dropped 8 mm from 272-312 for the U1/U3 shell seam at Y 315)', (-20, 264, 70), (20, 304, 80), PRO, 'proposed', (0, 0, 120), mass_g=20, material='proposed', opacity=0.75)
box('fan_intake_lp', 30, 'Intake fan 40 x 40 x 10 in the front panel grille, Y 193-233, on the 12/24 V DC-DC of the logic board (7 W) and the brake resistor', (-20, 193, 70), (20, 233, 80), PRO, 'proposed', (0, 0, 120), mass_g=20, material='proposed', opacity=0.75)
box('compute3', 31, 'Third compute / USB hub board envelope 90 x 60 x 20: no home yet (the pack and bus board fill the lower box); candidates: pelvis, or a second layer over the Pi', (-45, 60, -10), (45, 120, 10), PRO, 'future', (0, -330, 0), balloon=True, mass_g=60, material='proposed', opacity=0.4)
box('pack', 32, '13S2P 21700 pack, 95 x 162 x 79, 2.2 kg: 26 x Molicel P45B/P50B in a 4 x 7 holder grid, cells along Z, balance leads to the bus board, slides out the back through D2', (-PK_W / 2, 183, PK_Z1 - PK_D), (PK_W / 2, 183 + PK_H, PK_Z1),
    '#e0c23a', 'electronics', (0, 0, -200), balloon=True, mass_g=2200, material='custom pack, hardware/electrical/power/custom-21700-pack.md', opacity=0.9,
    note='46.8 V nominal, 54.6 V full, 421-468 Wh. 7.5 mm to the posts each side, 8 mm top and bottom, 25 mm inside the frame rear face. Fuse and XT90S at the top rear corner.')
box('pack_bms', 32, 'JK-BD6A17S8P BMS 133 x 81 x 18 on standoffs, pack rear face, long axis along Y, FET heatsink facing rearward to D2 (battery thread, Joey picked on-pack BMS, main d42e88f)', (-40.5, 186, -65), (40.5, 319, -45), COL['pdb'], 'electronics', (0, 0, -90), mass_g=180, material='JK-BD6A17S8P', opacity=0.8,
    note='Rides on the pack inside the D2 opening (|X| <= 60), 5 mm inside the frame rear face; slides out with the pack. Replaces the pack sense board.')
# battery thread 20:13Z (provisional until the case CAD): mated XT90S lying flat on the pack rear face, axis along X; XT60 charge pigtail beside it; fuse inside the case
box('pack_xt90', 32, 'Mated XT90S pair on a printed saddle, flat on the pack rear face, axis along X (battery thread, provisional)', (-15, 323, -60), (43, 345, -45), '#e0c23a', 'electronics', (0, 0, -90), mass_g=30, material='XT90S', opacity=0.8)
box('pack_chg_xt60', 32, 'XT60 charge pigtail, parked flat on the pack rear face (battery thread, on-pack BMS)', (-42, 325, -58), (-17, 345, -45), '#e0c23a', 'electronics', (0, 0, -90), mass_g=10, material='XT60', opacity=0.8)
for sx in (1, -1):
    # Joey 16:32Z cradle, SolidWorks model main f9621274 (../cradle/): web |X| 48-51.5, feet over the bottom side rail and under the shelf rail,
    # ledge and front lip, thumbscrew boss at Y 252.6 Z -5. Envelope boxes here; the STEP is the source.
    box(f'packrail_{sx}', 40, 'Pack cradle side, PETG, ~61 g (web |X| 48-51.5, Y 175-353.6, Z -50..37.5; feet on the bottom side rail and under the shelf rail on 2 x M5 each; ledge, front lip, M4 thumbscrew boss at Y 252.6 Z -5, 2 carrier insert pads at |X| 51.75); STEP in ../cradle/', (min(sx * 48, sx * 51.5), 175, -50), (max(sx * 48, sx * 51.5), 353.6, 37.5), COL['printed'], 'electronics', (0, 0, -60), balloon=(sx > 0), mass_g=61, material='PETG print, ../cradle/', opacity=0.8)
    box(f'cradle_ledge_{sx}', 40, 'cradle ledge', (min(sx * 40, sx * 48), 179, -45), (max(sx * 40, sx * 48), 183, 34), COL['printed'], 'electronics', (0, 0, -60), mass_g=0, material='', opacity=0.8)
    box(f'cradle_boss_{sx}', 40, 'cradle thumbscrew boss, M4 x 16 thumbscrew into the pack side insert (snug only: it pulls the pack toward its web)', (min(sx * 51.5, sx * 59), 244.6, -13), (max(sx * 51.5, sx * 59), 260.6, 3), COL['printed'], 'electronics', (0, 0, -60), mass_g=0, material='', opacity=0.8)
box('pi_stub', 16, 'Pi power stub 30 x 20 x 10 on the compute shelf beside the Pi: 12 V lead from the logic board, 5 V into the GPIO header (board thread PLAN.md 6a, main e7d0b62)', (22, Y_STEP_HI, 12), (52, Y_STEP_HI + 10, 32), COL['pdb'], 'electronics', (0, 80, 0), balloon=True, mass_g=10, material='PCB 30 x 20 x 10', opacity=0.8)
box('bno085', 33, 'Sensors (item 33): the BNO085 is the existing IMU on the shelf (item 15, X 30-50, Z -10..10; this entry duplicated it at the front corner); head cameras and mics are future, on the top rails', (30, Y_STEP_HI + 3, -10), (50, Y_STEP_HI + 9, 10), PRO, 'proposed', (0, 70, 0), balloon=False, mass_g=0, material='same part as imu_old', opacity=0.0)
# ---- spine pads (rear crush pads, outside the shell; shell manufacturing thread geometry, main c38d83e shell rev B: four pads, bridges removed for the D2 battery door) ----
first = True
for sx in (1, -1):
    for (x0, x1, y0, y1, nm) in ((68, 147, 368, 481, 'upper'), (73, 129, 166, 295, 'lower')):
        box(f'spine_{nm}_{sx}', 26, 'TPU spine pads (4), 30 mm proud of the shell back, on standoffs to H2/H3/H4 (shell part, outside the shell); inner edges 6 mm clear of D1/D2', (min(sx * x0, sx * x1), y0, -124), (max(sx * x0, sx * x1), y1, -77),
            COL['tpu'], 'shellpart', (0, 0, -120), balloon=first, mass_g=65, material='TPU 95A gyroid', opacity=0.7); first = False
# pelvis ring (draft, hip stage) and the shoulder joint line markers
box('pelvis_ghost', 28, 'Pelvis ring, DRAFT (hip stage, not this release)', (-107.5, 0, -130), (107.5, 115, 130), '#b9bec8', 'future', (0, -330, 0), balloon=True, mass_g=0, material='draft envelope 215 x 260 x 115', opacity=0.25)

# ---- "as built" single-box frame for the change sheet -----------------------------------------
built = []
def bbox(id, lo, hi, color):
    built.append(dict(id=id, item=0, name=id, type='box', lo=list(lo), hi=list(hi), color=color, group='built', explode=[0, 0, 0], balloon=False, opacity=1.0))
for sx in (1, -1):
    for sz in (1, -1):
        bbox(f'b_post_{sx}_{sz}', (sx * (HX_LO - E) if sx > 0 else -HX_LO, Y_BOT, sz * (HZ - E) if sz > 0 else -HZ), (HX_LO if sx > 0 else -(HX_LO - E), Y_TOP, HZ if sz > 0 else -(HZ - E)), COL['alu_keep'])
for y in (Y_BOT, Y_TOP - E):
    for sz in (1, -1):
        bbox(f'b_rail_{y}_{sz}', (-(HX_LO - E), y, sz * (HZ - E) if sz > 0 else -HZ), (HX_LO - E, y + E, HZ if sz > 0 else -(HZ - E)), COL['alu_keep'])
    for sx in (1, -1):
        bbox(f'b_side_{y}_{sx}', (sx * (HX_LO - E) if sx > 0 else -HX_LO, y, -(HZ - E)), (HX_LO if sx > 0 else -(HX_LO - E), y + E, HZ - E), COL['alu_keep'])
for sx in (1, -1):
    bbox(f'b_shelfside_{sx}', (sx * (HX_LO - E) if sx > 0 else -HX_LO, Y_STEP_LO, -(HZ - E)), (HX_LO if sx > 0 else -(HX_LO - E), Y_STEP_HI, HZ - E), COL['alu_keep'])
for sx in (1, -1):
    built.append(dict(id=f'b_rs03_{sx}', item=0, name='rs03', type='cyl', axis='x', c=[sx * (40.4 + RS03_L / 2), Y_SHOULDER, 0], r=RS03_D / 2, h=RS03_L, color=COL['motor'], group='built', explode=[0, 0, 0], balloon=False, opacity=1.0))

# ---- dimension anchor points -------------------------------------------------------------------
dims = {
 'front': [  # [p1, p2, offset_dir, offset_mm, text]
   ([-HX_UP, Y_TOP, HZ], [HX_UP, Y_TOP, HZ], 'y', 45, '226'),
   ([-HX_LO, Y_BOT, HZ], [HX_LO, Y_BOT, HZ], 'y', -45, '150'),
   ([-RS03_OUT, Y_SHOULDER, HZ], [RS03_OUT, Y_SHOULDER, HZ], 'y', 150, '270 across RS03 output faces'),
   ([HX_UP, Y_BOT, HZ], [HX_UP, Y_TOP, HZ], 'x', 95, '340'),
   ([HX_UP, Y_STEP_LO, HZ], [HX_UP, Y_TOP, HZ], 'x', 50, '141.4 upper box'),
   ([HX_UP, Y_BOT, HZ], [HX_UP, Y_STEP_LO, HZ], 'x', 50, '198.6 lower box'),
   ([-HX_UP, Y_BOT, HZ], [-HX_UP, Y_SHOULDER, HZ], 'x', -70, '269.9 to pitch axis'),
   ([-HX_UP, 285, HZ], [-HX_UP, 425, HZ], 'x', -130, 'gusset 140'),
   ([HX_LO, 300, HZ], [HX_UP, 300, HZ], 'y', -10, '38 step'),
 ],
 'side': [
   ([0, Y_TOP, -HZ], [0, Y_TOP, HZ], 'y', 45, '140'),
   ([0, Y_BOT, HZ], [0, Y_TOP, HZ], 'z', 60, '340'),
   ([0, Y_STEP_LO, -HZ], [0, Y_STEP_HI, -HZ], 'z', -60, '20 step rail'),
   ([0, Y_BOT, -HZ], [0, Y_SHOULDER, -HZ], 'z', -110, '269.9'),
 ],
 'top': [
   ([-HX_UP, Y_TOP, HZ], [HX_UP, Y_TOP, HZ], 'z', 60, '226'),
   ([HX_UP, Y_TOP, -HZ], [HX_UP, Y_TOP, HZ], 'x', 60, '140'),
   ([-(HX_UP - E), Y_TOP, -HZ], [HX_UP - E, Y_TOP, -HZ], 'z', -60, '186 inner clear'),
   ([-RS03_OUT, Y_SHOULDER, 0], [RS03_OUT, Y_SHOULDER, 0], 'z', 130, '270'),
 ],
}

json.dump(dict(parts=parts, built=built, dims=dims, meta=dict(n_brackets=N_BRACKETS, fasteners=FAST)), open('parts.json', 'w'))
print('fasteners', FAST)

# ---- cut list and BOM ---------------------------------------------------------------------------
rows = sorted(EXT.values(), key=lambda d: d['item'])
total_new = sum(d['length'] * d['qty'] for d in rows if d['source'] == 'new')
total = sum(d['length'] * d['qty'] for d in rows)
with open('cutlist.md', 'w') as f:
    f.write('| Item | Member | Qty | Length, mm | Source |\n| --- | --- | --- | --- | --- |\n')
    for d in rows:
        src = {'keep': 'existing, reused as is', 'cut': 'cut from the existing 340 mm posts (two cuts; the upper post must measure 121.38)', 'new': '**new stock**'}[d['source']]
        f.write(f"| {d['item']} | {d['name'].replace(' (new)', '')} | {d['qty']} | {d['length']:.2f} | {src} |\n")
    f.write(f"\nTotal extrusion {total:.0f} mm, of which {total_new:.0f} mm is new stock. {N_BRACKETS} corner brackets.\n")
bom = {}
for p in parts:
    if p['item'] == 0: continue
    if p['id'].startswith(('rs03_pitch_out', 'rs03_boss', 'pitch_wing', 'imu_old', 'pitch_flange')) or p['id'].endswith('_s') or re.match(r'^(t\d|gt)', p['id']): continue
    b = bom.setdefault(p['item'], dict(item=p['item'], name=p['name'].split(' (')[0] if p['group'] != 'extrusion' else p['name'].replace(' (new)', ''),
                                     qty=0, mass_g=0, material=p.get('material', ''), group=p['group']))
    if p['id'].startswith(('rs03_pitch_out', 'rs03_boss', 'pitch_wing', 'imu_old', 'pitch_flange')) or p['id'].endswith('_s'): continue
    b['qty'] += 1; b['mass_g'] += p.get('mass_g', 0)
json.dump(sorted(bom.values(), key=lambda b: b['item']), open('bom.json', 'w'), indent=1)
print(len(parts), 'parts;', N_BRACKETS, 'brackets; extrusion', total, 'mm, new', total_new)
print(open('cutlist.md').read())
