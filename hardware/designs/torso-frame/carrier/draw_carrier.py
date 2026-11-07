"""Front-view drawing of the board carrier with the cradle pads and the rev B board outline. Run: python3 draw_carrier.py"""
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.patches import FancyBboxPatch, Rectangle, Circle
from build_carrier import X0, X1, Y0, Y1, CARRIER_SCREWS, STANDOFFS, WINDOWS

fig, ax = plt.subplots(figsize=(7, 8.6), dpi=150)
ax.add_patch(FancyBboxPatch((X0, Y0), X1 - X0, Y1 - Y0, boxstyle='round,pad=0,rounding_size=4', fc='#e9e4d4', ec='#333', lw=1.5, label='Carrier 110 x 150 x 4 PETG'))
for x0, x1, y0, y1 in WINDOWS:
    ax.add_patch(FancyBboxPatch((x0, y0), x1 - x0, y1 - y0, boxstyle='round,pad=0,rounding_size=6', fc='white', ec='#333', lw=1))
for sx in (1, -1):
    for y in (200, 330):
        ax.add_patch(Rectangle((min(sx * 48, sx * 55.5), y - 5), 7.5, 10, fc='#9ec5e8', ec='#2a5d8a', lw=0.8, alpha=0.8, zorder=2))
ax.add_patch(Rectangle((-50, 190), 100, 150, fill=False, ec='#1f7a3a', lw=1.4, ls='--', label='Power board rev B outline (front)'))
for x, y in STANDOFFS:
    ax.add_patch(Circle((x, y), 3.25, fc='#c9b86a', ec='#6b5a12', zorder=3)); ax.add_patch(Circle((x, y), 1.7, fc='white', ec='#6b5a12', zorder=4))
for x, y in CARRIER_SCREWS:
    ax.add_patch(Circle((x, y), 3.1, fc='#d9534f', ec='#7a1f1d', zorder=3)); ax.add_patch(Circle((x, y), 1.7, fc='white', ec='#7a1f1d', zorder=4))
ax.plot([], [], 's', color='#9ec5e8', label='Cradle carrier pads (behind), M3 inserts')
ax.plot([], [], 'o', color='#d9534f', label='4 x M3 x 8 countersunk, front, into the pads')
ax.plot([], [], 'o', color='#c9b86a', label='6 x standoff, M3 x 6 button head from the rear in 2 mm counterbore')
for x, y in STANDOFFS + CARRIER_SCREWS:
    ax.annotate(f'({x:g}, {y:g})', (x, y), xytext=(0, -11 if y < 265 else 8), textcoords='offset points', ha='center', fontsize=6.5)
ax.set_xlim(-62, 62); ax.set_ylim(180, 350); ax.set_aspect('equal')
ax.set_xlabel('X (mm), front view, robot left (+X) on the right'); ax.set_ylabel('Y (mm)')
ax.set_title('Power board carrier, front view (torso coordinates)\nZ 37.5 to 41.5, ~49 g at 100 % PETG', fontsize=10)
ax.legend(loc='upper center', bbox_to_anchor=(0.5, -0.08), fontsize=7, ncol=1, frameon=False)
plt.tight_layout(); plt.savefig('CARRIER.png')
