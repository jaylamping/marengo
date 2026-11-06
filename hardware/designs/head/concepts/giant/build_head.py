"""Giant-inspired head concept for marengo (2026-11-06). Original restyle, not a replica.
Robot frame: X forward, Y left, Z up, mm; z = 0 at the top of the torso shell neck opening.
Writes one STL per part/material to out/<variant>/ plus parts.json for the three.js viewer."""
import json, os, sys
from build123d import *

def cranium(w=62, d=66, z0=92, zc=150, top=212):
    """Bucket body (elliptic in plan) with a dome cap."""
    body = extrude(Ellipse(w, d).rotate(Axis.Z, 90), amount=zc - z0).move(Location((0, 0, z0)))
    # Ellipse(x_radius, y_radius) is in XY; rotate so x_radius lies on Y (width) -> after rotate: width along Y
    cap = scale(Sphere(1), by=(d, w, top - zc)).move(Location((0, 0, zc)))
    cap = cap & Box(400, 400, 400, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, zc)))
    return body + cap

def build(v):
    p = dict(w=62, d=66, z0=95, zc=150, top=212, crest_sx=1.12, crest_sz=1.07, crest_t=9, eye_r=19, eye_z=124, eye_y=30,
             jaw_round=10, jaw_chamfer=0, jaw_fwd=8, jaw_top=90, jaw_bot=34, jaw_wx=3, jaw_flat=0, tabs=0, seams=0, bolts=0)
    p.update(v)
    W, D = p['w'], p['d']
    head = cranium(W, D, p['z0'], p['zc'], p['top'])
    # eye sockets: recessed round pockets in the front face
    sockets = Part()
    for s_ in (1, -1):
        y = s_ * p['eye_y']; xf = D * (1 - (y / W) ** 2) ** 0.5
        sockets += Cylinder(p['eye_r'] + 3.5, 60, rotation=(0, 90, 0)).move(Location((xf - 14 + 30, y, p['eye_z'])))
    head = head - sockets
    # crest: stepped keel on the centre line (wide low base + narrow blade), from the nose bridge over the dome
    def blade(sx, sz, t, zmin):
        g = scale(cranium(W, D, p['z0'], p['zc'], p['top']), by=(sx, 1.0, sz)).move(Location((0, 0, p['z0'] * (1 - sz))))
        b = g & Box(300, t, 300).move(Location((0, 0, 150)))
        return b & Box(300, 60, 300, align=(Align.CENTER, Align.CENTER, Align.MIN)).move(Location((0, 0, zmin)))
    crest = (blade(1 + (p['crest_sx'] - 1) * 0.45, 1 + (p['crest_sz'] - 1) * 0.45, p['crest_t'] + 7, p['eye_z'] - 14)
             + blade(p['crest_sx'], p['crest_sz'], p['crest_t'], p['eye_z'] - 6)) - head
    crest = crest & Box(300, 60, 400, align=(Align.MIN, Align.CENTER, Align.CENTER)).move(Location((-D * 0.55, 0, 150)))  # fades out behind the crown
    # eyes: thick bezel ring, dark socket, glowing disc (round display) set back; stereo cameras sit in the nose bridge
    bez, lens, glow = Part(), Part(), Part()
    for s_ in (1, -1):
        y = s_ * p['eye_y']; xf = D * (1 - (y / W) ** 2) ** 0.5
        ring = Cylinder(p['eye_r'] + 3.5, 9, rotation=(0, 90, 0)) - Cylinder(p['eye_r'] - 1.5, 12, rotation=(0, 90, 0))
        bez += ring.move(Location((xf - 8.5, y, p['eye_z'])))
        lens += Cylinder(p['eye_r'] + 3.4, 2, rotation=(0, 90, 0)).move(Location((xf - 13.5, y, p['eye_z'])))
        disc = scale(Sphere(1), by=(3.5, p['eye_r'] - 3, p['eye_r'] - 3)) & Box(10, 100, 100, align=(Align.MIN, Align.CENTER, Align.CENTER))
        glow += disc.move(Location((xf - 12.6, y, p['eye_z'])))
    for s_ in (1, -1):  # stereo camera windows either side of the crest base, between the eyes
        lens += Cylinder(2.6, 3, rotation=(0, 90, 0)).move(Location((D * 0.985 + 0.6, s_ * 9.5, p['eye_z'] - 9)))
    # jaw: the lower half of the bucket, a little wider and pushed forward at the chin, sloped underside at the back
    jt, jb, jw = p['jaw_top'], p['jaw_bot'], W + p['jaw_wx']
    jaw = extrude(Ellipse(jw, D + p['jaw_fwd']).rotate(Axis.Z, 90), amount=jt - jb).move(Location((p['jaw_fwd'] / 2, 0, jb)))
    side = Polyline((-120, jt + 1), (120, jt + 1), (120, jb - 1), (D * 0.2, jb - 1), (-D * 0.55, jb + 24), (-120, jb + 40), close=True)
    jaw = jaw & extrude(make_face(Plane.XZ * side), amount=150, both=True)
    if p['jaw_flat']:  # flat chin plane across the front
        xfront = p['jaw_fwd'] / 2 + D + p['jaw_fwd'] - p['jaw_flat']
        jaw = jaw & Box(400, 400, 400, align=(Align.MAX, Align.CENTER, Align.CENTER)).move(Location((xfront, 0, 60)))
    if p['jaw_round']:
        try: jaw = fillet(jaw.edges().filter_by(lambda e: e.center().Z < jt - 2), p['jaw_round'])
        except Exception as e: print('fillet skipped', e)
    if p['jaw_chamfer']:
        try: jaw = chamfer(jaw.edges().filter_by(lambda e: e.center().Z < jt - 2), p['jaw_chamfer'])
        except Exception as e: print('chamfer skipped', e)
    mz = (jt + jb) / 2 + 4
    grille = Part()
    xg = p['jaw_fwd'] / 2 + D + p['jaw_fwd'] - (p['jaw_flat'] or 0)
    for k in range(4):  # speaker / mic slots in the chin
        slot = Box(30, 2 * W - 64 - 10 * abs(k - 1.5), 2.6).move(Location((xg, 0, mz + 6 - 5.5 * k)))
        jaw = jaw - slot
        grille += Box(6, 2 * W - 64 - 10 * abs(k - 1.5), 2.6).move(Location((xg - 9, 0, mz + 6 - 5.5 * k)))
    jw = W
    # hinge bosses at the sides (jaw pivot + mic ports)
    ears = Part()
    for s in (1, -1):
        if p['tabs']:  # flat side tabs (jaw hinge covers) with a bolt
            tab = Box(30, 10, 34)
            try: tab = chamfer(tab.edges().filter_by(Axis.Y), 5)
            except Exception: pass
            ears += tab.move(Location((-4, s * (jw + 3), jt + 2)))
            ears += Cylinder(4.5, 14, rotation=(90, 0, 0)).move(Location((-4, s * (jw + 4), jt + 2)))
        else:
            ears += Cylinder(13, 12, rotation=(90, 0, 0)).move(Location((-6, s * (jw + 5), jt + 4)))
            ears += Cylinder(7, 16, rotation=(90, 0, 0)).move(Location((-6, s * (jw + 7), jt + 4)))
    seam = Part()
    if p['seams']:  # panel seams: dome/bucket joint, front/back split line, jaw mid line
        def shell_ring(w, d, z0, z1):
            return extrude(Ellipse(w, d).rotate(Axis.Z, 90), amount=z1 - z0).move(Location((0, 0, z0)))
        cut = cranium(W + 2, D + 2, p['z0'], p['zc'], p['top'] + 2) & Box(400, 400, 2.2).move(Location((0, 0, p['zc'] + 8)))
        cut += cranium(W + 2, D + 2, p['z0'], p['zc'], p['top'] + 2) & Box(2.2, 400, 400).move(Location((-12, 0, 150)))
        cut = cut - cranium(W - 1.6, D - 1.6, p['z0'] - 5, p['zc'], p['top'] - 1.6)
        cut = cut - (crest + Box(400, p['crest_t'] + 8, 400).move(Location((0, 0, 150))))
        head = head - cut
        seam += cranium(W - 1.2, D - 1.2, p['z0'], p['zc'], p['top'] - 1.2) & cut.bounding_box().to_solid() & cut
        jcut = (extrude(Ellipse(jw + 2, D + p['jaw_fwd'] + 2).rotate(Axis.Z, 90), amount=2.2).move(Location((p['jaw_fwd'] / 2, 0, jb + 12)))
                - extrude(Ellipse(jw - 1.6, D + p['jaw_fwd'] - 1.6).rotate(Axis.Z, 90), amount=2.2).move(Location((p['jaw_fwd'] / 2, 0, jb + 12))))
        jaw = jaw - jcut
    bolts = Part()
    if p['bolts']:
        import math
        for s_ in (1, -1):
            y = s_ * p['eye_y']; xf = D * (1 - (y / W) ** 2) ** 0.5
            for k in range(6):
                t = k * math.pi / 3 + math.pi / 6
                bolts += Cylinder(1.5, 2, rotation=(0, 90, 0)).move(Location((xf + 0.6, y + (p['eye_r'] + 1) * math.cos(t), p['eye_z'] + (p['eye_r'] + 1) * math.sin(t))))
    # neck: dark column + gunmetal collar ring the head sits in
    neck = Cylinder(30, 60).move(Location((0, 0, 30)))
    collar = (Cylinder(52, 10) - Cylinder(36, 12)).move(Location((0, 0, 8)))
    gap = extrude(Ellipse(W - 3, D - 3).rotate(Axis.Z, 90), amount=p['z0'] - jt + 6).move(Location((0, 0, jt - 3)))
    return dict(seam=seam, bolts=bolts, head=head, crest=crest, bezel=bez, lens=lens, glow=glow, jaw=jaw, grille=grille, ears=ears, neck=neck, collar=collar, gap=gap)

VARIANTS = {
    'A_rounded': (dict(), dict(head='white', crest='white', jaw='white', bezel='gun', lens='lens', glow='eye', grille='dark', ears='gun', neck='dark', collar='gun', gap='dark')),
    'A2': (dict(jaw_fwd=16, jaw_bot=26, jaw_wx=5, jaw_flat=7, jaw_round=8, crest_sx=1.2, crest_sz=1.12, crest_t=7, tabs=1, seams=1, bolts=1),
           dict(head='white', crest='white', jaw='white', bezel='gun', lens='lens', glow='eye', grille='dark', ears='gun', neck='dark', collar='gun', gap='dark', seam='dark', bolts='gun')),
    'B_angular': (dict(jaw_round=0, jaw_chamfer=5, jaw_fwd=12, crest_sx=1.17, crest_sz=1.10, crest_t=8), dict(head='white', crest='gun', jaw='gun', bezel='gun', lens='lens', glow='eye', grille='dark', ears='gun', neck='dark', collar='gun', gap='dark')),
    'C_gunmetal': (dict(jaw_round=6, crest_sx=1.14, crest_sz=1.08, eye_r=20, eye_y=31), dict(head='gun', crest='white', jaw='gun', bezel='white', lens='lens', glow='eye', grille='dark', ears='white', neck='dark', collar='white', gap='dark')),
}
if __name__ == '__main__':
    names = sys.argv[1:] or list(VARIANTS)
    for n in names:
        params, mats = VARIANTS[n]
        parts = build(params)
        os.makedirs(f'out/{n}', exist_ok=True); man = []
        for k, sol in parts.items():
            if sol is None or (hasattr(sol, 'volume') and sol.volume < 1): print('empty', n, k); continue
            fn = f'out/{n}/{k}.stl'; export_stl(sol, fn, tolerance=0.02, angular_tolerance=0.04)
            man.append(dict(file=f'{n}/{k}.stl', mat=mats.get(k, 'white')))
        if n.startswith('A'):
            export_step(Compound(list(parts.values())), f'out/{n}/head.step')
        json.dump(man, open(f'out/{n}/parts.json', 'w'), indent=1); print(n, [m['file'] for m in man])
