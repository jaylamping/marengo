#!/usr/bin/env python3
"""Parametric model of the Marengo internal torso frame, rev A (two-box stepped 2020 frame).

Writes parts.json (geometry for the viewer), bom.json and cutlist.md. Coordinates are the torso
STEP frame in mm: X lateral (robot left +), Y up from the pelvis bottom, Z forward. Robot frame:
robot X = torso Z, robot Y = torso X, robot Z = torso Y + 810 (shell generator convention).
All numbers come from hardware/designs/torso-stepped/README.md (arm thread) and
hardware/designs/body-shell/concept/generator/mounts.json (shell thread) unless marked here.
"""
import json, math

# ---- frame parameters -------------------------------------------------------------------------
E = 20.0                 # 2020 extrusion
Y_BOT = 155.0            # bottom of the frame (pelvis 115 + 40 mm waist bay)
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
        ext(f'post_lo_{sx}_{sz}', 1, 'Post, lower box, 198.6', (sx * (HX_LO - E) if sx > 0 else -HX_LO, Y_BOT, sz * (HZ - E) if sz > 0 else -HZ),
            (HX_LO if sx > 0 else -(HX_LO - E), Y_STEP_LO, HZ if sz > 0 else -(HZ - E)), 'cut',
            (sx * 25, -60, sz * 25), 'post_lo', balloon=first); first = False
first = True
for sz in (1, -1):
    ext(f'rail_bot_{sz}', 2, 'Rail, bottom front/rear, 110', (-(HX_LO - E), Y_BOT, sz * (HZ - E) if sz > 0 else -HZ),
        (HX_LO - E, Y_BOT + E, HZ if sz > 0 else -(HZ - E)), 'keep', (0, -120, sz * 50), 'rail_bot', balloon=first); first = False
first = True
for sx in (1, -1):
    ext(f'rail_botside_{sx}', 3, 'Rail, bottom side, 100', (sx * (HX_LO - E) if sx > 0 else -HX_LO, Y_BOT, -(HZ - E)),
        (HX_LO if sx > 0 else -(HX_LO - E), Y_BOT + E, HZ - E), 'keep', (sx * 50, -120, 0), 'rail_botside', balloon=first); first = False
# ---- step tier -------------------------------------------------------------------------------
first = True
for sz in (1, -1):
    ext(f'rail_shared_{sz}', 4, 'Rail, shared step, front/rear, 226 (new)', (-HX_UP, Y_STEP_LO, sz * (HZ - E) if sz > 0 else -HZ),
        (HX_UP, Y_STEP_HI, HZ if sz > 0 else -(HZ - E)), 'new', (0, 0, sz * 60), 'rail_shared', balloon=first); first = False
first = True
for sx in (1, -1):
    ext(f'rail_stepside_{sx}', 5, 'Rail, step side, 100', (sx * (HX_UP - E) if sx > 0 else -HX_UP, Y_STEP_LO, -(HZ - E)),
        (HX_UP if sx > 0 else -(HX_UP - E), Y_STEP_HI, HZ - E), 'keep', (sx * 60, 0, 0), 'rail_stepside', balloon=first); first = False
first = True
for sx in (1, -1):
    ext(f'rail_shelf_{sx}', 6, 'Rail, shelf, 100 (new)', (sx * (HX_SHELF - E) if sx > 0 else -HX_SHELF, Y_STEP_LO, -(HZ - E)),
        (HX_SHELF if sx > 0 else -(HX_SHELF - E), Y_STEP_HI, HZ - E), 'new', (sx * 0, 0, 0), 'rail_shelf', balloon=first); first = False
# ---- upper box -------------------------------------------------------------------------------
first = True
for sx in (1, -1):
    for sz in (1, -1):
        ext(f'post_up_{sx}_{sz}', 7, 'Post, upper box, 121.4', (sx * (HX_UP - E) if sx > 0 else -HX_UP, Y_STEP_HI, sz * (HZ - E) if sz > 0 else -HZ),
            (HX_UP if sx > 0 else -(HX_UP - E), Y_TOP, HZ if sz > 0 else -(HZ - E)), 'cut', (sx * 25, 60, sz * 25), 'post_up', balloon=first); first = False
first = True
for sz in (1, -1):
    ext(f'rail_top_{sz}', 8, 'Rail, top front/rear, 186 (new)', (-(HX_UP - E), Y_TOP - E, sz * (HZ - E) if sz > 0 else -HZ),
        (HX_UP - E, Y_TOP, HZ if sz > 0 else -(HZ - E)), 'new', (0, 120, sz * 50), 'rail_top', balloon=first); first = False
first = True
for sx in (1, -1):
    ext(f'rail_topside_{sx}', 9, 'Rail, top side, 100', (sx * (HX_UP - E) if sx > 0 else -HX_UP, Y_TOP - E, -(HZ - E)),
        (HX_UP if sx > 0 else -(HX_UP - E), Y_TOP, HZ - E), 'keep', (sx * 50, 120, 0), 'rail_topside', balloon=first); first = False

# ---- gusset plates, 3 mm 6061-T6, front and rear faces of each step corner -------------------
GUSSET_PTS = [(55, 285), (75, 285), (113, 335), (113, 425), (93, 425), (93, Y_STEP_HI), (55, Y_STEP_HI)]
GUSSET_R = [8, 8, 15, 8, 8, 8, 8]   # R15 at the X 113 / Y 335 corner (297 MPa peak in the arm thread FEA)
GUSSET_HOLES = [(65, 297), (65, 317), (65, 337), (62, 363.62), (84, 363.62), (104, 363.62), (103, 385), (103, 400), (103, 415)]
first = True
for sx in (1, -1):
    for sz in (1, -1):
        pts = [(sx * x, y) for x, y in GUSSET_PTS]
        add(id=f'gusset_{sx}_{sz}', item=10, name='Gusset plate, 3 mm 6061-T6', type='poly', pts=pts, radii=GUSSET_R,
            holes=[(sx * x, y, 2.75) for x, y in GUSSET_HOLES], z0=HZ if sz > 0 else -HZ - G_T, t=G_T,
            color=COL['gusset'], group='gusset', explode=[0, 0, sz * 130], balloon=first, material='6061-T6 sheet, 3 mm',
            mass_g=10, note='4 off; the left-front plate is the drawing, mirror X for the right. 9 x 5.5 mm holes for M5 T-nuts.')
        first = False

# ---- 2028 corner brackets --------------------------------------------------------------------
BR_L, BR_W, BR_T = 28.0, 20.0, 4.0
def bracket(id, corner, nA, nB, extent_axis, extent, explode, balloon=False):
    """L bracket at corner point; legs run along +nA and +nB (unit axis vectors as (axis, sign))."""
    legs = []
    for (ax, sg), (bx, bs) in ((nA, nB), (nB, nA)):
        lo = [0, 0, 0]; hi = [0, 0, 0]
        for i in range(3):
            if i == ax: lo[i], hi[i] = (corner[i], corner[i] + BR_L) if sg > 0 else (corner[i] - BR_L, corner[i])
            elif i == bx: lo[i], hi[i] = (corner[i], corner[i] + BR_T) if bs > 0 else (corner[i] - BR_T, corner[i])
            else: lo[i], hi[i] = corner[i] - extent / 2, corner[i] + extent / 2
        legs.append((lo, hi))
    add(id=id, item=11, name='Corner bracket, 2028, with 2 x M5 T-nuts', type='multibox', boxes=legs, color=COL['bracket'],
        group='bracket', explode=list(explode), balloon=balloon, material='Al 2028 bracket + M5 T-nuts', mass_g=18)

nb = 0
def BX(sx, sz, y, dy, which, explode):
    """post corner brackets: which='x' = bracket between post X-inward face and a front/rear rail; 'z' = post Z-inward face and a side rail."""
    global nb
    hx = HX_LO if y < Y_STEP_LO else HX_UP
    cx = sx * (hx - E); cz = sz * (HZ - E)   # inner faces of the post
    if which == 'x':
        bracket(f'br{nb}', (cx, y, sz * (HZ - E / 2)), (0, -sx), (1, dy), 2, E, explode, balloon=(nb == 0))
    else:
        bracket(f'br{nb}', (sx * (hx - E / 2), y, cz), (2, -sz), (1, dy), 0, E, explode, balloon=False)
    nb += 1
for sx in (1, -1):
    for sz in (1, -1):
        BX(sx, sz, Y_BOT + E, +1, 'x', (sx * 25, -60, sz * 25)); BX(sx, sz, Y_BOT + E, +1, 'z', (sx * 25, -60, sz * 25))      # bottom corners (8)
        BX(sx, sz, Y_STEP_LO, -1, 'x', (sx * 25, -60, sz * 25))                                                                 # lower post top to shared rail (4)
        BX(sx, sz, Y_STEP_HI, +1, 'x', (sx * 25, 60, sz * 25))                                                                  # upper post bottom to shared rail (4)
        BX(sx, sz, Y_TOP - E, -1, 'x', (sx * 25, 60, sz * 25)); BX(sx, sz, Y_TOP - E, -1, 'z', (sx * 25, 60, sz * 25))          # top corners (8)
# step-tier horizontal rails to the shared rails: shelf rails (4) and step side rails (4)
for sx in (1, -1):
    for sz in (1, -1):
        for hx in (HX_SHELF, HX_UP):
            corner = (sx * (hx - E), Y_STEP_LO + E / 2, sz * (HZ - E))
            bracket(f'br{nb}', corner, (0, -sx), (2, -sz), 1, E, (sx * 60 if hx == HX_UP else 0, 0, 0)); nb += 1
N_BRACKETS = nb

# ---- shoulder pitch actuators and brackets -----------------------------------------------------
for sx in (1, -1):
    cyl(f'rs03_pitch_{sx}', 12, 'RS03, shoulder pitch (existing)', 'x', (sx * (RS03_REAR + RS03_L / 2), Y_SHOULDER, 0), RS03_D / 2, RS03_L,
        COL['motor'], 'actuator', (sx * 170, 60, 0), balloon=(sx > 0), mass_g=880, material='RobStride RS03')
    cyl(f'rs03_pitch_out_{sx}', 12, 'RS03 output flange', 'x', (sx * (RS03_FACE + (RS03_OUT - RS03_FACE) / 2), Y_SHOULDER, 0), 40, RS03_OUT - RS03_FACE,
        '#9a9ea4', 'actuator', (sx * 170, 60, 0))
    box(f'pitch_bracket_{sx}', 13, 'Shoulder pitch bracket, PETG (existing, moved out 38 mm)',
        (min(sx * RS03_REAR, sx * (HX_UP - E)), Y_SHOULDER - 49, -HZ + E), (max(sx * RS03_REAR, sx * (HX_UP - E)), Y_SHOULDER + 49, HZ - E),
        COL['printed'], 'actuator', (sx * 100, 60, 0), balloon=(sx > 0), mass_g=120, material='PETG print',
        note='Captures the RS03 against the inner X faces of the upper posts; shown as its envelope, not the real part')
    cyl(f'collar_{sx}', 25, 'Shell collar half-pair on the 8 outboard M4 holes (shell part)', 'x', (sx * (RS03_OUT + 1), Y_SHOULDER, 0), 60, 2,
        COL['collar'], 'shell', (sx * 240, 60, 0), balloon=(sx > 0), mass_g=15, material='PETG print', opacity=0.9)

# ---- compute shelf, Pi, PDB, IMU -------------------------------------------------------------
box('shelf', 14, 'Compute shelf plate (existing, 110 mm span on the shelf rails)', (-65, Y_STEP_HI, -50), (65, Y_STEP_HI + 3, 50), COL['shelf'], 'electronics',
    (0, 45, 0), balloon=True, mass_g=60, material='existing part', note='Assumption: 130 x 100 x 3 plate bolted to the X +-65 shelf rails. Confirm on the robot.')
box('pi5', 15, 'Raspberry Pi 5 + 2-CH CAN HAT (existing)', (-37.5, Y_STEP_HI + 2.4, -71.8), (19.7, 397, 13.2), COL['pi'], 'electronics',
    (0, 100, 0), balloon=True, mass_g=110, material='electronics', note='Position from the arm thread CAD check; ports face the rear (-Z) and reach Z -71.8')
box('pdb', 16, 'PDB + E-stop/main switch envelope (proposed: lower box, behind the D2 power door)', (-50, 200, -48), (50, 290, -24), COL['pdb'], 'electronics',
    (0, -60, -110), balloon=True, mass_g=150, material='placeholder 100 x 90 x 28', opacity=0.6,
    note='Not designed yet (KiCad stub). Envelope reserved on standoffs to the rear lower posts, faces the D2 door (|X| < 58, Y 167-295).')
# harness trunks: Pi/PDB to each shoulder RS03 and down to the waist
for sx in (1, -1):
    cyl(f'harness_shoulder_{sx}', 27, 'Harness trunk to the shoulder RS03 connectors (XT30 + GH1.25), 24 V and CAN', 'x', (sx * 55, 460, 30), 5, 70, '#d8b84a', 'harness', (sx * 120, 60, 0), balloon=(sx > 0), mass_g=40, material='silicone 14 AWG + CAN pair', opacity=0.95)
    cyl(f'harness_down_{sx}', 27, 'Harness trunk down the inner front corner to the waist and legs', 'y', (sx * 45, 275, 40), 5, 210, '#d8b84a', 'harness', (sx * 0, -60, -110), mass_g=60, material='silicone 12-14 AWG + CAN pair', opacity=0.95)
cyl('harness_bus', 27, 'Harness bus along the step rail, Pi to PDB', 'x', (0, 345, -40), 5, 110, '#d8b84a', 'harness', (0, 0, -110), mass_g=20, material='', opacity=0.95)
box('imu_old', 15, 'BNO085 IMU (existing)', (30, Y_STEP_HI + 3, -10), (50, Y_STEP_HI + 9, 10), '#8fd36b', 'electronics', (0, 100, 0), mass_g=5, material='electronics')

# ---- waist interface ---------------------------------------------------------------------------
box('waist_adapter', 18, 'Waist yaw adapter plate, PETG (existing; bolts to the bottom rectangle, pattern unverified)', (-HX_LO, Y_BOT - 6, -HZ), (HX_LO, Y_BOT, HZ),
    COL['adapter'], 'waist', (0, -170, 0), balloon=True, mass_g=120, material='PETG print', opacity=0.9,
    note='Placeholder 150 x 140 x 6 plate under the bottom rectangle. Joey to confirm how it bolts on.')
cyl('rs03_waist', 17, 'RS03, waist yaw (existing; sits in the 40 mm bay, 57 mm tall: open item)', 'y', (0, Y_BOT - 6 - 28.5, 0), RS03_D / 2, 57,
    COL['motor'], 'waist', (0, -240, 0), balloon=True, mass_g=880, material='RobStride RS03')

# ---- shell hardpoints (torso coords; from mounts.json: torso X = robot Y, torso Y = robot Z - 810, torso Z = robot X) ----
HP = {
 'H1': dict(item=19, face='+Z', pts=[(60, 485, 70), (-60, 485, 70)], carries='U1 upper front, top edge'),
 'H2': dict(item=20, face='-Z', pts=[(60, 485, -70), (-60, 485, -70)], carries='U2 upper back top edge, D3 catch, spine pack'),
 'H3': dict(item=21, face='+-Z', pts=[(35, 364, 70), (-35, 364, 70), (35, 364, -70), (-35, 364, -70)], carries='U1/U2 bottom, U3/U4 top, D1 frame, spine pack'),
 'H4': dict(item=22, face='+-Z', pts=[(40, 165, 70), (-40, 165, 70), (40, 165, -70), (-40, 165, -70)], carries='U3/U4 bottom, D2 frame, spine pack'),
 'H5': dict(item=23, face='+-X', pts=[(75, 255, 60), (75, 255, -60), (-75, 255, 60), (-75, 255, -60)], carries='U3/U4 sides, V1 grilles'),
 'H6': dict(item=24, face='+-X', pts=[(75, 165, 35), (75, 165, -35), (-75, 165, 35), (-75, 165, -35)], carries='abdomen band carrier (PROPOSED: outer side faces of the bottom side rails, was the under-face)'),
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
        tier = 60 if y > Y_STEP_HI else (-60 if y < Y_STEP_LO else 0)
        box(f'{hid}_{i}', h['item'], f'Shell hardpoint bracket {hid}, PETG with M3 inserts, on M5 T-nut', lo, hi, COL['hard'], 'shell',
            (n[0] * 90, tier, n[2] * 90), balloon=(i == 0), mass_g=8, material='PETG print', carries=h['carries'])

# ---- packaging envelopes for Joey's component list (2026-11-06): proposed positions, not designed parts ----
PRO = '#26b3c9'
box('jetson', 29, 'Jetson Orin Nano dev kit envelope 103 x 90.5 x 35 (+10 fan), second shelf above the Pi, ports to the rear', (-51.5, 410, -45.5), (51.5, 445, 45), PRO, 'proposed', (0, 150, 0), balloon=True, mass_g=250, material='proposed', opacity=0.75,
    note='Fits the upper bay between the RS03 rears (156.8 wide); 30 mm left above it under the top rails for a fan.')
box('fan_exhaust_1', 30, 'Exhaust fans 40 x 40 x 10 (2), upper rear under the top rail, blowing out through the D1 door louvres', (-55, 448, -50), (-15, 488, -40), PRO, 'proposed', (0, 150, -60), balloon=True, mass_g=20, material='proposed', opacity=0.75)
box('fan_exhaust_2', 30, 'Exhaust fan', (15, 448, -50), (55, 488, -40), PRO, 'proposed', (0, 150, -60), mass_g=20, material='proposed', opacity=0.75)
box('fan_intake', 30, 'Intake fan 40 x 40 x 10, lower front panel grille (Y 250-295)', (-20, 250, 40), (20, 295, 50), PRO, 'proposed', (0, -60, 90), mass_g=20, material='proposed', opacity=0.75)
box('compute3', 31, 'Third compute / USB hub board envelope 90 x 60 x 20, lower box front, standing', (-45, 190, 18), (45, 280, 38), PRO, 'proposed', (0, -60, 90), balloon=True, mass_g=60, material='proposed', opacity=0.75)
for sx in (1, -1):
    box(f'battery_{sx}', 32, 'OPTION: in-torso battery, 2 x 6S 5000 mAh (155 x 50 x 45 each) standing in the lower box. Baseline keeps the main 48 V pack in the pelvis.',
        (min(sx * 2, sx * 52), 175, -20), (max(sx * 2, sx * 52), 330, 25), '#e0c23a', 'option', (0, -60, 0), balloon=(sx > 0), mass_g=750, material='option', opacity=0.6)
box('bno085', 33, 'Sensors: BNO085 IMU on the shelf (fitted); head cameras and mics are future, on the top rails', (-50, Y_STEP_HI + 3, 20), (-30, Y_STEP_HI + 9, 40), PRO, 'proposed', (0, 100, 0), balloon=True, mass_g=5, material='proposed', opacity=0.75)
# ---- spine pack (rear crush pads, outside the shell; interim spec from the shell manufacturing thread 2026-11-06) ----
first = True
for sx in (1, -1):
    for (x0, x1, y0, y1, nm) in ((62, 135, 370, 480, 'upper'), (58, 110, 170, 295, 'lower')):
        box(f'spine_{nm}_{sx}', 26, 'TPU spine pads, 30 mm rear crush, on standoffs to H2/H3/H4 (shell part, outside the shell)', (min(sx * x0, sx * x1), y0, -125), (max(sx * x0, sx * x1), y1, -95),
            COL['tpu'], 'shell', (0, 0, -160), balloon=first, mass_g=90, material='TPU 95A gyroid', opacity=0.7); first = False
box('spine_bridge', 26, 'TPU spine bridge between the doors', (-60, 300, -125), (60, 355, -95), COL['tpu'], 'shell', (0, 0, -160), mass_g=50, material='TPU 95A gyroid', opacity=0.7)
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

json.dump(dict(parts=parts, built=built, dims=dims, meta=dict(n_brackets=N_BRACKETS)), open('parts.json', 'w'))

# ---- cut list and BOM ---------------------------------------------------------------------------
rows = sorted(EXT.values(), key=lambda d: d['item'])
total_new = sum(d['length'] * d['qty'] for d in rows if d['source'] == 'new')
total = sum(d['length'] * d['qty'] for d in rows)
with open('cutlist.md', 'w') as f:
    f.write('| Item | Member | Qty | Length, mm | Source |\n| --- | --- | --- | --- | --- |\n')
    for d in rows:
        src = {'keep': 'existing, reused as is', 'cut': 'cut from the existing 340 mm posts', 'new': '**new stock**'}[d['source']]
        f.write(f"| {d['item']} | {d['name'].replace(' (new)', '')} | {d['qty']} | {d['length']:.2f} | {src} |\n")
    f.write(f"\nTotal extrusion {total:.0f} mm, of which {total_new:.0f} mm is new stock. {N_BRACKETS} corner brackets.\n")
bom = {}
for p in parts:
    if p['item'] == 0: continue
    b = bom.setdefault(p['item'], dict(item=p['item'], name=p['name'].split(' (')[0] if p['group'] != 'extrusion' else p['name'].replace(' (new)', ''),
                                     qty=0, mass_g=0, material=p.get('material', ''), group=p['group']))
    if p['id'].startswith('rs03_pitch_out'): continue
    b['qty'] += 1; b['mass_g'] += p.get('mass_g', 0)
json.dump(sorted(bom.values(), key=lambda b: b['item']), open('bom.json', 'w'), indent=1)
print(len(parts), 'parts;', N_BRACKETS, 'brackets; extrusion', total, 'mm, new', total_new)
print(open('cutlist.md').read())
