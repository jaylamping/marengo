"""Picture sheet for the upper-arm clamshell draft: assembled and exploded, two views. usage: python ua_sheet.py OUT.png"""
import sys
OUT = sys.argv[1]; sys.argv = ['x']
import numpy as np, trimesh, matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import elbow_diagram as E
U = 'stl/ua/'
B = {p['key']: p['mesh'] for p in E.build()}
P0 = [('upper half', 'ua_upper', '#e9e5da', (0, 0, 45)), ('lower half', 'ua_lower', '#d9d4c6', (0, 0, -45)), ('seat ring', 'ua_ring', '#b8c7d6', (0, -35, 0)),
      ('hub disc', 'ua_hub', '#c9d6b8', (0, 35, 0)), ('loop clip', 'ua_loop_clip', '#ff6a13', (-30, 0, 0))]
fig, axs = plt.subplots(2, 2, figsize=(26, 26))
for i, (d, lab) in enumerate((([0.55, -0.35, 0.75], 'from behind, inboard'), ([-0.55, -0.35, -0.75], 'from the front, outboard'))):
    for j, ex in enumerate((False, True)):
        P = [dict(key=n, label=n, mesh=trimesh.load(U + f + '.STL', force='mesh'), col=c, ex=np.array(e, float), alpha=1.0) for n, f, c, e in P0]
        P += [dict(key='carrier', label='carrier (unchanged)', mesh=B['carrier'], col='#9aa0a8', ex=np.array((-55 if ex else 0, 0, 0), float), alpha=0.8),
              dict(key='rs02', label='RS02', mesh=B['rs02e'], col=E.C['act'], ex=np.array((55 if ex else 0, 0, 0), float), alpha=1.0)]
        img, _ = E.render(P, d, [0, 1, 0], 900, (170, 250, 5), 115, explode=ex, scale=1.0)
        ax = axs[i, j]; ax.imshow(img); ax.axis('off'); ax.set_title(f'{"exploded" if ex else "assembled"}, {lab}', fontsize=22)
fig.suptitle('Upper arm clamshell draft 3: halves (Z split), seat ring, hub disc, loop clip on the unchanged carrier. All screw heads on the back (lower) half', fontsize=24)
plt.tight_layout(rect=(0, 0, 1, 0.97)); plt.savefig(OUT, dpi=45); print('saved', OUT)
