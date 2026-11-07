"""Neck layout study, option C (Joey 2026-11-07 06:30Z): 3 x STS3215-class 12 V bus servos.

Head frame (Argon baseline): X fwd, Y left, Z up, mm; z = 0 is the collar bottom.
ASSUMED: collar bottom 10 mm above the frame top-rail face (Y 495 -> z -10).
Fixed limits from the frame thread (04:46Z): top rails z -30..-10 at |X| 50..70; box interior |X| < 50 at all
heights, |Y| < 78 between the shoulder motors; Jetson box is a placeholder (top Y 455 -> z -50); Pi 5 top Y 397.

Topology: a hollow yaw tube on a thin-section bearing whose housing sits on the top rails. Under it, two
pitch/roll servos lie flat, stacked, and turn with the tube. Each drives a near-vertical pushrod up the tube
to a ball stud on the head plate, which sits on a U-joint at z = UJ_Z. Both rods sit behind the pivot, at
+Y and -Y: moving together gives pitch, moving opposite gives roll. A fixed yaw servo beside the stack
drives a ring gear on the tube.
Run: python layout_study.py  -> prints the checks, writes LAYOUT_STUDY.png
"""
import math
import itertools

# ---------------------------------------------------------------- parameters
UJ_Z = 70.0            # U-joint centre (head thread: COM z~140 on the centre line)
ROD_X = -18.0          # pushrod ball studs on the head plate, behind the pivot
ROD_Y = 15.0           # +/- lateral
HORN_R = 19.0          # servo horn radius (printed or metal disc horn)
PITCH = (-40.0, 25.0)  # deg, negative = look down (chin toward chest)
ROLL = (-20.0, 20.0)
YAW = (-90.0, 90.0)
HEAD_KG = (1.0, 1.3)   # head thread estimate 0.95 kg
COM_Z = 140.0
ALPHA = 20.0           # rad/s^2
STS = dict(L=45.2, W=24.7, H=35.0, shaft_from_end=11.0, rated=0.98, stall=2.94)   # vendor listing; shaft offset ASSUMED
BEARING = dict(name="61814 (70 x 90 x 10)", bore=70.0, od=90.0, w=10.0, z=4.0)
ROD_BALL_D = 10.0      # ball-link body diameter
BELLOWS_RI = 36.0 - 2.5  # visible neck r36 minus TPU wall -> free bore inside the boot
SWEEP_LIMIT = 48.0     # rotating parts inside the box: |X| < 50, keep 2 mm

# servo stack: lying flat, shaft axis along Y, horn swings in the X-Z plane at y = +/-ROD_Y
SERVO_TOP_Z = 1.0      # upper servo sits inside the bearing tower (ID ~80) that stands on the rail face; bearing above it at z 4..14
GAP = 2.0


def rod_dz(pitch_deg, roll_deg, y):
    """Vertical travel of a ball stud at (ROD_X, y, UJ_Z) for a pitch then roll about the U-joint.
    Pitch about +Y (nose down = negative per our sign); roll about +X."""
    p, r = math.radians(pitch_deg), math.radians(roll_deg)
    x0, y0 = ROD_X, y
    # pitch: rotate in X-Z: nose-down (p<0) lifts the rear stud
    z1 = x0 * math.sin(p)          # p<0, x0<0 -> positive (rear stud rises when looking down)
    x1 = x0 * math.cos(p)
    # roll: rotate in Y-Z about X
    z2 = z1 * math.cos(r) + y0 * math.sin(r)
    return z2


def main():
    print(__doc__.split("Run:")[0])
    # 1. stroke and horn angle over the ROM grid
    worst = 0.0
    grid = [(p, r) for p in range(int(PITCH[0]), int(PITCH[1]) + 1, 5) for r in range(int(ROLL[0]), int(ROLL[1]) + 1, 5)]
    for p, r in grid:
        for y in (ROD_Y, -ROD_Y):
            worst = max(worst, abs(rod_dz(p, r, y)))
    horn_deg = math.degrees(math.asin(min(1.0, worst / HORN_R)))
    print(f"1. max stud travel {worst:.1f} mm -> horn {horn_deg:.0f} deg from level (limit 65)  "
          f"{'OK' if horn_deg <= 65 else 'FAIL'}")
    # 2. rods inside the bearing bore and the bellows
    r_rod = math.hypot(ROD_X, ROD_Y) + ROD_BALL_D / 2
    print(f"2. rod envelope r {r_rod:.1f}: bearing bore r {BEARING['bore']/2:.0f} "
          f"{'OK' if r_rod < BEARING['bore']/2 - 1 else 'FAIL'}; bellows free bore r {BELLOWS_RI:.1f} "
          f"{'OK' if r_rod < BELLOWS_RI - 1 else 'FAIL -> neck must grow to r %.0f' % (r_rod + 1 + 2.5)}")
    # 3. servo stack sweep radius (shaft at x = ROD_X + HORN_R so the level horn tip sits under the stud)
    shaft_x = ROD_X + HORN_R
    body_x = (shaft_x - STS['shaft_from_end'], shaft_x - STS['shaft_from_end'] + STS['L'])
    corners = [math.hypot(x, y) for x in body_x for y in (-STS['H'] / 2, STS['H'] / 2)]
    horn_sweep = math.hypot(ROD_X - 4, ROD_Y + 3)
    sweep = max(max(corners), horn_sweep)
    print(f"3. servo shaft x {shaft_x:.0f}; body x {body_x[0]:.0f}..{body_x[1]:.0f}; stack sweep r {sweep:.1f} "
          f"(limit {SWEEP_LIMIT}) {'OK' if sweep <= SWEEP_LIMIT else 'FAIL'}")
    z_bot = SERVO_TOP_Z - 2 * STS['W'] - GAP
    print(f"4. stack bottom z {z_bot:.1f} = frame Y {495 + z_bot + 10:.0f}; Jetson placeholder top Y 455 "
          f"({'clear' if 495 + z_bot + 10 >= 455 else 'eats %.0f mm of bay' % (455 - (495 + z_bot + 10))})")
    # 5. torques
    for m in HEAD_KG:
        L = (COM_Z - UJ_Z) / 1000
        tp = m * 9.81 * L * math.sin(math.radians(40)) + (m * (0.13**2 + 0.185**2) / 12 + m * L**2) * ALPHA
        f_rod = tp / (abs(ROD_X) / 1000 * math.cos(math.radians(40))) / 2
        t_servo = f_rod * HORN_R / 1000
        print(f"5. {m} kg: pitch {tp:.2f} N*m -> {f_rod:.0f} N per rod -> {t_servo:.2f} N*m per servo "
              f"(rated {STS['rated']}, stall {STS['stall']}) {'OK' if t_servo < STS['rated'] else 'CHECK'}")
    ring_r, pin_r = 44.0, 12.0
    print(f"6. yaw: ring gear r {ring_r:.0f} on the servo cage at z -8..-2, pinion r {pin_r:.0f} on a fixed servo at "
          f"y {ring_r + pin_r:.0f} (limit |Y| < 78): ratio {ring_r / pin_r:.2f}:1, yaw torque need 0.08 N*m -> "
          f"{0.08 / (ring_r / pin_r):.3f} N*m at the servo")
    print(f"7. yaw bearing {BEARING['name']}: z {BEARING['z']:.0f}..{BEARING['z'] + BEARING['w']:.0f} on a printed tower "
          f"(OD ~120 so it lands on the top rails at |X| 50..70; ID ~80 round the upper servo, ~96 with a pinion window at the ring gear) inside the collar r75")
    # 8. chin vs collar on look-down (chin at X 83, Z 50 per the head thread; collar top z 14, keep 3 mm)
    def chin(deg, uj=UJ_Z, lift=0.0):
        t = math.radians(deg); dx, dz = 83.0, 50.0 + lift - uj
        return dx * math.cos(t) + dz * math.sin(t), uj - dx * math.sin(t) + dz * math.cos(t)
    lim = max(d for d in range(0, 61) if chin(d)[1] >= 17.0)
    print(f"8. chin clears the collar top (z 14 + 3) down to {lim} deg look-down with the head as modelled; "
          f"at 40 deg the chin reaches z {chin(40)[1]:.0f}")
    for lift in (10, 20, 30):
        l2 = max(d for d in range(0, 61) if chin(d, UJ_Z + lift, lift)[1] >= 17.0)
        print(f"   head + U-joint raised {lift} mm (taller neck): look-down limit {l2} deg")
    return dict(worst=worst, horn_deg=horn_deg, r_rod=r_rod, shaft_x=shaft_x, body_x=body_x, z_bot=z_bot)


def draw(res):
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    from matplotlib.patches import Rectangle, Circle, FancyBboxPatch, Polygon
    import numpy as np
    fig, axs = plt.subplots(1, 2, figsize=(16, 9.6))
    ORANGE, BLUE, GREY, DARK, RED, TEAL = "#f0883e", "#3b7dd8", "#c9ccd2", "#2b2d31", "#d9534f", "#1aa39a"

    def common(ax, horiz, title):
        ax.set_title(title, loc="left", fontsize=11, fontweight="bold")
        # rails: side view shows rail sections at X +-50..70; front view shows a rail face-on (dashed)
        if horiz == "X":
            for x0 in (-70, 50):
                ax.add_patch(Rectangle((x0, -30), 20, 20, fc=GREY, ec="k", lw=0.7))
            ax.text(60, -40, "top rails\n(2020)", ha="center", fontsize=7)
            ax.axvline(-50, color="#999", lw=0.5, ls=":"); ax.axvline(50, color="#999", lw=0.5, ls=":")
        else:
            ax.add_patch(Rectangle((-93, -30), 186, 20, fc=GREY, ec="k", lw=0.5, ls="--", alpha=0.3))
            ax.axvline(-78, color="#999", lw=0.5, ls=":"); ax.axvline(78, color="#999", lw=0.5, ls=":")
            ax.text(-76, -105, "shoulder RS03\nfaces |Y| 78", fontsize=6.5)
        ax.add_patch(Rectangle((-51.5, -95), 103, 45, fc="#eee", ec="k", lw=0.6, ls="--"))
        ax.text(0, -75, "Jetson placeholder (top Y 455)", ha="center", fontsize=7)
        # tower + bearing
        for sgn in (-1, 1):
            ax.add_patch(Rectangle((sgn * 40 if sgn > 0 else -60, -10), 20, 14, fc="#ddd", ec="k", lw=0.6))
            ax.add_patch(Rectangle((sgn * 35 if sgn > 0 else -45, 4), 10, 10, fc=BLUE, ec="k", lw=0.6))
        # collar + bellows
        ax.add_patch(Rectangle((-75, 0), 150, 8, fc="none", ec="k", lw=0.8, ls="--"))
        ax.add_patch(Rectangle((-68, 8), 136, 6, fc="none", ec="k", lw=0.8, ls="--"))
        ax.add_patch(Rectangle((-36, 14), 72, 40, fc=DARK, alpha=0.18, ec="k", lw=0.5))
        for zr in (18, 27, 36):
            ax.add_patch(Rectangle((-41, zr), 82, 4, fc=DARK, alpha=0.25, ec="none"))
        # yaw tube
        for sgn in (-1, 1):
            ax.add_patch(Rectangle((sgn * 32 if sgn > 0 else -35, -2), 3, 58, fc="#bbb", ec="k", lw=0.5))
        # U-joint + head plate
        ax.add_patch(Rectangle((-5, 14), 10, UJ_Z - 14, fc="#999", ec="k", lw=0.5))
        ax.add_patch(Circle((0, UJ_Z), 6, fc=RED, ec="k", lw=0.8, zorder=6))
        ax.add_patch(Rectangle((-40, UJ_Z + 4), 80, 5, fc="#aaa", ec="k", lw=0.6))
        ax.text(42, UJ_Z + 4, "head plate on U-joint (z 70)", fontsize=7)
        ax.set_aspect("equal"); ax.set_xlim(-130, 130); ax.set_ylim(-110, 260)
        ax.plot([-130, 130], [0, 0], color="#bbb", lw=0.5, ls=":")

    # ---- side view (X-Z)
    ax = axs[0]; common(ax, "X", "Side view (X fwd to the right), mm")
    # head envelope (head thread numbers): shell X-65..65 Z56..236, jaw X-61..83 Z50..172
    ax.add_patch(FancyBboxPatch((-65, 56), 130, 180, boxstyle="round,pad=0,rounding_size=45", fc="none", ec="k", lw=1))
    ax.add_patch(Polygon([(-61, 50), (83, 50), (83, 102), (-6, 104), (-45, 172), (-61, 172)], fc=GREY, alpha=0.25, ec="k", lw=0.6))
    # pitched ghost (40 deg down about the U-joint)
    th = math.radians(-40)
    def rot(px, pz):
        dx, dz = px, pz - UJ_Z
        return (dx * math.cos(th) + dz * math.sin(th) * -1 * -1 if False else dx * math.cos(th) - dz * math.sin(th),
                UJ_Z + dx * math.sin(th) + dz * math.cos(th))
    ghost = [(-61, 50), (83, 50), (83, 102), (65, 110), (65, 200), (30, 236), (-30, 236), (-65, 200), (-65, 172), (-61, 172)]
    ax.add_patch(Polygon([rot(*g) for g in ghost], fc="none", ec=RED, lw=0.9, ls="--"))
    ax.text(*rot(83, 40), "40 deg look-down:\nchin hits the collar", color=RED, fontsize=7)
    # servo stack (side): bodies X body_x, Z stacked; horn + rod
    bx = res["body_x"]
    for k in range(2):
        zt = SERVO_TOP_Z - k * (STS["W"] + GAP)
        ax.add_patch(Rectangle((bx[0], zt - STS["W"]), STS["L"], STS["W"], fc=ORANGE, ec="k", lw=0.8))
        ax.text(bx[0] + STS["L"] / 2, zt - STS["W"] / 2, f"servo {'A' if k == 0 else 'B'}", ha="center", va="center", fontsize=7)
        zs = zt - STS["W"] / 2
        ax.add_patch(Circle((res["shaft_x"], zs), 2, fc="k"))
        ax.plot([res["shaft_x"], ROD_X], [zs, zs], color="k", lw=2.2)
        ax.plot([ROD_X, ROD_X], [zs, UJ_Z + 4], color="k", lw=1.4, ls="-" if k == 0 else "--")
    ax.text(ROD_X - 3, 30, "pushrods\n(A solid, B behind)", fontsize=6.5, ha="right")
    # ring gear + yaw servo pinion window
    ax.add_patch(Rectangle((-44, -8), 88, 6, fc="#7fb3ff", ec="k", lw=0.6, alpha=0.7))
    ax.text(-128, -6, "ring gear r44", fontsize=7, color=BLUE)
    ax.text(-128, 9, "61814 bearing", fontsize=7, color=BLUE)

    # ---- front view (Y-Z)
    ax = axs[1]; common(ax, "Y", "Front view (Y left to the left), mm")
    ax.add_patch(FancyBboxPatch((-61, 56), 122, 180, boxstyle="round,pad=0,rounding_size=45", fc="none", ec="k", lw=1))
    ax.add_patch(Rectangle((-71, 50), 142, 52, fc=GREY, alpha=0.25, ec="k", lw=0.6))
    for y in (-27.5, 27.5):
        ax.add_patch(Circle((y, 150), 14, fc="#fff3b0", ec="k", lw=0.6))
    for k in range(2):
        zt = SERVO_TOP_Z - k * (STS["W"] + GAP)
        ax.add_patch(Rectangle((-STS["H"] / 2, zt - STS["W"]), STS["H"], STS["W"], fc=ORANGE, ec="k", lw=0.8))
        ax.text(0, zt - STS["W"] / 2, f"servo {'A' if k == 0 else 'B'}", ha="center", va="center", fontsize=7)
        y = ROD_Y if k == 0 else -ROD_Y
        zs = zt - STS["W"] / 2
        ax.plot([y, y], [zs, UJ_Z + 4], color="k", lw=1.4)
        ax.add_patch(Circle((y, UJ_Z + 4), 3, fc="k"))
    # yaw servo + pinion (fixed)
    ax.add_patch(Rectangle((56 - 12, -10 - STS["L"] + 11), STS["W"], STS["L"], fc=TEAL, ec="k", lw=0.8))
    ax.text(56, -30, "yaw\nservo\n(fixed)", ha="center", fontsize=6.5, color="white")
    ax.add_patch(Rectangle((-44, -8), 88, 6, fc="#7fb3ff", ec="k", lw=0.6, alpha=0.7))
    ax.add_patch(Circle((56, -5), 12, fc="none", ec=TEAL, lw=1.5))
    ax.text(-128, 30, "TPU bellows (the\nringed neck)", fontsize=7)
    fig.suptitle("marengo neck layout study, option C (3 bus servos): to scale, schematic. "
                 "Assumes collar bottom 10 mm above the top-rail face (Y 495).", fontsize=11)
    fig.tight_layout(rect=(0, 0, 1, 0.96))
    fig.savefig("LAYOUT_STUDY.png", dpi=110)


if __name__ == "__main__":
    draw(main())
