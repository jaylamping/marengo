"""Sheet 1: how power and CAN reach each left-arm motor. Schematic, not to scale."""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import FancyBboxPatch, Rectangle
import numpy as np

RED, BLK, BLUE, GRN, YEL, ORG, GRAY = "#d62728", "#222222", "#1f77b4", "#2ca02c", "#f2c200", "#ff7f0e", "#9a9a9a"
MOT = [("M11\npitch\nRS03", 2.3), ("M12\nroll\nRS03", 4.1), ("M13\nu-arm yaw\nRS02", 5.9), ("M14\nelbow\nRS02", 7.7), ("M15\nfore yaw\nRS00", 9.5)]
JOINTS = [(3.2, "pitch\n-50..180°"), (5.0, "roll\n-15..180°"), (6.8, "yaw\n±90°"), (8.6, "elbow\n0..135°"), (10.4, "wrist\n(later)")]
LINKS = [(0, 3.2, "TORSO"), (3.2, 5.0, "P1"), (5.0, 6.8, "P2"), (6.8, 8.6, "UPPER ARM"), (8.6, 10.4, "FOREARM")]
Y_TRUNK, Y_MOT = 0.0, -1.25

def base(ax, title, sub, faded=False):
    ax.set_xlim(-0.2, 17.6); ax.set_ylim(-2.2, 1.6); ax.axis("off")
    for x0, x1, name in LINKS:
        ax.add_patch(Rectangle((x0, -2.0), x1 - x0, 3.3, fc="#f4f4f4" if LINKS.index((x0, x1, name)) % 2 else "#ebebeb", ec="none", zorder=0))
        ax.text((x0 + x1) / 2, 1.15, name, ha="center", va="center", fontsize=8, color="#666")
    for x, lab in JOINTS:
        ax.plot([x, x], [-2.0, 1.0], ls="--", color="#888", lw=1, zorder=1)
        ax.text(x, -2.12, lab, ha="center", va="top", fontsize=6.5, color="#555")
    ax.add_patch(FancyBboxPatch((0.1, -0.45), 1.1, 0.9, boxstyle="round,pad=0.03", fc="#cfe3f7", ec=BLK, lw=1, zorder=3))
    ax.text(0.65, 0, "power\nboard\narm port", ha="center", va="center", fontsize=6.5, zorder=4)
    for lab, x in MOT:
        ax.add_patch(FancyBboxPatch((x - 0.38, Y_MOT - 0.42), 0.76, 0.84, boxstyle="round,pad=0.03", fc="#c8c8c8", ec=BLK, lw=1, zorder=3))
        ax.text(x, Y_MOT, lab, ha="center", va="center", fontsize=6, zorder=4)
    ax.text(10.9, 1.25, title, fontsize=11, fontweight="bold", va="top", color="#999" if faded else BLK)
    ax.text(10.9, 0.85, sub, fontsize=7.6, va="top", linespacing=1.45, color="#999" if faded else BLK)

def pair(ax, xs, ys, lw=2.2, power=True, can=True, zorder=2, color_p=RED):
    xs, ys = np.asarray(xs), np.asarray(ys)
    if power: ax.plot(xs, ys + 0.06, color=color_p, lw=lw, zorder=zorder, solid_capstyle="butt")
    if can: ax.plot(xs, ys - 0.06, color=BLUE, lw=lw * 0.6, zorder=zorder, solid_capstyle="butt")

def fuse(ax, x, y, w=0.22, h=0.14):
    ax.add_patch(Rectangle((x - w / 2, y - h / 2), w, h, fc=YEL, ec=BLK, lw=0.8, zorder=6))

def conn(ax, x, y):
    ax.add_patch(Rectangle((x - 0.07, y - 0.13), 0.14, 0.26, fc=BLK, ec=BLK, zorder=6))

def wavy(ax, x0, x1, y):
    t = np.linspace(0, 1, 80); xs = x0 + (x1 - x0) * t; ys = y + 0.12 * np.sin(t * np.pi * 4)
    ax.plot(xs, ys + 0.06, color=ORG, lw=2.4, zorder=2); ax.plot(xs, ys - 0.06, color=BLUE, lw=1.3, zorder=2)

fig, axes = plt.subplots(4, 1, figsize=(16, 13.5))
fig.suptitle("Left arm wiring trunk: how each motor gets power + CAN (schematic, not to scale)", fontsize=14, fontweight="bold", y=0.995)

# A junction boards + plug-in jumpers
ax = axes[0]
base(ax, "A. Junction boards + plug-in joint jumpers  (recommended)",
     "+ every joint crossing is a short plug-in wear part: swap in minutes\n"
     "+ drop fuse, CAN surge clamp, terminator on one cheap board per motor\n"
     "+ CAN stays a clean line; one board design reused 5x per arm (10 per robot)\n"
     "+ fits the 9 mm channel: only 4 wires everywhere\n"
     "- needs a small custom PCB (~30 x 18 x 8 mm) and a pocket in each link\n"
     "- more connectors in series (2 per joint): must latch")
jb = [2.3, 4.1, 5.9, 7.7, 9.5]
segs = [(1.2, 2.0)] + [(jb[i] + 0.3, JOINTS[i][0] - 0.25) for i in range(4)]
pair(ax, [1.2, 1.95], [0, 0])
for i, x in enumerate(jb):
    ax.add_patch(FancyBboxPatch((x - 0.3, -0.18), 0.6, 0.36, boxstyle="round,pad=0.02", fc=GRN, ec=BLK, lw=1, zorder=5))
    fuse(ax, x, -0.32, 0.16, 0.1)
    pair(ax, [x, x], [-0.2, Y_MOT + 0.42], lw=1.8)
    if i < 4:
        nxt = jb[i + 1]
        conn(ax, x + 0.36, 0); pair(ax, [x + 0.3, x + 0.36], [0, 0])
        wavy(ax, x + 0.43, nxt - 0.43, 0)
        conn(ax, nxt - 0.36, 0)
    ax.text(x, 0.3, "JB-1%d" % (i + 1), ha="center", fontsize=6.5)
ax.text(9.5, 0.47, "+120 Ω", ha="center", fontsize=6, color=GRN)
conn(ax, 1.95, 0); conn(ax, 1.25, 0)

# B splices + inline fuses
ax = axes[1]
base(ax, "B. One continuous trunk, spliced drops with inline fuse holders",
     "+ no custom PCB: off-the-shelf wire, solder sleeves, mini-blade holders\n"
     "+ fewest connectors in series\n"
     "- the flexing joint sections are part of one long cable: a worn joint\n"
     "   means re-making the trunk, opening every link\n"
     "- inline holders are bulky (~12 x 30 mm) in a 9 mm channel\n"
     "- no CAN surge protection unless added by hand at each splice\n"
     "(this is today's bench harness)")
pair(ax, [1.2, 10.0], [0, 0])
for x in jb:
    ax.plot(x, 0, "o", color=BLK, ms=5, zorder=6)
    pair(ax, [x, x], [0, Y_MOT + 0.42], lw=1.8)
    fuse(ax, x - 0.06, -0.55, 0.16, 0.3)
ax.text(10.05, 0.25, "120 Ω", fontsize=6)

# C home runs
ax = axes[2]
base(ax, "C. Home runs: each motor wired straight to the power board",
     "+ per-motor fuse and current meter on the power board, no splices\n"
     "+ nothing in the arm but wire\n"
     "- 5 power pairs through the pitch joint: bundle ~15 mm vs 9 mm channel\n"
     "- the hardest joints (pitch, roll) carry the most wire\n"
     "- power board needs 5 ports per arm (10 + waist + legs)\n"
     "- CAN still needs a trunk or long stubs (bad at 1 Mbit/s)")
for k, x in enumerate(jb):
    y = 0.42 - k * 0.16
    ax.plot([1.2, x - 0.12 + 0.05 * k], [y, y], color=RED, lw=1.4, zorder=2)
    ax.plot([x - 0.12 + 0.05 * k] * 2, [y, Y_MOT + 0.42], color=RED, lw=1.4, zorder=2)
    fuse(ax, 1.35, y, 0.14, 0.1)
ax.plot([1.2, 10.0], [-0.42, -0.42], color=BLUE, lw=1.2)
for x in jb: ax.plot([x + 0.12] * 2, [-0.42, Y_MOT + 0.42], color=BLUE, lw=1.0)

# D daisy chain (ruled out)
ax = axes[3]
base(ax, "D. Daisy-chain through each motor's second port  (already ruled out by you)",
     "+ least wire, vendor's intended wiring\n"
     "- whole arm's current passes through every drive's board and connector\n"
     "- one fault can take out a $500 actuator, not a fuse\n"
     "- RS03 second port is boxed in at the pitch bracket\n"
     "Shown for completeness only.", faded=True)
prev = 1.2
for x in jb:
    pair(ax, [prev, x - 0.38], [Y_MOT + 0.2, Y_MOT + 0.2], lw=1.8, color_p="#e9a0a0")
    prev = x + 0.38
ax.text(5.9, -0.3, "power and CAN go IN one port and OUT the other on every motor", ha="center", fontsize=8, color="#999")

# legend
from matplotlib.lines import Line2D
h = [Line2D([], [], color=RED, lw=2.5, label="VBUS / GND"), Line2D([], [], color=BLUE, lw=1.5, label="CAN pair"),
     Line2D([], [], color=ORG, lw=2.5, label="flexing jumper (wear part)"),
     Rectangle((0, 0), 1, 1, fc=GRN, ec=BLK, label="junction board"), Rectangle((0, 0), 1, 1, fc=YEL, ec=BLK, label="fuse"),
     Rectangle((0, 0), 1, 1, fc=BLK, label="connector")]
fig.legend(handles=h, loc="lower center", ncol=6, fontsize=9, frameon=False, bbox_to_anchor=(0.5, 0.0))
plt.tight_layout(rect=(0, 0.025, 1, 0.985))
fig.savefig(__file__.replace("topology_sheet.py", "TRUNK_OPTIONS_TOPOLOGY.png"), dpi=150)
