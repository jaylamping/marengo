"""marengo head, Iron Giant bake-off entry "fable" (build123d 0.13).

Robot frame: X forward, Y left, Z up, mm.  z = 0 is the bottom of the neck collar.
Proportions measured from ref2 (front close-up) / ref6 (poster) and ref1/ref3/ref4 for the side, plan and jaw;
see NOTES.md.  Run:  python3 build_head.py   ->  out/*.stl, out/parts.json, out/head.step
"""
import json, math, os
from build123d import *

# ----------------------------------------------------------------------------- parameters (mm)
P = dict(
    # cranium: rounded-square plan, straight walls, elliptical dome
    hw=62.5, hd=62.0,            # half width (Y) / half depth (X) of the cranium
    n_front=4.2, n_back=3.0,     # superellipse exponents (front half / back half of the plan)
    wall_top=140.0, top=196.0,   # straight walls end at wall_top; dome apex at top
    dome_m=1.65,                 # dome profile r = (1 - f^m)^(1/m): 2 = ellipse, <2 = pointed arch
    dome_n_top=2.3,              # the plan relaxes from n_front/n_back to this (rounder) at the apex
    face_bottom=56.0,            # the cranium shell continues down inside the jaw to here
    # eyes
    eye_z=114.0, eye_y=29.5, eye_ro=20.5, eye_ri=15.2, eye_depth=6.0,
    # jaw (a thin plate wrapping the lower face, open at the top, ledge at the top edge)
    ledge_z=64.0, chin_z=22.0, jaw_wall=4.0, jaw_gap=1.5,
    chin_fwd=11.0,                # the jaw front rakes forward going down: this much at the chin
    scoop_r=16.0,                # radius of the rounded lower edge (scoop) at the sides
    scoop_r_front=11.0,          # tighter round at the chin so it keeps a crisp forward edge
    scoop_back=1.0,              # the lower edge rounds in more at the back (x < 0)
    bottom_sag=8.0,              # front view: jaw bottom is a shallow arc, this much lower at the centre
    bottom_flat_x=15.0,          # side view: bottom is flat (chin) ahead of this x ...
    bottom_back_z=40.0,          # ... and rises in a straight line to this z at the back of the head
    hinge_x=-32.0, hinge_z=51.0, hinge_r=11.0,
    # crest / mohawk
    crest_tip_z=73.0, crest_h=11.0, crest_b=17.0, crest_end_deg=112.0, spike_h=18.0,
    # ears, collar
    ear_z=117.0, ear_x=0.0,
    collar_r1=75.0, collar_r2=68.0, collar_h1=9.0, collar_h2=7.0,
)
MATS = dict(head='white', ears='white', collar='white', jaw='grey1', hinge='grey1', crest='grey2',
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


# ----------------------------------------------------------------------------- cranium
def build_cranium():
    hw, hd, z0, zc, top = P['hw'], P['hd'], P['face_bottom'], P['wall_top'], P['top']
    Hd = top - zc
    # one loft from the bottom of the face over the crown: straight walls, then a superellipse arch whose
    # rounded-square plan relaxes toward a circle at the apex (keeps the 3/4 view a full dome, not a cone)
    walls = extrude(sec(z0, hw, hd), amount=zc - z0)
    secs = [sec(zc - 10, hw, hd), sec(zc, hw, hd)]   # ghost section below the junction: loft leaves vertical
    m, nt = P['dome_m'], P['dome_n_top']
    for f in (0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.78, 0.85, 0.91, 0.955, 0.985):
        r = (1 - f ** m) ** (1 / m)         # superellipse arch: pointed for m < 2
        k = f ** 1.0
        nf = P['n_front'] + (nt - P['n_front']) * k; nb = P['n_back'] + (nt - P['n_back']) * k
        secs.append(sec(zc + Hd * f, hw * r, hd * r, n_f=nf, n_b=nb))
    return walls + loft(secs + [Vertex(0, 0, top)])


def dome_profile(N=240):
    """Centre-line (y = 0) profile of the dome from the front wall top over the crown to the back, as (x, z)."""
    hd, zc, Hd, m = P['hd'], P['wall_top'], P['top'] - P['wall_top'], P['dome_m']
    pts = []
    for i in range(N + 1):
        a = math.pi * i / N; c, s_ = math.cos(a), math.sin(a)
        pts.append((hd * math.copysign(abs(c) ** (2 / m), c), zc + Hd * abs(s_) ** (2 / m)))
    return pts


# ----------------------------------------------------------------------------- eyes
def build_eyes(head):
    ro, ri, dep, ez = P['eye_ro'], P['eye_ri'], P['eye_depth'], P['eye_z']
    pockets = Part(); bezel = Part(); glow = Part()
    for s in (1, -1):
        y = s * P['eye_y']; xf = face_x(y)
        at = lambda x_: Location((x_, y, ez), (0, 90, 0))                      # cylinder axis along +X
        pockets += at(xf - dep) * Cylinder(ro, 40, align=(Align.CENTER, Align.CENTER, Align.MIN))
        # dark bezel: ring on the pocket floor + thin dark lining of the pocket wall
        bezel += at(xf - dep - 1.5) * (Cylinder(ro + 0.05, 3, align=(Align.CENTER, Align.CENTER, Align.MIN))
                                       - Cylinder(ri, 4, align=(Align.CENTER, Align.CENTER, Align.MIN)))
        lining = at(xf - dep) * (Cylinder(ro + 0.3, dep + 10, align=(Align.CENTER, Align.CENTER, Align.MIN))
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

    u_crown = (L1 + sum(seg[:len(seg) // 2 * 90 // 90]) ) / L  # approx; crown is at angle 90 deg
    # locate crown u exactly: angle 90deg corresponds to index N/2 of the full profile
    n_full = 240; i90 = n_full // 2
    u_crown = (L1 + sum(seg[:i90])) / L
    secs = []
    U = [0, 0.03, 0.08, 0.15, 0.24, 0.34, 0.44, 0.54, 0.62, 0.70, 0.78, 0.85, 0.92, 0.97, 1.0]
    for u in U:
        pt, nrm, tan = frame(u)
        g = min(1.0, u / 0.42)                         # grows from a point to full section at the forehead
        if u <= u_crown:
            fade = 1.0
        else:
            fade = max(0.08, 1 - (u - u_crown) / (1 - u_crown))   # thins out behind the crown
        h = max(0.5, P['crest_h'] * (0.15 + 0.85 * g ** 1.3) * fade) if u > 0 else 0.5
        b = max(0.8, P['crest_b'] * (0.10 + 0.90 * g ** 0.6) * (0.5 + 0.5 * fade))
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
    """Plan section of the jaw at height z, offset `off` outside the cranium plan; chin bulge + scoop rounding."""
    lz, cz = P['ledge_z'], P['chin_z']
    f = (lz - z) / (lz - cz)                      # 0 at the ledge, 1 at the chin
    fwd = P['chin_fwd'] * max(0.0, f)             # linear rake: the chin juts forward
    def rnd(r):
        if z >= cz + r:
            return 0.0
        d = (cz + r - z) / r
        return r * (1 - math.sqrt(max(0.0, 1 - min(1.0, d) ** 2)))
    shrink, shrink_f = rnd(P['scoop_r']), rnd(P['scoop_r_front'])
    hw = P['hw'] + off - shrink
    hd_f = P['hd'] + off - shrink_f
    hd_b = P['hd'] + off - shrink * P['scoop_back']
    return sec(z, hw, hd_f, hd_b, x0=fwd)


def build_jaw():
    lz, cz, t, gap = P['ledge_z'], P['chin_z'], P['jaw_wall'], P['jaw_gap']
    r = P['scoop_r']
    zs = [cz + 0.01, cz + 0.05 * r, cz + 0.15 * r, cz + 0.32 * r, cz + 0.55 * r, cz + 0.8 * r, cz + r]
    zs += [cz + r + (lz - cz - r) * k / 5 for k in (1, 2, 3, 4, 5)]
    outer = loft([jaw_section(z, gap + t) for z in zs])
    inner = loft([jaw_section(z, gap) for z in [cz - 0.5] + zs[1:]] + [jaw_section(lz + 3, gap)])
    # bottom: flat under the chin, rising in a straight line toward the back (side view); shallow arc across
    hwj = P['hw'] + gap + t
    Ry = (hwj ** 2) / (2 * P['bottom_sag']) + P['bottom_sag'] / 2
    across = Cylinder(Ry, 400, rotation=(0, 90, 0)).move(Location((0, 0, cz + Ry)))
    xb = -(P['hd'] + 30)
    keep = xz_prism([(300, cz), (P['bottom_flat_x'], cz), (xb, P['bottom_back_z']), (xb, 400), (300, 400)])
    bottom = keep & across
    jaw = (outer & bottom) - (inner & bottom.moved(Location((0, 0, t))))
    # neck hole through the floor plate
    jaw = jaw - Cylinder(40, 100, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((-6, 0, -10)))
    # hinge bosses on each side toward the back
    hinge = Part()
    hz, hx, hr = P['hinge_z'], P['hinge_x'], P['hinge_r']
    for s in (1, -1):
        ys = side_y(hx, P['hw'] + gap + t - 0.5, P['hd'] + gap + t)
        base = Location((hx, s * ys, hz), (90, 0, 0))
        hinge += base * Cylinder(hr, 8)                                             # disc, 4 mm proud
        hinge += Location((hx, s * (ys + 4), hz), (90, 0, 0)) * (Cylinder(hr * 0.6, 4) - Cylinder(hr * 0.32, 5))
        hinge += Location((hx, s * (ys + 4), hz), (90, 0, 0)) * Cylinder(hr * 0.16, 4.5)
    return jaw, hinge


# ----------------------------------------------------------------------------- ears, collar, dark parts
def build_ears():
    ears = Part()
    for s in (1, -1):
        ye = side_y(P['ear_x'])
        ears += Box(17, 9, 27).move(Location((P['ear_x'], s * (ye + 2.5), P['ear_z'])))     # base plate
        ears += Box(12, 6, 18).move(Location((P['ear_x'], s * (ye + 8.0), P['ear_z'] + 1)))   # stepped tab
    return ears


def build_collar():
    c = Cylinder(P['collar_r1'], P['collar_h1'], align=(Align.CENTER, Align.CENTER, Align.MIN))
    c += Cylinder(P['collar_r2'], P['collar_h2'], align=(Align.CENTER, Align.CENTER, Align.MIN)).move(
        Location((0, 0, P['collar_h1'])))
    return c


def build_dark():
    gap = P['jaw_gap']
    core = loft([jaw_section(z, gap - 2.5) for z in (P['chin_z'] + 2, P['chin_z'] + 14, P['ledge_z'] - 2)])
    core = core & Box(400, 400, P['ledge_z'] - 2 - 8, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, 8)))
    neck = Cylinder(38, 50, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((-4, 0, -6)))
    return core, neck


# ----------------------------------------------------------------------------- assembly
def build():
    head = build_cranium()
    pockets, bezel, glow = build_eyes(head)
    head = head - pockets
    crest = build_crest(head)
    jaw, hinge = build_jaw()
    ears = build_ears()
    collar = build_collar()
    core, neck = build_dark()
    return dict(head=head, crest=crest, bezel=bezel, glow=glow, jaw=jaw, hinge=hinge, ears=ears,
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
    print('cranium W x D x Z:', round(bb.size.Y, 1), round(bb.size.X, 1), round(bb.max.Z, 1))
    print('jaw W x D, z range:', round(jb.size.Y, 1), round(jb.size.X, 1), round(jb.min.Z, 1), round(jb.max.Z, 1))
    print([m['file'] for m in man])
