"""Elbow loop options: back loop in loom vs inboard side spiral, at 0 / 60 / 124 deg. Meshes only.
usage: python elbow_loop_options.py OUT.png"""
import sys
OUT = sys.argv[1]
sys.argv = ['x']
import numpy as np, trimesh
from scipy.optimize import brentq
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import elbow_diagram as E

CY, CZ = 209.860668, 5.985875
B = {p['key']: p['mesh'] for p in E.build()}
FB = trimesh.load('stl/fa/forearm_b.STL', force='mesh')
RS00 = B['rs00']
OD = 8.0


def rot(m, th):
    a = -np.radians(th)
    T = trimesh.transformations.rotation_matrix(a, [1, 0, 0], [0, CY, CZ])
    q = m.copy(); q.apply_transform(T); return q


def P(r, phi, x):
    return np.array([x, CY + r * np.cos(phi), CZ - r * np.sin(phi)])


def bow(L, c):
    t = brentq(lambda t: L * np.sin(t) / t - c, 1e-4, np.pi - 1e-4); rho = L / (2 * t); return rho * (1 - np.cos(t)), rho


def arc_pts(a, b, L, away):
    """Circular arc of length L from a to b, bulging toward 'away' (unit vector)."""
    c = np.linalg.norm(b - a); s, rho = bow(L, c)
    u = (b - a) / c; n = away - (away @ u) * u; n /= np.linalg.norm(n)
    t = brentq(lambda t: L * np.sin(t) / t - c, 1e-4, np.pi - 1e-4)
    ctr = (a + b) / 2 - n * rho * np.cos(t)
    out = []
    for k in np.linspace(-t, t, 40):
        out.append(ctr + rho * (np.sin(k) * u + np.cos(k) * n))
    return np.array(out), s, rho


# back loop: anchors r 48 at 70 / 110 deg behind the axis, wraps the carrier/hoop at r 44, free 148 mm
def back_loop(th, L=148.0, R=44.0, x=141.0):
    pA, pF = np.radians(70), np.radians(110 + th)
    phis = np.linspace(pA, pF, 60)
    base = lambda h: [P(48 + h * np.sin(np.pi * (p - pA) / (pF - pA)) if (pF - pA) > 1e-6 else 48, p, x) for p in phis]
    length = lambda h: np.sum(np.linalg.norm(np.diff(np.array(base(h)), axis=0), axis=1))
    h = brentq(lambda h: length(h) - L, -3.9, 200) if length(-3.9) < L else -3.9
    pts = np.array(base(h))
    return pts, h + 48 - 44, P(48, pA, x), P(48, pF, x)


# side spiral: anchors on the inboard face plane X 126: upper arm r 36 at -20 deg, cheek cap r 24 at 100 deg + flexion; free 75 mm
def side_loop(th, L=75.0, x=126.0):
    a = P(36, np.radians(-20), x); f = P(24, np.radians(100 + th), x)
    mid = (a + f) / 2; away = mid - np.array([x, CY, CZ]); away[0] = 0
    pts, s, rho = arc_pts(a, f, L, away / np.linalg.norm(away))
    return pts, s, rho, a, f


ROWS = [('Back loop in loom', back_loop), ('Inboard side spiral', side_loop)]
fig = plt.figure(figsize=(34, 22))
gs = fig.add_gridspec(2, 4, width_ratios=[1, 1, 1, 0.9])
d = np.array([0.62, -0.25, 0.75])
for i, (ttl, fn) in enumerate(ROWS):
    for j, th in enumerate((0, 60, 124)):
        ax = fig.add_subplot(gs[i, j])
        if i == 0:
            pts, bulge, a, f = fn(th); info = f'loop stands {bulge:.0f} mm off the carrier rim (r 44)'
        else:
            pts, s, rho, a, f = fn(th); info = f'bow {s:.0f} mm, bend r {rho:.0f} mm'
        Pp = [dict(key=k, label=k, mesh=m, col=c, ex=np.zeros(3), alpha=al) for k, m, c, al in (
            ('upper arm', B['arm'], E.C['print'], 0.55), ('carrier', B['carrier'], E.C['print'], 0.7), ('elbow RS02', B['rs02e'], E.C['act'], 1.0),
            ('forearm', rot(FB, th), E.C['print'], 0.55), ('RS00', rot(RS00, th), E.C['act'], 1.0))]
        Pp.append(dict(key='loom', label='loom', mesh=E.tube(pts, OD / 2), col='#1d1d1f', ex=np.zeros(3), alpha=1.0))
        for k, q in (('A', a), ('F', f)):
            Pp.append(dict(key=k, label=k, mesh=trimesh.creation.icosphere(2, 4.0).apply_translation(q), col='#ff6a13', ex=np.zeros(3), alpha=1.0))
        img, _ = E.render(Pp, d, [0, 1, 0], 800, (160, 185, 5), 120)
        ax.imshow(img); ax.axis('off'); ax.set_title(f'{ttl}, elbow {th}°\n{info}', fontsize=17)
ax = fig.add_subplot(gs[0, 3]); ax.axis('off')
ax.text(0, 0.95, 'Back loop (best case)', fontsize=20, fontweight='bold', va='top')
ax.text(0, 0.85, 'Wraps the carrier/hoop at r ≥ 44 on the back,\nso the path grows ~95 mm from 0° to 124°.\n\nFree 148 mm, slack 115 mm at 0°\nBow at 0°: ~53 mm out the back\nBend r ≥ 44 mm (min 24 = 3 × OD)\nWidth: unchanged\n\nOrange dots = anchors (clamps in the halves)', fontsize=15, va='top')
ax = fig.add_subplot(gs[1, 3]); ax.axis('off')
ax.text(0, 0.95, 'Inboard side spiral', fontsize=20, fontweight='bold', va='top')
ax.text(0, 0.85, 'Flat loop on the inboard face, outside the\ncheek cap: one end near the axis, so the path\nchanges only ~9 mm over 0–124°.\n\nFree 75 mm, bow ≤ 23 mm\nBend r ≥ 24 mm (3 × OD)\nWidth: +10 mm toward the torso\n\nEst. gap to the waist shell (rev B, 193 wide):\n  ~62 mm at roll 0, ~43 mm at the 5° inward limit', fontsize=15, va='top')
# small front-view schematic
ins = fig.add_axes([0.80, 0.06, 0.18, 0.2])
ins.add_patch(plt.Rectangle((-96.5, 150), 193, 120, fc='#3a3f4a', alpha=0.25)); ins.text(0, 205, 'waist shell\n193 wide', ha='center', fontsize=10)
for roll, col in ((0, '#2e9e4a'), (5, '#d0342c')):
    sx = 208.5 - 215 * np.sin(np.radians(roll))
    ins.plot([208.5, sx], [425, 210], color=col, lw=2)
    ins.add_patch(plt.Rectangle((sx - 49.5, 195), 49.5 + 39.5, 30, fc='none', ec=col, lw=1.5))
    ins.annotate(f'roll {roll}°: gap {sx - 49.5 - 96.5:.0f} mm', (sx - 49.5, 225 - 12 * roll), fontsize=10, color=col)
ins.set_xlim(-120, 300); ins.set_ylim(140, 440); ins.set_aspect('equal'); ins.set_title('Front view at the elbow (schematic, shell rev B dims)', fontsize=10); ins.tick_params(labelsize=8)
fig.suptitle('Elbow cable crossing: back loop vs inboard side spiral, 8 mm loom, elbow 0 / 60 / 124°. 3/4 view from behind, inboard side. Concept for Joey to pick.', fontsize=22)
plt.tight_layout(rect=(0, 0, 1, 0.96)); plt.savefig(OUT, dpi=55); print('saved', OUT)
