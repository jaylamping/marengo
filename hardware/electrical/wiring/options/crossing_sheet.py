"""Sheet 2: ways to get the trunk across a shoulder RS03 joint (pitch 230°, roll 195°). Rough 3D, approximate sizes (mm)."""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

ORG, GRAY, LINK, FIX = "#ff7f0e", "#b5b5b5", "#f3a35c", "#7fa7d1"
R, H = 50, 57  # RS03 approx radius / axial length

def cyl(ax, r, z0, z1, color, alpha=1.0, n=60):
    t = np.linspace(0, 2 * np.pi, n); z = np.array([z0, z1])
    T, Z = np.meshgrid(t, z)
    ax.plot_surface(r * np.cos(T), r * np.sin(T), Z, color=color, alpha=alpha, linewidth=0, shade=True)
    for zz in (z0, z1):
        rr = np.linspace(0, r, 2); T2, RR = np.meshgrid(t, rr)
        ax.plot_surface(RR * np.cos(T2), RR * np.sin(T2), np.full_like(RR, zz), color=color, alpha=alpha, linewidth=0, shade=True)

def box(ax, c, s, color, alpha=1.0):
    x, y, z = c; dx, dy, dz = s
    X = [x - dx / 2, x + dx / 2]; Y = [y - dy / 2, y + dy / 2]; Z = [z - dz / 2, z + dz / 2]
    for a in (0, 1):
        xx, yy = np.meshgrid(X, Y); ax.plot_surface(xx, yy, np.full_like(xx, Z[a]), color=color, alpha=alpha)
        xx, zz = np.meshgrid(X, Z); ax.plot_surface(xx, np.full_like(xx, Y[a]), zz, color=color, alpha=alpha)
        yy, zz = np.meshgrid(Y, Z); ax.plot_surface(np.full_like(yy, X[a]), yy, zz, color=color, alpha=alpha)

def scene(ax, title):
    cyl(ax, R, 0, H, GRAY)
    box(ax, (0, 0, -6), (150, 150, 12), FIX, 0.35)          # torso bracket (fixed)
    box(ax, (0, 0, H + 8), (70, 70, 16), LINK, 0.9)          # output link (rotates)
    box(ax, (60, 0, H + 8), (110, 30, 16), LINK, 0.9)
    ax.set_xlim(-110, 110); ax.set_ylim(-110, 110); ax.set_zlim(-30, 140)
    ax.set_box_aspect((1, 1, 0.77), zoom=1.55); ax.view_init(24, -58); ax.set_axis_off()

def cable(ax, x, y, z, lw=3.2, color=ORG):
    ax.plot(x, y, z, color=color, lw=lw, solid_capstyle="round", zorder=10)

fig = plt.figure(figsize=(15, 19))
PANEL_H = 0.225
def panel(i):
    y0 = 0.935 - PANEL_H * i
    return fig.add_axes([0.0, y0 - PANEL_H + 0.01, 0.42, PANEL_H - 0.01], projection="3d"), y0

fig.suptitle("Crossing a shoulder RS03 (pitch -50..180° = 230°, roll -15..180° = 195°). RS03 has no hole through the middle.\n"
             "Rough 3D, sizes approximate. Blue = fixed side, orange block = link that rotates, gray = RS03, orange line = jumper.",
             fontsize=12.5, fontweight="bold", y=0.99)

notes = {
 1: ("A. Spiral coil around the housing (round wires)",
     "Jumper coils 2-3 turns in a ring around the RS03 (r 55 to 75),\ninner end on a sleeve that turns with the link, outer end fixed.\n"
     "Rotating 230° just tightens or loosens the coil a little.\n"
     "+ normal round flex wire works: bend radius ~50 mm > 30 mm min\n+ fits inside the r78 shoulder cavity; visible but tidy (no shell here)\n"
     "- adds ~14 mm ring + a rotating sleeve; needs a printed cover\n- most design work (both pitch and roll)"),
 2: ("B. Flat-flex clock-spring cassette",
     "Flat ribbon cable wound in a thin cassette, like a car steering wheel.\n"
     "+ smallest and longest-lasting over many cycles\n+ ~8 mm tall ring, r 52 to 62\n"
     "- flat cable that carries 15 A at 60 V is a special/custom part\n- ribbon connectors at both ends; harder to repair at home"),
 3: ("C. Free service loop beside the joint",
     "A slack loop of loose wires hangs outside the joint.\n"
     "+ simplest, zero parts, good for the bench\n"
     "- over 230° it snags, pinches or rubs (the old L05-L11 studies never closed it)\n- visible, flops around, catches on things in a fall"),
 4: ("D. Rotary cable carrier (drag-chain arc)",
     "Plastic chain links guide the cable in a U-turn arc around the joint.\n"
     "+ off-the-shelf (igus-style), well proven, round wire OK\n"
     "- U-turn needs ~2x bend radius: ring out to r ~110+,\n   well outside the r78 shoulder cavity and the shell\n- heavy and bulky for an arm; looks industrial"),
}

# A spiral
ax, y0 = panel(0); scene(ax, notes[1][0]); fig.text(0.44, y0 - 0.02, notes[1][0], fontsize=13, fontweight="bold", va="top"); fig.text(0.44, y0 - 0.05, notes[1][1], fontsize=10.5, va="top", linespacing=1.5)
cyl(ax, 52, 20, 42, "#dddddd", 0.25); cyl(ax, 78, 20, 42, "#cccccc", 0.12)
t = np.linspace(0, 2 * np.pi * 2.6, 400); r = 56 + (74 - 56) * t / t[-1]
cable(ax, r * np.cos(t), r * np.sin(t), np.full_like(t, 31))
cable(ax, [74 * np.cos(t[-1]), 95, 95], [74 * np.sin(t[-1]), -60, -80], [31, 10, 0])  # outer end to torso
cable(ax, [56, 56, 60], [0, 0, 0], [31, H, H + 8])                                       # inner end up the sleeve to link

# B flat-flex cassette (vertical ribbon spiral)
ax, y0 = panel(1); scene(ax, notes[2][0]); fig.text(0.44, y0 - 0.02, notes[2][0], fontsize=13, fontweight="bold", va="top"); fig.text(0.44, y0 - 0.05, notes[2][1], fontsize=10.5, va="top", linespacing=1.5)
t = np.linspace(0, 2 * np.pi * 5, 500); r = 53 + 9 * t / t[-1]
T, Z = np.meshgrid(t, [36, 44]); RR = 53 + 9 * T / t[-1]
ax.plot_surface(RR * np.cos(T), RR * np.sin(T), Z, color=ORG, alpha=0.85, linewidth=0)
cyl(ax, 64, 34, 46, "#cccccc", 0.18)
cable(ax, [62, 95, 95], [0, -60, -80], [40, 10, 0], lw=2.5)
cable(ax, [53, 53, 60], [0, 0, 0], [40, H, H + 8], lw=2.5)

# C service loop
ax, y0 = panel(2); scene(ax, notes[3][0]); fig.text(0.44, y0 - 0.02, notes[3][0], fontsize=13, fontweight="bold", va="top"); fig.text(0.44, y0 - 0.05, notes[3][1], fontsize=10.5, va="top", linespacing=1.5)
s = np.linspace(0, 1, 200)
x = 95 - 10 * np.sin(np.pi * s) + 15 * s; y = -80 + 60 * s + 45 * np.sin(np.pi * s); z = 0 + (H + 8) * s - 55 * np.sin(np.pi * s)
cable(ax, x, y, z); cable(ax, [x[-1], 90], [y[-1], 0], [z[-1], H + 8])

# D drag chain
ax, y0 = panel(3); scene(ax, notes[4][0]); fig.text(0.44, y0 - 0.02, notes[4][0], fontsize=13, fontweight="bold", va="top"); fig.text(0.44, y0 - 0.05, notes[4][1], fontsize=10.5, va="top", linespacing=1.5)
ax.set_xlim(-140, 140); ax.set_ylim(-140, 140)
for k, th in enumerate(np.linspace(np.pi * 0.15, np.pi * 1.55, 26)):
    rr = 118 if th > np.pi * 1.25 else 88
    box(ax, (rr * np.cos(th), rr * np.sin(th), 28), (14, 14, 18), "#444444", 0.85)
t = np.linspace(np.pi * 1.25, np.pi * 1.55, 40)
cable(ax, 103 + 15 * np.cos(np.linspace(0, np.pi, 40)) * 0 + 0 * t, 0 * t, 28 + 0 * t, lw=0.1)
th = np.linspace(np.pi * 0.15, np.pi * 1.25, 120)
cable(ax, 88 * np.cos(th), 88 * np.sin(th), np.full_like(th, 28), lw=2)
cyl(ax, 78, 20, 36, "#d9534f", 0.10)

fig.text(0.44, 0.935 - PANEL_H * 3 - 0.2, "red ring = r78 shoulder cavity", fontsize=9, color="#d9534f")
fig.savefig(__file__.replace("crossing_sheet.py", "TRUNK_OPTIONS_SHOULDER_CROSSING.png"), dpi=150)
