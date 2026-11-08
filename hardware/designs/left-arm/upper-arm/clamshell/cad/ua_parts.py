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
R_RING, CL = 43.6, 0.15                # ring OD radius, capture clearance
HUB_Y = 302.0                          # hub disc / halves joint
SPIG_Y0, SPIG_R = 296.0, 35.8          # hub spigot, inside the halves' sleeve
LUG = dict(x0=151.8, x1=165.2, r0=R_RING - 1.0, r1=56.0, hz=5.0)      # ring lug on +Y, split by the seam
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


LUGY0, LUGY1 = CY + 42.6, CY + 56.0
RING_COL = (158.5, CY + 50.0)
TUBE_COLS = [(141.3, 262.0), (138.9, 278.0), (199.7, 262.0), (202.1, 278.0)]   # embedded ~2 mm in the 3.3 mm wall
HUB_COLS = [(139.0, 292.0), (202.0, 292.0)]
HUB_LUGS = [(135.5, 143.0), (198.0, 205.5)]


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
    elbow_revolve(d, 'KEEP_RING', [(100, 0), (LUG['x0'] - 1, 0), (LUG['x0'] - 1, R_RING), (LUG['x1'] + 1, R_RING), (LUG['x1'] + 1, 0), (260, 0), (260, 200), (100, 200)], cut=True)
    drop_slivers(d)
    # RS02 plug-housing pocket run through to the seat face over 161-191 deg (db71b23b left a 1.7 mm lip Joey had to file)
    pl = plane(d, 'X', 151.0, 'POCKET_P')
    with Sk(d, pl, 'POCKET') as s:
        A = [math.radians(a) for a in range(161, 192, 2)]
        s.poly([(151.0, CY + 32.5 * math.cos(a), CZ + 32.5 * math.sin(a)) for a in A] + [(151.0, CY + 36.9 * math.cos(a), CZ + 36.9 * math.sin(a)) for a in A[::-1]])
    feat(d, s.feat, 'POCKET_THRU', 15.0, True, want=(0, 1, 151.0))
    # lug on +Y, split by the seam plane: captured in recesses in both halves, clamped by one seam screw
    rect_z(d, 'RING_LUG', LUG['x0'], LUG['x1'], LUGY0, LUGY1, Z0, 2 * LUG['hz'], cut=False)
    cyl_z(d, 'RING_LUG_HOLE', [RING_COL], 1.7, Z0 + 6, 12, True, -1)
    finish(d, p, 'ua_ring')


def hub():
    d, p = open_copy('marengo_upper_arm_hub_left.SLDPRT')
    box_cut(d, 'BELOW_HUB', 'Y', SPIG_Y0, -1, [(100, SPIG_Y0, -80), (240, SPIG_Y0, -80), (240, SPIG_Y0, 90), (100, SPIG_Y0, 90)])
    drop_slivers(d)
    tube_revolve(d, 'SLEEVE_ZONE', [(SPIG_R, SPIG_Y0 - 1), (50, SPIG_Y0 - 1), (50, HUB_Y), (SPIG_R, HUB_Y)], cut=True)
    tube_revolve(d, 'SPIGOT', [(28.0, SPIG_Y0), (SPIG_R, SPIG_Y0), (SPIG_R, HUB_Y + 0.5), (28.0, HUB_Y + 0.5)], cut=False)
    # lighten the 17 mm solid disc: keep a 6 mm plate at the RS02 face, the spigot band and Ø10 bosses round the M4 holes
    pl = plane(d, 'Y', 313.86, 'LIGHTEN_P')
    with Sk(d, pl, 'LIGHTEN') as s:
        s.circle((TX, 313.86, TZ), 33.0)
        for x, z in HUB_HOLES:
            s.circle((x, 313.86, z), 5.0)
    feat(d, s.feat, 'LIGHTEN', 11.3, True, want=(1, -1, 313.86))
    # six windows through the plate between the bosses (bolts at r 12, every 60 deg from 37.9): r 19-29, +-20 deg
    with Sk(d, pl, 'WINDOWS') as s:
        for k in range(6):
            c = math.radians(7.9 + 60 * k)
            A = [c + math.radians(a) for a in range(-20, 21, 5)]
            s.poly([(TX + 19 * math.cos(a), 313.86, TZ + 19 * math.sin(a)) for a in A] + [(TX + 29 * math.cos(a), 313.86, TZ + 29 * math.sin(a)) for a in A[::-1]])
    feat(d, s.feat, 'WINDOWS', 8.0, True, want=(1, 1, 313.86))
    # the old tube floor (Y 298-303) inside the spigot carries nothing now; keep only the bolt bosses through it
    pl = plane(d, 'Y', 297.0, 'FLOOR_P')
    with Sk(d, pl, 'FLOOR') as s:
        s.circle((TX, 297.0, TZ), 27.6)
        for x, z in HUB_HOLES:
            s.circle((x, 297.0, z), 5.0)
    feat(d, s.feat, 'FLOOR_OPEN', 6.4, True, want=(1, 1, 297.0))
    for i, (x0, x1) in enumerate(HUB_LUGS):
        rect_z(d, f'HUB_LUG{i}', x0, x1, 288.0, SPIG_Y0 + 0.5, Z0, 10.0, cut=False)
    cyl_z(d, 'HUB_LUG_HOLES', HUB_COLS, 1.7, Z0 + 6, 12, True, -1)
    finish(d, p, 'ua_hub')


def half(upper):
    nm = 'upper' if upper else 'lower'
    d, p = open_copy(f'marengo_upper_arm_{nm}_left.SLDPRT')
    elbow_revolve(d, 'RING_ZONE', [(LUG['x0'] - 1.15, 0), (LUG['x1'] + 1.15, 0), (LUG['x1'] + 1.15, R_RING + CL), (LUG['x0'] - 1.15, R_RING + CL)], cut=True)
    rect_z(d, 'RING_LUG_RECESS', LUG['x0'] - CL, LUG['x1'] + CL, LUGY0 - CL, LUGY1 + CL, Z0, 2 * LUG['hz'] + 2 * CL, cut=True)
    box_cut(d, 'ABOVE_HUB', 'Y', HUB_Y, 1, [(100, HUB_Y, -80), (240, HUB_Y, -80), (240, HUB_Y, 90), (100, HUB_Y, 90)])
    tube_revolve(d, 'SPIGOT_SEAT', [(5, SPIG_Y0 - CL), (SPIG_R + CL, SPIG_Y0 - CL), (SPIG_R + CL, HUB_Y + 1), (5, HUB_Y + 1)], cut=True)
    for i, (x0, x1) in enumerate(HUB_LUGS):
        rect_z(d, f'HUB_LUG_RECESS{i}', x0 - CL, x1 + CL, 288.0 - CL, SPIG_Y0 + 0.5, Z0, 10.0 + 2 * CL, cut=True)
    if upper:
        box_cut(d, 'LOWER_SIDE', 'Z', Z0, -1, [(100, 150, Z0), (240, 150, Z0), (240, 340, Z0), (100, 340, Z0)])
    else:
        box_cut(d, 'UPPER_SIDE', 'Z', Z0, 1, [(100, 150, Z0), (240, 150, Z0), (240, 340, Z0), (100, 340, Z0)])
    print('  bodies', bodies(d), flush=True)
    if upper:
        bosses_z(d, 'SEAM_BOSS', TUBE_COLS, 4.0, Z0, 11.0, 1)
        bosses_z(d, 'LUG_BOSS', HUB_COLS + [RING_COL], 4.0, Z0 + LUG['hz'] + CL, 9.0, 1)
        cyl_z(d, 'SEAM_INSERTS', TUBE_COLS, 2.0, Z0 - 0.01, 7.0, True, 1)
        cyl_z(d, 'LUG_INSERTS', HUB_COLS + [RING_COL], 2.0, Z0 + LUG['hz'] + CL - 0.01, 7.0, True, 1)
        cyl_z(d, 'SEAM_PIN_SOCKET', TUBE_COLS, 3.2, Z0 - 0.01, 1.4, True, 1)
    else:
        bosses_z(d, 'SEAM_COLUMN', TUBE_COLS, 4.0, Z0, 15.5, -1)
        bosses_z(d, 'LUG_COLUMN', HUB_COLS, 4.0, Z0 - LUG['hz'] - CL, 10.5, -1)
        bosses_z(d, 'RING_COLUMN', [RING_COL], 4.0, Z0 - LUG['hz'] - CL, 21.0, -1)
        bosses_z(d, 'SEAM_PIN', TUBE_COLS, 3.0, Z0, 1.2, 1)
        cyl_z(d, 'SCREW_HOLES', TUBE_COLS + HUB_COLS + [RING_COL], 1.7, Z0 + 2, 60.0, True, -1)
        cyl_z(d, 'CBORE_TUBE', TUBE_COLS + HUB_COLS, 3.25, -10.8, 20.0, True, -1)
        cyl_z(d, 'CBORE_RING', [RING_COL], 3.25, -23.0, 20.0, True, -1)
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


def carrier():
    """The carrier (R4_SPRING_SEAT body of the master) with the upper-arm loop anchor at phi 60."""
    p = os.path.join(DIR, 'marengo_upper_arm_carrier_left.SLDPRT')
    shutil.copy(MASTER, p)
    e, w = L(), L()
    d = sw.OpenDoc6(p, 1, 1, '', e, w)
    for b in bodies(d):
        if b[0] != 'R4_SPRING_SEAT':
            delete_body(d, b[0])
    loop_anchor(d, 'LOOP_ANCHOR', 60.0, +1, web_to_x=144.0)
    finish(d, p, 'ua_carrier')


if __name__ == '__main__':
    jobs = dict(ring=ring, hub=hub, carrier=carrier, upper=lambda: half(True), lower=lambda: half(False))
    for k in sys.argv[1:] or jobs:
        print('==', k, flush=True)
        jobs[k]()
