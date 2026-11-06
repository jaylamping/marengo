"""Left arm trunk, option B (spliced trunk, inline drop fuses, plug-in joint breaks). Harness diagram, not to scale."""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import FancyBboxPatch, Rectangle
import numpy as np

RED, BLK, YEL, GRN, GRAY = "#d62728", "#111111", "#e6b800", "#2e9e3e", "#bdbdbd"
fig, ax = plt.subplots(figsize=(19, 10.5))
ax.set_xlim(-0.3, 19.2); ax.set_ylim(-6.6, 3.2); ax.axis("off")
fig.suptitle("Left arm wiring trunk, option B: spliced trunk, inline drop fuses, plug-in break at each joint  (rev 0, 2026-11-06, lengths are estimates)",
             fontsize=13.5, fontweight="bold", y=0.985)

def bundle(x0, x1, y=0.0):
    for dy, c, lw in ((0.18, RED, 3.2), (0.06, BLK, 3.2), (-0.06, YEL, 1.6), (-0.18, GRN, 1.6)):
        ax.plot([x0, x1], [y + dy, y + dy], color=c, lw=lw, solid_capstyle="butt", zorder=2)

def drop(x, y0, y1):
    for dx, c, lw in ((-0.15, RED, 2.6), (-0.05, BLK, 2.6), (0.05, YEL, 1.3), (0.15, GRN, 1.3)):
        ax.plot([x + dx, x + dx], [y0, y1], color=c, lw=lw, solid_capstyle="butt", zorder=2)

def plug(x, label=None, y=0.0):
    ax.add_patch(Rectangle((x - 0.17, y - 0.33), 0.16, 0.66, fc="#555", ec=BLK, zorder=5))
    ax.add_patch(Rectangle((x + 0.01, y - 0.33), 0.16, 0.66, fc="#222", ec=BLK, zorder=5))
    if label: ax.text(x, y + 0.5, label, ha="center", fontsize=7.4, zorder=6)

# power board
ax.add_patch(FancyBboxPatch((-0.2, -0.9), 1.5, 1.8, boxstyle="round,pad=0.04", fc="#cfe3f7", ec=BLK, zorder=3))
ax.text(0.55, 0.15, "POWER BOARD\nL-arm port\nXT30(2+2)-F\n20 A trip /\n15 A fuse\n120 Ω (switch)", ha="center", va="center", fontsize=7.6, zorder=4)

X = {"M11": 2.6, "M12": 6.2, "M13": 9.8, "M14": 13.4, "M15": 16.6}
JP = [(4.4, "J1 pitch\n-50..180°"), (8.0, "J2 roll\n-15..180°"), (11.6, "J3 u-arm yaw\n±90°"), (15.0, "J4 elbow\n0..133°")]
LINKS = [(1.3, 4.4, "TORSO"), (4.4, 8.0, "P1"), (8.0, 11.6, "P2"), (11.6, 15.0, "UPPER ARM"), (15.0, 18.0, "FOREARM")]
for i, (a, b, n) in enumerate(LINKS):
    ax.add_patch(Rectangle((a, -6.0), b - a, 8.6, fc="#f3f3f3" if i % 2 else "#e9e9e9", ec="none", zorder=0))
    ax.text((a + b) / 2, 2.35, n, ha="center", fontsize=10, color="#666", fontweight="bold")

# trunk segments with lengths
SEG = [(1.3, X["M11"], "S0  ≈300 → cut 400"), (X["M11"], X["M12"], "S1  ≈350 → cut 450\n(pitch crossing: provisional)"),
       (X["M12"], X["M13"], "S2  ≈300 → cut 400\n(roll crossing: provisional)"), (X["M13"], X["M14"], "S3  ≈400 → cut 500\n(yaw twist 185 free + elbow slack loop 130)"),
       (X["M14"], X["M15"], "S4  ≈200 → cut 280\n(posterior elbow route)")]
for a, b, lab in SEG:
    bundle(a, b)
    ax.text((a + b) / 2 + (0.15 if "S0" in lab else 0), 1.25, lab, ha="center", va="bottom", fontsize=7.6, color="#333")
bundle(X["M15"], X["M15"] + 0.35)
ax.add_patch(Rectangle((X["M15"] + 0.35, -0.25), 0.25, 0.5, fc="#fff", ec=BLK, zorder=5))
ax.text(X["M15"] + 0.48, 0, "end\ncap", ha="center", va="center", fontsize=5.5, zorder=6)
plug(1.45)
ax.text(1.45, -1.25, "XT30(2+2)-M\nto board", ha="center", fontsize=7)
for x, lab in JP:
    ax.plot([x, x], [-6.0, 2.2], ls="--", color="#888", lw=1, zorder=1)
    plug(x)
    ax.text(x, -6.15, lab, ha="center", va="top", fontsize=8, color="#444")
ax.text(4.4, 0.55, "joint break", ha="center", fontsize=6.8, color="#333", zorder=6)

MOT = {"M11": ("M11  pitch RS03\nCAN id 11\ntop-rear flat port\n(low-front capped)", "7.5 A", "XT30UW-F\n+ GH1.25-2 (CAN)"),
       "M12": ("M12  roll RS03\nCAN id 12", "7.5 A", "XT30UW-F\n+ GH1.25-2 (CAN)"),
       "M13": ("M13  upper-arm yaw RS02\nCAN id 13", "5 A", "XT30(2+2)-F"),
       "M14": ("M14  elbow RS02\nCAN id 14 (proposed)\nupper port", "5 A", "XT30(2+2)-F"),
       "M15": ("M15  lower-arm yaw RS00\nCAN id 15 (proposed)", "3 A", "XT30(2+2)-F")}
for k, x in X.items():
    name, f, con = MOT[k]
    ax.plot(x, 0.12, "o", ms=7, mfc="white", mec=BLK, zorder=6); ax.plot(x, -0.12, "o", ms=7, mfc="white", mec=BLK, zorder=6)
    drop(x, -0.25, -3.2)
    ax.add_patch(Rectangle((x - 0.27, -1.75), 0.24, 0.75, fc=YEL, ec=BLK, zorder=5))
    ax.text(x - 0.4, -1.38, "ATM\n" + f, ha="right", va="center", fontsize=7.6, fontweight="bold")
    ax.text(x + 0.3, -0.75, "drop ≤150\ncut 200", fontsize=6.8, color="#333")
    ax.add_patch(Rectangle((x - 0.35, -3.55), 0.7, 0.35, fc="#333", ec=BLK, zorder=5))
    ax.text(x + 0.45, -3.38, con, fontsize=6.8, va="center")
    ax.add_patch(FancyBboxPatch((x - 0.85, -5.35), 1.7, 1.5, boxstyle="round,pad=0.03", fc=GRAY, ec=BLK, zorder=3))
    ax.text(x, -4.6, name, ha="center", va="center", fontsize=7.2, zorder=4)
ax.text(X["M15"] + 0.3, -2.4, "120 Ω ¼ W across\nCAN_H–CAN_L,\nheat-shrunk at\nthis splice", fontsize=6.8, color="#0b6b20")
ax.text(X["M11"] - 1.05, 0.55, "splice: T-splice + solder sleeve\non all 4 conductors (o)", fontsize=6.6, color="#333")

# legend / notes
y = -6.55
leg = [(RED, "VBUS + : 16 AWG silicone (18 AWG after M14)"), (BLK, "GND − : same"), (YEL, "CAN_H : 26 AWG, twisted with CAN_L ~1 turn / 25 mm"), (GRN, "CAN_L")]
for i, (c, t) in enumerate(leg):
    ax.plot([0.0 + i * 4.6, 0.6 + i * 4.6], [y, y], color=c, lw=4); ax.text(0.75 + i * 4.6, y, t, va="center", fontsize=8)
notes = ("Joint break = XT30 pair for power (FEMALE on the board/upstream side, so the live side has no exposed pins) + JST-GH 2-pin latching for CAN.\n"
         "Lengths: S = trunk segment between splices, ≈ = routed estimate from the URDF + frame coordinates, cut = with allowance. Pitch and roll crossings are not designed yet:\n"
         "cut long, string-test the real route through full range, then trim. Optional: NUP2105L-class CAN TVS at each drop splice (sacrificial). Plug/unplug only with the E-stop open.")
ax.text(0.0, -7.05, notes, fontsize=7.8, va="top", color="#222")
plt.subplots_adjust(left=0.01, right=0.99, top=0.94, bottom=0.11)
fig.savefig(__file__.replace("wiring_diagram.py", "LEFT_ARM_TRUNK_WIRING.png"), dpi=150)
