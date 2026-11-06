"""Giant-reference head, pass 4 (2026-11-06): proportions taken from Joey's three reference frames
(front close-up, side poster, film still). Robot frame X fwd, Y left, Z up, mm; z = 0 = bottom of the collar.
Changes from pass 3: rounded-square (superellipse) plan instead of an ellipse, flat-ish face and straight sides;
the jaw is a THIN plate (4 mm) that wraps the lower face with a ledge at the top and a raked chin, pivoting on a
side boss; the crest is a smooth lofted blade that starts as a point just above the jaw and grows into a spike
at the crown. Colours: white head, very light grey jaw, darker grey crest (Joey 11:21-11:25Z)."""
import json, math, os, sys
from build123d import *


def se_pts(hw, hd, n, x0=0.0, N=64):
    pts = []
    for i in range(N):
        a = 2 * math.pi * i / N; c, s = math.cos(a), math.sin(a)
        pts.append((x0 + hd * math.copysign(abs(c) ** (2 / n), c), hw * math.copysign(abs(s) ** (2 / n), s)))
    return pts


def sec(z, hw, hd, n, x0=0.0):
    return make_face(Wire([Spline(*[(x, y, z) for x, y in se_pts(hw, hd, n, x0)], periodic=True)]))


def se_x(y, hw, hd, n):  # front surface x at lateral position y
    return hd * max(0.0, 1 - abs(y / hw) ** n) ** (1 / n)


def xz_prism(pts, half=150):
    return extrude(make_face(Plane.XZ * Polyline(*pts, close=True)), amount=half, both=True)


def cranium(W, D, n, z0, zc, top, grow=0.0, m=2.0):
    W, D, Hd = W + grow, D + grow, top - zc + grow
    S = [sec(zc, W, D, n)]
    if m == 2.0:
        for t in range(12, 90, 12):
            r = math.cos(math.radians(t)); S.append(sec(zc + Hd * math.sin(math.radians(t)), W * r, D * r, n))
        dome = loft(S + [Vertex(0, 0, zc + Hd)])
    else:  # flatter crown: r = (1 - f^m)^(1/m), closed by a small flat top
        for f in (0.2, 0.4, 0.55, 0.7, 0.8, 0.88, 0.94, 0.98, 1.0):
            r = max(0.18, (1 - f ** m) ** (1 / m)); S.append(sec(zc + Hd * f, W * r, D * r, n))
        dome = loft(S)
    return extrude(sec(z0, W, D, n), amount=zc - z0) + dome  # straight walls + dome


def dome_path(D, Hd, zc, m, N=400):
    """Centre-line (y=0) front profile over the dome: list of (x, z) from the front wall top to the back."""
    pts = []
    for i in range(N + 1):
        a = math.pi * i / N  # 0 = front, pi = back
        c, s_ = math.cos(a), math.sin(a)
        pts.append((D * math.copysign(abs(c) ** (2 / m), c), zc + Hd * abs(s_) ** (2 / m)))
    return pts


def build(v):
    p = dict(W=64, D=66, n=3.4, top=212, zc=150, eye_z=146, eye_y=30, eye_ro=21, eye_r=14.5, eye_dep=6,
             jaw_top=93, jaw_bot=30, jaw_t=4, jaw_gap=1.5, jaw_ledge=3, chin_back=12, jaw_taper=5,
             hinge_r=14, hinge_back=0.52, chin_drop=0, chin_fwd=6, chin_flat=0.35, chin_chamfer=0, crest_tip=99, crest_h=13, crest_b=16, crest_end=100,
             ear_z=150, ear_x=-4, dome_m=2.0, jaw_facet=0, collar_r=58)
    p.update(v)
    W, D, n, top, zc = p['W'], p['D'], p['n'], p['top'], p['zc']
    jt, jb, t, gap = p['jaw_top'], p['jaw_bot'], p['jaw_t'], p['jaw_gap']
    head = cranium(W, D, n, jt - 5, zc, top, m=p['dome_m'])  # face ends just under the jaw ledge; dark core below
    hr_ = p['hinge_r']; hx_ = -D * p['hinge_back'] + hr_
    head = head + (extrude(sec(jb + 6, W, D, n), amount=jt - jb - 5) & Box(300, 300, 300, align=(Align.MAX, Align.CENTER, Align.CENTER)).move(Location((hx_ - hr_ - 1.5, 0, 100))))  # back skirt behind the jaw hinge
    core = extrude(sec(jb + 3, W - 12, D - 24, n, x0=-8), amount=jt - jb - 3)  # dark inner face / neck housing seen under and behind the jaw
    # eyes: round pocket, dark ring at the bottom, glowing disc (round display) inside it
    sockets = Part(); bez = Part(); glow = Part()
    ro, ri, dep = p['eye_ro'], p['eye_r'], p['eye_dep']
    for s in (1, -1):
        y = s * p['eye_y']; xf = se_x(y, W, D, n)
        ax = lambda x_: Location((x_, y, p['eye_z']), (0, 90, 0))
        sockets += ax(xf - dep) * Cylinder(ro, 40, align=(Align.CENTER, Align.CENTER, Align.MIN))
        bez += ax(xf - dep - 1) * (Cylinder(ro + 0.01, 3, align=(Align.CENTER, Align.CENTER, Align.MIN)) - Cylinder(ri, 4, align=(Align.CENTER, Align.CENTER, Align.MIN)))
        bez += (ax(xf - dep) * (Cylinder(ro + 0.4, dep + 8, align=(Align.CENTER, Align.CENTER, Align.MIN)) - Cylinder(ro - 0.6, dep + 9, align=(Align.CENTER, Align.CENTER, Align.MIN)))) & head
        disc = scale(Sphere(1), by=(2.5, ri, ri)) & Box(10, 100, 100, align=(Align.MIN, Align.CENTER, Align.CENTER))
        glow += disc.move(Location((xf - dep + 1.2, y, p['eye_z'])))
    head = head - sockets
    # crest: trapezoid sections lofted along the centre line, point just above the jaw, spike at the crown
    Hd = top - zc
    path = dome_path(D, Hd, zc, p['dome_m'])
    a_end = math.radians(p['crest_end']); path = path[:int(len(path) * a_end / math.pi) + 1]
    seg = [((path[i + 1][0] - path[i][0]) ** 2 + (path[i + 1][1] - path[i][1]) ** 2) ** 0.5 for i in range(len(path) - 1)]
    L1 = zc - p['crest_tip']; L2 = sum(seg)
    def cl(u):  # centre-line point, outward normal, tangent; u in [0,1] from the tip up and over the crown
        s_ = u * (L1 + L2)
        if s_ <= L1:
            return Vector(D, 0, p['crest_tip'] + s_), Vector(1, 0, 0), Vector(0, 0, 1)
        s_ -= L1; i = 0
        while i < len(seg) - 1 and s_ > seg[i]:
            s_ -= seg[i]; i += 1
        (x0, z0_), (x1, z1) = path[i], path[i + 1]; f = min(1.0, s_ / max(seg[i], 1e-9))
        tan = Vector(x1 - x0, 0, z1 - z0_).normalized()
        return Vector(x0 + (x1 - x0) * f, 0, z0_ + (z1 - z0_) * f), Vector(tan.Z, 0, -tan.X), tan
    secs = []
    U = [0, 0.03, 0.08, 0.16, 0.26, 0.38, 0.5, 0.6, 0.68, 0.76, 0.84, 0.91, 0.96, 1.0]
    for u in U:
        pt, nrm, tan = cl(u)
        g = min(1.0, u / 0.7)                       # grows to full size at the crown
        fade = 1.0 if u < 0.8 else max(0.05, 1 - (u - 0.8) / 0.2)  # sweeps back and fades behind the crown
        h = max(0.4, p['crest_h'] * (0.25 + 0.75 * g ** 1.4) * fade if u > 0 else 0.4)
        b = max(0.6, p['crest_b'] * (g ** 0.5) * (0.6 + 0.4 * fade))
        pl = Plane(origin=pt, x_dir=(0, 1, 0), z_dir=tan)
        out = pl.y_dir.dot(nrm)  # sign of the local y axis relative to the outward normal
        sink = 6.0
        poly = [(-b / 2, -sink * out), (b / 2, -sink * out), (b * 0.3, h * out), (-b * 0.3, h * out)]
        secs.append(pl * make_face(Polyline(*poly, close=True)))
    crest = loft(secs) - head
    # jaw: thin plate wrapping the lower face; ledge at the top. With chin_drop the front hangs lower than the hinge
    # (in front of the neck, like the side poster) and the chin juts forward instead of raking back.
    jbf = jb - p['chin_drop']                     # chin bottom (front); the hinge end keeps jb
    def jsec(z, off):
        f = (jt - z) / (jt - jbf)
        fwd = p['jaw_ledge'] * (1 - f) + (p['chin_fwd'] if p['chin_drop'] else -p['chin_back']) * f
        tw = p['jaw_taper'] * f
        if p['jaw_facet']:  # flat front, flat sides, angled corner facets (plan view)
            hw, hd, c = W + off - tw, D + off - tw, p['jaw_facet']
            pts = [(fwd + hd, -hw + c), (fwd + hd, hw - c), (fwd + hd - c, hw), (-hd, hw), (-hd, -hw), (fwd + hd - c, -hw)]
            return make_face(Polyline(*[(x_, y_, z) for x_, y_ in pts], close=True))
        return sec(z, W + off - tw, D + off - tw, n, x0=fwd)
    outer = loft([jsec(jbf, gap + t), jsec(jt, gap + t)])
    inner = loft([jsec(jbf - 0.01, gap), jsec(jt + 0.01, gap)])
    inner = extrude(inner.faces().sort_by(Axis.Z)[-1], amount=2) + inner + extrude(inner.faces().sort_by(Axis.Z)[0], amount=-2)
    hr = p['hinge_r']; hx = -D * p['hinge_back'] + hr; hz = jb + hr + 2
    def profile(lift):
        pr = [(300, jt + 0.5), (hx - hr, jt + 0.5)]
        for a in range(180, 271, 10):
            pr.append((hx + hr * math.cos(math.radians(a)), hz + hr * math.sin(math.radians(a)) + lift))
        xc = D * p['chin_flat']                   # flat chin underside from here forward
        pr += [(hx, jb - 1 + lift), (xc, jbf - 1 + lift), (300, jbf - 1 + lift)]
        return xz_prism(pr)
    side = profile(0)
    jaw = (outer & side) - (inner & profile(t))  # open at the back/top, closed underneath (sloped floor)
    if p['chin_chamfer']:  # bevel the front-bottom edge of the chin
        c = p['chin_chamfer']; xf = D + gap + t + p['chin_fwd'] + 2
        jaw = jaw - xz_prism([(xf - c, jbf - 2), (xf + 30, jbf - 2), (xf + 30, jbf + c + 30)])
    ears = Part()
    for s in (1, -1):
        ys = se_x(hx, D, W, n)  # half-width at x = hx (swap roles: y as a function of x)
        ears += Location((hx, s * (ys + gap + t), hz), (90, 0, 0)) * Cylinder(hr - 4, 4)
        ears += Location((hx, s * (ys + gap + t + 2.5), hz), (90, 0, 0)) * (Cylinder(5.5, 3) - Cylinder(3.2, 4))
        ye = se_x(p['ear_x'], D, W, n)  # ear block at the top of the eyes: a base plate and a narrower stepped tab
        ears += Box(14, 8, 26).move(Location((p['ear_x'], s * (ye + 2), p['ear_z'])))
        ears += Box(8, 7, 16).move(Location((p['ear_x'], s * (ye + 7), p['ear_z'] + 2)))
    collar = Cylinder(p['collar_r'], 7, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, 4))) + Cylinder(p['collar_r'] - 10, 10, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, 11)))
    neck = Cylinder(36, 40, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, -20)))
    return dict(core=core, head=head, crest=crest, bezel=bez, glow=glow, jaw=jaw, ears=ears, collar=collar, neck=neck)


MATS = dict(core='dark', head='white', crest='grey2', bezel='dark', glow='eye', jaw='grey1', ears='white', collar='white', neck='dark')
VARIANTS = {'G1': dict(), 'G2': dict(chin_drop=34, chin_fwd=16, chin_chamfer=12, jaw_taper=6, chin_flat=0.5),  # G2 rejected: jaw got bigger
            'G3': dict(jaw_top=68, crest_tip=74, hinge_r=11, chin_drop=6, chin_fwd=7, chin_chamfer=5, jaw_taper=5, chin_flat=0.45),
            'G4': dict(W=64, D=64, top=180, zc=144, dome_m=2.6, eye_z=114, eye_y=34, eye_ro=21, eye_r=15, jaw_bot=26, jaw_top=70, crest_tip=76, crest_end=95,
                       hinge_r=10, chin_drop=3, chin_fwd=3, chin_chamfer=5, jaw_taper=3, chin_flat=0.45, jaw_facet=16, jaw_ledge=3, ear_z=124, ear_x=-2, collar_r=74)}
VARIANTS['G5'] = dict(VARIANTS['G4'], top=205, zc=169)  # G4 with the top of the head stretched 25 mm (Joey 11:50Z)
VARIANTS['G6'] = dict(VARIANTS['G5'], jaw_facet=0)  # rounded jaw, no flats (Joey 11:53Z)  # G3: smaller jaw, top ~25 mm lower, small chin (Joey 11:42Z)

if __name__ == '__main__':
    for nm in (sys.argv[1:] or list(VARIANTS)):
        parts = build(VARIANTS[nm]); os.makedirs(f'out/{nm}', exist_ok=True); man = []
        for k_, sol in parts.items():
            if sol is None or sol.volume < 1: print('empty', k_); continue
            export_stl(sol, f'out/{nm}/{k_}.stl', tolerance=0.02, angular_tolerance=0.04); man.append(dict(file=f'{nm}/{k_}.stl', mat=MATS[k_]))
        export_step(Compound([s for s in parts.values() if s.volume >= 1]), f'out/{nm}/head.step')
        json.dump(man, open(f'out/{nm}/parts.json', 'w'), indent=1); print(nm, [m['file'] for m in man])
