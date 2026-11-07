"""Neck load hand-calc (thread "Design the neck mechanism", 2026-11-07).

All inputs are ASSUMPTIONS until the head is modelled and weighed. Head frame as in the Argon
baseline (hardware/designs/head/concepts/bakeoff/argon/build_head.py): X fwd, Z up, z=0 = collar bottom,
jaw bottom z=50, crown ~z=235. Run: python neck_loads.py
"""
import math

G = 9.81
STS3215_STALL = 30 * 0.0981   # N*m at 12 V (vendor listing: 30 kg*cm stall)
STS3215_RATED = 10 * 0.0981   # N*m at 12 V (vendor listing: 10 kg*cm rated)

HEAD_W, HEAD_D, HEAD_H = 0.136, 0.130, 0.185   # m, Argon envelope (jaw band width, depth, chin->crown)
COM_Z, COM_X = 0.140, 0.010                     # m, assumed head COM: mid-height, 10 mm forward
ALPHA = 20.0                                     # rad/s^2, brisk but not violent head motion
PIVOTS = {"high (U-joint just above jaw floor, z=70)": 0.070, "low (at the collar, z=10)": 0.010}
TILTS_DEG = (0, 20, 40)                         # forward pitch (looking down)


def pitch_torque(m, pivot_z, tilt_deg):
    L = COM_Z - pivot_z
    t = math.radians(tilt_deg)
    grav = m * G * (L * math.sin(t) + COM_X * math.cos(t))
    i_cm = m * (HEAD_D ** 2 + HEAD_H ** 2) / 12
    inertia = (i_cm + m * (L ** 2 + COM_X ** 2)) * ALPHA
    return grav, inertia


def main():
    print(f"STS3215 @12 V: stall {STS3215_STALL:.2f} N*m, rated {STS3215_RATED:.2f} N*m (vendor listing)\n")
    for m in (1.0, 1.3):
        print(f"head mass {m:.1f} kg")
        for name, pz in PIVOTS.items():
            for tilt in TILTS_DEG:
                g, a = pitch_torque(m, pz, tilt)
                tot = g + a
                print(f"  pitch {name:44s} tilt {tilt:2d} deg: gravity {g:4.2f} + accel {a:4.2f} = {tot:4.2f} N*m"
                      f"  -> per servo (2 sharing) {tot/2:4.2f}")
        i_yaw = m * (HEAD_W ** 2 + HEAD_D ** 2) / 12 + m * COM_X ** 2
        print(f"  yaw: inertia {i_yaw*1e4:.1f} kg*cm^2, torque at {ALPHA:.0f} rad/s^2 = {i_yaw*ALPHA:.2f} N*m (no gravity term)\n")


if __name__ == "__main__":
    main()
