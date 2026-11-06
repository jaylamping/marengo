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


def cranium(W, D, n, z0, zc, top, grow=0.0):
    W, D, Hd = W + grow, D + grow, top - zc + grow
    S = [sec(zc, W, D, n)]
    for t in range(12, 90, 12):
        r = math.cos(math.radians(t)); S.append(sec(zc + Hd * math.sin(math.radians(t)), W * r, D * r, n))
    return extrude(sec(z0, W, D, n), amount=zc - z0) + loft(S + [Vertex(0, 0, zc + Hd)])  # straight walls + dome


def build(v):
    p = dict(W=64, D=66, n=3.4, top=212, zc=150, eye_z=146, eye_y=30, eye_ro=21, eye_r=14.5, eye_dep=6,
             jaw_top=93, jaw_bot=30, jaw_t=4, jaw_gap=1.5, jaw_ledge=3, chin_back=12, jaw_taper=5,
             hinge_r=14, hinge_back=0.52, crest_tip=99, crest_h=13, crest_b=16, crest_end=100,
             ear_z=150, ear_x=-4)
    p.update(v)
    W, D, n, top, zc = p['W'], p['D'], p['n'], p['top'], p['zc']
    jt, jb, t, gap = p['jaw_top'], p['jaw_bot'], p['jaw_t'], p['jaw_gap']
    head = cranium(W, D, n, jt - 5, zc, top)  # face ends just under the jaw ledge; dark core below
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
    def cl(u):  # centre-line point, outward normal, tangent; u in [0,1] from tip up and over to the back end
        z_tip = p['crest_tip']; a_end = math.radians(p['crest_end'])
        L1 = zc - z_tip; L2 = Hd * a_end
        s_ = u * (L1 + L2)
        if s_ <= L1:
            return Vector(D, 0, z_tip + s_), Vector(1, 0, 0), Vector(0, 0, 1)
        a = (s_ - L1) / Hd  # angle over the dome (approx: elliptic arc treated as circular)
        pt = Vector(D * math.cos(a), 0, zc + Hd * math.sin(a))
        nrm = Vector(math.cos(a) / D, 0, math.sin(a) / Hd).normalized()
        tan = Vector(-D * math.sin(a), 0, Hd * math.cos(a)).normalized()
        return pt, nrm, tan
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
    # jaw: thin plate wrapping the lower face; ledge at the top, chin raked back, narrows slightly at the bottom
    def jsec(z, off):
        f = (jt - z) / (jt - jb)
        fwd = p['jaw_ledge'] * (1 - f) - p['chin_back'] * f
        tw = p['jaw_taper'] * f
        return sec(z, W + off - tw, D + off - tw, n, x0=fwd)
    outer = loft([jsec(jb, gap + t), jsec(jt, gap + t)])
    inner = loft([jsec(jb - 0.01, gap), jsec(jt + 0.01, gap)])
    inner = extrude(inner.faces().sort_by(Axis.Z)[-1], amount=2) + inner + extrude(inner.faces().sort_by(Axis.Z)[0], amount=-2)
    plate = outer - inner
    under = loft([jsec(jb, gap + t - 0.3), jsec(jb + t, gap + t - 0.3)])  # underside panel closes the jaw from below
    hr = p['hinge_r']; hx = -D * p['hinge_back'] + hr; hz = jb + hr + 2
    prof = [(300, jt + 0.5), (hx - hr, jt + 0.5)]
    for a in range(180, 271, 10):
        prof.append((hx + hr * math.cos(math.radians(a)), hz + hr * math.sin(math.radians(a))))
    prof += [(hx, jb - 1), (300, jb - 1)]
    side = xz_prism(prof)
    jaw = (plate + under) & side
    ears = Part()
    for s in (1, -1):
        ys = se_x(hx, D, W, n)  # half-width at x = hx (swap roles: y as a function of x)
        ears += Location((hx, s * (ys + gap + t), hz), (90, 0, 0)) * Cylinder(hr - 4, 4)
        ears += Location((hx, s * (ys + gap + t + 2.5), hz), (90, 0, 0)) * (Cylinder(5.5, 3) - Cylinder(3.2, 4))
        ye = se_x(p['ear_x'], D, W, n)  # ear block at the top of the eyes: a base plate and a narrower stepped tab
        ears += Box(14, 8, 26).move(Location((p['ear_x'], s * (ye + 2), p['ear_z'])))
        ears += Box(8, 7, 16).move(Location((p['ear_x'], s * (ye + 7), p['ear_z'] + 2)))
    collar = Cylinder(58, 7, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, 4))) + Cylinder(48, 10, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, 11)))
    neck = Cylinder(36, 40, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, -20)))
    return dict(core=core, head=head, crest=crest, bezel=bez, glow=glow, jaw=jaw, ears=ears, collar=collar, neck=neck)


MATS = dict(core='dark', head='white', crest='grey2', bezel='dark', glow='eye', jaw='grey1', ears='white', collar='white', neck='dark')
VARIANTS = {'G1': dict()}

if __name__ == '__main__':
    for nm in (sys.argv[1:] or list(VARIANTS)):
        parts = build(VARIANTS[nm]); os.makedirs(f'out/{nm}', exist_ok=True); man = []
        for k_, sol in parts.items():
            if sol is None or sol.volume < 1: print('empty', k_); continue
            export_stl(sol, f'out/{nm}/{k_}.stl', tolerance=0.02, angular_tolerance=0.04); man.append(dict(file=f'{nm}/{k_}.stl', mat=MATS[k_]))
        export_step(Compound([s for s in parts.values() if s.volume >= 1]), f'out/{nm}/head.step')
        json.dump(man, open(f'out/{nm}/parts.json', 'w'), indent=1); print(nm, [m['file'] for m in man])
