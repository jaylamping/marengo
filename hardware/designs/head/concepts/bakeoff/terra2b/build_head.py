"""marengo head, Iron Giant bake-off entry "fable", round 3 (build123d 0.13).

Robot frame: X forward, Y left, Z up, mm.  z = 0 is the bottom of the neck collar.
Judged only against MASTER_A (front from below), MASTER_B (red poster 3/4) and MASTER_C (barn still, side).
The head now sits on a visible ringed neck above a lowered collar; see NOTES.md for the measured ratios.
Run:  python3 build_head.py  ->  out/*.stl, out/parts.json, out/head.step
"""
import json, math, os
from build123d import *

# ----------------------------------------------------------------------------- vertical layout (mm)
CHIN_Z = 50.0                       # bottom of the jaw (was 22; raised so the neck shows, MASTER_A/B/C)
LEDGE_Z = CHIN_Z + 54.0             # top edge of the jaw band at the sides (knee); 10 mm lower at the front
EYE_Z = LEDGE_Z + 50.0              # eye centre                           (MASTER_C: knee -> eye = 0.36 D)
TOP_Z = EYE_Z + 82.0                # crown apex                           (MASTER_C: crown -> eye = 0.65 D)

P = dict(
    # cranium: rounded-square plan, straight walls, then the dome
    hw=61.0, hd=65.0,            # half width (Y) / half depth (X) of the cranium
    n_front=4.2, n_back=3.0,     # superellipse exponents (front half / back half of the plan)
    wall_top=EYE_Z + 34.0, top=TOP_Z,
    back_start=EYE_Z - 4.0,      # the back of the head starts curving here, lower than the sides (MASTER_C, B)
    apex_fwd=10.0,               # crown apex sits this far forward of the plan centre (MASTER_C)
    dome_my=1.65,                # front-view dome profile r = (1 - f^m)^(1/m): pointed arch (MASTER_A)
    dome_mx_f=2.15, dome_mx_b=1.55,  # side-view dome: flattish forehead, long sloping curve at the back (MASTER_C)
    dome_n_top=2.3,              # the plan relaxes from n_front/n_back to this (rounder) at the apex
    face_bottom=CHIN_Z + 6.0,    # the cranium shell continues down inside the jaw hoop to here
    # eyes: MASTER_A glow 0.21 W, dark ring 0.29 W, spacing 0.44 W
    eye_z=EYE_Z, eye_y=27.5, eye_ro=18.0, eye_ri=15.0, eye_depth=6.0, eye_chamfer=2.0,
    # jaw: front band (hoop) with a floor, horizontal seam, side straps up to the pivot bosses
    ledge_z=LEDGE_Z, chin_z=CHIN_Z, jaw_wall=5.5, jaw_gap=1.5,
    ledge_dip=10.0,              # the ledge runs slightly downhill toward the front (MASTER_C)
    chin_fwd=12.0,               # the jaw front rakes forward going down: this much at the chin (MASTER_C)
    scoop_r=16.0, scoop_r_front=6.0,   # rounded lower edge: sides / chin
    bottom_sag=6.0,              # front view: jaw bottom is a shallow arc, this much lower at the centre
    bottom_flat_x=10.0, bottom_back_z=CHIN_Z + 8.0,   # side view: flat under the chin, rising gently to the back
    seam_frac=0.42, seam_h=2.0, seam_depth=1.2,       # horizontal seam across the jaw front (MASTER_A, MASTER_C)
    jaw_rear_x=-58.0,            # rear edge of the band / strap, at the back of the head (MASTER_C)
    jaw_diag=(-58.0, CHIN_Z + 30.0, -8.0, CHIN_Z),    # diagonal lower-rear cut of the band (MASTER_C)
    knee_x=-6.5,                # the strap's front edge meets the ledge here, in a concave notch (MASTER_C)
    hinge_x=-45.0, hinge_z=EYE_Z + 6.0, strap_r=16.0,  # pivot boss near the back of the face, at eye height
    ear_pad=4.0,                 # the strap end stands proud of the head (the "ear" tab of MASTER_A)
    boss_r=8.5, boss_step_r=5.5, boss_pin_r=2.8,      # concentric boss: disc, raised step, recessed centre
    floor_t=4.0, neck_hole_r=42.0,
    # crest / mohawk: a thin sharp fin (MASTER_A): fine point between the eyes, narrow over the forehead,
    # peak at the top front of the dome, then a low ridge back over the crown
    crest_tip_z=EYE_Z - 32.0, crest_w=4.5, crest_h_face=4.5, crest_h_peak=21.0, crest_peak_deg=80.0,
    crest_h_back=1.0, crest_end_deg=110.0, crest_lean_deg=40.0,   # blade leans back over the dome (MASTER_A, B)
    # collar (lowered) and ringed neck
    collar_r1=75.0, collar_r2=68.0, collar_h1=8.0, collar_h2=6.0,
    neck_r=36.0, neck_rib_r=41.0, neck_rib_h=4.0, neck_ribs=(18.0, 27.0, 36.0),
)
MATS = dict(head='white', collar='white', jaw='grey1', hinge='grey1', crest='grey2',
            bezel='dark', seam='dark', core='dark', neck='dark', glow='eye')


# ----------------------------------------------------------------------------- helpers
def plan_pts(hw, hd_f, hd_b, n_f, n_b, x0=0.0, N=96):
    """Rounded-square plan: superellipse, different depth/exponent for the front (x>0) and back (x<0)."""
    pts = []
    for i in range(N):
        a = 2 * math.pi * i / N
        c, s = math.cos(a), math.sin(a)
        hd, n = (hd_f, n_f) if c >= 0 else (hd_b, n_b)
        pts.append((x0 + math.copysign(hd * abs(c) ** (2 / n), c), math.copysign(hw * abs(s) ** (2 / n), s)))
    return pts


def sec(z, hw, hd_f, hd_b=None, n_f=None, n_b=None, x0=0.0):
    hd_b = hd_f if hd_b is None else hd_b
    n_f = P['n_front'] if n_f is None else n_f
    n_b = P['n_back'] if n_b is None else n_b
    return make_face(Wire([Spline(*[(x, y, z) for x, y in plan_pts(hw, hd_f, hd_b, n_f, n_b, x0)], periodic=True)]))


def face_x(y, hw=None, hd=None, n=None):
    """x of the front face at lateral position y."""
    hw = P['hw'] if hw is None else hw; hd = P['hd'] if hd is None else hd; n = P['n_front'] if n is None else n
    return hd * max(0.0, 1 - abs(y / hw) ** n) ** (1 / n)


def side_y(x, hw=None, hd=None):
    """y of the side wall at fore-aft position x."""
    hw = P['hw'] if hw is None else hw; hd = P['hd'] if hd is None else hd
    n = P['n_front'] if x >= 0 else P['n_back']
    return hw * max(0.0, 1 - abs(x / hd) ** n) ** (1 / n)


def xz_prism(pts, half=200):
    return extrude(make_face(Plane.XZ * Polyline(*pts, close=True)), amount=half, both=True)


def yz_prism(pts, half=200):
    return extrude(make_face(Plane.YZ * Polyline(*pts, close=True)), amount=half, both=True)


def arc_pts(cx, cz, r, a0, a1, n=12):
    return [(cx + r * math.cos(math.radians(a0 + (a1 - a0) * i / n)), cz + r * math.sin(math.radians(a0 + (a1 - a0) * i / n)))
            for i in range(n + 1)]


# ----------------------------------------------------------------------------- cranium
def dome_radii(z):
    """Half-sizes of the cranium section at height z: sides and front curve from wall_top, the back from
    back_start (lower), each with its own superellipse exponent; the plan rounds toward a circle at the apex."""
    hw, hd, top = P['hw'], P['hd'], P['top']
    fy = max(0.0, (z - P['wall_top']) / (top - P['wall_top']))
    fb = max(0.0, (z - P['back_start']) / (top - P['back_start']))
    my, mxf, mxb = P['dome_my'], P['dome_mx_f'], P['dome_mx_b']
    ry = (1 - min(fy, 1.0) ** my) ** (1 / my); rxf = (1 - min(fy, 1.0) ** mxf) ** (1 / mxf)
    rxb = (1 - min(fb, 1.0) ** mxb) ** (1 / mxb)
    x0 = P['apex_fwd'] * fy ** 1.5
    return hw * ry, hd * rxf + x0, hd * rxb - x0, fy, x0


def build_cranium():
    hw, hd, z0, top = P['hw'], P['hd'], P['face_bottom'], P['top']
    zs = min(P['wall_top'], P['back_start'])
    walls = extrude(sec(z0, hw, hd), amount=zs - 10 - z0)   # walls stop exactly where the loft starts (clean union)
    secs = [sec(zs - 10, hw, hd)]                            # ghost section below the junction: loft leaves vertical
    nt = P['dome_n_top']
    N = 18
    for i in range(N):
        z = zs + (top - zs) * (i / N) ** 1.0
        rw, rf, rb, fy, x0 = dome_radii(z)
        nf = P['n_front'] + (nt - P['n_front']) * fy; nb = P['n_back'] + (nt - P['n_back']) * fy
        secs.append(sec(z, rw, rf - x0, rb + x0, n_f=nf, n_b=nb, x0=x0))
    z = top - 2.0; rw, rf, rb, fy, x0 = dome_radii(z)
    secs.append(sec(z, rw, rf - x0, rb + x0, n_f=nt, n_b=nt, x0=x0))
    return walls + loft(secs + [Vertex(P['apex_fwd'], 0, top)])


def dome_profile(N=240):
    """Centre-line (y = 0) profile of the dome from the front wall top over the crown to the back, as (x, z),
    sampled by angle so the crest path parameter stays smooth."""
    top = P['top']; pts = []
    for i in range(N + 1):
        a = math.pi * i / N
        if a <= math.pi / 2:
            zc = P['wall_top']; z = zc + (top - zc) * math.sin(a)
            rw, rf, rb, fy, x0 = dome_radii(z); pts.append((rf, z))
        else:
            zc = P['back_start']; z = zc + (top - zc) * math.sin(a)
            rw, rf, rb, fy, x0 = dome_radii(z); pts.append((-rb, z))
    return pts


# ----------------------------------------------------------------------------- eyes
def build_eyes(head):
    ro, ri, dep, ez, ch = P['eye_ro'], P['eye_ri'], P['eye_depth'], P['eye_z'], P['eye_chamfer']
    pockets = Part(); bezel = Part(); glow = Part()
    for s in (1, -1):
        y = s * P['eye_y']; xf = face_x(y)
        at = lambda x_: Location((x_, y, ez), (0, 90, 0))                      # axis along +X
        pockets += at(xf - dep) * Cylinder(ro, 40, align=(Align.CENTER, Align.CENTER, Align.MIN))
        pockets += at(xf - ch) * Cone(ro, ro + 7, 7, align=(Align.CENTER, Align.CENTER, Align.MIN))   # bevelled rim
        bezel += at(xf - dep - 1.5) * (Cylinder(ro + 0.05, 3, align=(Align.CENTER, Align.CENTER, Align.MIN))
                                       - Cylinder(ri, 4, align=(Align.CENTER, Align.CENTER, Align.MIN)))
        lining = at(xf - dep) * (Cylinder(ro + 0.3, dep - ch + 0.2, align=(Align.CENTER, Align.CENTER, Align.MIN))
                                 - Cylinder(ro - 0.8, dep + 11, align=(Align.CENTER, Align.CENTER, Align.MIN)))
        bezel += lining & head
        disc = scale(Sphere(1), by=(2.2, ri, ri)) & Box(10, 100, 100, align=(Align.MIN, Align.CENTER, Align.CENTER))
        glow += disc.move(Location((xf - dep + 1.0, y, ez)))
    return pockets, bezel, glow


# ----------------------------------------------------------------------------- crest (mohawk)
def build_crest(head):
    hd, zc, top = P['hd'], P['wall_top'], P['top']
    prof = dome_profile()
    a_end = math.radians(P['crest_end_deg'])
    n_end = int(len(prof) * a_end / math.pi) + 1
    prof = prof[:n_end]
    seg = [math.dist(prof[i], prof[i + 1]) for i in range(len(prof) - 1)]
    L1 = zc - P['crest_tip_z']; L2 = sum(seg); L = L1 + L2
    u_face = L1 / L
    i_peak = int(240 * P['crest_peak_deg'] / 180); u_peak = (L1 + sum(seg[:i_peak])) / L
    i_90 = int(240 * 96 / 180); u_crown = (L1 + sum(seg[:i_90])) / L   # steep drop just behind the peak

    def frame(u):
        s = u * L
        if s <= L1:
            return Vector(hd, 0, P['crest_tip_z'] + s), Vector(1, 0, 0), Vector(0, 0, 1)
        s -= L1; i = 0
        while i < len(seg) - 1 and s > seg[i]:
            s -= seg[i]; i += 1
        (x0, z0), (x1, z1) = prof[i], prof[i + 1]; f = min(1.0, s / max(seg[i], 1e-9))
        tan = Vector(x1 - x0, 0, z1 - z0).normalized()
        return Vector(x0 + (x1 - x0) * f, 0, z0 + (z1 - z0) * f), Vector(tan.Z, 0, -tan.X), tan

    def height(u):   # fin height along the path
        if u <= u_face:                                   # up the forehead: point -> crest_h_face
            return P['crest_h_face'] * (u / u_face) ** 1.1
        if u <= u_peak:                                   # over the top front of the dome: rise to the peak
            t = (u - u_face) / (u_peak - u_face)          # stays low over the brow, rises only near the top
            return P['crest_h_face'] + (P['crest_h_peak'] - P['crest_h_face']) * t ** 3.0
        if u <= u_crown:                                  # steep drop behind the peak
            t = (u - u_peak) / (u_crown - u_peak)
            return P['crest_h_peak'] + (P['crest_h_back'] - P['crest_h_peak']) * math.sin(0.5 * math.pi * t) ** 0.8
        t = (u - u_crown) / (1 - u_crown)                 # thin ridge fading down the back
        return P['crest_h_back'] * (1 - t) ** 0.8

    def width(u):
        if u <= u_face:
            return P['crest_w'] * (0.12 + 0.88 * (u / u_face) ** 0.6)
        if u <= u_crown:
            return P['crest_w']
        return P['crest_w'] * (0.45 + 0.55 * (1 - (u - u_crown) / (1 - u_crown)))

    secs = []
    U = [0, 0.02, 0.05, 0.1, 0.16, 0.23, 0.3] + [0.3 + 0.7 * k / 26 for k in range(1, 27)]
    for u in U:
        pt, nrm, tan = frame(u)
        h = max(0.5, height(u)); b = max(0.8, width(u))
        if u > u_face:   # on the dome: rotate the section so the blade leans back (toward -x) instead of following the normal
            ramp = min(1.0, (u - u_face) / (0.35 * (u_peak - u_face)))
            th = math.radians(P['crest_lean_deg'] * ramp)
            nx, nz = nrm.X, nrm.Z
            nrm = Vector(nx * math.cos(th) - nz * math.sin(th), 0, nx * math.sin(th) + nz * math.cos(th))
            tan = Vector(-nrm.Z, 0, nrm.X) if tan.X < 0 or True else tan
            tan = Vector(nrm.Z, 0, -nrm.X)
        pl = Plane(origin=pt, x_dir=(0, 1, 0), z_dir=tan)
        out = 1.0 if pl.y_dir.dot(nrm) > 0 else -1.0
        sink = 6.0
        bb = b * (h + sink) / h                     # widen the buried base so the fin is b wide AT the surface
        poly = [(-bb / 2, -sink * out), (bb / 2, -sink * out), (0.35, h * out), (-0.35, h * out)]
        secs.append(pl * make_face(Polyline(*poly, close=True)))
    return loft(secs, ruled=True) - head


# ----------------------------------------------------------------------------- jaw
def ledge_at(x):
    """Ledge height: level at the sides, dipping toward the front (MASTER_C)."""
    f = max(0.0, min(1.0, (x + 30.0) / (P['hd'] + 30.0)))
    return P['ledge_z'] - P['ledge_dip'] * f


def jaw_section(z, off):
    """Plan section of the jaw shell at height z, offset `off` outside the cranium plan; chin rake + scoop rounding."""
    lz, cz = P['ledge_z'], P['chin_z']
    f = (lz - z) / (lz - cz)
    fwd = P['chin_fwd'] * max(0.0, f)

    def rnd(r):
        if z >= cz + r:
            return 0.0
        d = (cz + r - z) / r
        return r * (1 - math.sqrt(max(0.0, 1 - min(1.0, d) ** 2)))
    shrink, shrink_f = rnd(P['scoop_r']), rnd(P['scoop_r_front'])
    return sec(z, P['hw'] + off - shrink, P['hd'] + off - shrink_f, P['hd'] + off - shrink, x0=fwd)


def band_profile(lift=0.0):
    """Side outline (XZ) of the jaw band: sloping ledge on top, vertical rear edge, diagonal lower-rear cut."""
    xr = P['jaw_rear_x']; x1, z1, x2, z2 = P['jaw_diag']
    top = [(x, ledge_at(x) + lift) for x in (300, 80, 40, 0, -30)]
    return xz_prism(top + [(xr, ledge_at(xr) + lift), (xr, z1), (x2, z2), (x2, -5), (300, -5)])


def strap_profile():
    """Side outline (XZ) of the strap: a wide plate from the band's rear up to the rounded pivot end; its front
    edge meets the ledge in a concave knee (MASTER_C)."""
    xr, xk = P['jaw_rear_x'], P['knee_x']
    hx, hz, r = P['hinge_x'], P['hinge_z'], P['strap_r']
    zk = ledge_at(xk) - 3

    def tangent(px, pz, side):
        d = math.hypot(hx - px, hz - pz); a = math.atan2(hz - pz, hx - px); b = math.asin(r / d)
        ang = a + side * b; L = math.sqrt(d * d - r * r)
        return (px + L * math.cos(ang), pz + L * math.sin(ang))
    tf = tangent(xk, zk, -1); tb = tangent(xr, zk, +1)
    af = math.degrees(math.atan2(tf[1] - hz, tf[0] - hx)); ab = math.degrees(math.atan2(tb[1] - hz, tb[0] - hx))
    if ab < af: ab += 360
    return xz_prism([(xk, zk)] + arc_pts(hx, hz, r, af, ab, 16) + [(xr, zk)])


def strap_profile_offset(d):
    """strap_profile grown by d (bigger pivot circle, knee and rear edge pushed out)."""
    xr, xk = P['jaw_rear_x'] - d, P['knee_x'] + d
    hx, hz, r = P['hinge_x'], P['hinge_z'], P['strap_r'] + d
    zk = ledge_at(P['knee_x']) - 3 - d

    def tangent(px, pz, side):
        dd = math.hypot(hx - px, hz - pz); a = math.atan2(hz - pz, hx - px); b = math.asin(r / dd)
        ang = a + side * b; L = math.sqrt(dd * dd - r * r)
        return (px + L * math.cos(ang), pz + L * math.sin(ang))
    tf = tangent(xk, zk, -1); tb = tangent(xr, zk, +1)
    af = math.degrees(math.atan2(tf[1] - hz, tf[0] - hx)); ab = math.degrees(math.atan2(tb[1] - hz, tb[0] - hx))
    if ab < af: ab += 360
    return xz_prism([(xk, zk)] + arc_pts(hx, hz, r, af, ab, 16) + [(xr, zk)])


def build_jaw():
    lz, cz, t, gap = P['ledge_z'], P['chin_z'], P['jaw_wall'], P['jaw_gap']
    r = P['scoop_r']
    zs = [cz + 0.01, cz + 0.05 * r, cz + 0.15 * r, cz + 0.32 * r, cz + 0.55 * r, cz + 0.8 * r, cz + r]
    zs += [cz + r + (lz - cz - r) * k / 5 for k in (1, 2, 3, 4, 5)]
    outer = loft([jaw_section(z, gap + t) for z in zs])
    inner = loft([jaw_section(z, gap) for z in [cz - 0.5] + zs[1:]] + [jaw_section(lz + 3, gap)])
    hwj = P['hw'] + gap + t
    Ry = (hwj ** 2) / (2 * P['bottom_sag']) + P['bottom_sag'] / 2
    across = Cylinder(Ry, 400, rotation=(0, 90, 0)).move(Location((0, 0, cz + Ry)))
    xb = -(P['hd'] + 30)
    keep = xz_prism([(300, cz), (P['bottom_flat_x'], cz), (xb, P['bottom_back_z']), (xb, 400), (300, 400)])
    bottom = keep & across
    band = (outer & bottom) - (inner & bottom.moved(Location((0, 0, P['floor_t']))))   # shell with a floor plate
    band = band - Cylinder(P['neck_hole_r'], 100, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((-4, 0, -10)))
    band = band & band_profile()
    # horizontal seam across the front of the band (MASTER_A, MASTER_C): a shallow groove, filled dark
    zs_ = lz - P['seam_frac'] * (lz - cz)
    seam_cut = loft([jaw_section(z, gap + t + 1) for z in (zs_ - P['seam_h'] / 2, zs_ + P['seam_h'] / 2)]) \
        - loft([jaw_section(z, gap + t - P['seam_depth']) for z in (zs_ - P['seam_h'] / 2 - 0.1, zs_ + P['seam_h'] / 2 + 0.1)])
    seam_cut = seam_cut & Box(400, 400, 400, align=(Align.MIN, Align.CENTER, Align.CENTER)).move(Location((-40, 0, zs_)))
    seam = seam_cut & band
    band = band - seam_cut
    # strap: a shell hugging the head side, cut to the strap outline; thicker pad at the rounded end
    ztop = P['hinge_z'] + P['strap_r'] + 2
    zb = lz - P['ledge_dip'] - 3
    ring = sec(zb, P['hw'] + gap + t, P['hd'] + gap + t); hole = sec(zb, P['hw'] + gap, P['hd'] + gap)
    strap = (extrude(ring, amount=ztop - zb) - extrude(hole, amount=ztop - zb + 2)) & strap_profile()
    zp = P['hinge_z'] - 10
    pad_ring = sec(zp, P['hw'] + gap + t + P['ear_pad'], P['hd'] + gap + t + P['ear_pad'])
    pad_hole = sec(zp, P['hw'] + gap + t - 1, P['hd'] + gap + t - 1)
    pad = (extrude(pad_ring, amount=ztop - zp) - extrude(pad_hole, amount=ztop - zp + 2)) & strap_profile()
    jaw = band + strap + pad
    # outline groove around the strap plate (MASTER_C draws the plate with a clear edge): a 1.2 mm dark lip
    # on the head side just outside the strap, from the knee up and over the boss and down the rear edge
    lip_in = sec(zb, P['hw'] + gap - 0.5, P['hd'] + gap - 0.5); lip_out = sec(zb, P['hw'] + gap + 1.2, P['hd'] + gap + 1.2)
    lip = (extrude(lip_out, amount=ztop + 3 - zb) - extrude(lip_in, amount=ztop + 5 - zb))
    seam_strap = lip & (strap_profile_offset(1.6) - strap_profile())
    seam_strap = seam_strap & Box(400, 400, 400, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, zb + 2)))
    # pivot bosses: disc, raised step, recessed centre (MASTER_C)
    hinge = Part()
    hx, hz = P['hinge_x'], P['hinge_z']
    for s in (1, -1):
        ys = side_y(hx, P['hw'] + gap + t + P['ear_pad'], P['hd'] + gap + t + P['ear_pad']) - 0.5
        disc = Location((hx, s * (ys + 1.5), hz), (90, 0, 0)) * Cylinder(P['boss_r'], 6)
        step = Location((hx, s * (ys + 4.5), hz), (90, 0, 0)) * Cylinder(P['boss_step_r'], 3)
        pin = Location((hx, s * (ys + 4.5), hz), (90, 0, 0)) * Cylinder(P['boss_pin_r'], 4)
        hinge += (disc + step) - pin
    return jaw, seam + seam_strap, hinge


# ----------------------------------------------------------------------------- collar, neck, dark parts
def build_collar():
    c = Cylinder(P['collar_r1'], P['collar_h1'], align=(Align.CENTER, Align.CENTER, Align.MIN))
    c += Cylinder(P['collar_r2'], P['collar_h2'], align=(Align.CENTER, Align.CENTER, Align.MIN)).move(
        Location((0, 0, P['collar_h1'])))
    return c


def build_dark():
    gap = P['jaw_gap']
    core = loft([jaw_section(z, gap - 2.5) for z in (P['chin_z'] + 3, P['chin_z'] + 14, P['ledge_z'] - 2)])
    core = core & Box(400, 400, P['ledge_z'] - 2 - (P['chin_z'] + 3), align=(Align.CENTER, Align.CENTER, Align.MIN)).move(
        Location((0, 0, P['chin_z'] + 3)))
    # ringed neck from the collar up into the jaw (MASTER_B stack of rings, MASTER_C tube)
    z0 = P['collar_h1'] + P['collar_h2'] - 2
    neck = Cylinder(P['neck_r'], P['chin_z'] + 12 - z0, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((-4, 0, z0)))
    for zr in P['neck_ribs']:
        neck += Cylinder(P['neck_rib_r'], P['neck_rib_h'], align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((-4, 0, zr)))
    return core, neck


# ----------------------------------------------------------------------------- assembly
def build():
    head = build_cranium()
    pockets, bezel, glow = build_eyes(head)
    head = head - pockets
    crest = build_crest(head)
    jaw, seam, hinge = build_jaw()
    collar = build_collar()
    core, neck = build_dark()
    return dict(head=head, crest=crest, bezel=bezel, glow=glow, jaw=jaw, seam=seam, hinge=hinge,
                collar=collar, core=core, neck=neck)


if __name__ == '__main__':
    here = os.path.dirname(os.path.abspath(__file__))
    out = os.path.join(here, 'out'); os.makedirs(out, exist_ok=True)
    parts = build(); man = []
    for k, sol in parts.items():
        if sol is None or sol.volume < 1:
            print('empty part', k); continue
        tol = (0.05, 0.1) if k in ('crest', 'glow') else (0.02, 0.04)
        export_stl(sol, os.path.join(out, f'{k}.stl'), tolerance=tol[0], angular_tolerance=tol[1])
        man.append(dict(file=f'{k}.stl', mat=MATS[k]))
    try:
        export_step(Compound([s for s in parts.values() if s.volume >= 1]), os.path.join(out, 'head.step'))
    except Exception as e:
        print('step export failed:', e)
    json.dump(man, open(os.path.join(out, 'parts.json'), 'w'), indent=1)
    bb = parts['head'].bounding_box(); jb = parts['jaw'].bounding_box()
    print('cranium W x D, z:', round(bb.size.Y, 1), round(bb.size.X, 1), round(bb.min.Z, 1), round(bb.max.Z, 1))
    print('jaw W x D, z range:', round(jb.size.Y, 1), round(jb.size.X, 1), round(jb.min.Z, 1), round(jb.max.Z, 1))
    print({k: (round(v.volume), v.is_valid) for k, v in parts.items()})
