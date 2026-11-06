"""marengo head, Iron Giant bake-off entry "fable", round 2 (build123d 0.13).

Robot frame: X forward, Y left, Z up, mm.  z = 0 is the bottom of the neck collar.
Proportions measured from ref2/ref6/ref9 (front), ref10/ref1/ref8 (side, jaw strap and pivot boss), ref3/ref4/ref11
(plan, ledge, skirt, eye rim); see NOTES.md.  Run:  python3 build_head.py  ->  out/*.stl, out/parts.json, out/head.step
"""
import json, math, os
from build123d import *

# ----------------------------------------------------------------------------- parameters (mm)
P = dict(
    # cranium: rounded-square plan, straight walls, then the dome
    hw=62.5, hd=65.0,            # half width (Y) / half depth (X) of the cranium
    n_front=4.2, n_back=3.0,     # superellipse exponents (front half / back half of the plan)
    wall_top=140.0, top=196.0,   # straight walls end at wall_top; dome apex at top
    dome_my=1.65,                # front-view dome profile r = (1 - f^m)^(1/m): pointed arch (ref2, ref6, ref9)
    dome_mx_f=2.6, dome_mx_b=1.8,   # side-view dome: flattish forehead, rounder longer curve at the back (ref10, ref8)
    dome_n_top=2.3,              # the plan relaxes from n_front/n_back to this (rounder) at the apex
    face_bottom=56.0,            # the cranium shell continues down inside the jaw hoop to here
    skirt_bottom=15.0,           # behind the jaw's rear edge the shell continues down into the collar (ref4, ref10)
    # eyes
    eye_z=114.0, eye_y=29.5, eye_ro=20.5, eye_ri=15.2, eye_depth=6.0, eye_chamfer=2.5,
    # jaw: front band (hoop) + side straps up to the pivot bosses
    ledge_z=64.0, chin_z=22.0, jaw_wall=4.0, jaw_gap=1.5,
    chin_fwd=11.0,               # the jaw front rakes forward going down: this much at the chin
    scoop_r=16.0,                # radius of the rounded lower edge (scoop) at the sides
    scoop_r_front=11.0,          # tighter round at the chin so it keeps a crisp forward edge
    bottom_sag=8.0,              # front view: jaw bottom is a shallow arc, this much lower at the centre
    bottom_flat_x=15.0,          # side view: bottom is flat (chin) ahead of this x ...
    bottom_back_z=30.0,          # ... and rises gently toward the back
    jaw_rear_x=-50.0,            # vertical rear edge of the band / strap (ref4, ref10)
    jaw_diag=(-50.0, 50.0, -6.0, 22.0),    # diagonal lower-rear cut of the band: (x1, z1) -> (x2, z2) (ref10, ref1)
    knee_x=-26.0,                # the strap's front edge meets the ledge here, in a concave notch (ref10, ref1)
    hinge_x=-33.0, hinge_z=119.0, strap_r=13.0,   # rounded strap end around the pivot boss, at eye height (ref10)
    ear_pad=5.0,                 # the strap end is thicker (a pad) so it stands proud like the ear tab in ref2/ref3
    boss_r=9.0, boss_step_r=6.0, boss_pin_r=3.0,  # concentric boss: disc, raised step, recessed centre
    # crest / mohawk
    crest_tip_z=73.0, crest_h=9.5, crest_b=17.0, crest_end_deg=140.0, spike_h=18.0,
    crest_fade_start=0.62,       # fraction of the way to the crown where the blade starts thinning (ref8, ref11)
    # collar, neck
    collar_r1=75.0, collar_r2=68.0, collar_h1=10.0, collar_h2=8.0,
    neck_r=38.0, neck_hole_r=41.0,
)
MATS = dict(head='white', collar='white', jaw='grey1', hinge='grey1', crest='grey2',
            bezel='dark', core='dark', neck='dark', glow='eye')


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


def diag_offset(d):
    """The band's diagonal lower-rear edge pushed outward (back/down) by d, as ((x1, z1), (x2, z2))."""
    x1, z1, x2, z2 = P['jaw_diag']
    dx, dz = x2 - x1, z2 - z1; L = math.hypot(dx, dz)
    nx, nz = -dz / L, dx / L                      # perpendicular, pointing back/down for this edge
    if nx > 0: nx, nz = -nx, -nz
    return (x1 + nx * d, z1 + nz * d), (x2 + nx * d, z2 + nz * d)


# ----------------------------------------------------------------------------- cranium
def build_cranium():
    hw, hd, z0, zc, top = P['hw'], P['hd'], P['face_bottom'], P['wall_top'], P['top']
    Hd = top - zc
    walls = extrude(sec(z0, hw, hd), amount=zc - 10 - z0)   # walls stop exactly where the loft starts (clean union)
    # dome: pointed arch across (my), broad flatter crown fore-aft (mx); plan rounds toward a circle at the apex
    secs = [sec(zc - 10, hw, hd), sec(zc, hw, hd)]   # ghost section below the junction: loft leaves vertical
    my, mxf, mxb, nt = P['dome_my'], P['dome_mx_f'], P['dome_mx_b'], P['dome_n_top']
    for f in (0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.78, 0.85, 0.91, 0.955, 0.985):
        ry = (1 - f ** my) ** (1 / my); rxf = (1 - f ** mxf) ** (1 / mxf); rxb = (1 - f ** mxb) ** (1 / mxb)
        nf = P['n_front'] + (nt - P['n_front']) * f; nb = P['n_back'] + (nt - P['n_back']) * f
        secs.append(sec(zc + Hd * f, hw * ry, hd * rxf, hd * rxb, n_f=nf, n_b=nb))
    head = walls + loft(secs + [Vertex(0, 0, top)])
    # skirt: the shell continues down behind / below the jaw's rear edge into the collar (ref4, ref10, ref11)
    g = P['jaw_gap']
    (ax, az), (bx, bz) = diag_offset(g)
    xr = P['jaw_rear_x'] - g
    # extend the offset diagonal down to the skirt bottom and up to the rear edge
    t_bot = (P['skirt_bottom'] - az) / (bz - az); xbot = ax + (bx - ax) * t_bot
    t_top = (az - az) ; xtop = xr
    S = [(-300, P['skirt_bottom']), (xbot, P['skirt_bottom']), (ax, az), (xr, az), (xr, 300), (-300, 300)]
    skirt = extrude(sec(P['skirt_bottom'], hw, hd), amount=z0 - P['skirt_bottom'] + 1) & xz_prism(S)
    return head + skirt


def dome_profile(N=240):
    """Centre-line (y = 0) profile of the dome from the front wall top over the crown to the back, as (x, z)."""
    hd, zc, Hd = P['hd'], P['wall_top'], P['top'] - P['wall_top']
    pts = []
    for i in range(N + 1):
        a = math.pi * i / N; c, s_ = math.cos(a), math.sin(a)
        m = P['dome_mx_f'] if c >= 0 else P['dome_mx_b']
        pts.append((hd * math.copysign(abs(c) ** (2 / m), c), zc + Hd * abs(s_) ** (2 / m)))
    return pts


# ----------------------------------------------------------------------------- eyes
def build_eyes(head):
    ro, ri, dep, ez, ch = P['eye_ro'], P['eye_ri'], P['eye_depth'], P['eye_z'], P['eye_chamfer']
    pockets = Part(); bezel = Part(); glow = Part()
    for s in (1, -1):
        y = s * P['eye_y']; xf = face_x(y)
        at = lambda x_: Location((x_, y, ez), (0, 90, 0))                      # axis along +X
        pockets += at(xf - dep) * Cylinder(ro, 40, align=(Align.CENTER, Align.CENTER, Align.MIN))
        # bevelled rim around the socket (bright ring outside the dark ring: ref4, ref11)
        pockets += at(xf - ch) * Cone(ro, ro + 8, 8, align=(Align.CENTER, Align.CENTER, Align.MIN))
        # dark bezel ring on the pocket floor + thin dark lining of the pocket wall
        bezel += at(xf - dep - 1.5) * (Cylinder(ro + 0.05, 3, align=(Align.CENTER, Align.CENTER, Align.MIN))
                                       - Cylinder(ri, 4, align=(Align.CENTER, Align.CENTER, Align.MIN)))
        lining = at(xf - dep) * (Cylinder(ro + 0.3, dep - ch + 0.2, align=(Align.CENTER, Align.CENTER, Align.MIN))
                                 - Cylinder(ro - 0.8, dep + 11, align=(Align.CENTER, Align.CENTER, Align.MIN)))
        bezel += lining & head
        # glowing disc: a shallow dome (the round display) sitting in the bezel
        disc = scale(Sphere(1), by=(2.2, ri, ri)) & Box(10, 100, 100, align=(Align.MIN, Align.CENTER, Align.CENTER))
        glow += disc.move(Location((xf - dep + 1.0, y, ez)))
    return pockets, bezel, glow


# ----------------------------------------------------------------------------- crest (mohawk)
def build_crest(head):
    hd, zc, top = P['hd'], P['wall_top'], P['top']
    prof = dome_profile()
    a_end = math.radians(P['crest_end_deg'])
    prof = prof[:int(len(prof) * a_end / math.pi) + 1]
    seg = [math.dist(prof[i], prof[i + 1]) for i in range(len(prof) - 1)]
    L1 = zc - P['crest_tip_z']; L2 = sum(seg); L = L1 + L2
    i90 = 240 // 2
    u_crown = (L1 + sum(seg[:i90])) / L

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

    secs = []
    U = [0, 0.03, 0.08, 0.15, 0.24, 0.34] + [0.34 + 0.66 * k / 24 for k in range(1, 25)]   # dense over the crown
    for u in U:
        pt, nrm, tan = frame(u)
        g = min(1.0, u / 0.42)                         # grows from a point to full section at the forehead
        u0 = u_crown * P['crest_fade_start']
        if u <= u0:
            k = 1.0
        else:                                           # over and behind the crown: shrink height AND width together
            k = max(0.06, (1 - (u - u0) / (1 - u0)) ** 0.7)   # so the ridge stays a crisp thin blade (ref8, ref11)
        h = max(0.5, P['crest_h'] * (0.15 + 0.85 * g ** 1.3) * k) if u > 0 else 0.5
        b = max(0.8, P['crest_b'] * (0.10 + 0.90 * g ** 0.6) * k)
        pl = Plane(origin=pt, x_dir=(0, 1, 0), z_dir=tan)
        out = 1.0 if pl.y_dir.dot(nrm) > 0 else -1.0
        sink = 6.0
        bb = b * (h + sink) / h                     # widen the buried base so the ridge is b wide AT the surface
        poly = [(-bb / 2, -sink * out), (bb / 2, -sink * out), (0.6, h * out), (-0.6, h * out)]
        secs.append(pl * make_face(Polyline(*poly, close=True)))
    blade = loft(secs)
    # spike at the crown: a fin in the XZ plane whose thickness tapers to a point
    tip = (-1.0, top + P['spike_h'])
    fin = xz_prism([(17.0, top - 12), (-14.0, top - 12), tip])
    taper = yz_prism([(-P['crest_b'] * 0.42, top - 12), (P['crest_b'] * 0.42, top - 12), (0, tip[1] + 1.5)])
    spike = fin & taper
    return (blade + spike) - head


# ----------------------------------------------------------------------------- jaw
def jaw_section(z, off):
    """Plan section of the jaw shell at height z, offset `off` outside the cranium plan; chin rake + scoop rounding."""
    lz, cz = P['ledge_z'], P['chin_z']
    f = (lz - z) / (lz - cz)                      # 0 at the ledge, 1 at the chin, <0 above the ledge
    fwd = P['chin_fwd'] * max(0.0, f)             # linear rake: the chin juts forward

    def rnd(r):
        if z >= cz + r:
            return 0.0
        d = (cz + r - z) / r
        return r * (1 - math.sqrt(max(0.0, 1 - min(1.0, d) ** 2)))
    shrink, shrink_f = rnd(P['scoop_r']), rnd(P['scoop_r_front'])
    return sec(z, P['hw'] + off - shrink, P['hd'] + off - shrink_f, P['hd'] + off - shrink, x0=fwd)


def band_profile():
    """Side outline (XZ) of the jaw band: ledge on top, vertical rear edge and a diagonal lower-rear cut
    (ref4, ref10, ref1)."""
    lz = P['ledge_z']; xr = P['jaw_rear_x']
    x1, z1, x2, z2 = P['jaw_diag']
    return xz_prism([(300, lz + 1), (xr, lz + 1), (xr, z1), (x2, z2), (x2, -5), (300, -5)])


def strap_profile():
    """Side outline (XZ) of the strap: a plate leaning back as it descends from the rounded pivot end to the band,
    its front edge meeting the ledge in a concave knee; its rear edge runs down to the band's rear edge
    (ref10, ref1, ref8)."""
    lz = P['ledge_z']; xr, xk = P['jaw_rear_x'], P['knee_x']
    hx, hz, r = P['hinge_x'], P['hinge_z'], P['strap_r']
    # tangent points: front edge from the knee to the circle, rear edge from (xr, lz) to the circle
    def tangent(px, pz, side):
        d = math.hypot(hx - px, hz - pz); a = math.atan2(hz - pz, hx - px); b = math.asin(r / d)
        ang = a + side * b                         # direction of the tangent line from the point
        L = math.sqrt(d * d - r * r)
        return (px + L * math.cos(ang), pz + L * math.sin(ang))
    tf = tangent(xk, lz - 3, -1); tb = tangent(xr, lz - 3, +1)
    af = math.degrees(math.atan2(tf[1] - hz, tf[0] - hx)); ab = math.degrees(math.atan2(tb[1] - hz, tb[0] - hx))
    if ab < af: ab += 360
    pts = [(xk, lz - 3)] + arc_pts(hx, hz, r, af, ab, 16) + [(xr, lz - 3)]   # arc ends are the tangent points
    return xz_prism(pts)


def build_jaw():
    lz, cz, t, gap = P['ledge_z'], P['chin_z'], P['jaw_wall'], P['jaw_gap']
    r = P['scoop_r']
    zs = [cz + 0.01, cz + 0.05 * r, cz + 0.15 * r, cz + 0.32 * r, cz + 0.55 * r, cz + 0.8 * r, cz + r]
    zs += [cz + r + (lz - cz - r) * k / 5 for k in (1, 2, 3, 4, 5)]
    outer = loft([jaw_section(z, gap + t) for z in zs])
    inner = loft([jaw_section(z, gap) for z in [cz - 0.5] + zs[1:]] + [jaw_section(lz + 3, gap)])
    # bottom: flat under the chin, rising gently toward the back (side view); shallow arc across (front view)
    hwj = P['hw'] + gap + t
    Ry = (hwj ** 2) / (2 * P['bottom_sag']) + P['bottom_sag'] / 2
    across = Cylinder(Ry, 400, rotation=(0, 90, 0)).move(Location((0, 0, cz + Ry)))
    xb = -(P['hd'] + 30)
    keep = xz_prism([(300, cz), (P['bottom_flat_x'], cz), (xb, P['bottom_back_z']), (xb, 400), (300, 400)])
    bottom = keep & across
    band = (outer & bottom) - (inner & bottom.moved(Location((0, 0, t))))
    band = band - Cylinder(P['neck_hole_r'], 100, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((-6, 0, -10)))
    band = band & band_profile()
    # strap: a plain shell hugging the head side, cut to the strap outline
    ztop = P['hinge_z'] + P['strap_r'] + 2
    ring = sec(lz - 3, P['hw'] + gap + t, P['hd'] + gap + t)
    hole = sec(lz - 3, P['hw'] + gap, P['hd'] + gap)
    strap = (extrude(ring, amount=ztop - (lz - 3)) - extrude(hole, amount=ztop - (lz - 3) + 2)) & strap_profile()
    # ear pad: the rounded end is thicker, standing proud of the head's widest point (ref2 tab, ref3 block)
    pad_ring = sec(P['hinge_z'] - 8, P['hw'] + gap + t + P['ear_pad'], P['hd'] + gap + t + P['ear_pad'])
    pad_hole = sec(P['hinge_z'] - 8, P['hw'] + gap + t - 1, P['hd'] + gap + t - 1)
    pad = (extrude(pad_ring, amount=ztop - P['hinge_z'] + 8) - extrude(pad_hole, amount=ztop - P['hinge_z'] + 10)) & strap_profile()
    pad = pad & Box(400, 400, 400, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, P['hinge_z'] - 6)))
    jaw = band + strap + pad
    # pivot bosses on the strap ends: disc, raised step, recessed centre (ref1, ref8, ref10)
    hinge = Part()
    hx, hz = P['hinge_x'], P['hinge_z']
    for s in (1, -1):
        ys = side_y(hx, P['hw'] + gap + t + P['ear_pad'], P['hd'] + gap + t + P['ear_pad']) - 0.5   # pad outer face
        disc = Location((hx, s * (ys + 1.5), hz), (90, 0, 0)) * Cylinder(P['boss_r'], 6)          # 3 mm proud + 3 embedded
        step = Location((hx, s * (ys + 4.5), hz), (90, 0, 0)) * Cylinder(P['boss_step_r'], 3)   # +1.5 raised step
        pin = Location((hx, s * (ys + 4.5), hz), (90, 0, 0)) * Cylinder(P['boss_pin_r'], 4)     # recessed centre
        hinge += (disc + step) - pin
    return jaw, hinge


# ----------------------------------------------------------------------------- collar, dark parts
def build_collar():
    c = Cylinder(P['collar_r1'], P['collar_h1'], align=(Align.CENTER, Align.CENTER, Align.MIN))
    c += Cylinder(P['collar_r2'], P['collar_h2'], align=(Align.CENTER, Align.CENTER, Align.MIN)).move(
        Location((0, 0, P['collar_h1'])))
    return c


def build_dark():
    gap = P['jaw_gap']
    core = loft([jaw_section(z, gap - 2.5) for z in (P['chin_z'] + 2, P['chin_z'] + 14, P['ledge_z'] - 2)])
    core = core & Box(400, 400, P['ledge_z'] - 2 - 8, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, 8)))
    # ribbed dark neck between the collar and the jaw (ref9, ref7)
    neck = Cylinder(P['neck_r'], 50, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((-6, 0, -6)))
    for zr in (19.5, 25.0, 30.5):
        neck += Cylinder(P['neck_r'] + 2.2, 2.5, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((-6, 0, zr)))
    return core, neck


# ----------------------------------------------------------------------------- assembly
def build():
    head = build_cranium()
    pockets, bezel, glow = build_eyes(head)
    head = head - pockets
    crest = build_crest(head)
    jaw, hinge = build_jaw()
    collar = build_collar()
    core, neck = build_dark()
    return dict(head=head, crest=crest, bezel=bezel, glow=glow, jaw=jaw, hinge=hinge,
                collar=collar, core=core, neck=neck)


if __name__ == '__main__':
    here = os.path.dirname(os.path.abspath(__file__))
    out = os.path.join(here, 'out'); os.makedirs(out, exist_ok=True)
    parts = build(); man = []
    for k, sol in parts.items():
        if sol is None or sol.volume < 1:
            print('empty part', k); continue
        tol = (0.05, 0.1) if k in ('crest', 'glow') else (0.02, 0.04)   # coarser mesh on the two heavy lofts
        export_stl(sol, os.path.join(out, f'{k}.stl'), tolerance=tol[0], angular_tolerance=tol[1])
        man.append(dict(file=f'{k}.stl', mat=MATS[k]))
    try:
        export_step(Compound([s for s in parts.values() if s.volume >= 1]), os.path.join(out, 'head.step'))
    except Exception as e:
        print('step export failed:', e)
    json.dump(man, open(os.path.join(out, 'parts.json'), 'w'), indent=1)
    bb = parts['head'].bounding_box(); jb = parts['jaw'].bounding_box()
    print('cranium W x D x Z:', round(bb.size.Y, 1), round(bb.size.X, 1), round(bb.min.Z, 1), round(bb.max.Z, 1))
    print('jaw W x D, z range:', round(jb.size.Y, 1), round(jb.size.X, 1), round(jb.min.Z, 1), round(jb.max.Z, 1))
    print([m['file'] for m in man])
