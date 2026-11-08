"""Clamshell concepts from the current meshes (no SolidWorks): upper arm (db71b23b) split on the Z = 6 plane with a separate
RS02 seat ring and hub disc; forearm B split on the forearm-axis plane X = 170.49. Print orientation, support preview
(red = support on a visible outer face, blue = support on hidden faces only), mass, seam channel for the trunk.
usage: python clam_concept.py OUT_DIR"""
import sys
OUT = sys.argv[1]
sys.argv = ['x']
import json, numpy as np, trimesh
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import elbow_diagram as E
from support_check import check

CY, CZ = 209.860668, 5.985875
HX, HZ = 170.49, 5.985875
RHO = 1.27
B = {p['key']: p['mesh'] for p in E.build()}
UA = trimesh.load('J:/code/marengo/hardware/designs/left-arm/prints/upper-arm/cad/upper_arm.stl', force='mesh')
FB = trimesh.load('stl/fa/forearm_b.STL', force='mesh')
g = lambda m: round(m.volume / 1000 * RHO, 1)


def box(x0, x1, y0, y1, z0, z1):
    b = trimesh.creation.box(extents=[x1 - x0, y1 - y0, z1 - z0]); b.apply_translation([(x0 + x1) / 2, (y0 + y1) / 2, (z0 + z1) / 2]); return b


def xcyl(r, x0, x1, n=128):
    c = trimesh.creation.cylinder(radius=r, height=x1 - x0, sections=n)
    c.apply_transform(trimesh.transformations.rotation_matrix(np.pi / 2, [0, 1, 0])); c.apply_translation([(x0 + x1) / 2, CY, CZ]); return c


def half(m, n, p):
    return trimesh.intersections.slice_mesh_plane(m, n, p, cap=True)


# ---------------- upper arm
ring_cut = xcyl(43.6, 150.5, 166.5)
hub_cut = box(120, 220, 298.5, 330, -60, 70)
ring = UA.intersection(ring_cut)
hub = UA.intersection(hub_cut)
rest = UA.difference(ring_cut).difference(hub_cut)
Z0 = 6.0
# seam rib on the -X inner wall: seam flange for the M3 inserts and the trunk channel (10 x 4 mm, on the seam = neutral plane of the sweep bending)
rib = box(133.5, 149.0, 252.0, 298.5, Z0 - 6.0, Z0 + 6.0).intersection(UA.convex_hull)
chan = box(136.0, 146.0, 250.0, 300.0, Z0 - 2.0, Z0 + 2.0)
rest2 = rest.union(rib).difference(chan)
up_h = half(rest2, [0, 0, 1], [0, 0, Z0]); lo_h = half(rest2, [0, 0, -1], [0, 0, Z0])
# hub disc, lightened: 6 mm plate at the RS02 hub face (Y 313.86-319.86), Ø10 bosses round the six M4 holes, and the tube rim
HOLES = [(159.4, 10.5), (161.0, -1.4), (172.1, -5.9), (181.6, 1.5), (180.0, 13.4), (168.8, 17.9)]
tcyl = trimesh.creation.cylinder(radius=35.5, height=15.4, sections=128); tcyl.apply_transform(trimesh.transformations.rotation_matrix(np.pi / 2, [1, 0, 0])); tcyl.apply_translation([170.5, 306.2, 6.0])
bosses = trimesh.util.concatenate([trimesh.creation.cylinder(radius=5.0, height=15.4, sections=48, transform=trimesh.transformations.concatenate_matrices(
    trimesh.transformations.translation_matrix([x, 306.2, z]), trimesh.transformations.rotation_matrix(np.pi / 2, [1, 0, 0]))) for x, z in HOLES])
hub_l = hub.difference(tcyl.difference(bosses))
REF_UA = trimesh.util.concatenate([UA, B['rs02e'], B['carrier'], B['rs02y'], B['brg'], FB, B['cover']])
UA_PARTS = [('upper half (+Z)', up_h, [0, 0, 1]), ('lower half (-Z)', lo_h, [0, 0, -1]), ('RS02 seat ring', ring, [1, 0, 0]), ('hub disc', hub_l, [0, -1, 0])]

# ---------------- forearm B
fa_up = half(FB, [1, 0, 0], [HX, 0, 0]); fa_lo = half(FB, [-1, 0, 0], [HX, 0, 0])


def loc(n):
    q = trimesh.load('stl/fa/' + n + '.STL', force='mesh'); q.apply_translation([HX, 0, HZ]); return q


REF_FA = trimesh.util.concatenate([FB, B['rs02e'], B['carrier'], B['arm'], B['brg'], B['rs00'], loc('forearm_rs00_ring'), loc('forearm_sleeve')])
FA_PARTS = [('outboard half (+X)', fa_up, [1, 0, 0]), ('inboard half (-X)', fa_lo, [-1, 0, 0])]


def colour_parts(name, m, up, ref):
    r, outer, inner = check(m, up, ref=ref)
    P = []
    rest_ = ~(outer | inner)
    for mask, col, tag in ((rest_, E.C['print'], ''), (inner, '#4f8fd8', ' support (hidden)'), (outer, '#d0342c', ' support ON OUTER SKIN')):
        if mask.any():
            P.append(dict(key=name + tag, label=name + tag, mesh=m.submesh([np.flatnonzero(mask)], append=True), col=col, ex=np.zeros(3), alpha=1.0))
    return P, r


def sheet(parts, ref, title, fn):
    n = len(parts)
    fig, ax = plt.subplots(2, n, figsize=(9 * n, 18))
    ax = np.atleast_2d(ax).reshape(2, n)
    rows = []
    for j, (name, m, up) in enumerate(parts):
        P, r = colour_parts(name, m, up, ref)
        U = np.asarray(up, float); H = np.cross(U, [0, 1, 0] if abs(U[1]) < 0.9 else [1, 0, 0]); H /= np.linalg.norm(H); H2 = np.cross(U, H)
        c = m.bounds.mean(0); half_ = np.abs(m.bounds - c).max() * 1.25
        for i, (d, lab) in enumerate(((-(0.9 * U) + H + 0.6 * H2, 'from above (as printed)'), ((1.2 * U) + H - 0.5 * H2, 'from below: bed side and overhangs'))):
            img, _ = E.render(P, d, U, 900, c, half_)
            a = ax[i, j]; a.imshow(img); a.axis('off')
            head = f'{name}\nprint: up = {tuple(int(v) for v in up)}, cut face on the bed\n{g(m)} g solid · support {r["support_mm2"]:.0f} mm², on outer skin {r["outer_mm2"]:.0f} mm²\n'
            a.set_title((head if i == 0 else '') + lab, fontsize=14)
        rows.append(dict(part=name, g_solid=g(m), up=up, **r))
    fig.suptitle(title, fontsize=18)
    plt.tight_layout(rect=(0, 0, 1, 0.96)); plt.savefig(fn, dpi=60); plt.close(fig)
    return rows


def ua_assembly(ax):
    P = []
    for k, m, col, ex in (('upper half', up_h, E.C['print'], (0, 0, 45)), ('lower half', lo_h, '#e2dfd6', (0, 0, -45)), ('seat ring', ring, '#b8c99a', (-40, 0, 0)),
                          ('hub disc', hub_l, '#b8c99a', (0, 40, 0)), ('elbow RS02', B['rs02e'], E.C['act'], (0, 0, 0))):
        P.append(dict(key=k, label=k, mesh=m, col=col, ex=np.asarray(ex, float), alpha=1.0))
    trunk = [(141, 330, Z0), (141, 300, Z0), (141, 256, Z0), (152, 250, Z0), (158, 246, Z0)]
    P.append(dict(key='trunk', label='trunk in the seam channel (power pair + CAN pair, 10 × 4 mm)', mesh=E.tube(trunk, 2.2), col=E.C['wire'], ex=np.array([0, 0, -45.0]), alpha=1.0))
    for i, y in enumerate((268.0, 286.0)):
        P.append(dict(key=f'fuse{i}', label='mini-blade fuse holder in a wall cradle' if i == 0 else 'splice pocket', mesh=box(146.5, 157.5, y - 7, y + 7, Z0 - 9, Z0 - 1),
                      col='#3a3f4a', ex=np.array([0, 0, -45.0]), alpha=1.0))
    E.panel(ax, P, 'Exploded: halves along Z, ring along X, hub disc along Y', [-0.7, -0.35, -0.6], [0, 1, 0], (170, 250, 6), 110, 1100, explode=True)


rows_ua = sheet(UA_PARTS, REF_UA, 'Upper arm clamshell concept (from db71b23b): print orientation and support preview. Red = support on a visible outer surface, blue = hidden faces only',
                OUT + '/upper_arm_clamshell_concept.png')
fig, ax = plt.subplots(figsize=(30, 16)); ua_assembly(ax)
fig.suptitle('Upper arm clamshell concept, exploded. Trunk (orange) runs in a 10 x 4 mm channel on the seam (neutral plane of the sweep bending), fuse holder and splice in wall cradles', fontsize=20)
plt.tight_layout(rect=(0, 0, 1, 0.96)); plt.savefig(OUT + '/upper_arm_clamshell_exploded.png', dpi=60); plt.close(fig)
rows_fa = sheet(FA_PARTS, REF_FA, 'Forearm B clamshell concept: split on the forearm-axis plane X 170.49, each half cut face down. Red = support on outer skin, blue = hidden',
                OUT + '/forearm_b_clamshell_concept.png')
r1 = check(FB, [1, 0, 0], ref=REF_FA)[0]
r_ua = check(UA, [0, -1, 0], ref=REF_UA)[0]
json.dump(dict(upper_arm=rows_ua, upper_arm_onepiece_minus_y=r_ua, upper_arm_onepiece_g=g(UA), forearm=rows_fa, forearm_onepiece_plusx=r1, forearm_onepiece_g=g(FB),
               ua_parts_g=dict(ring=g(ring), hub=g(hub), hub_lightened=g(hub_l), rest=g(rest), rest_with_rib=g(rest2))),
          open(OUT + '/clamshell_concept.json', 'w'), indent=1, default=float)
print(json.dumps(dict(ua=rows_ua, fa=rows_fa, fa1=r1, ua1=r_ua), default=float, indent=0))
