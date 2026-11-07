"""RS00 support options: section through the forearm axis (plane Z = 5.99), C direct mount vs A support bearings.
Real meshes for the fork B, RS00 and distal tube; option A parts drawn from catalogue sizes."""
import sys
OUT = sys.argv[1]
sys.argv = ['x']
import numpy as np, trimesh
from shapely.geometry import Polygon, box
from shapely.ops import unary_union
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.patches import PathPatch, FancyArrowPatch, Rectangle
from matplotlib.path import Path
import elbow_diagram as E

HX, HZ = 170.49, 5.985875
S = 'C:/Users/joeyl/Documents/Codex/2026-10-29/task/LEFT_ARM_V1_LAYOUT_20261106/pkg/stl/fa/'
B = {p['key']: p['mesh'] for p in E.build()}
fork = trimesh.load(S + 'forearm_b.STL')
rs00 = B['rs00']
distal = trimesh.load('J:/code/marengo/cad/exports/forearm/marengo_forearm_distal_left.STL')
C = dict(fork='#e9e6dc', rs00='#8c9096', distal='#8fbbe0', brg='#a9adb3', coup='#f0c060', red='#d0342c', green='#2e9e4a', edge='#333333')


def sect(m, ymin=-200, ymax=400):
    s = m.section([0, 0, 1], [0, 0, HZ])
    geo = None
    for ent in s.discrete:
        if len(ent) < 4:
            continue
        p = Polygon(np.c_[ent[:, 0] - HX, ent[:, 1]]).buffer(0)
        geo = p if geo is None else geo.symmetric_difference(p)
    return geo.intersection(box(-60, ymin, 60, ymax))


def patch(ax, geo, fc, ec=C['edge'], lw=0.8, z=2, alpha=1.0, hatch=None):
    polys = getattr(geo, 'geoms', [geo])
    for g in polys:
        if g.is_empty or g.geom_type != 'Polygon':
            continue
        verts, codes = [], []
        for ring in [g.exterior] + list(g.interiors):
            xy = np.asarray(ring.coords)
            verts += xy.tolist(); codes += [Path.MOVETO] + [Path.LINETO] * (len(xy) - 2) + [Path.CLOSEPOLY]
        ax.add_patch(PathPatch(Path(verts, codes), fc=fc, ec=ec, lw=lw, zorder=z, alpha=alpha, hatch=hatch))


def rect(ax, x0, x1, y0, y1, fc, z=3, hatch=None, ec=C['edge'], lw=0.8, mirror=True):
    for s_ in ((1, -1) if mirror else (1,)):
        a, b = sorted((s_ * x0, s_ * x1))
        ax.add_patch(Rectangle((a, y0), b - a, y1 - y0, fc=fc, ec=ec, lw=lw, zorder=z, hatch=hatch))


def label(ax, txt, xy, xytext, col='#1d1d1f', fs=14):
    ax.annotate(txt, xy=xy, xytext=xytext, fontsize=fs, color=col, ha='left' if xytext[0] > xy[0] else 'right', va='center',
                arrowprops=dict(arrowstyle='-', lw=0.9, color='#555'), bbox=dict(boxstyle='round,pad=0.25', fc='white', ec='#bbb', lw=0.6), zorder=20)


def dim(ax, x0, x1, y, txt, fs=12):
    ax.annotate('', xy=(x0, y), xytext=(x1, y), arrowprops=dict(arrowstyle='<->', lw=0.9, color='#444'), zorder=15)
    ax.text((x0 + x1) / 2, y - 3.5, txt, ha='center', va='top', fontsize=fs, zorder=15, bbox=dict(fc='white', ec='none', pad=0.5))


def hand_load(ax, y_hand=-75.1):
    ax.add_patch(Rectangle((-30, y_hand - 25), 60, 50, fc='none', ec='#999', ls='--', lw=1, zorder=1))
    ax.text(0, y_hand + 6, 'hand / test weights\nCoM 90 mm past the wrist', ha='center', va='bottom', fontsize=11, color='#666')
    ax.add_patch(FancyArrowPatch((0, y_hand), (34, y_hand), arrowstyle='-|>', mutation_scale=22, lw=2.2, color='k', zorder=16))
    ax.text(36, y_hand, '49 N (5 kg)', fontsize=14, va='center', fontweight='bold')


fig = plt.figure(figsize=(20, 17))
gs = fig.add_gridspec(2, 2, height_ratios=[5.2, 1])
axs = [fig.add_subplot(gs[0, 0]), fig.add_subplot(gs[0, 1])]
txt = [fig.add_subplot(gs[1, 0]), fig.add_subplot(gs[1, 1])]
for t_ in txt: t_.axis('off')
YLO, YHI = -105, 250
fk = sect(fork, 60, YHI); r0 = sect(rs00); ds = sect(distal)
for ax in axs:
    ax.set_xlim(-128, 128); ax.set_ylim(YLO, YHI); ax.set_aspect('equal'); ax.axis('off')
    ax.axvline(0, color='#888', lw=0.8, ls='-.', zorder=0)
    ax.text(1, YLO + 2, 'forearm axis', ha='left', va='bottom', fontsize=10, color='#888')
    ax.text(0, YHI - 1, '↑ to the elbow', ha='center', va='top', fontsize=12, color='#666')

# ---------------- C: direct mount
ax = axs[0]
patch(ax, fk, C['fork']); patch(ax, r0, C['rs00']); patch(ax, ds, C['distal'])
ax.add_patch(Rectangle((-17.3, 89.9), 34.6, 6.0, fc=C['red'], ec=C['red'], alpha=0.55, zorder=6))
label(ax, 'RS00 output bearing (inside the RS00)\n≈ 8.1 N·m bending at 5 kg\nNO published rating', (10, 93), (38, 40), col=C['red'])
label(ax, 'forearm shell (fork B), static', (-33, 125), (-45, 150))
label(ax, 'RS00 housing\n6×M3 on Ø50 into the shell ring', (-26, 115), (-45, 112))
label(ax, 'distal tube, turns with the RS00\n6×M3 on Ø27 to the output', (-25, 50), (-45, 62))
label(ax, 'wrist flange: 6×M3 heat-set\non Ø50, Ø35 pilot', (-27, 16), (-45, 22))
hand_load(ax)
ax.add_patch(FancyArrowPatch((0, -50), (0, 85), connectionstyle='arc3,rad=0', arrowstyle='-|>', mutation_scale=18, lw=3, color=C['red'], alpha=0.8, zorder=12))
ax.text(-8, -30, 'load path:\nhand → tube →\nRS00 output bearing\n→ RS00 → shell', fontsize=13, color=C['red'], zorder=13, ha='right')
dim(ax, -32, 32, 4, 'Ø64')
ax.set_title('C   Direct mount (today)', fontsize=22, fontweight='bold', pad=8)
txt[0].text(0.02, 0.95, 'Added mass 0 g   ·   diameter unchanged Ø64   ·   cost 0\n'
        'Risk: the RS00 output bearing carries the whole hand moment\n(≈ 8 N·m at 5 kg, ≈ 1.7 N·m at the 1 kg working limit).\n'
        'RobStride publishes no output bearing rating (RS00/02/03/04 manuals).\n'
        'Mitigation: step up the test weights and check output play.', fontsize=15, va='top', transform=txt[0].transAxes,
        bbox=dict(fc='#fff6f5', ec=C['red'], lw=1))

# ---------------- A: support bearings
ax = axs[1]
patch(ax, fk.intersection(box(-60, 86.3, 60, YHI)), C['fork']); patch(ax, r0, C['rs00'])
# static sleeve: the shell continues down to the wrist, bore Ø52 at the bearings, Ø60 outside
YB1, YB2 = (79.0, 86.0), (23.0, 30.0)
rect(ax, 26.0, 30.0, 20.5, 86.3, C['fork'])
rect(ax, 23.5, 26.0, 20.5, 23.0, C['fork'])
# 6808-2RS pair, 40 x 52 x 7
for y0, y1 in (YB1, YB2):
    rect(ax, 20.0, 26.0, y0, y1, C['brg'], z=5)
    for s_ in (1, -1):
        ax.add_patch(plt.Circle((s_ * 23.0, (y0 + y1) / 2), 2.0, fc='white', ec='#555', lw=0.8, zorder=6))
# inner rotating tube Ø40 / Ø34, wrist flange below the sleeve
rect(ax, 17.0, 20.0, 19.86, 86.5, C['distal'])
rect(ax, 17.0, 30.0, 14.86, 19.86, C['distal'])
# torque coupling: plate on the RS00 output (Ø27 pattern) with 3 dogs into slots in the tube top, float all round
rect(ax, 0.0, 17.3, 86.9, 89.9, C['coup'], mirror=True, z=6)
rect(ax, 17.5, 20.5, 82.0, 87.0, C['coup'], z=7)
label(ax, 'printed torque coupling: plate on the RS00\noutput (6×M3 Ø27), 3 dogs in tube slots,\n0.3 mm float: torque only', (8, 88), (38, 118))
label(ax, '6808-2RS  40×52×7 (upper)', (24, 82.5), (38, 72), col='#333')
label(ax, '6808-2RS  40×52×7 (lower)', (24, 25.5), (38, 30), col='#333')
label(ax, 'shell continues as a static sleeve\nto the wrist, Ø60 outside', (-29, 50), (-45, 66))
label(ax, 'inner tube Ø40 / Ø34 turns\n(hand cables inside)', (-18.5, 40), (-45, 38))
label(ax, 'ribbon clock-spring in the\nannulus between the bearings', (-23, 55), (-45, 92))
label(ax, 'wrist flange turns below\nthe sleeve: 6×M3 on Ø50', (-29, 17), (-45, 8))
hand_load(ax)
gp = [(4, -50), (4, 18), (21, 25.5), (28, 30), (28, 82), (30, 100), (36, 140)]
ax.plot([p[0] for p in gp], [p[1] for p in gp], color=C['green'], lw=4, alpha=0.85, zorder=12, solid_capstyle='round')
ax.add_patch(FancyArrowPatch(gp[-2], gp[-1], arrowstyle='-|>', mutation_scale=22, lw=4, color=C['green'], zorder=12))
ax.text(40, 165, 'bending goes round the RS00:\ntube → 6808 pair → sleeve → shell', fontsize=11.5, color=C['green'], fontweight='bold', zorder=13)
ax.text(36, 52, 'span 57', ha='left', va='center', fontsize=12)
dim(ax, 30.5, 30.5, 0, '')
ax.annotate('', xy=(34, (YB1[0] + YB1[1]) / 2), xytext=(34, (YB2[0] + YB2[1]) / 2), arrowprops=dict(arrowstyle='<->', lw=0.9, color='#444'))
ax.set_title('A   Support bearings round the RS00 output', fontsize=22, fontweight='bold', pad=8)
txt[1].text(0.02, 0.95, 'Added ≈ +85 g: 2 × 6808-2RS ≈ 62 g, coupling ≈ 8 g,\nsleeve + inner tube ≈ 17 g more than today\'s tube\n'
        'Diameter: Ø64 → Ø60 below the RS00 (bearing OD 52 + 4 mm walls)\n'
        'Buy: 2 × 6808-2RS (same as the elbow), about $6–12.  RS00 bearing sees torque only.\n'
        'Downsides: printed bearing seats need a fit coupon; inner tube held axially\n'
        'at the wrist; clock-spring moves; hand cable bore shrinks Ø58 → Ø34.', fontsize=15, va='top', transform=txt[1].transAxes,
        bbox=dict(fc='#f3fbf5', ec=C['green'], lw=1))
fig.suptitle('Left forearm: how the RS00 carries the hand load\nsection through the forearm axis, arm frame, mm. Shell, RS00 and C tube from CAD; A parts from catalogue sizes', fontsize=17, y=0.995)
plt.tight_layout(rect=(0, 0, 1, 0.96))
plt.savefig(OUT, dpi=90)
print('saved', OUT)
