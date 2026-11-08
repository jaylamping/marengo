"""Upper arm clamshell parts from the master (db71b23b with the cover window closed). Arm frame, mm.
Parts: upper half (Z > 6), lower half (Z < 6), RS02 seat ring, hub disc. usage: python ua_parts.py [ring hub upper lower]"""
import math, os, shutil, sys
import numpy as np
from swg import *

sw = sw_app()
sw.CloseAllDocuments(True)
DIR = r'J:\code\marengo\cad\parts\left_arm'
MASTER = os.path.join(DIR, 'marengo_upper_arm_clam_master_left.SLDPRT')
EXP = r'C:\Users\joeyl\Documents\Codex\2026-10-29\task\LEFT_ARM_V1_LAYOUT_20261106\pkg\stl\ua'
CY, CZ = 209.860668, 5.985875          # elbow axis, along X
TX, TZ = 170.5, 6.0                    # tube axis, along Y
Z0 = 6.0                               # seam plane
R_RING, CL = 44.25, 0.1                # ring cut radius (just outside the master hoop OD, so the ring keeps that OD), capture clearance
HUB_Y = 302.0                          # hub disc / halves joint
SPIG_Y0, SPIG_R = 296.0, 35.8          # hub spigot, inside the halves' sleeve
LUG = dict(x0=151.8, x1=165.2, r0=39.0, r1=56.0, hz=5.0)        # root runs into the ring body, below the port-slot floor      # ring lug on +Y, split by the seam
HUB_LUG = dict(y0=289.0, y1=SPIG_Y0, hz=5.0, w=8.0)                     # hub lugs at both seam sides, inside the tube
HUB_HOLES = [(159.4, 10.5), (161.0, -1.4), (172.1, -5.9), (181.6, 1.5), (180.0, 13.4), (168.8, 17.9)]


def open_copy(name):
    p = os.path.join(DIR, name)
    shutil.copy(MASTER, p)
    e, w = L(), L()
    d = sw.OpenDoc6(p, 1, 1, '', e, w)
    if d is None:
        raise SystemExit('open ' + p)
    for b in bodies(d):
        if b[0] == 'R4_SPRING_SEAT':           # the carrier is its own printed part
            delete_body(d, b[0])
    return d, p


def finish(d, p, stl):
    d.ForceRebuild3(False)
    print('  bodies', bodies(d), flush=True)
    e, w = L(), L()
    d.Save3(1, e, w)
    os.makedirs(EXP, exist_ok=True)
    d.Extension.SaveAs(os.path.join(EXP, stl + '.STL'), 0, 1, NUL, e, w)
    d.Extension.SaveAs(os.path.join(EXP, stl + '.STEP'), 0, 1, NUL, e, w)
    sw.CloseDoc(d.GetTitle)


def elbow_revolve(d, name, poly_xr, cut):
    """Revolve a polygon given as (X, r) about the elbow axis."""
    pl = plane(d, 'Z', CZ, name + '_P')
    with Sk(d, pl, name) as s:
        s.poly([(x, CY + r, CZ) for x, r in poly_xr])
        centerline(s, (90, CY, CZ), (260, CY, CZ))
    return revolve(d, s.feat, name, None, cut)


def tube_revolve(d, name, poly_ry, cut):
    """Revolve a polygon given as (r, Y) about the tube axis."""
    pl = plane(d, 'Z', TZ, name + '_P')
    with Sk(d, pl, name) as s:
        s.poly([(TX + r, y, TZ) for r, y in poly_ry])
        centerline(s, (TX, 150, TZ), (TX, 340, TZ))
    return revolve(d, s.feat, name, None, cut)


def box_cut(d, name, axis, at, sign, rect):
    """Cut everything on the 'sign' side of a plane (rect = 4 model points on the plane). Picks the direction whose
    remaining bodies all lie on the other side."""
    pl = plane(d, axis, at, name + '_P')
    with Sk(d, pl, name) as s:
        s.poly(rect)
    ax = 'XYZ'.index(axis)
    for rev in (False, True):
        v0 = vol(d)
        d.ClearSelection2(True); s.feat.Select2(False, 0)
        f = d.FeatureManager.FeatureCut4(True, False, rev, 0, 0, 0.4, 0.4, False, False, False, False, 0, 0, False, False, False, False, False, False, True, False, False, False, 0, 0, False, False)
        if f is None:
            continue
        d.ForceRebuild3(False)
        bb = [b.GetBodyBox() for b in d.GetBodies2(0, True)]
        ok = bb and all((x[ax + 3] * 1e3 <= at + 0.5) if sign > 0 else (x[ax] * 1e3 >= at - 0.5) for x in bb)
        print('   try', rev, [[round(v * 1e3, 1) for v in x] for x in bb], flush=True)
        if ok and v0 - vol(d) > 1:
            f.Name = name; print(f'  {name}: cut {v0 - vol(d):.1f} mm3', flush=True); return f
        d.EditUndo2(1)
    raise SystemExit(name + ' failed')


LUGY0, LUGY1 = CY + LUG['r0'], CY + 56.0
Y_FLARE = 242.0                 # ring / halves boundary plane under the tube flare
RING_PIN_Y, RING_PIN_Z = 246.5, (29.6, -17.6)     # ring pin screws: ring OD surface along Z at this Y
RING_PIN_X = (158.5,)           # one pin from the back (lower half): no holes in the front skin; two side by side hit the ring end faces
SLOT_Z = (-9.5, 18.0)            # ring top slot (trunk port to the elbow RS02), widened from the master's -7.5..16.3
Y_SKIRT = 240.0                 # RS02-side skirt trimmed above its down-facing tip (no support on visible skin)
Y_TIP = 236.5                   # below this the flare skins feather out tangent to the ring OD: trimmed
RING_COL = (158.5, CY + 50.0)
# seam posts 23 mm off the tube axis: a vertical post nearer the wall pokes out through the curved outer skin.
# Each lower post ends 0.25 mm into the inner skin at its centre, still inside the outer skin at its outboard edge.
TUBE_COLS = [(147.5, 262.0), (147.5, 278.0), (193.5, 262.0), (193.5, 278.0)]
COL_BOT = [-15.39, -18.85, -15.37, -18.83]
COL_TOP = [27.41, 30.8, 27.39, 30.78]          # upper posts end 0.25 mm into the upper inner skin
POST_R = 3.75
HUB_COLS = [(147.5, 292.0), (193.5, 292.0)]
LUG_BOT = [-20.67, -20.66]
LUG_TOP = [32.67, 32.66]
HUB_LUGS = [(141.0, 151.75), (189.25, 200.0)]     # reach r 29.9 to root in the spigot (r 28-35.8)
HUB_LUG_Y = (288.0, 300.0)


def rect_z(d, name, x0, x1, y0, y1, z, h, cut):
    """Box from plane Z = z, mid-plane thickness h in Z."""
    pl = plane(d, 'Z', z, name + '_P')
    with Sk(d, pl, name) as s:
        s.poly([(x0, y0, z), (x1, y0, z), (x1, y1, z), (x0, y1, z)])
    return feat(d, s.feat, name, h, cut, mode=MID)


def cyl_z(d, name, pts, r, z, depth, cut, sign):
    """Cylinders from Z = z, depth toward sign. Bosses are mid-plane extrusions from the middle (blind extrusions fail here)."""
    zc = z if cut else z + sign * depth / 2
    pl = plane(d, 'Z', zc, name + '_P')
    with Sk(d, pl, name) as s:
        for x, y in pts:
            if cut:
                s.circle((x, y, zc), r)
            else:
                s.poly([(x + r * math.cos(a), y + r * math.sin(a), zc) for a in np.linspace(0, 2 * math.pi, 25)[:-1]])
    if cut:
        return feat(d, s.feat, name, depth, cut, want=(2, sign, z))
    return feat(d, s.feat, name, depth, cut, mode=MID)


def bosses_z(d, name, pts, r, z, depth, sign):
    """One mid-plane boss per point (a failed boolean then names its point)."""
    for i, q in enumerate(pts):
        cyl_z(d, f'{name}{i}', [q], r, z, depth, False, sign)


def drop_slivers(d):
    for b in bodies(d):
        if b[1] < 0.2:
            delete_body(d, b[0])


def ring():
    d, p = open_copy('marengo_upper_arm_ring_left.SLDPRT')
    x0, x1 = LUG['x0'] - 1, LUG['x1'] + 1
    elbow_revolve(d, 'KEEP_RING_IN', [(100, 0), (x0, 0), (x0, 200), (100, 200)], cut=True)
    elbow_revolve(d, 'KEEP_RING_OUT', [(x1, 0), (260, 0), (260, 200), (x1, 200)], cut=True)
    # above Y_FLARE the ring is r < 43.6; below it the ring keeps the skin flare too, so the halves end on a plane
    # instead of feathering out tangent to the ring OD (mesh gate)
    xm = (x0 + x1) / 2
    pl = plane(d, 'X', xm, 'KEEP_RING_TOP_P')
    hb = math.asin(math.sqrt(R_RING ** 2 - (Y_FLARE - CY) ** 2) / R_RING)
    arc = [(xm, CY + R_RING * math.cos(b), CZ + R_RING * math.sin(b)) for b in np.linspace(-hb, hb, 60)]
    with Sk(d, pl, 'KEEP_RING_TOP') as s:
        s.poly([(xm, Y_FLARE, -120)] + arc + [(xm, Y_FLARE, 130), (xm, 400, 130), (xm, 400, -120)])
    feat(d, s.feat, 'KEEP_RING_TOP', x1 - x0 + 4, True, mode=MID)
    hb = math.asin(math.sqrt(R_RING ** 2 - (Y_TIP - CY) ** 2) / R_RING)
    arc = [(xm, CY + R_RING * math.cos(b), CZ + R_RING * math.sin(b)) for b in np.linspace(hb, 2 * math.pi - hb, 120)]
    with Sk(d, pl, 'KEEP_RING_TIP') as s:            # flare tips below Y_TIP feather out tangent to the OD
        s.poly([(xm, Y_TIP, 130)] + arc + [(xm, Y_TIP, -120), (xm, 100, -120), (xm, 100, 130)])
    feat(d, s.feat, 'KEEP_RING_TIP', x1 - x0 + 4, True, mode=MID)
    drop_slivers(d)
    # RS02 plug-housing pocket run through to the seat face over 161-191 deg (db71b23b left a 1.7 mm lip Joey had to file)
    pl = plane(d, 'X', 151.0, 'POCKET_P')
    with Sk(d, pl, 'POCKET') as s:
        A = [math.radians(a) for a in range(161, 192, 2)]
        s.poly([(151.0, CY + 32.5 * math.cos(a), CZ + 32.5 * math.sin(a)) for a in A] + [(151.0, CY + 36.9 * math.cos(a), CZ + 36.9 * math.sin(a)) for a in A[::-1]])
    feat(d, s.feat, 'POCKET_THRU', 15.0, True, want=(0, 1, 151.0))
    # no lug: the top of the ring is the master's port slot. Each half pins the ring with one M3 x 10 along Z into a
    # Ruthex insert at Y RING_PIN_Y, outside the slot (Z -7.5..16)
    cyl_z(d, 'RING_INSERT_LO', [(x, RING_PIN_Y) for x in RING_PIN_X], 2.0, -22.5, (RING_PIN_Z[1] + 6.0) + 22.5, True, 1)   # start in air
    # the master's port slot leaves 0.03-0.5 mm skins on the ring top beside the lug: clear them down to Y 250,
    # out to where the OD is below Y 250 on both sides, so the cut ends in air
    # widen the master's port slot (Z -7.5..16.3) to clean Z planes: its walls met the r 43.6 cut at grazing angles
    rect_z(d, 'PORT_SLOT', 148.0, 169.0, 238.0, 262.0, (SLOT_Z[0] + SLOT_Z[1]) / 2, SLOT_Z[1] - SLOT_Z[0], cut=True)
    finish(d, p, 'ua_ring')


def hub():
    d, p = open_copy('marengo_upper_arm_hub_left.SLDPRT')
    box_cut(d, 'BELOW_HUB', 'Y', SPIG_Y0, -1, [(100, SPIG_Y0, -80), (240, SPIG_Y0, -80), (240, SPIG_Y0, 90), (100, SPIG_Y0, 90)])
    drop_slivers(d)
    tube_revolve(d, 'SLEEVE_ZONE', [(SPIG_R, SPIG_Y0 - 1), (50, SPIG_Y0 - 1), (50, HUB_Y), (SPIG_R, HUB_Y)], cut=True)
    tube_revolve(d, 'SPIGOT', [(28.0, SPIG_Y0), (SPIG_R, SPIG_Y0), (SPIG_R, HUB_Y + 0.5), (28.0, HUB_Y + 0.5)], cut=False)
    # lighten the 17 mm solid disc: keep a 4 mm plate at the RS02 face, the spigot band and Ø10 bosses round the M4 holes
    pl = plane(d, 'Y', 315.86, 'LIGHTEN_P')
    with Sk(d, pl, 'LIGHTEN') as s:
        s.circle((TX, 315.86, TZ), 33.0)
        for x, z in HUB_HOLES:
            s.circle((x, 315.86, z), 5.0)
    feat(d, s.feat, 'LIGHTEN', 13.36, True, want=(1, -1, 315.86))   # ends on the spigot top, Y 302.5
    # six windows through the plate between the bosses (bolts at r 12, every 60 deg from 37.9): r 19-29, +-20 deg
    with Sk(d, pl, 'WINDOWS') as s:
        for k in range(6):
            c = math.radians(7.9 + 60 * k)
            A = [c + math.radians(a) for a in range(-20, 21, 5)]
            s.poly([(TX + 19 * math.cos(a), 315.86, TZ + 19 * math.sin(a)) for a in A] + [(TX + 29 * math.cos(a), 315.86, TZ + 29 * math.sin(a)) for a in A[::-1]])
    feat(d, s.feat, 'WINDOWS', 8.0, True, want=(1, 1, 315.86))
    # the old tube floor (Y 298-303) inside the spigot carries nothing now; keep only the bolt bosses through it
    pl = plane(d, 'Y', 297.0, 'FLOOR_P')
    with Sk(d, pl, 'FLOOR') as s:
        s.circle((TX, 297.0, TZ), 27.6)
        for x, z in HUB_HOLES:
            s.circle((x, 297.0, z), 4.4)     # inside the master's r 16.5 floor step, so no cusps
    feat(d, s.feat, 'FLOOR_OPEN', 6.4, True, want=(1, 1, 297.0))
    for i, (x0, x1) in enumerate(HUB_LUGS):
        rect_z(d, f'HUB_LUG{i}', x0, x1, HUB_LUG_Y[0], HUB_LUG_Y[1], Z0, 10.0, cut=False)
    cyl_z(d, 'HUB_LUG_HOLES', HUB_COLS, 1.7, Z0 + 6, 12, True, -1)
    finish(d, p, 'ua_hub')


TRUNK_IN = (171.0, 289.0)      # trunk entry through the back wall (Z -25.8..-31.8), below the hub spigot seat
LOOP_OUT = (270.0, -16.3)      # RS00 branch out through the back-inboard wall (Y, Z), along X, toward the loop clip


def tie_boss(d, name, x0, x1, y0, y1, z_wall, h):
    """Cable-tie anchor on the inner back wall: a block from inside the wall up h, with a 4.5 x 2 tunnel along X."""
    zc = z_wall + (h - 1.0) / 2
    rect_z(d, name, x0, x1, y0, y1, zc, h + 1.0, cut=False)
    ym = (y0 + y1) / 2
    pl = plane(d, 'X', (x0 + x1) / 2, name + '_TIE_P')
    zt = z_wall + h - 3.5
    with Sk(d, pl, name + '_TIE') as s:
        s.poly([((x0 + x1) / 2, ym - 2.25, zt), ((x0 + x1) / 2, ym + 2.25, zt), ((x0 + x1) / 2, ym + 2.25, zt + 2.0), ((x0 + x1) / 2, ym - 2.25, zt + 2.0)])
    feat(d, s.feat, name + '_TIE', x1 - x0 + 2.0, True, mode=MID)


def harness_lower(d):
    """Trunk entry and strain relief, RS00 branch exit and tie point (lower half)."""
    cyl_z(d, 'TRUNK_ENTRY', [TRUNK_IN], 5.0, -20.0, 20.0, True, -1)
    tie_boss(d, 'TRUNK_RELIEF', 164.0, 178.0, 274.0, 280.0, -25.0, 6.0)
    pl = plane(d, 'X', 140.0, 'LOOP_EXIT_P')
    with Sk(d, pl, 'LOOP_EXIT') as s:
        s.circle((140.0, LOOP_OUT[0], LOOP_OUT[1]), 3.4)
    feat(d, s.feat, 'LOOP_EXIT', 11.0, True, want=(0, 1, 140.0))
    tie_boss(d, 'LOOP_RELIEF', 153.0, 162.0, 276.0, 282.0, -21.9, 5.5)


def half(upper):
    nm = 'upper' if upper else 'lower'
    d, p = open_copy(f'marengo_upper_arm_{nm}_left.SLDPRT')
    elbow_revolve(d, 'RING_ZONE', [(LUG['x0'] - 1.15, 0), (LUG['x1'] + 1.15, 0), (LUG['x1'] + 1.15, R_RING + CL), (LUG['x0'] - 1.15, R_RING + CL)], cut=True)
    xa, xb = LUG['x0'] - 1.15, LUG['x1'] + 1.15
    rect_x = plane(d, 'X', (xa + xb) / 2, 'RING_FLARE_P')
    with Sk(d, rect_x, 'RING_FLARE') as s:
        s.poly([((xa + xb) / 2, y, z) for y, z in ((100, -80), (Y_FLARE + CL, -80), (Y_FLARE + CL, 90), (100, 90))])
    feat(d, s.feat, 'RING_FLARE', xb - xa, True, mode=MID)
    rect_x = plane(d, 'X', (xb + 214.0) / 2, 'SKIRT_TIP_P')
    with Sk(d, rect_x, 'SKIRT_TIP') as s:        # skirt ends feather out tangent to the RS02 outboard of the ring
        s.poly([((xb + 214.0) / 2, y, z) for y, z in ((100, -80), (Y_SKIRT, -80), (Y_SKIRT, 90), (100, 90))])
    feat(d, s.feat, 'SKIRT_TIP', 214.0 - xb, True, mode=MID)
    box_cut(d, 'ABOVE_HUB', 'Y', HUB_Y, 1, [(100, HUB_Y, -80), (240, HUB_Y, -80), (240, HUB_Y, 90), (100, HUB_Y, 90)])
    tube_revolve(d, 'SPIGOT_SEAT', [(5, SPIG_Y0 - CL), (SPIG_R + CL, SPIG_Y0 - CL), (SPIG_R + CL, HUB_Y + 1), (5, HUB_Y + 1)], cut=True)
    for i, (x0, x1) in enumerate(HUB_LUGS):
        try:                                     # the lugs sit in the cavity; cut only if they reach half material
            rect_z(d, f'HUB_LUG_RECESS{i}', x0 - CL, x1 + CL, HUB_LUG_Y[0] - CL, HUB_LUG_Y[1], Z0, 10.0 + 2 * CL, cut=True)
        except SystemExit:
            d.EditUndo2(1)
    if upper:
        box_cut(d, 'LOWER_SIDE', 'Z', Z0, -1, [(100, 150, Z0), (240, 150, Z0), (240, 340, Z0), (100, 340, Z0)])
    else:
        box_cut(d, 'UPPER_SIDE', 'Z', Z0, 1, [(100, 150, Z0), (240, 150, Z0), (240, 340, Z0), (100, 340, Z0)])
    print('  bodies', bodies(d), flush=True)
    if upper:
        for i, (q, zt) in enumerate(zip(TUBE_COLS, COL_TOP)):
            cyl_z(d, f'SEAM_BOSS{i}', [q], POST_R, Z0, zt - Z0, False, 1)
        for i, (q, zt) in enumerate(zip(HUB_COLS, LUG_TOP)):
            cyl_z(d, f'LUG_BOSS{i}', [q], POST_R, Z0 + LUG['hz'] + CL, zt - (Z0 + LUG['hz'] + CL), False, 1)
        cyl_z(d, 'SEAM_INSERTS', TUBE_COLS, 2.0, Z0 - 0.01, 7.0, True, 1)
        cyl_z(d, 'SEAM_PILOT', TUBE_COLS, 1.6, Z0 + 6.9, 3.6, True, 1)          # M3 x 30 tips past the insert
        cyl_z(d, 'LUG_INSERTS', HUB_COLS, 2.0, Z0 + LUG['hz'] + CL - 0.01, 7.0, True, 1)
        cyl_z(d, 'LUG_PILOT', HUB_COLS, 1.6, Z0 + LUG['hz'] + CL + 6.9, 2.6, True, 1)

        cyl_z(d, 'SEAM_PIN_SOCKET', TUBE_COLS, 2.95, Z0 - 0.01, 1.4, True, 1)
    else:
        for i, (q, zb) in enumerate(zip(TUBE_COLS, COL_BOT)):
            cyl_z(d, f'SEAM_COLUMN{i}', [q], POST_R, Z0, Z0 - zb, False, -1)
        for i, (q, zb) in enumerate(zip(HUB_COLS, LUG_BOT)):
            cyl_z(d, f'LUG_COLUMN{i}', [q], POST_R, Z0 - LUG['hz'] - CL, Z0 - LUG['hz'] - CL - zb, False, -1)
        bosses_z(d, 'SEAM_PIN', TUBE_COLS, 2.75, Z0, 1.2, 1)
        cyl_z(d, 'SCREW_HOLES', TUBE_COLS + HUB_COLS, 1.7, Z0 + 2, 60.0, True, -1)
        cyl_z(d, 'RING_SCREW', [(x, RING_PIN_Y) for x in RING_PIN_X], 1.7, -14.5, 18.0, True, -1)
        cyl_z(d, 'RING_SCREW_CB', [(x, RING_PIN_Y) for x in RING_PIN_X], 3.25, -24.5, 10.0, True, -1)
        for i, (q, zb) in enumerate(zip(TUBE_COLS + HUB_COLS, COL_BOT + LUG_BOT)):
            cyl_z(d, f'CBORE{i}', [q], 3.25, zb, 20.0, True, -1)               # head bears on the post end
        harness_lower(d)
    drop_slivers(d)
    finish(d, p, 'ua_' + nm)


# ---- back-loop anchor (elbow service loop, 8 mm loom). Loop plane X = LOOP_X, centre r 48.
LOOP_X, LOOP_R, LOOM_R = 132.0, 48.0, 4.2


def anchor_frame(phi_deg, sense, alpha_deg=15.0):   # 15 deg outward: free loom stays R>=25 over 0-124 (pkg/loop_sweep.py)
    """Anchor centre C and in-plane unit vectors (Y, Z): u along the groove toward the free loop, w out of the floor."""
    f, a = math.radians(phi_deg), math.radians(alpha_deg)
    C = np.array([CY + LOOP_R * math.cos(f), CZ - LOOP_R * math.sin(f)])
    t = sense * np.array([-math.sin(f), -math.cos(f)]); n = np.array([math.cos(f), -math.sin(f)])
    u = math.cos(a) * t + math.sin(a) * n
    w = -math.sin(a) * t + math.cos(a) * n
    if w @ n < 0:
        w = -w
    return C, u, w


def floor_curve(flat=4.0, R=25.0, sweep=45.0, n=12):
    """Groove floor in (s, w) about the loom centre: flat +-flat, then R25 arcs falling away at both ends."""
    pts = []
    for sg in (-1, 1):
        side = [(sg * (flat + R * math.sin(math.radians(k))), -LOOM_R - R * (1 - math.cos(math.radians(k)))) for k in np.linspace(sweep, 0, n)]
        pts += side if sg < 0 else side[::-1]
    return pts


def loop_anchor(d, name, phi_deg, sense, web_to_x=None, web_r=40.0, xp=LOOP_X):
    """Tie-down saddle for the loom: block, U-groove with R25 floor run-outs, cable-tie tunnel under the floor (X)."""
    C, u, w = anchor_frame(phi_deg, sense)
    P = lambda s_, w_: (xp, *(C + s_ * u + w_ * w))
    pl = plane(d, 'X', xp, name + '_P')
    blk = [(-19, -12), (19, -12), (16, -4), (10, 2), (-10, 2), (-16, -4)]   # long R25 run-outs, chamfered ends: loom >= 2 mm off the block (loop_sweep)
    with Sk(d, pl, name + '_BLOCK') as s:
        s.poly([P(*q) for q in blk])
    feat(d, s.feat, name + '_BLOCK', 2 * LOOM_R + 5.0, False, mode=MID)
    if web_to_x is not None:          # web from the block back to the part it hangs on
        f0 = math.radians(phi_deg)
        foot = [(xp, CY + web_r * math.cos(f0 + k), CZ - web_r * math.sin(f0 + k)) for k in (math.radians(7), math.radians(-7))]
        from shapely.geometry import MultiPoint
        q2 = [P(-9, -12), P(9, -12), P(9, -10.2), P(-9, -10.2)] + foot
        hull = list(MultiPoint([(q[1], q[2]) for q in q2]).convex_hull.exterior.coords)[:-1]
        xm = (xp + web_to_x) / 2
        plw = plane(d, 'X', xm, name + '_WEB_P')
        with Sk(d, plw, name + '_WEB') as s:
            s.poly([(xm, y, z) for y, z in hull])
        feat(d, s.feat, name + '_WEB', web_to_x - xp, False, mode=MID)
    fc = floor_curve()
    with Sk(d, pl, name + '_GROOVE') as s:
        s.poly([P(*q) for q in fc] + [P(fc[-1][0], 12), P(fc[0][0], 12)])
    feat(d, s.feat, name + '_GROOVE', 2 * LOOM_R, True, mode=MID)
    with Sk(d, pl, name + '_TIE') as s:
        s.poly([P(-2.25, -LOOM_R - 3.7), P(2.25, -LOOM_R - 3.7), P(2.25, -LOOM_R - 1.7), P(-2.25, -LOOM_R - 1.7)])
    feat(d, s.feat, name + '_TIE', 2 * LOOM_R + 5.2, True, mode=MID)


BORES = [(36.5, 44.0), (36.5, 84.0)]          # carrier Ø9.4 screw-access bores (r, phi), screw heads from X 146.7
RETAINERS = [(29.6, 64.0), (29.5, 30.0)]      # 6808 retainer screw heads on the carrier inboard face
CARRIER_FACE = 140.5


def clip():
    """Bolt-free loop clip for the unchanged carrier: the tie-down saddle plus a 3 mm foot on the carrier inboard face,
    located by two crush-ribbed spigots in the screw-access bores at 44 and 84 deg. Prints standing on its flat -s end."""
    p = os.path.join(DIR, 'marengo_upper_arm_loop_clip_left.SLDPRT')
    shutil.copy(MASTER, p)
    e, w = L(), L()
    d = sw.OpenDoc6(p, 1, 1, '', e, w)
    for b in bodies(d):
        delete_body(d, b[0])
    C, u, w_ = anchor_frame(60.0, +1)
    P = lambda s_, w2, x=LOOP_X: (x, *(C + s_ * u + w2 * w_))
    sw_ = lambda y, z: ((np.array([y, z]) - C) @ u, (np.array([y, z]) - C) @ w_)
    pol = lambda r, f: (CY + r * math.cos(math.radians(f)), CZ - r * math.sin(math.radians(f)))
    S0 = -20.5                                    # flat print face
    pl = plane(d, 'X', LOOP_X, 'CLIP_P')
    with Sk(d, pl, 'CLIP_BLOCK') as s:
        s.poly([P(*q) for q in [(S0, -12.5), (19, -12.5), (16, -3), (10, 2), (S0, 2)]])
    feat(d, s.feat, 'CLIP_BLOCK', 2 * LOOM_R + 5.0, False, mode=MID)
    from shapely.geometry import MultiPoint, Point
    pts = [Point(*sw_(*pol(r, f))).buffer(7.0, 24) for r, f in BORES]
    hull = MultiPoint([c for g in pts for c in g.exterior.coords] + [(S0, -12.5), (16, -12.5), (S0, -4)]).convex_hull
    hull = hull.intersection(MultiPoint([(S0, -60), (40, -60), (40, 20), (S0, 20)]).convex_hull)
    for r, f in RETAINERS:                         # keep off the bearing-retainer heads
        hull = hull.difference(Point(*sw_(*pol(r, f))).buffer(4.5, 24))
    from shapely.geometry import Polygon
    hull = hull.difference(Polygon([sw_(*pol(31.5, f)) for f in range(0, 360, 3)]))   # forearm hub runs at r < 29.5
    hull = hull.buffer(-0.8).buffer(0.8)           # no necks or slivers left by the cut-outs
    if hull.geom_type == 'MultiPolygon':
        hull = max(hull.geoms, key=lambda g: g.area)
    xf = CARRIER_FACE - 1.5
    plf = plane(d, 'X', xf, 'CLIP_FOOT_P')
    with Sk(d, plf, 'CLIP_FOOT') as s:
        s.poly([P(a, b, xf) for a, b in list(hull.exterior.coords)[:-1]])
    feat(d, s.feat, 'CLIP_FOOT', 3.0, False, mode=MID)
    xs = CARRIER_FACE + 2.25                      # spigots X 140.5-145.0, four crush ribs: 0.1 mm interference in the Ø9.4 bore, pops off by hand
    pls = plane(d, 'X', xs, 'CLIP_SPIGOT_P')
    with Sk(d, pls, 'CLIP_SPIGOT') as s:
        for r, f in BORES:
            y0, z0 = pol(r, f)
            a = np.linspace(0, 2 * math.pi, 49)[:-1]
            rr = 4.5 + 0.3 * (np.cos(4 * a) > 0.8)
            s.poly([(xs, y0 + q * math.cos(t), z0 + q * math.sin(t)) for q, t in zip(rr, a)])
    feat(d, s.feat, 'CLIP_SPIGOT', 4.5, False, mode=MID)
    fc = floor_curve()
    with Sk(d, pl, 'CLIP_GROOVE') as s:
        s.poly([P(*q) for q in fc] + [P(fc[-1][0], 12), P(fc[0][0], 12)])
    feat(d, s.feat, 'CLIP_GROOVE', 2 * LOOM_R, True, mode=MID)
    with Sk(d, pl, 'CLIP_TIE') as s:
        s.poly([P(-2.25, -LOOM_R - 3.7), P(2.25, -LOOM_R - 3.7), P(2.25, -LOOM_R - 1.7), P(-2.25, -LOOM_R - 1.7)])
    feat(d, s.feat, 'CLIP_TIE', 2 * LOOM_R + 5.2, True, mode=MID)
    print('  print up (arm coords):', [round(v, 4) for v in u], flush=True)
    finish(d, p, 'ua_loop_clip')


def fea_model(slit_on=False):
    """One body for the seam FEA: the master with the hub lightening and the seam columns, and a 0.2 mm slit on the
    seam plane through the tube walls everywhere except the bolted column footprints and the hub lugs. Ring and hub
    stay bonded (they bear in their pockets; the screws only locate them)."""
    p = str(W / 'fea' / ('UA_CLAM.SLDPRT' if slit_on else 'UA_CLAM_BONDED.SLDPRT'))
    shutil.copy(MASTER, p)
    e, w = L(), L()
    d = sw.OpenDoc6(p, 1, 1, '', e, w)
    for b in bodies(d):
        if b[0] == 'R4_SPRING_SEAT':
            delete_body(d, b[0])
    pl = plane(d, 'Y', 315.86, 'LIGHTEN_P')
    with Sk(d, pl, 'LIGHTEN') as s:
        s.circle((TX, 315.86, TZ), 33.0)
        for x, z in HUB_HOLES:
            s.circle((x, 315.86, z), 5.0)
    feat(d, s.feat, 'LIGHTEN', 13.36, True, want=(1, -1, 315.86))
    with Sk(d, pl, 'WINDOWS') as s:
        for k in range(6):
            c = math.radians(7.9 + 60 * k)
            A = [c + math.radians(a) for a in range(-20, 21, 5)]
            s.poly([(TX + 19 * math.cos(a), 315.86, TZ + 19 * math.sin(a)) for a in A] + [(TX + 29 * math.cos(a), 315.86, TZ + 29 * math.sin(a)) for a in A[::-1]])
    feat(d, s.feat, 'WINDOWS', 8.0, True, want=(1, 1, 315.86))
    for i, (q, zt) in enumerate(zip(TUBE_COLS, COL_TOP)):
        cyl_z(d, f'SEAM_BOSS{i}', [q], POST_R, Z0, zt - Z0, False, 1)
    for i, (q, zb) in enumerate(zip(TUBE_COLS, COL_BOT)):
        cyl_z(d, f'SEAM_COLUMN{i}', [q], POST_R, Z0, Z0 - zb, False, -1)
    # the trims the printed parts have: RS02-side skirt below Y_SKIRT, ring flare tips below Y_TIP, widened port slot
    xb = LUG['x1'] + 1.15
    rx = plane(d, 'X', (xb + 214.0) / 2, 'SKIRT_TIP_P')
    with Sk(d, rx, 'SKIRT_TIP') as s:
        s.poly([((xb + 214.0) / 2, y, z) for y, z in ((100, -80), (Y_SKIRT, -80), (Y_SKIRT, 90), (100, 90))])
    feat(d, s.feat, 'SKIRT_TIP', 214.0 - xb, True, mode=MID)
    xm = (LUG['x0'] + LUG['x1']) / 2
    pt = plane(d, 'X', xm, 'TIP_P')
    hb = math.asin(math.sqrt(R_RING ** 2 - (Y_TIP - CY) ** 2) / R_RING)
    arc = [(xm, CY + R_RING * math.cos(b), CZ + R_RING * math.sin(b)) for b in np.linspace(hb, 2 * math.pi - hb, 120)]
    with Sk(d, pt, 'RING_TIP') as s:
        s.poly([(xm, Y_TIP, 130)] + arc + [(xm, Y_TIP, -120), (xm, 100, -120), (xm, 100, 130)])
    feat(d, s.feat, 'RING_TIP', LUG['x1'] - LUG['x0'] + 2.3, True, mode=MID)
    rect_z(d, 'PORT_SLOT', 148.0, 169.0, 238.0, 254.0, (SLOT_Z[0] + SLOT_Z[1]) / 2, SLOT_Z[1] - SLOT_Z[0], cut=True)
    if not slit_on:
        d.ForceRebuild3(False); print('  bodies', bodies(d), flush=True); d.Save3(1, e, w); sw.CloseDoc(d.GetTitle)
        return
    from shapely.geometry import Polygon, Point, box as sbox
    keep = [Point(x, y).buffer(POST_R, 32) for x, y in TUBE_COLS] + [sbox(x0, 288.0, x1, 296.5) for x0, x1 in HUB_LUGS]
    slit = sbox(120.0, Y_FLARE, 220.0, 296.0).difference(sbox(LUG['x0'] - 1.15, Y_FLARE, LUG['x1'] + 1.15, CY + R_RING + CL))
    for k in keep:
        slit = slit.difference(k)
    plz = plane(d, 'Z', Z0, 'SLIT_P')
    with Sk(d, plz, 'SLIT') as s:
        geoms = slit.geoms if slit.geom_type == 'MultiPolygon' else [slit]
        for g in geoms:
            s.poly([(x, y, Z0) for x, y in list(g.exterior.coords)[:-1]])
            for h in g.interiors:
                s.poly([(x, y, Z0) for x, y in list(h.coords)[:-1]])
    feat(d, s.feat, 'SLIT', 0.2, True, mode=MID)
    d.ForceRebuild3(False)
    print('  bodies', bodies(d), flush=True)
    d.Save3(1, e, w)
    sw.CloseDoc(d.GetTitle)


if __name__ == '__main__':
    jobs = dict(ring=ring, hub=hub, clip=clip, fea=fea_model, fea_slit=lambda: fea_model(True), upper=lambda: half(True), lower=lambda: half(False))
    for k in sys.argv[1:] or jobs:
        print('==', k, flush=True)
        jobs[k]()
