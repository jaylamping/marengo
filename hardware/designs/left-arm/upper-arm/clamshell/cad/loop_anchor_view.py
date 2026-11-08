"""Back loop at the chosen anchors (env LOOP_*), views from inboard (+X look) and 3/4 behind, elbow 0 / 60 / 124.
usage: python loop_anchor_view.py OUT.png X [extra STL for the upper arm half]"""
import sys, os
OUT, X = sys.argv[1], float(sys.argv[2])
EXTRA = sys.argv[3:]
sys.argv = ['x']
import numpy as np, trimesh
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import elbow_diagram as E
import loop_sweep as S

UA = [trimesh.load(p, force='mesh') for p in EXTRA] or [S.B['arm']]
fig, axs = plt.subplots(2, 3, figsize=(27, 18))
for j, th in enumerate((0, 60, 124)):
    pts, info = S.loop(th, X)
    a, f = S.P(48, np.radians(S.KW['pa']), X), S.P(48, np.radians(S.KW['pf'] + th), X)
    P = [dict(key=f'ua{i}', label='', mesh=m, col=E.C['print'], ex=np.zeros(3), alpha=0.6) for i, m in enumerate(UA)]
    P += [dict(key=k, label=k, mesh=m, col=c, ex=np.zeros(3), alpha=al) for k, m, c, al in (
        ('carrier', S.B['carrier'], E.C['print'], 0.7), ('rs02', S.B['rs02e'], E.C['act'], 1.0),
        ('fa', S.rot(S.FB, th), E.C['fork'], 0.6), ('rs00', S.rot(S.RS00, th), E.C['act'], 1.0),
        ('loom', E.tube(pts, 4.0), '#1d1d1f', 1.0))]
    for k, q in (('A', a), ('F', f)):
        P.append(dict(key=k, label=k, mesh=trimesh.creation.icosphere(2, 3.0).apply_translation(q), col='#ff6a13', ex=np.zeros(3), alpha=1.0))
    for i, (d, up) in enumerate((([1, 0, 0], [0, 1, 0]), ([0.62, -0.25, 0.75], [0, 1, 0]))):
        img, _ = E.render(P, d, up, 800, (150, 195, 0), 115)
        ax = axs[i, j]; ax.imshow(img); ax.axis('off')
        ax.set_title(f'elbow {th}°, {"from inboard" if i == 0 else "3/4 behind"}  ' + ', '.join(f'{k} {v}' for k, v in info.items()), fontsize=13)
fig.suptitle(f'Back loop, X {X}, anchors r 48 at {S.KW["pa"]}° (upper arm) / {S.KW["pf"]}° (forearm), free {S.KW["L"]} mm, 8 mm loom', fontsize=20)
plt.tight_layout(rect=(0, 0, 1, 0.96)); plt.savefig(OUT, dpi=55); print('saved', OUT)
