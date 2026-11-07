"""Left elbow assembly diagrams, assembled and exploded, from the current meshes in arm coordinates.
Arm frame: X along the elbow axis (inboard -X), Y up the arm (shoulder +Y), Z sideways.
Elbow axis through (Y 209.860668, Z 5.985875); tube and hub axis along Y through (X 170.49, Z 5.99)."""
import sys
import numpy as np
import trimesh
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.colors import to_rgb

T = 'C:/Users/joeyl/Documents/Codex/2026-10-29/task/LEFT_ARM_V1_LAYOUT_20261106/pkg/stl/'
V = 'J:/code/marengo/cad/reviews/upper-arm/20261106_upper-arm-revc/scripts/stl/'
F = 'J:/code/marengo/cad/exports/forearm/'
OUT = sys.argv[1] if len(sys.argv) > 1 else '.'
COMMIT = sys.argv[2] if len(sys.argv) > 2 else ''
CY, CZ = 209.860668, 5.985875
HX, HZ = 170.49, 5.99

C = {'print': '#f2f0ea', 'cover': '#e4e2dc', 'act': '#8c9096', 'screw': '#5d6167', 'washer': '#d4a537',
     'insert': '#c08a2c', 'bearing': '#a9adb3', 'fork': '#4f8fc0', 'distal': '#8fbbe0', 'fuse': '#3a3f4a',
     'wire': '#ff6a13', 'guard': '#ff6a13'}


def load(p):
    return trimesh.load(p, force='mesh')


def xf(m, M, t):
    m = m.copy()
    A = np.eye(4); A[:3, :3] = M; A[:3, 3] = t
    m.apply_transform(A)
    return m


def along(mesh_z, p0, axis):
    """Place a mesh built along +Z (from z=0 upward) at p0, pointing along axis."""
    a = np.asarray(axis, float); a /= np.linalg.norm(a)
    T_ = trimesh.geometry.align_vectors([0, 0, 1], a)
    T_[:3, 3] = p0
    m = mesh_z.copy(); m.apply_transform(T_)
    return m


def cyl(r, h, n=40):
    m = trimesh.creation.cylinder(radius=r, height=h, sections=n); m.apply_translation([0, 0, h / 2]); return m


def ring(r0, r1, h, n=48):
    m = trimesh.creation.annulus(r_min=r0, r_max=r1, height=h, sections=n); m.apply_translation([0, 0, h / 2]); return m


def button(dk, k, d, L):
    head = cyl(dk / 2, k); sh = cyl(d / 2 * 0.95, L); sh.apply_translation([0, 0, k]); return trimesh.util.concatenate([head, sh])


def shcs(dk, k, d, L):
    return button(dk, k, d, L)


def tube(pts, r=2.6, dash=None):
    out = []
    pts = np.asarray(pts, float)
    segs = []
    for a, b in zip(pts, pts[1:]):
        L = np.linalg.norm(b - a); n = max(1, int(L / 2.0))
        for i in range(n):
            segs.append((a + (b - a) * i / n, a + (b - a) * (i + 1) / n))
    for i, (a, b) in enumerate(segs):
        if dash and (i // dash) % 2:
            continue
        L = np.linalg.norm(b - a)
        if L < 1e-6:
            continue
        out.append(along(cyl(r, L, 14), a, b - a))
        out.append(along(trimesh.creation.icosphere(1, r), a, [0, 0, 1]))
    return trimesh.util.concatenate(out)


def build():
    P = []   # (key, label, mesh, colour, group, explode vectors {'x': , 'y': }, alpha)

    def add(key, label, mesh, col, ex=(0, 0, 0), alpha=1.0):
        P.append(dict(key=key, label=label, mesh=mesh, col=col, ex=np.asarray(ex, float), alpha=alpha))

    add('arm', 'Upper arm, printed PETG (db71b23b)', load(T + 'UA2_ARM.STL'), C['print'])
    add('carrier', 'Carrier, printed PETG', load(T + 'UA2_CARRIER.STL'), C['print'], ex=(-55, 0, 0))
    add('cover', 'Cable-window cover, printed PLA, 4 x M3 x 6 + 4 heat-set inserts', load(T + 'UA2_COVER.STL'), C['cover'], ex=(0, 0, -40))
    # elbow RS02: shoulder on the hoop seat at X 165.19 (vendor model: shoulder at local z -30.37)
    rs = load(V + 'ELBOW_RS02.STL')
    ph = np.radians(90.0)  # plugs toward the port slot and the lower port relief
    c, s = np.cos(ph), np.sin(ph)
    M = np.array([[0, 0, 1], [c, -s, 0], [s, c, 0]], float)
    add('rs02e', 'Elbow actuator, RobStride RS02', xf(rs, M, [165.19 + 30.37, CY, CZ]), C['act'], ex=(55, 0, 0))
    # yaw RS02: output hub face on the hub at Y 319.86, body up the arm
    ry = load(V + 'ELBOW_RS02.STL')  # same RS02 model, in its own frame
    My = np.array([[1, 0, 0], [0, 0, -1], [0, 1, 0]], float)
    add('rs02y', 'Upper-arm yaw actuator, RobStride RS02 (hub end only)', xf(ry, My, [HX, 319.86 + 3.5, HZ]), C['act'], ex=(0, 72, 0))
    # 6808-2RS in the carrier seat, X 143.0 to 150.0
    add('brg', '6808-2RS bearing, 40 x 52 x 7', along(ring(20.0, 26.0, 7.0), [143.0, CY, CZ], [1, 0, 0]), C['bearing'], ex=(-40, 0, 0))
    # forearm (not released): fork + distal, RS00 in the skirt
    add('fork', 'Forearm fork, printed PETG (NOT RELEASED)', load(F + 'marengo_forearm_left.STL'), C['fork'], ex=(0, -70, 0))
    add('distal', 'Forearm distal tube, printed (NOT RELEASED)', load(F + 'marengo_forearm_distal_left.STL'), C['distal'], ex=(0, -125, 0))
    r0 = load(V + 'FOREARM_RS00.STL')
    M0 = np.array([[1, 0, 0], [0, 0, 1], [0, -1, 0]], float)
    add('rs00', 'Forearm yaw actuator, RobStride RS00', xf(r0, M0, [170.32, 95.3, 6.0]), C['act'], ex=(0, -98, 0))  # output end down on the distal boss at Y 89.9; stator on the fork lip
    # hoop screws: M3 x 24 button head, DIN 6796, DIN 9021, from the carrier side into the RS02 shoulder
    hs, hw, hc = [], [], []
    for th in (63.9, 103.9, 143.9, 223.9, 263.9, 303.9):
        a = np.radians(th); y, z = CY - 36.5 * np.cos(a), CZ + 36.5 * np.sin(a)
        hw.append(along(ring(1.6, 4.5, 0.8), [148.95, y, z], [1, 0, 0]))
        hc.append(along(ring(1.6, 3.5, 0.6), [148.35, y, z], [1, 0, 0]))
        hs.append(along(button(5.7, 1.65, 3.0, 24.0), [146.7, y, z], [1, 0, 0]))
    add('hs', '6 x M3 x 25 cut to 24, button head, 0.35 N m, dry', trimesh.util.concatenate(hs), C['screw'], ex=(-85, 0, 0))
    add('hc', '6 x DIN 6796 M3 conical spring washer, cone to the head', trimesh.util.concatenate(hc), C['washer'], ex=(-75, 0, 0))
    add('hw', '6 x DIN 9021 M3 washer, in the carrier seats', trimesh.util.concatenate(hw), C['washer'], ex=(-66, 0, 0))
    # 6808 retainers: M3 x 6 + DIN 9021 on the outer race
    rs_, rw = [], []
    for y, z in ((179.97, 7.99), (222.84, -20.58), (235.45, -8.76)):
        rw.append(along(ring(1.6, 4.5, 0.8), [142.2, y, z], [1, 0, 0]))
        rs_.append(along(button(5.7, 1.65, 3.0, 6.0), [140.55, y, z], [1, 0, 0]))
    add('rs', '3 x M3 x 6 button head, bearing retainers', trimesh.util.concatenate(rs_), C['screw'], ex=(-78, 0, 0))
    add('rw', '3 x DIN 9021 M3 washer, clamp the 6808 outer race', trimesh.util.concatenate(rw), C['washer'], ex=(-70, 0, 0))
    # hub screws: M4 x 30 socket head + 3 x DIN 125, from inside the hub into the yaw RS02 output
    hub_xz = [(159.38, 10.51), (179.97, 13.36), (168.83, 17.86), (172.12, -5.9), (181.62, 1.46), (161.01, -1.37)]
    ub, uw = [], []
    for x, z in hub_xz:
        for k in range(3):
            uw.append(along(ring(2.15, 4.5, 0.8), [x, 298.06 - 0.8 * (k + 1), z], [0, 1, 0]))
        ub.append(along(shcs(7.0, 4.0, 4.0, 30.0), [x, 298.06 - 2.4 - 4.0, z], [0, 1, 0]))
    add('ub', '6 x M4 x 30 socket head, hub to yaw RS02 output, 0.6 N m, dry', trimesh.util.concatenate(ub), C['screw'], ex=(0, 32, 0))
    add('uw', '18 x DIN 125 M4 washer, three under each hub screw', trimesh.util.concatenate(uw), C['washer'], ex=(0, 42, 0))
    # fork screws: M4 x 14 button heads, outboard cheek into the elbow RS02 output
    fb = []
    for y, z in ((209.84, 18.01), (209.84, -5.99), (199.46, 0.0), (199.46, 11.98), (220.26, 0.0), (220.26, 11.97)):
        fb.append(along(button(7.6, 2.2, 4.0, 14.0), [204.89, y, z], [-1, 0, 0]))
    add('fb', '6 x M4 x 14 button head, fork to elbow RS02 output (bought with the forearm)', trimesh.util.concatenate(fb), C['screw'], ex=(30, -70, 0))
    # cover screws and inserts
    cov = load(T + 'UA2_COVER.STL'); I = trimesh.ray.ray_triangle.RayMeshIntersector(cov)
    cs, ci = [], []
    for (x, y, zi0, zi1) in ((155.49, 268.0, -25.55, -19.96), (185.49, 268.0, -25.55, -19.96), (155.49, 304.0, -29.16, -23.46), (185.49, 304.0, -29.16, -23.46)):
        loc, _, _ = I.intersects_location([[x, y, -80]], [[0, 0, 1]], multiple_hits=False)
        zc = float(loc[0, 2]) if len(loc) else -33.0
        cs.append(along(button(5.7, 1.65, 3.0, 6.0), [x, y, zc - 1.65], [0, 0, 1]))
        ci.append(along(ring(1.6, 2.1, zi1 - zi0), [x, y, zi0], [0, 0, 1]))
    add('cs', '4 x M3 x 6 button head, cover', trimesh.util.concatenate(cs), C['screw'], ex=(0, 0, -55))
    add('ci', '4 x M3 heat-set insert, in the cover bosses', trimesh.util.concatenate(ci), C['insert'], ex=(0, 0, -12))
    # fuse holders in the two pockets on the wall opposite the window
    fu = [trimesh.creation.box(bounds=[[157.0, y0, 24.5], [184.0, y1, 31.4]]) for y0, y1 in ((262.2, 273.7), (276.1, 287.6))]
    add('fuse', '2 x inline mini-blade fuse holder, hook-and-loop in the pockets', trimesh.util.concatenate(fu), C['fuse'], ex=(0, 0, 45))
    # wiring: fused trunk (XT30 power + CAN) and the planned rolling loop
    trunk = [(171, 352, -46), (171, 318, -40), (171, 304, -31), (171, 296, -14), (171, 291, 14), (171, 282, 22),
             (171, 262, 18), (166, 252, 10), (162, 244.5, 6.0)]
    add('trunk', 'Fused trunk: XT30 power + CAN, through the cable window, fuses, port slot to the elbow RS02', tube(trunk, 2.8), C['wire'])
    yaw_lead = [(171, 352, -46), (190, 352, -36), (205, 345, -20)]
    add('yawlead', 'Lead to the yaw RS02 (shoulder side)', tube(yaw_lead, 2.2), C['wire'])
    loop = [(171, 300, -31), (171, 290, -44), (171, 255, -50), (171, 222, -52), (171, 200, -52), (171, 178, -44),
            (171, 160, -32), (171, 140, -26), (171, 122, -24)]
    add('loop', 'PLANNED, NOT DESIGNED: rolling ribbon loop to the RS00, under a printed guard', tube(loop, 2.4, dash=2), C['wire'], alpha=0.75)
    g = load(T + 'GUARD.STL')
    add('guard', 'PLANNED, NOT DESIGNED: elbow cable guard (draft outline)', g, C['guard'], alpha=0.18)
    return P


def render(P, d, up, px, center, half, explode=None, scale=1.0, show=None):
    d = np.asarray(d, float); d /= np.linalg.norm(d); up = np.asarray(up, float)
    r = np.cross(d, up); r /= np.linalg.norm(r); u = np.cross(r, d)
    zb = np.full((px, px), np.inf); img = np.ones((px, px, 3)) * 0.985; own = np.full((px, px), -1)
    over = []
    L = -d + 0.55 * u + 0.4 * r; L /= np.linalg.norm(L)
    anchors = {}
    s = px / (2 * half)
    for pi, p in enumerate(P):
        if show and p['key'] not in show:
            continue
        m = p['mesh']
        off = p['ex'] * scale if explode else 0
        Vv = m.vertices + off - np.asarray(center)
        X = (Vv @ r + half) * s; Y = (half - Vv @ u) * s; Z = Vv @ d
        n = m.face_normals
        sh = 0.32 + 0.6 * np.clip(n @ L, 0, 1) + 0.12 * np.clip(-(n @ d), 0, 1)
        col = np.array(to_rgb(p['col']))
        tgt = zb if p['alpha'] >= 0.99 else None
        layer = np.full((px, px), np.inf) if tgt is None else None
        limg = np.zeros((px, px, 3)) if tgt is None else None
        for f, c in zip(m.faces, sh):
            xs, ys, zs = X[f], Y[f], Z[f]
            x0, x1 = int(max(xs.min(), 0)), int(min(xs.max() + 1, px)); y0, y1 = int(max(ys.min(), 0)), int(min(ys.max() + 1, px))
            if x1 <= x0 or y1 <= y0:
                continue
            den = (ys[1] - ys[2]) * (xs[0] - xs[2]) + (xs[2] - xs[1]) * (ys[0] - ys[2])
            if abs(den) < 1e-12:
                continue
            gx, gy = np.meshgrid(np.arange(x0, x1) + .5, np.arange(y0, y1) + .5)
            a = ((ys[1] - ys[2]) * (gx - xs[2]) + (xs[2] - xs[1]) * (gy - ys[2])) / den
            b = ((ys[2] - ys[0]) * (gx - xs[2]) + (xs[0] - xs[2]) * (gy - ys[2])) / den
            cc = 1 - a - b; ins = (a >= -1e-6) & (b >= -1e-6) & (cc >= -1e-6)
            z = a * zs[0] + b * zs[1] + cc * zs[2]
            buf = tgt if tgt is not None else layer
            sub = buf[y0:y1, x0:x1]; w = ins & (z < sub); sub[w] = z[w]
            (img if tgt is not None else limg)[y0:y1, x0:x1][w] = np.clip(col * c, 0, 1)
            if tgt is not None:
                own[y0:y1, x0:x1][w] = pi
        if tgt is None:
            over.append((layer, limg, p['alpha'], pi))
        # anchor = a visible-ish point: mesh vertex nearest the mesh centroid, projected
        cen = m.vertices.mean(0) + off
        vi = np.argmin(np.linalg.norm(m.vertices + off - cen, axis=1))
        q = m.vertices[vi] + off - np.asarray(center)
        anchors[p['key']] = ((q @ r + half) * s, (half - q @ u) * s)
    vis = {}
    for layer, limg, al, pi in over:
        w = layer < zb
        img[w] = img[w] * (1 - al) + limg[w] * al
        vis[pi] = np.argwhere(w)
    for pi, p in enumerate(P):
        pts = vis.get(pi, np.argwhere(own == pi))
        if len(pts) < 25:
            anchors.pop(p['key'], None)
            continue
        med = np.median(pts, 0)
        q = pts[np.argmin(np.linalg.norm(pts - med, axis=1))]
        anchors[p['key']] = (q[1] + 0.5, q[0] + 0.5)
    return img, anchors


SEEN = set()


def panel(ax, P, title, d, up, center, half, px, explode=False, scale=1.0):
    img, anc = render(P, d, up, px, center, half, explode, scale)
    SEEN.update(anc)
    ax.imshow(img, extent=[0, px, px, 0]); ax.set_xlim(-0.95 * px, 1.95 * px); ax.set_ylim(px * 1.02, -px * 0.02); ax.axis('off')
    ax.set_title(title, fontsize=26, pad=10)
    keys = [p for p in P if p['key'] in anc]
    left = sorted([p for p in keys if anc[p['key']][0] < px / 2], key=lambda p: anc[p['key']][1])
    right = sorted([p for p in keys if anc[p['key']][0] >= px / 2], key=lambda p: anc[p['key']][1])
    for side, items in ((-1, left), (1, right)):
        n = len(items)
        for i, p in enumerate(items):
            x, y = anc[p['key']]
            ty = px * (0.04 + 0.92 * (i + 0.5) / max(n, 1))
            tx = -0.03 * px if side < 0 else 1.03 * px
            ax.annotate(p['label'], xy=(x, y), xytext=(tx, ty), fontsize=15, ha='right' if side < 0 else 'left', va='center',
                        color='#b04000' if 'PLANNED' in p['label'] or 'NOT RELEASED' in p['label'] else '#1d1d1f',
                        arrowprops=dict(arrowstyle='-', lw=0.8, color='#555', shrinkA=2, shrinkB=0),
                        bbox=dict(boxstyle='round,pad=0.25', fc='white', ec='#bbb', lw=0.6))
    return anc


if __name__ == '__main__':
    P = build()
    head = f'marengo left elbow assembly  |  upper arm {COMMIT or "db71b23b"}  |  forearm NOT RELEASED  |  loop and guard PLANNED, NOT DESIGNED  |  arm frame, mm'
    ctr, half, px = (172, 190, 0), 180, 1500
    views = [('Iso, outboard side', [-0.75, -0.3, -0.6]), ('Front: outboard side (fork cheek, elbow RS02)', [-1, -0.02, -0.12]),
             ('Back: inboard side (carrier, 6808, hoop screws)', [1, -0.02, 0.12])]
    fig, ax = plt.subplots(3, 1, figsize=(34, 54))
    for a_, (ti, d) in zip(ax, views):
        panel(a_, P, ti, d, [0, 1, 0], ctr, half, px)
    fig.suptitle(head, fontsize=28, y=0.997)
    plt.tight_layout(rect=(0, 0, 1, 0.99)); plt.savefig(OUT + '/ELBOW_ASSEMBLY.png', dpi=80); plt.close(fig)
    fig, ax = plt.subplots(2, 1, figsize=(34, 36))
    panel(ax[0], P, 'Exploded along the elbow axis (X); forearm parts drop along Y', [-0.55, -0.25, -0.8], [0, 1, 0], (170, 175, 0), 230, px, explode=True)
    hub = [p for p in P if p['key'] in ('arm', 'rs02y', 'ub', 'uw', 'cover', 'cs', 'ci', 'fuse', 'trunk', 'yawlead')]
    panel(ax[1], hub, 'Exploded along the hub axis (Y); cover off along Z', [-0.6, -0.2, -0.8], [0, 1, 0], (172, 325, -5), 120, px, explode=True)
    fig.suptitle(head, fontsize=28, y=0.997)
    plt.tight_layout(rect=(0, 0, 1, 0.99)); plt.savefig(OUT + '/ELBOW_EXPLODED.png', dpi=80); plt.close(fig)
    missing = [p['label'] for p in P if p['key'] not in SEEN]
    print('labelled in at least one view:', len(SEEN), 'of', len(P), '; never visible:', missing)
    print('done')
