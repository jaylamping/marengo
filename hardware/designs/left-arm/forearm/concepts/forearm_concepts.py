"""Left forearm shape concepts: blended implicit envelopes around the fixed elbow and RS00 interfaces.
Arm frame as in elbow_diagram.py: X along the elbow axis (inboard -X), Y up the arm, Z fore-aft (anterior +Z).
Flexion rotates the forearm about the elbow axis toward +Z. Envelopes only: walls, pockets and joints come later.
usage: python forearm_concepts.py OUT_DIR [outboard_cheek_shift_mm]"""
import sys, json
import numpy as np
import trimesh
from scipy.interpolate import PchipInterpolator
from skimage.measure import marching_cubes
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

sys_argv = sys.argv[:]
sys.argv = ['x']
import elbow_diagram as E
sys.argv = sys_argv

OUT = sys.argv[1] if len(sys.argv) > 1 else '.'
DX = float(sys.argv[2]) if len(sys.argv) > 2 else 0.0   # outboard cheek shift, pending the RS02 caliper check
CY, CZ, HX, HZ = E.CY, E.CZ, E.HX, E.HZ
Y_WRIST, Y_SPLIT, Y_RS00_TOP = 14.86, 89.9, 141.3      # 195 mm forearm; RS00 output face; RS00 rear
CHEEK_IN = (131.7, 139.7)
CHEEK_OUT = (197.19 + DX, 207.19 + DX)
R_CLEAR = 47.5            # between the cheeks: hoop r 42.3, carrier r 42.0; 47.5 as on the current fork bridge, clears the fold
R_DISC = 29.0
BORE_D, BORE_REQ_L, BORE_COM = 58.0, 100.0, 140.0
WHITE = '#f2f0ea'


def smin(a, b, k):
    h = np.clip(0.5 + 0.5 * (b - a) / k, 0, 1)
    return b * (1 - h) + a * h - k * h * (1 - h)


def smax(a, b, k):
    return -smin(-a, -b, k)


def pchip(ys, vs):
    return PchipInterpolator(ys, vs, extrapolate=True)


# ---- concept definitions: per-quadrant radii about the hub axis (X 170.49, Z 5.99), superellipse exponent n
COMMON_Y = [Y_WRIST, 24, 40, Y_SPLIT, 120, 141.5, 147, 158, 172, 186]
CONCEPTS = {
    'A': dict(name='A  Clean taper',
              xp=[31.0, 32.2, 32.5, 32.5, 33.5, 35.0, 36.0, 36.7 + DX, 36.7 + DX, 36.7 + DX],
              xm=[31.0, 32.2, 32.5, 32.5, 33.8, 35.8, 37.4, 38.6, 38.8, 38.8],
              zp=[31.0, 32.2, 32.5, 32.5, 32.5, 31.2, 12.0, 4.5, 4.5, 4.5],
              zm=[31.0, 32.2, 32.5, 32.5, 32.5, 32.5, 32.5, 32.5, 32.0, 30.0],
              n=[2, 2, 2, 2, 2, 2.15, 2.4, 2.7, 3.0, 3.0], ridge=None, belly=None, bridge=False),
    'B': dict(name='B  Muscle belly',
              xp=[31.0, 32.2, 32.8, 34.0, 38.5, 43.5, 44.5, 41.0 + DX, 36.7 + DX, 36.7 + DX],
              xm=[31.0, 32.2, 32.8, 34.0, 38.8, 44.0, 45.0, 42.0, 38.8, 38.8],
              zp=[31.0, 32.2, 32.5, 32.5, 32.5, 31.2, 12.0, 4.5, 4.5, 4.5],
              zm=[31.0, 32.2, 32.5, 32.8, 34.0, 35.0, 34.5, 33.0, 32.0, 30.0],
              n=[2, 2, 2, 2, 2, 2.0, 2.1, 2.4, 2.8, 3.0], ridge=None, belly=True, bridge=False),
    'C': dict(name='C  Ulna ridge',
              xp=[31.0, 32.2, 32.5, 32.5, 33.5, 35.0, 36.0, 36.7 + DX, 36.7 + DX, 36.7 + DX],
              xm=[31.0, 32.2, 32.5, 32.5, 33.8, 35.8, 37.4, 38.6, 38.8, 38.8],
              zp=[31.0, 32.2, 32.5, 32.5, 32.5, 31.2, 12.0, 4.5, 4.5, 4.5],
              zm=[31.0, 32.2, 32.5, 32.5, 32.5, 32.5, 32.5, 32.5, 32.0, 30.0],
              n=[2, 2, 2, 2, 2, 2.15, 2.4, 2.7, 3.0, 3.0],
              ridge=dict(y=[Y_SPLIT + 4, 105, 150, 175, 186], h=[0.0, 8.0, 11.0, 12.0, 9.0], w=0.38), belly=None, bridge=True),
}


def body_sdf(c, x, y, z):
    f = {k: pchip(COMMON_Y, c[k]) for k in ('xp', 'xm', 'zp', 'zm', 'n')}
    yy = np.clip(y, Y_WRIST, COMMON_Y[-1])
    dx, dz = x - HX, z - HZ
    ax = np.where(dx >= 0, f['xp'](yy), f['xm'](yy))
    bz = np.where(dz >= 0, f['zp'](yy), f['zm'](yy))
    if c['ridge']:
        r = c['ridge']; h = np.clip(pchip(r['y'], r['h'])(yy), 0, None)
        h = np.where((yy < r['y'][0]), 0.0, h)
        th = np.arctan2(dz, dx)      # posterior is -Z: th = -pi/2
        g = np.exp(-((th + np.pi / 2) / r['w']) ** 2)
        bz = bz + np.where(dz < 0, h * g, 0)
        ax = ax + np.where(dz < 0, h * g * 0.25, 0)
    n = f['n'](yy)
    rho = np.sqrt(dx * dx + dz * dz) + 1e-9
    rn = ((np.abs(dx) / ax) ** n + (np.abs(dz) / bz) ** n) ** (1 / n)
    d = rho * (1 - 1 / rn)
    d = smax(d, Y_WRIST - y, 1.5)                   # wrist face
    d = smax(d, y - COMMON_Y[-1], 2.0)
    # yaw split groove at the RS00 output face: the distal tube turns below it
    rs = np.sqrt(dx * dx + dz * dz)
    groove = np.sqrt((rs - 32.5) ** 2 + (y - Y_SPLIT) ** 2) - 0.9
    d = np.maximum(d, -groove)
    return d


def cheek_sdf(x, y, z, x0, x1, top_pad=0.0):
    # disc r 29 on the elbow axis, running down into the body as a tapered web
    p = np.stack([y - CY, z - CZ], -1)
    a = np.array([0.0, 0.0]); b = np.array([165.0 - CY, -8.0])
    ab = b - a; t = np.clip(((p - a) @ ab) / (ab @ ab), 0, 1)
    q = p - (a + t[..., None] * ab)
    rad = R_DISC + (18.5 - R_DISC) * t
    d2 = np.linalg.norm(q, axis=-1) - rad
    xm, hw = (x0 + x1) / 2, (x1 - x0) / 2
    dxx = np.abs(x - xm) - hw
    return smax(d2, dxx, 1.5)


def bridge_sdf(x, y, z):
    # C: posterior olecranon bridge between the cheeks, r 44.5..50, from straight down to 70 deg back
    re = np.sqrt((y - CY) ** 2 + (z - CZ) ** 2)
    ph = np.degrees(np.arctan2(-(z - CZ), -(y - CY)))  # 0 = straight down, +90 = posterior
    dr = np.maximum(R_CLEAR - re, re - 57.0)
    dphi = np.maximum(-5 - ph, ph - 58) * re * np.pi / 180
    dx = np.abs(x - (CHEEK_IN[1] + CHEEK_OUT[0]) / 2) - ((CHEEK_OUT[0] - CHEEK_IN[1]) / 2 + 2)
    return np.maximum(np.maximum(dr, dphi), dx)


def concept_mesh(key, h=1.1):
    c = CONCEPTS[key]
    xs = np.arange(118, 224 + DX, h); ys = np.arange(8, 246, h); zs = np.arange(-52, 52, h)
    X, Y, Z = np.meshgrid(xs, ys, zs, indexing='ij')
    d = body_sdf(c, X, Y, Z)
    for x0, x1 in (CHEEK_IN, CHEEK_OUT):
        d = smin(d, cheek_sdf(X, Y, Z, x0, x1), 14.0)
    if c['bridge']:
        d = smin(d, bridge_sdf(X, Y, Z), 9.0)
    # clear the elbow RS02, carrier and hoop between the cheeks
    re = np.sqrt((Y - CY) ** 2 + (Z - CZ) ** 2)
    xm, hw = (CHEEK_IN[1] + CHEEK_OUT[0]) / 2, (CHEEK_OUT[0] - CHEEK_IN[1]) / 2
    cut = np.maximum(re - R_CLEAR, np.abs(X - xm) - hw)
    d = smax(d, -cut, 2.0)
    v, f, _, _ = marching_cubes(d, 0.0, spacing=(h, h, h))
    v += [xs[0], ys[0], zs[0]]
    m = trimesh.Trimesh(v, f[:, ::-1], process=True)
    trimesh.smoothing.filter_taubin(m, iterations=6)
    if len(m.faces) > 90000:
        m = m.simplify_quadric_decimation(face_count=90000)
    m.fix_normals()
    return m


# ---- elbow pose and the rest of the scene
def rotx(m, deg):
    a = np.radians(-deg)    # flexion toward +Z
    c, s = np.cos(a), np.sin(a)
    T = np.eye(4); T[1, 1], T[1, 2], T[2, 1], T[2, 2] = c, -s, s, c
    T[:3, 3] = np.array([0, CY, CZ]) - T[:3, :3] @ np.array([0, CY, CZ])
    m = m.copy(); m.apply_transform(T); return m


def test_weights():
    parts = []
    ad = E.cyl(28.0, 10.0, 64); parts.append(('adapter', 'Test-weight wrist adapter, PETG, M8 stud captured', E.along(ad, [HX, Y_WRIST, HZ], [0, -1, 0]), '#d9d6cc'))
    stud = E.cyl(4.0, 112.0, 20); parts.append(('stud', 'M8 stud, height marks', E.along(stud, [HX, Y_WRIST - 10, HZ], [0, -1, 0]), E.C['screw']))
    pk = []
    for i in range(2):
        pk.append(E.along(E.cyl(30.0, 30.0, 64), [HX, Y_WRIST - 10.5 - 30.5 * i, HZ], [0, -1, 0]))
    parts.append(('pucks', '2 x 250 g puck, PLA, filled (stack 500 g)', trimesh.util.concatenate(pk), '#c9c3b2'))
    nut = trimesh.creation.cylinder(radius=7.5, height=6.5, sections=6); nut.apply_translation([0, 0, 3.25])
    parts.append(('nut', 'M8 nut', E.along(nut, [HX, Y_WRIST - 10.5 - 61.5, HZ], [0, -1, 0]), E.C['screw']))
    return parts


def wire_path(key):
    post = [(170.0, 232.0, -52.0), (170.0, 205.0, -56.5), (170.0, 182.0, -50.0), (171.0, 163.0, -38.0)]
    if key == 'A':
        inside = [(171.0, 150.0, -26.8), (171.0, 120.0, -29.8), (171.0, 95.0, -29.8), (171.0, 80.0, -27.5)]
    elif key == 'B':
        inside = [(195.0, 152.0, -18.0), (206.0, 140.0, -2.0), (205.0, 120.0, 2.0), (199.5, 95.0, 4.0), (198.0, 80.0, 6.0)]
    else:
        inside = [(170.0, 150.0, -31.0), (170.0, 120.0, -32.0), (170.0, 95.0, -30.5), (170.5, 80.0, -27.5)]
    return post + inside


def scene(key, shell, deg, ghost=False):
    P = []
    def add(k, label, mesh, col, alpha=1.0, fore=False):
        P.append(dict(key=k, label=label, mesh=rotx(mesh, deg) if fore else mesh, col=col, ex=np.zeros(3), alpha=alpha))
    B = {p['key']: p for p in E.build()}
    for k in ('arm', 'carrier', 'cover', 'rs02e', 'rs02y', 'brg', 'hs', 'hc', 'hw', 'cs', 'ci'):
        p = B[k]; add(k, p['label'], p['mesh'], p['col'])
    add('shell', CONCEPTS[key]['name'], shell, WHITE, alpha=0.28 if ghost else 1.0, fore=True)
    add('fb', B['fb']['label'], B['fb']['mesh'], E.C['screw'], fore=True)
    add('rs00', 'RS00', B['rs00']['mesh'], E.C['act'], fore=True)
    for k, label, m, col in test_weights():
        add(k, label, m, col, fore=True)
    if ghost:
        bore = E.along(E.cyl(BORE_D / 2, Y_SPLIT - (Y_WRIST + 2.5)), [HX, Y_WRIST + 2.5, HZ], [0, 1, 0])
        add('bore', 'Clear bore, 58 mm', bore, '#3c8d3c', alpha=0.45, fore=True)
        lo = Y_SPLIT; hi = CY - BORE_COM + BORE_REQ_L / 2
        add('conf', 'Requested bore overlapping the RS00', E.along(E.cyl(BORE_D / 2 + 0.5, hi - lo), [HX, lo, HZ], [0, 1, 0]), '#d0342c', alpha=0.35, fore=True)
        add('wire', 'Cable route', E.tube(wire_path(key), 2.6), E.C['wire'], fore=True)
    return P


def draw(ax, P, title, d, center, half, px=900):
    img, _ = E.render(P, d, [0, 1, 0] if abs(d[1]) < 0.9 else [0, 0, 1], px, center, half)
    ax.imshow(img); ax.set_title(title, fontsize=17, pad=6); ax.axis('off')


# ---- checks
def fold_and_fit(key, shell):
    B = {p['key']: p['mesh'] for p in E.build()}
    armset = trimesh.util.concatenate([B['arm'], B['carrier']])
    rng = np.random.default_rng(1)
    v = shell.vertices[rng.choice(len(shell.vertices), min(6000, len(shell.vertices)), replace=False)]
    out = {}
    for deg in (0, 60, 90, 124, 130):
        w = rotx(trimesh.PointCloud(v), deg).vertices
        sd = np.concatenate([trimesh.proximity.signed_distance(B['arm'], w[i:i + 3000]) for i in range(0, len(w), 3000)])
        sc = np.concatenate([trimesh.proximity.signed_distance(B['carrier'], w[i:i + 3000]) for i in range(0, len(w), 3000)])
        s = np.maximum(sd, sc)
        out[f'arm_pen_{deg}'] = round(float(max(s.max(), 0)), 2)
        out[f'arm_gap_{deg}'] = round(float(-s.max()), 2)
    # RS00 inside the envelope: wall room = distance from RS00 surface points to the envelope surface
    r0 = B['rs00'].vertices[rng.choice(len(B['rs00'].vertices), 4000, replace=False)]
    sd = trimesh.proximity.signed_distance(shell, r0)
    out['rs00_min_room'] = round(float(sd.min()), 2)
    # 58 mm bore room below the RS00: envelope radius minus 29
    ring = []
    for y in np.arange(Y_WRIST + 4, Y_SPLIT, 4):
        t = np.linspace(0, 2 * np.pi, 72, endpoint=False)
        pts = np.c_[HX + 29 * np.cos(t), np.full_like(t, y), HZ + 29 * np.sin(t)]
        ring.append(trimesh.proximity.signed_distance(shell, pts).min())
    out['bore_wall_min'] = round(float(min(ring)), 2)
    out['envelope_cm3'] = round(shell.volume / 1000, 1)
    out['area_cm2'] = round(shell.area / 100, 1)
    # rough printed mass: 2.5 mm skin (6 walls) + cheeks solid, PETG 1.27 g/cm3
    out['mass_est_g'] = round((shell.area * 2.5 + 2 * 9 * np.pi * 29 ** 2 * 0.6) / 1000 * 1.27)
    return out


if __name__ == '__main__':
    REBUILD = sys.argv[3] if len(sys.argv) > 3 else 'ABC'
    import os
    res = json.load(open(f'{OUT}/concept_checks.json')) if os.path.exists(f'{OUT}/concept_checks.json') else {}
    shells = {}
    for key in CONCEPTS:
        if key not in REBUILD:
            shells[key] = trimesh.load(f'{OUT}/concept_{key}.stl'); continue
        m = concept_mesh(key)
        shells[key] = m
        m.export(f'{OUT}/concept_{key}.stl')
        res[key] = fold_and_fit(key, m)
        print(key, res[key], flush=True)
    json.dump(res, open(f'{OUT}/concept_checks.json', 'w'), indent=1)
    ISO = np.array([-0.75, -0.3, -0.6])
    for key, m in shells.items():
        if key not in REBUILD:
            continue
        fig, ax = plt.subplots(2, 3, figsize=(30, 21))
        P0 = scene(key, m, 0); P90 = scene(key, m, 90)
        draw(ax[0, 0], P0, 'Elbow 0 deg, iso (outboard, front)', ISO, (172, 150, 0), 225)
        draw(ax[0, 1], P0, 'Elbow 0 deg, side (outboard)', [-1, 0, 0], (172, 150, 5), 225)
        draw(ax[0, 2], P0, 'Elbow 0 deg, front (anterior)', [0, -0.05, -1], (172, 150, 5), 225)
        draw(ax[1, 0], P90, 'Elbow 90 deg, iso', ISO, (172, 240, 90), 225)
        draw(ax[1, 1], P90, 'Elbow 90 deg, side (outboard)', [-1, 0, 0], (172, 240, 90), 225)
        draw(ax[1, 2], scene(key, m, 0, ghost=True), 'Elbow 0 deg, ghost: RS00, 58 mm bore (green), requested bore over the RS00 (red), cables (orange)', [-1, 0, 0], (172, 150, 5), 225)
        r = res[key]
        fig.suptitle(f'marengo left forearm, concept {CONCEPTS[key]["name"]}  |  envelope only, NOT DESIGNED  |  upper arm db71b23b  |  '
                     f'test weights: adapter + 2 x 250 g  |  fold gap 124 deg {r["arm_gap_124"]} mm, 130 deg {r["arm_gap_130"]} mm', fontsize=20, y=0.995)
        plt.tight_layout(rect=(0, 0, 1, 0.975)); plt.savefig(f'{OUT}/concept_{key}.png', dpi=70); plt.close(fig)
        print('rendered', key, flush=True)
    # side-by-side comparison
    fig, ax = plt.subplots(3, 3, figsize=(30, 31))
    for i, (key, m) in enumerate(shells.items()):
        P0 = scene(key, m, 0)
        draw(ax[0, i], P0, CONCEPTS[key]['name'] + ', iso', ISO, (172, 150, 0), 225)
        draw(ax[1, i], P0, CONCEPTS[key]['name'] + ', side (outboard), front to the left', [-1, 0, 0], (172, 150, 5), 225)
        draw(ax[2, i], P0, CONCEPTS[key]['name'] + ', front (anterior)', [0, -0.05, -1], (172, 150, 5), 225)
    fig.suptitle('marengo left forearm concepts, side by side  |  envelopes only, NOT DESIGNED  |  elbow 0 deg, test weights 2 x 250 g', fontsize=22, y=0.997)
    plt.tight_layout(rect=(0, 0, 1, 0.985)); plt.savefig(f'{OUT}/concepts_compare.png', dpi=70); plt.close(fig)
    print('done')
