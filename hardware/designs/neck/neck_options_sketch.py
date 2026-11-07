"""Front-view (Y-Z) schematic of the three neck options, to scale in mm. Head/collar/neck from the Argon
baseline; torso top ASSUMED 10 mm below the collar bottom (frame top rails Y 475-495, Jetson top Y 455).
Actuator boxes are vendor envelopes placed by hand: a picture for discussion, not a layout.
Run: python neck_options_sketch.py  -> NECK_OPTIONS.png"""
import matplotlib
matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle, FancyBboxPatch, Circle, Polygon

WHITE, GREY, DARK, RED, BLUE, ORANGE, TEAL = "#f4f5f7", "#c9ccd2", "#2b2d31", "#d9534f", "#3b7dd8", "#f0883e", "#1aa39a"
RAIL_TOP, RAIL_BOT, JETSON_TOP = -10, -30, -50     # head-frame z of the frame top rails / Jetson (assumed)


def base(ax, title):
    ax.set_title(title, fontsize=11, fontweight="bold", loc="left")
    # torso top: rails (front view: we see the front top rail end-on? no: rail runs along Y) and Jetson
    ax.add_patch(Rectangle((-113, RAIL_BOT), 226, 20, fc=GREY, ec="k", lw=0.6, ls="--", alpha=0.35))
    ax.text(-110, RAIL_BOT - 9, "front/rear top rails (Y 475-495), 100 mm gap between them", fontsize=6.5)
    ax.add_patch(Rectangle((-51.5, JETSON_TOP - 45), 103, 45, fc="#e8e8e8", ec="k", lw=0.6, ls="--"))
    ax.text(0, JETSON_TOP - 25, "Jetson envelope\n(top Y 455)", ha="center", fontsize=7)
    ax.plot([-140, 140], [0, 0], color="#999", lw=0.5, ls=":")
    # collar, neck, head (front view)
    ax.add_patch(Rectangle((-75, 0), 150, 8, fc=WHITE, ec="k", lw=0.8))
    ax.add_patch(Rectangle((-68, 8), 136, 6, fc=WHITE, ec="k", lw=0.8))
    ax.add_patch(Rectangle((-36, 12), 72, 50, fc=DARK, ec="k", lw=0.5, alpha=0.35))
    for zr in (18, 27, 36):
        ax.add_patch(Rectangle((-41, zr), 82, 4, fc=DARK, ec="k", lw=0.3, alpha=0.35))
    ax.add_patch(FancyBboxPatch((-68, 50), 136, 52, boxstyle="round,pad=0,rounding_size=8", fc=GREY, ec="k", lw=0.8, alpha=0.35))
    ax.add_patch(FancyBboxPatch((-61, 95), 122, 140, boxstyle="round,pad=0,rounding_size=40", fc=WHITE, ec="k", lw=0.8, alpha=0.5))
    for y in (-27.5, 27.5):
        ax.add_patch(Circle((y, 150), 14, fc="#fff3b0", ec="k", lw=0.6))
    ax.set_xlim(-150, 150); ax.set_ylim(-115, 250); ax.set_aspect("equal")
    ax.set_xlabel("Y (mm)"); ax.set_ylabel("Z above collar bottom (mm)")


def rs00(ax, y, z, label, rot=False):
    w, h = (51.4, 57) if rot else (57, 51.4)
    ax.add_patch(Rectangle((y - w / 2, z), w, h, fc=TEAL, ec="k", lw=0.8))
    ax.text(y, z + h / 2, label, ha="center", va="center", fontsize=7, color="white", fontweight="bold")


def servo(ax, y, z, label, w=24.7, h=45.2):
    ax.add_patch(Rectangle((y - w / 2, z), w, h, fc=ORANGE, ec="k", lw=0.8))
    ax.text(y, z + h / 2, label, ha="center", va="center", fontsize=6.5, rotation=90)


def bearing(ax, z, ri, ro):
    for s in (-1, 1):
        ax.add_patch(Rectangle((s * ri if s > 0 else -ro, z), ro - ri, 10, fc=BLUE, ec="k", lw=0.6))


def ujoint(ax, z):
    ax.add_patch(Circle((0, z), 6, fc=RED, ec="k", lw=0.8, zorder=5))


fig, axs = plt.subplots(1, 3, figsize=(18, 7.6))
# A: all RobStride
ax = axs[0]; base(ax, "A  All RobStride (3x RS00, CAN 48 V)")
rs00(ax, 0, -61.4, "RS00 yaw")
ax.add_patch(Rectangle((-28.5, -61.4), 57, 11.4, fc="none", ec=RED, lw=1.5, hatch="///"))
ax.text(32, -75, "clash: RS00 hangs\nto Y ~444, Jetson\ntop at 455", color=RED, fontsize=7)
rs00(ax, -40, 60, "RS00\npitch", rot=True); rs00(ax, 40, 60, "RS00\nroll", rot=True)
ax.text(0, 205, "2 x 310 g in the head; side by side\n= 103 mm in a 122 mm cranium,\nnot clear of the jaw sweep", ha="center", fontsize=7, color=RED)
ujoint(ax, 70)
# B: hybrid
ax = axs[1]; base(ax, "B  Hybrid (RS00 yaw + 2 bus servos pitch/roll)")
rs00(ax, 0, -61.4, "RS00 yaw")
ax.add_patch(Rectangle((-28.5, -61.4), 57, 11.4, fc="none", ec=RED, lw=1.5, hatch="///"))
ax.text(32, -75, "same Jetson clash\n(~11 mm)", color=RED, fontsize=7)
servo(ax, -22, 62, "STS3215"); servo(ax, 22, 62, "STS3215")
ujoint(ax, 70)
ax.text(0, 205, "two buses (CAN + servo TTL),\nRS00 needs a support bearing\nanyway (no published load rating)", ha="center", fontsize=7)
# C: all servos (recommended)
ax = axs[2]; base(ax, "C  All bus servos (3x STS3215-class, 12 V)  <- recommended")
bearing(ax, 2, 30, 39)          # 61812-class thin bearing 60/78 in the collar
ax.text(-140, -4, "thin bearing\n60/78 in collar", fontsize=7, color=BLUE)
ax.add_patch(Rectangle((-30, -8), 60, 6, fc="#bbb", ec="k", lw=0.6))   # yaw turntable
servo(ax, -16, -48, "pitch/roll A", h=40); servo(ax, 16, -48, "pitch/roll B", h=40)
ax.plot([-16, -20], [-8, 66], color="k", lw=1.6); ax.plot([16, 20], [-8, 66], color="k", lw=1.6)
ax.text(46, 40, "pushrods up the\nhollow neck", fontsize=7)
servo(ax, 62, -45, "yaw (fixed)", h=40)
ax.text(80, 5, "pinion -> ring\ngear on turntable", fontsize=7)
ujoint(ax, 70)
ax.text(10, 74, "U-joint", fontsize=7, color=RED)
ax.text(0, 205, "servos ride the yaw stage under the collar;\nhead carries only the U-joint\nand two ball links", ha="center", fontsize=7)
ax.text(-140, -112, "~2 mm to the Jetson: frame thread to confirm", fontsize=7, color=RED)
fig.suptitle("marengo neck options, front view, mm, to scale (Argon head baseline). Schematic for discussion, not a layout.",
             fontsize=12)
fig.tight_layout(rect=(0, 0, 1, 0.95))
fig.savefig("NECK_OPTIONS.png", dpi=110)
