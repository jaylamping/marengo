"""Arm turntable with the planned harness: concept clamshell upper arm + forearm option A, elbow straight. Meshes only.
usage: python harness_turntable.py OUT_DIR [frames] [px]"""
import sys, os, time
OUTDIR = sys.argv[1]; NF = int(sys.argv[2]) if len(sys.argv) > 2 else 36; PX = int(sys.argv[3]) if len(sys.argv) > 3 else 640
sys.argv = ['x', 'clam']
import numpy as np, trimesh
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
src = open('clam_concept.py', encoding='utf-8').read()
exec(src[:src.index('def colour_parts')])          # up_h, lo_h, ring, hub_l, B, E, box, CY, CZ, HX, HZ

def loc(n):
    q = trimesh.load('stl/fa/' + n + '.STL', force='mesh'); q.apply_translation([HX, 0, HZ]); return q

def brg(y0):
    return E.along(E.ring(20.0, 26.0, 7.0, 64), [HX, y0, HZ], [0, 1, 0])

def smooth(pts, it=4):
    p = np.asarray(pts, float)
    for _ in range(it):
        q = [p[0]]
        for a, b in zip(p[:-1], p[1:]):
            q += [0.75 * a + 0.25 * b, 0.25 * a + 0.75 * b]
        q.append(p[-1]); p = np.array(q)
    return p

def bundle(path, wires):
    """wires: [(offset_vec, radius, colour, label)] offsets applied in a frame that follows the path."""
    p = smooth(path)
    out = []
    for off, r, col, lab in wires:
        q = []
        for i in range(len(p)):
            t = p[min(i + 1, len(p) - 1)] - p[max(i - 1, 0)]; t /= np.linalg.norm(t) + 1e-9
            a = np.cross(t, [1, 0, 0]); a = a / np.linalg.norm(a) if np.linalg.norm(a) > 0.2 else np.cross(t, [0, 0, 1]) / np.linalg.norm(np.cross(t, [0, 0, 1]))
            b = np.cross(t, a)
            q.append(p[i] + off[0] * a + off[1] * b)
        out.append((lab, E.tube(q[::2], r), col))
    return out

def helix(c, r, y0, y1, turns, n=120):
    t = np.linspace(0, 1, n); a = np.radians(270) + 2 * np.pi * turns * t
    return np.c_[c[0] + r * np.cos(a), y0 + (y1 - y0) * t, c[1] + r * np.sin(a)]

PRINT, PA, ACT = E.C['print'], 0.30, E.C['act']
parts = [('Upper arm, clamshell halves (concept)', up_h.union(lo_h) if False else trimesh.util.concatenate([up_h, lo_h]), PRINT, PA),
         ('RS02 seat ring (concept)', ring, PRINT, PA), ('Hub disc (concept)', hub_l, PRINT, PA),
         ('Upper-arm yaw RS02', B['rs02y'], ACT, 1.0), ('Elbow RS02', B['rs02e'], ACT, 1.0), ('Elbow carrier', B['carrier'], PRINT, PA),
         ('6808 elbow bearing', B['brg'], E.C['bearing'], 1.0),
         ('Forearm shell (fork B)', FB, PRINT, PA), ('RS00 wrist yaw', B['rs00'], ACT, 1.0), ('RS00 ring', loc('forearm_rs00_ring'), PRINT, PA),
         ('Sleeve', loc('forearm_sleeve'), PRINT, PA), ('Inner tube (turns)', loc('forearm_tube'), '#8fbbe0', 0.35), ('Wrist cap', loc('forearm_wrist_cap'), PRINT, PA),
         ('Wrist flange', loc('forearm_wrist_flange'), '#8fbbe0', 0.5), ('6808 pair', trimesh.util.concatenate([brg(69.0), brg(24.36)]), E.C['bearing'], 1.0)]

# ---- harness centre lines, arm frame (mm). Posterior = -Z. Seam channel on the -X inner wall at Z 6.
a = np.radians(np.linspace(0, 180, 9))
yaw_turn = [(HX + 46 * np.cos(t), 372 - 40 * t / np.pi, 6 + 46 * np.sin(t) * 0.0 + 0) for t in a]   # half turn round the yaw RS02 housing
trunk = yaw_turn + [(126.0, 328, 6), (137.0, 318, 6), (141, 300, 6), (141, 262, 6)]
power = [((0, -2.0), 1.45, '#d0342c', 'VBUS 16 AWG'), ((0, 2.0), 1.45, '#222222', 'GND 16 AWG'), ((0, 5.4), 0.65, '#e8c21a', 'CAN_H'), ((0, 6.8), 0.65, '#2e9e4a', 'CAN_L')]
H = []
H += bundle(trunk, power)
H += bundle([(141, 266, 6), (148, 258, 3), (155, 250, 3), (158, 246, 4)], power[:2] + power[2:])        # elbow RS02 drop to its plug
ribbon = [(141, 262, 6), (141, 252, -12), (141, 238, -34), (141, 224, -50), (141, 209.9, -52), (141, 196, -50), (150, 182, -42), (160, 168, -33), (170.49, 152, -25.5)]
H.append(('S4 14-way ribbon, rolling loop', E.tube(smooth(ribbon)[::2], 1.6), '#8e44ad'))
fa_chan = [(170.49, 152, -25.5), (170.49, 128, -25.5), (170.49, 100, -25.5), (170.49, 92, -24.0)]
H.append(('Forearm back channel', E.tube(smooth(fa_chan)[::2], 1.6), '#8e44ad'))
H.append(('RS00 drop', E.tube(smooth([(170.49, 128, -25.5), (163, 132, -22), (155, 136, -14)])[::2], 1.2), '#d0342c'))
hand = [(170.49, 92, -24.0), (170.49, 87, -23.0), (170.49, 80, -20.3), (170.49, 58, -20.3)]
coil = helix((HX, HZ), 23.8, 56.0, 47.0, 1.5)
hand2 = list(coil) + [(170.49, 46, -13.3), (170.49, 40, -6), (170.49, 22, 0), (170.49, 8, 2)]
H.append(('Hand feed: ring and sleeve slots', E.tube(smooth(hand)[::2], 1.0), '#c2185b'))
H.append(('Hand feed: clock-spring, tube, hand', E.tube(np.asarray(hand2)[::2], 1.0), '#c2185b'))
fuses = [('Fuse holder (wall cradle)', box(146.5, 157.5, 261, 275, -3, 5), '#3a3f4a'), ('Splice M14 (wall cradle)', box(146.5, 157.5, 279, 293, -3, 5), '#3a3f4a'),
         ('Splice M15', box(165, 176, 122, 134, -29, -22), '#3a3f4a'), ('Strain relief (shoulder)', box(122, 132, 322, 330, 1, 11), '#555'),
         ('Loop guard (concept)', None, None)]
guard = trimesh.creation.annulus(r_min=55, r_max=57, height=16, sections=64)
guard.apply_transform(trimesh.transformations.rotation_matrix(np.pi / 2, [0, 1, 0])); guard.apply_translation([141, CY, CZ])
guard = guard.slice_plane([0, CY, CZ], [0, 0, -1]) if hasattr(guard, 'slice_plane') else guard
P = [dict(key=l, label=l, mesh=m, col=c, ex=np.zeros(3), alpha=al) for l, m, c, al in parts]
P += [dict(key=l, label=l, mesh=m, col=c, ex=np.zeros(3), alpha=1.0) for l, m, c in H]
P += [dict(key=l, label=l, mesh=m, col=c, ex=np.zeros(3), alpha=1.0) for l, m, c in fuses if m is not None]
P.append(dict(key='Loop guard (concept)', label='Loop guard (concept)', mesh=guard, col='#ff6a13', alpha=0.18, ex=np.zeros(3)))
CEN, HALF = (170, 205, 2), 205
TAG = 'CONCEPT: harness path and elbow crossing not final'

def view(th):
    return [np.sin(th), -0.28, -np.cos(th)]

LABELS = {'Upper-arm yaw RS02', 'Hub disc (concept)', 'Upper arm, clamshell halves (concept)', 'RS02 seat ring (concept)', 'Elbow RS02', 'Elbow carrier',
          'Forearm shell (fork B)', 'RS00 wrist yaw', 'RS00 ring', 'Sleeve', 'Wrist flange', 'VBUS 16 AWG', 'CAN_H', 'Fuse holder (wall cradle)',
          'Splice M14 (wall cradle)', 'Strain relief (shoulder)', 'S4 14-way ribbon, rolling loop', 'Loop guard (concept)', 'Splice M15', 'RS00 drop',
          'Hand feed: clock-spring, tube, hand', 'Hand feed: ring and sleeve slots'}
NICE = {'VBUS 16 AWG': 'Trunk power pair, red/black 16 AWG', 'CAN_H': 'CAN pair, yellow/green 26 AWG'}


def crop(img, anc, pad=12):
    m = np.any(np.abs(img - 0.985) > 0.02, axis=2)
    ys, xs = np.nonzero(m)
    y0, y1, x0, x1 = max(ys.min() - pad, 0), ys.max() + pad, max(xs.min() - pad, 0), xs.max() + pad
    return img[y0:y1, x0:x1], {k: (v[0] - x0, v[1] - y0) for k, v in anc.items()}


def still(ax, th, px=1100):
    img, anc = E.render(P, view(th), [0, 1, 0], px, CEN, HALF)
    img, anc = crop(img, anc)
    h, w = img.shape[:2]
    ax.imshow(img); ax.set_xlim(-0.9 * w, 1.9 * w); ax.set_ylim(h, 0); ax.axis('off')
    keys = [k for k in anc if k in LABELS]
    L = sorted([k for k in keys if anc[k][0] < w / 2], key=lambda k: anc[k][1]); R = sorted([k for k in keys if anc[k][0] >= w / 2], key=lambda k: anc[k][1])
    for side, items in ((-1, L), (1, R)):
        for i, k in enumerate(items):
            ty = h * (0.03 + 0.94 * (i + 0.5) / max(len(items), 1)); tx = -0.04 * w if side < 0 else 1.04 * w
            ax.annotate(NICE.get(k, k), xy=anc[k], xytext=(tx, ty), fontsize=11, ha='right' if side < 0 else 'left', va='center',
                        color='#b04000' if 'concept' in k.lower() or 'ribbon' in k or 'guard' in k.lower() else '#1d1d1f',
                        arrowprops=dict(arrowstyle='-', lw=0.6, color='#777'), bbox=dict(boxstyle='round,pad=0.2', fc='white', ec='#bbb', lw=0.5))


if __name__ == '__main__':
    os.makedirs(OUTDIR, exist_ok=True)
    fig, ax = plt.subplots(2, 4, figsize=(44, 30))
    for i, a_ in enumerate(ax.ravel()):
        still(a_, np.radians(45 * i)); a_.set_title(f'view {45 * i}°', fontsize=18)
    fig.suptitle('Left arm, upper-arm yaw RS02 to wrist flange, elbow straight, with the planned harness.  ' + TAG +
                 '.\nPrinted parts translucent white, actuators grey, turning parts blue. Purple = 14-way elbow ribbon, magenta = hand feed.', fontsize=24)
    plt.tight_layout(rect=(0, 0, 1, 0.95)); plt.savefig(os.path.join(OUTDIR, 'arm_harness_views.png'), dpi=50); plt.close(fig)
    print('stills done', flush=True)
    import imageio.v2 as iio
    frames = []; t0 = time.time()
    for k in range(NF):
        img, _ = E.render(P, view(2 * np.pi * k / NF), [0, 1, 0], PX, CEN, HALF)
        w = img.shape[1]; img = img[:, int(0.22 * w):int(0.78 * w)]
        im = (np.clip(img, 0, 1) * 255).astype(np.uint8)
        fig = plt.figure(figsize=(im.shape[1] / 100, im.shape[0] / 100 + 0.3), dpi=100); plt.imshow(im); plt.axis('off')
        plt.title(TAG, fontsize=7, color='#b04000'); plt.tight_layout(pad=0.1)
        fig.canvas.draw(); fr = np.asarray(fig.canvas.buffer_rgba())[..., :3].copy(); plt.close(fig)
        frames.append(fr[: fr.shape[0] // 2 * 2, : fr.shape[1] // 2 * 2])
        print('frame', k, round(time.time() - t0), flush=True)
    iio.mimsave(os.path.join(OUTDIR, 'arm_harness_turntable.mp4'), frames, fps=12, quality=8, macro_block_size=2)
    from PIL import Image
    ims = [Image.fromarray(f).resize((f.shape[1] * 2 // 3, f.shape[0] * 2 // 3)).quantize(colors=128) for f in frames]
    ims[0].save(os.path.join(OUTDIR, 'arm_harness_turntable.gif'), save_all=True, append_images=ims[1:], duration=90, loop=0, optimize=True)
    print('done')
