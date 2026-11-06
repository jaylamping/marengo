"""Waist yaw (RS03) load estimate. Hand-level model, point masses.

Torso frame coords (torso README): X left(+)/right, Y up, Z forward. Waist yaw axis = Y through X=Z=0.
Every mass and lever arm below is an estimate; sources are noted per line. Run: python3 waist_loads.py
"""
import math

g = 9.81

# ---- Upper body above the waist joint (kg, radius from yaw axis m, for inertia) ----
# Mass, radius of gyration about the vertical axis.
torso_items = [
    ("frame 2020 + brackets + gussets (torso README 1-11)", 2.10, 0.085),
    ("2x shoulder pitch RS03 (0.88 each) at X ~0.105", 1.76, 0.105),
    ("2x pitch brackets 185 g", 0.37, 0.095),
    ("13S2P pack (power/custom-21700-pack.md)", 2.20, 0.045),
    ("power/BMS board + fans + harness", 0.53, 0.06),
    ("compute: Pi+HAT, Jetson, 3rd board, shelf", 0.49, 0.05),
    ("shell + hardpoints (shell mfg README) + TPU pads", 1.71, 0.12),
    ("waist adapter plate", 0.12, 0.05),
    ("future head allowance", 1.00, 0.06),
]

# ---- One arm, distal of the shoulder pitch output (left-arm layout + link READMEs) ----
# (name, kg, distance along the arm from the shoulder joint line, m)
arm_items = [
    ("roll RS03", 0.88, 0.00),
    ("printed shoulder P1/P2", 0.37, 0.02),
    ("upper-arm yaw RS02", 0.405, 0.07),
    ("upper-arm link rev B", 0.25, 0.12),
    ("elbow RS02", 0.405, 0.215),
    ("forearm draft", 0.16, 0.31),
    ("RS00 lower-arm yaw", 0.31, 0.335),
    ("hand allowance", 0.40, 0.46),
    ("cables/fasteners", 0.15, 0.20),
]
X_JOINT = 0.2085      # shoulder joint line, X +-208.49
Y_SHOULDER = 0.42486  # pitch axis height (torso Y)
Y_WAIST = 0.12        # waist yaw output plane, ~waist bay (Y 115-155)
PAYLOAD = 1.0         # kg per hand, at 0.46 m

m_torso = sum(m for _, m, _ in torso_items)
m_arm = sum(m for _, m, _ in arm_items)
mom_arm = sum(m * d for _, m, d in arm_items)
d_arm = mom_arm / m_arm
I_torso = sum(m * r * r for _, m, r in torso_items)
I_torso += 2.10 * (0.226**2 + 0.14**2) / 12 * 0.0  # radius-of-gyration form already used

def arm_I(pose, payload):
    """Arm inertia about the waist axis. pose 'down' | 'forward' | 'side'."""
    I = 0.0
    pts = [(m, d) for _, m, d in arm_items] + ([(payload, 0.46)] if payload else [])
    for m, d in pts:
        if pose == "down":
            r2 = X_JOINT**2
        elif pose == "forward":
            r2 = X_JOINT**2 + d**2
        else:  # side, T-pose
            r2 = (X_JOINT + d) ** 2
        I += m * r2
    return I

m_up = m_torso + 2 * m_arm
print(f"torso (no arms) {m_torso:.2f} kg, one arm {m_arm:.2f} kg (CoM {d_arm*1000:.0f} mm from joint line)")
print(f"upper body on the waist: {m_up:.2f} kg, +{2*PAYLOAD:.0f} kg payload = {m_up+2*PAYLOAD:.2f} kg")
print(f"I torso about yaw: {I_torso:.3f} kg m2")

# Yaw motion targets
ALPHA = {"brisk 20 rad/s2": 20.0, "aggressive 40 rad/s2": 40.0}
OMEGA = 6.0  # rad/s (~340 deg/s), fast human torso twist
print("\n-- Case 1: yaw with arms out + payload --")
for pose in ("down", "forward", "side"):
    for pl in (0.0, PAYLOAD):
        I = I_torso + 2 * arm_I(pose, pl)
        s = f"  pose {pose:7s} payload {pl:.0f} kg: I={I:.3f} kg m2"
        for k, a in ALPHA.items():
            s += f" | {k}: {I*a:5.1f} Nm"
        print(s)
I_worst = I_torso + 2 * arm_I("side", PAYLOAD)
print(f"  KE at {OMEGA} rad/s, T-pose+payload: {0.5*I_worst*OMEGA**2:.1f} J;"
      f" stop in 30 deg -> avg {0.5*I_worst*OMEGA**2/math.radians(30):.1f} Nm")
print(f"  RS03 speed: 200 rpm no-load = {200*2*math.pi/60:.1f} rad/s; target {OMEGA} rad/s")

# Static bending moment on the waist from offset CoM (arms forward, payload)
M_static = 2 * (mom_arm + PAYLOAD * 0.46) * g
print(f"\n  static tipping moment, both arms forward + 1 kg each: {M_static:.1f} Nm")
print(f"  one arm forward + 1 kg, other down: {(mom_arm+PAYLOAD*0.46)*g:.1f} Nm")
print(f"  axial (weight) on waist: {(m_up+2*PAYLOAD)*g:.0f} N static, x2 walking {(m_up+2*PAYLOAD)*g*2:.0f} N")
# Walking lateral accel 0.3 g at upper-body CoM height above the waist
h_cg = 0.20  # m, upper body CoM above the waist output plane (approx)
print(f"  walking: 0.3 g lateral at h={h_cg} m -> {0.3*g*(m_up+2)*h_cg:.0f} Nm extra bending")

print("\n-- Case 2: arm swing reaction --")
I_arm_sh = sum(m * d * d for _, m, d in arm_items) + PAYLOAD * 0.46**2
print(f"  arm (+1 kg) inertia about the shoulder: {I_arm_sh:.3f} kg m2")
for a in (20, 40, 80):
    print(f"  horizontal swing (roll axis vertical, arm raised forward) at {a} rad/s2: {I_arm_sh*a:5.1f} Nm per arm,"
          f" both same way {2*I_arm_sh*a:5.1f} Nm")
# Pitch swing: arms counter-swinging (gait) -> forward force at +-X joint line
for a in (20, 40):
    F = (mom_arm + PAYLOAD * 0.46) * a  # tangential force at CoM, arm horizontal
    print(f"  counter pitch swing at {a} rad/s2: F={F:.0f} N per arm -> yaw {2*F*X_JOINT:.1f} Nm")
print("  ceiling if the roll RS03 were driven at its 60 Nm peak: 60 Nm per arm, 120 Nm both")

print("\n-- Case 3: ground-level tip-over from standing --")
H = 1.52           # 5 ft robot
m_lower = 10.0     # pelvis + legs + 2x RS04 etc, assumption
m_tot = m_up + m_lower
y_cg = 0.85        # whole-body CoM height, assumption
E = m_tot * g * y_cg * 0.75  # rigid fall about the feet, ~75% of PE is ground-impact energy (rest lost/hips)
print(f"  whole robot {m_tot:.1f} kg, CoM {y_cg} m -> {m_tot*g*y_cg:.0f} J potential")
omega_fall = math.sqrt(3 * g / H)
print(f"  rigid-rod fall: omega at impact {omega_fall:.1f} rad/s; shoulder speed {omega_fall*(H-0.25):.1f} m/s")
# Legs decelerated through the waist when the torso lands first
L_leg = 0.85
I_leg = m_lower * L_leg**2 / 3
for t in (0.015, 0.030):
    M = I_leg * omega_fall / t
    print(f"  legs stopped through the waist in {t*1000:.0f} ms: bending {M:.0f} Nm, shear {M/ (L_leg*0.6):.0f} N")
# Twist: shoulder hits ground, friction at lateral offset
for Fn in (2000, 4000):
    print(f"  shoulder-first impact Fn={Fn} N, mu 0.5 at 0.2 m lever -> twist {0.5*Fn*0.2:.0f} Nm into the yaw drive")
