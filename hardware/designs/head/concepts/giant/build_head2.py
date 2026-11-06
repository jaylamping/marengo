"""Giant-inspired head, pass 2 (2026-11-06): proportions from Joey's side and front references.
Original restyle for marengo, not a replica. Robot frame X fwd, Y left, Z up, mm; z = 0 = bottom of the collar.
Key changes from pass 1: the jaw is a separate scoop plate that wraps the lower face and pivots on a side boss;
crest is a tapered blade that starts as a point between the eyes; bigger bevelled eye sockets; small side ear
blocks at eye height; the head sits straight on a stepped collar ring (no visible neck)."""
import json, os, sys
from build123d import *

def ell(w, d, z0, z1, x0=0):
    return extrude(Ellipse(w, d).rotate(Axis.Z, 90), amount=z1 - z0).move(Location((x0, 0, z0)))

def cranium(W, D, z0, zc, top):
    cap = scale(Sphere(1), by=(D, W, top - zc)).move(Location((0, 0, zc)))
    cap = cap & Box(400, 400, 400, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, zc)))
    return ell(W, D, z0, zc) + cap

def xz_prism(pts, half=150):
    return extrude(make_face(Plane.XZ * Polyline(*pts, close=True)), amount=half, both=True)

def build(v):
    p = dict(bands=3, jaw_taper_w=8, jaw_taper_d=8, chin_slope=0.3, W=60, D=64, z0=26, zc=150, top=214, eye_r=21, eye_y=31, eye_z=136, crest_k=0.55, crest_h=14, crest_t=6, crest_base=13,
             jaw_top=104, jaw_bot=26, jaw_t=8, jaw_fwd=14, hinge_x=-26, hinge_z=80, hinge_r=21, ear=0)
    p.update(v)
    W, D = p['W'], p['D']
    head = cranium(W, D, p['jaw_top'] - 6, p['zc'], p['top'])
    core = ell(W - 12, D - 18, p['z0'], p['jaw_top'] - 5)
    # eye sockets: round pocket with a bevelled (conical) lip
    sockets = Part(); bez = Part(); lens = Part(); glow = Part()
    for s in (1, -1):
        y = s * p['eye_y']; xf = D * (1 - (y / W) ** 2) ** 0.5
        loc = Location((xf - 12, y, p['eye_z']), (0, 90, 0))
        sockets += loc * Cylinder(p['eye_r'] + 2, 40, align=(Align.CENTER, Align.CENTER, Align.MIN))
        sockets += Location((xf - 4, y, p['eye_z']), (0, 90, 0)) * Cone(p['eye_r'] + 2, p['eye_r'] + 9, 30, align=(Align.CENTER, Align.CENTER, Align.MIN))
        bez += loc * (Cylinder(p['eye_r'] + 2.2, 7, align=(Align.CENTER, Align.CENTER, Align.MIN)) - Cylinder(p['eye_r'] - 1, 8, align=(Align.CENTER, Align.CENTER, Align.MIN)))
        lens += Location((xf - 12.5, y, p['eye_z']), (0, 90, 0)) * Cylinder(p['eye_r'] + 2, 1.5)
        disc = scale(Sphere(1), by=(4, p['eye_r'] - 1.5, p['eye_r'] - 1.5)) & Box(10, 100, 100, align=(Align.MIN, Align.CENTER, Align.CENTER))
        glow += disc.move(Location((xf - 11.8, y, p['eye_z'])))
    head = head - sockets
    # crest: blade on the centre line, pointed tip between the eyes, growing taller over the forehead, fading at the back
    tip = p['eye_z'] - p['eye_r'] - 2
    k, h = p['crest_k'], p['crest_h']
    wedge = xz_prism([(-300, -100), (D, -100), (D, tip), (D + k * 400, tip + 400), (-300, tip + 400)])
    def blade(grow, t):
        g = cranium(W + 0.01, D + grow, p['z0'], p['zc'], p['top'] + grow)
        return g & Box(400, t, 600).move(Location((0, 0, 0))) & wedge & Box(400, 50, 600, align=(Align.MIN, Align.CENTER, Align.MIN)).move(Location((-D * 0.6, 0, tip)))
    crest = Part()
    for i in range(6):  # stack of blades approximating a tapered wedge cross-section
        f = i / 5
        crest += blade(h * (0.25 + 0.75 * f), p['crest_base'] + (p['crest_t'] - p['crest_base']) * f)
    crest = crest - head
    # jaw: scoop plate over the lower face and sides, flat front, pivots on a round side boss
    jt, jb, t = p['jaw_top'], p['jaw_bot'], p['jaw_t']
    tw, td = p['jaw_taper_w'], p['jaw_taper_d']   # jaw narrows and recedes toward the bottom
    def jring(z, grow=0.0):
        f = (jt - z) / (jt - jb)
        return W + t - tw * f + grow, D + t + p['jaw_fwd'] - td * f + grow, p['jaw_fwd'] / 2 - td * f / 2
    def jsec(z, grow=0.0):
        w_, d_, x_ = jring(z, grow)
        return Plane.XY.offset(z) * Pos(x_, 0) * Ellipse(w_, d_).rotate(Axis.Z, 90)
    outer = loft([jsec(jb), jsec(jt)])
    inner = ell(W + 0.5, D + 0.5, jb - 1, jt + 1)
    shellj = outer - inner
    hx, hz, hr = p['hinge_x'], p['hinge_z'], p['hinge_r']
    prof = [(200, jt), (hx, jt)]
    import math
    for a in range(90, 271, 15):  # round end around the hinge boss
        prof.append((hx + hr * math.cos(math.radians(a)), hz + hr * math.sin(math.radians(a))))
    prof += [(hx + 10, jb), (200, jb)]
    jaw = shellj & xz_prism(prof)
    xg = D + t + p['jaw_fwd'] * 0.9
    chin = xz_prism([(-300, jb - 50), (xg - p['chin_slope'] * (jt - jb + 50), jb - 50), (xg, jt), (xg, jt + 50), (-300, jt + 50)])
    jaw = jaw & chin  # flat chin plane, raked back toward the bottom
    jaw_under = loft([jsec(jb, -0.5), jsec(jb + 4, -0.5)]) & xz_prism(prof) & chin
    jaw = jaw + jaw_under
    # speed lines: chrome bands wrapping the lower jaw with dark speaker slots between them (1930s streamliner nose)
    grille = Part(); speed = Part()
    chin = xz_prism([(-300, jb - 50), (xg + 1.2 - p['chin_slope'] * (jt - jb + 50), jb - 50), (xg + 1.2, jt), (xg + 1.2, jt + 50), (-300, jt + 50)])
    for kk in range(p['bands']):
        zb_ = jb + 9 + 7 * kk
        band = (loft([jsec(zb_, 1.2), jsec(zb_ + 2.4, 1.2)]) - loft([jsec(zb_ - 1, -1.2), jsec(zb_ + 3.4, -1.2)])) & xz_prism(prof) & chin
        speed += band
        xs_ = xg - p['chin_slope'] * (jt - zb_ - 4.6)
        slot = Box(30, 2 * W - 30, 2.2).move(Location((xs_, 0, zb_ + 4.6)))
        jaw = jaw - slot
        grille += Box(4, 2 * W - 30, 2.2).move(Location((xs_ - t - 1, 0, zb_ + 4.6)))
    # cheek louvres: stepped slats in a recessed pocket behind each eye, framed by a chrome arc (cooling exhaust)
    louv = Part(); trim = Part()
    import math as _m
    for s_ in (1, -1):
        pocket = Box(30, 30, 40).move(Location((-2, s_ * (W + 9), p['eye_z'] - 2)))
        head = head - pocket
        for kk in range(5):
            sl = Box(28, 3, 3.2).move(Location((-2, s_ * (W * 0.86 + 1.5), p['eye_z'] - 16 + 7.5 * kk)))
            louv += sl.rotate(Axis((22, s_ * (W * 0.86), p['eye_z'] - 16 + 7.5 * kk), (1, 0, 0)), s_ * 0) 
        pts = []
        for a_ in range(-80, 81, 10):
            th = _m.radians(a_)
            xx = 13 + 6 * _m.cos(th); zz = p['eye_z'] - 2 + 24 * _m.sin(th)
            yy = W * (1 - (xx / D) ** 2) ** 0.5 if abs(xx) < D else 0
            pts.append((xx, s_ * (yy + 0.6), zz))
        path = Spline(*pts)
        trim += sweep(Plane(path @ 0, z_dir=path % 0) * Circle(1.5), path)
    # hinge boss and pin on each side
    ears = Part()
    for s in (1, -1):
        ears += Location((hx, s * (W + t), hz), (90, 0, 0)) * Cylinder(hr - 7, 6)
        ears += Location((hx, s * (W + t + 2), hz), (90, 0, 0)) * Cylinder(7, 6)
        if p['ear']:  # small ear block at eye height
            eb = Box(16, 10, 22)
            ears += eb.move(Location((-4, s * (W + 3), p['eye_z'] + 4)))
    # collar: stepped ring the head sits on (the neck yaw bearing lives inside)
    collar = Cylinder(56, 8, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, 14))) + Cylinder(46, 10, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, 4)))
    neck = Cylinder(34, 30, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, -20)))
    return dict(core=core, speed=speed, louv=louv, trim=trim, head=head, crest=crest, bezel=bez, lens=lens, glow=glow, jaw=jaw, grille=grille, ears=ears, collar=collar, neck=neck)

MATS = dict(core='dark', speed='chrome', louv='dark', trim='chrome', head='white', crest='white', jaw='white', bezel='chrome', lens='lens', glow='eye', grille='dark', ears='gun', collar='gun', neck='dark')
VARIANTS = {'A3': dict()}
if __name__ == '__main__':
    for n in (sys.argv[1:] or list(VARIANTS)):
        parts = build(VARIANTS[n]); os.makedirs(f'out/{n}', exist_ok=True); man = []
        for k_, sol in parts.items():
            if sol is None or sol.volume < 1: print('empty', k_); continue
            export_stl(sol, f'out/{n}/{k_}.stl', tolerance=0.02, angular_tolerance=0.04); man.append(dict(file=f'{n}/{k_}.stl', mat=MATS[k_]))
        export_step(Compound(list(parts.values())), f'out/{n}/head.step')
        json.dump(man, open(f'out/{n}/parts.json', 'w'), indent=1); print(n, [m['file'] for m in man])
