#!/usr/bin/env python3
"""Clearance from the left arm to the body shell, for a shell and shoulder variant.

The shell is the concept loft: rows of height, half-width, front and back depth and squareness, with each
section a superellipse. Inside 62 mm of the pitch axis the side steps in to the socket floor.
Run from the envelope folder with sweep.py, stl/ and the layout JSON files beside it.
Output: shell_gaps.json.
"""
import json

SRC = open("sweep.py").read().split("def main():")[0]
exec(SRC)

F0, F1, ZS = 155.0, 495.0, 424.860667548
SOCK_R = 62.0
BORE_R = 43.0  # collar bore, open to the rotating output
PITCH_ONLY = ("P1", "roll_RS03")
BASE = [[F0 - 6, 79, 80, 76, 3.6], [ZS - 200, 84, 92, 80, 3.6], [ZS - 100, 90, 104, 85, 3.8], [ZS - 55, 108, 108, 88, 4.2],
        [ZS + 10, 120, 108, 88, 4.6], [F1 + 12, 121, 100, 88, 4.8], [F1 + 28, 106, 88, 82, 4.2], [F1 + 38, 64, 60, 64, 3.0], [F1 + 48, 42, 44, 46, 2.4]]
WALL = 149.0
WIDE = [[F0 - 6, 79, 80, 76, 3.6], [ZS - 200, WALL - 60, 92, 80, 3.6], [ZS - 160, WALL - 46, 100, 84, 3.8], [ZS - 111, WALL - 23, 106, 86, 4.0],
        [ZS - 55, WALL - 7, 108, 88, 4.4], [ZS + 10, WALL - 5, 108, 88, 4.8], [F1 + 12, WALL, 100, 88, 4.8], [F1 + 28, WALL - 10, 88, 82, 4.2],
        [F1 + 38, 64, 60, 64, 3.0], [F1 + 48, 42, 44, 46, 2.4]]


def wall_x(rows, y, z):
    r = np.array(rows, float)
    hw, front, back, n = (np.interp(y, r[:, 0], r[:, i]) for i in (1, 2, 3, 4))
    d = np.where(z >= 0, front, back)
    t = np.clip(np.abs(z) / d, 0, None)
    inside = (t < 1) & (y >= r[0, 0]) & (y <= r[-1, 0])
    x = np.zeros_like(y)
    x[inside] = hw[inside] * (1 - t[inside] ** n[inside]) ** (1 / n[inside])
    return x, inside


def posed(cloud, pitch, roll, shift):
    p = (cloud - SHOULDER) @ (rot(PITCH_AXIS, pitch) @ rot(ROLL_AXIS, roll)).T + SHOULDER
    p = p.copy()
    p[:, 0] += shift
    return p


def yawed(cloud, yaw):
    return (cloud - YAW_O) @ rot(YAW_AXIS, yaw).T + YAW_O


def gap_of(p, rows, floor):
    x, inside = wall_x(rows, p[:, 1], p[:, 2])
    rad = np.hypot(p[:, 1] - Y0, p[:, 2] - Z0)
    x = np.where(rad < SOCK_R, floor, x)
    inside = inside & (rad >= BORE_R)
    if not inside.any():
        return None
    g = (p[:, 0] - x)[inside]
    j = int(np.argmin(g))
    return round(float(g[j]), 1), p[inside][j].round(1).tolist()


def sweep(sets, rows, floor, shift, rolls, pitches):
    out = {}
    for name, (cloud, yaws, keep_low) in sets.items():
        worst = {}
        for roll in rolls:
            best = None
            for pitch in pitches:
                for yaw in yaws:
                    base = yawed(cloud, yaw) if yaw else cloud
                    p = posed(base, pitch, 0.0 if name in PITCH_ONLY else roll, shift)
                    if keep_low:
                        p = np.vstack([p, posed(base, pitch, roll, 0.0)])
                    g = gap_of(p, rows, floor)
                    if g and (best is None or g[0] < best[0]):
                        best = (g[0], float(pitch), float(yaw), g[1])
            worst[float(roll)] = best
        out[name] = worst
    return out


def allowed(sets, heights, floor_shift, roll, pitches):
    """Largest shell half-width at each height with 5 mm to the arm at this roll, across pitch."""
    table = {}
    for h in heights:
        y = ZS - h
        low = None
        for name, (cloud, yaws, _) in sets.items():
            for pitch in pitches:
                for yaw in yaws:
                    p = posed(yawed(cloud, yaw) if yaw else cloud, pitch, roll, floor_shift)
                    m = (np.abs(p[:, 1] - y) <= 4) & (p[:, 2] >= -88) & (p[:, 2] <= 108) & (np.hypot(p[:, 1] - Y0, p[:, 2] - Z0) >= SOCK_R)
                    if m.any():
                        j = int(np.argmin(p[m, 0]))
                        if low is None or p[m, 0][j] < low[0]:
                            low = (round(float(p[m, 0][j]), 1), name, float(pitch), float(yaw))
        table[h] = None if low is None else {"max_shell_half_width": round(low[0] - 5, 1), "limited_by": low[1], "pitch": low[2], "yaw": low[3]}
    return table


export = json.loads((HERE.parent / "cad_export.json").read_text())
comp = export["components"]
raw = json.loads((HERE.parent / "component_boxes_mm.json").read_text())
boxes = {k.replace("parts/", "").replace(".SLDPRT", ""): v for k, v in raw.items() if k.startswith("parts/")}
boxes["marengo_upper_arm_left"] = [131.24, 167.611, -36.264, 209.74, 319.86, 48.236]
files = {"P1_L01": "P1_L01", "ROLL_RS03": "ROLL_RS03", "SPACER": "SPACER", "P2_L04B": "P2_L04B", "YAW_RS02": "YAW_RS02",
         "marengo_upper_arm_left": "UPPER_ARM", "ELBOW_RS02": "ELBOW_RS02"}
pts = {}
for n, f in files.items():
    s = sample(world(n, read_stl(HERE / "stl" / f"{f}.STL"), comp, boxes), 2.0)
    pts[n] = s[:: max(1, len(s) // 12000)]
ua = pts["marengo_upper_arm_left"]
YAWS = list(range(-90, 91, 30))
NOYAW = [0]


def sets_for(p1_stretch):
    return {
        "P1": (pts["P1_L01"], NOYAW, p1_stretch),
        "roll_RS03": (pts["ROLL_RS03"], NOYAW, False),
        "P2_spacer": (np.vstack([pts["P2_L04B"], pts["SPACER"]]), NOYAW, False),
        "yaw_RS02": (pts["YAW_RS02"], NOYAW, False),
        "tube": (ua[ua[:, 1] > 254.0], NOYAW, False),
        "upper_arm_with_carrier": (np.vstack([ua, pts["ELBOW_RS02"]]), YAWS, False),
    }


# P1 and roll RS03 only follow pitch.
def fix_roll(sets):
    return sets


PITCH = list(range(-50, 181, 10))
FW = 38.0
TOP = 124.0 + FW
WX = 10.0
SHELL_38 = [[F0 - 6, 79 + WX, 80, 76, 3.6], [ZS - 200, TOP - 60 + 0.6 * WX, 92, 80, 3.6], [ZS - 160, TOP - 46, 100, 84, 3.8], [ZS - 111, TOP - 23, 106, 86, 4.0],
            [ZS - 55, TOP - 7, 108, 88, 4.4], [ZS + 10, TOP - 5, 108, 88, 4.8], [F1 + 12, TOP, 100, 88, 4.8], [F1 + 28, TOP - 10, 88, 82, 4.2],
            [F1 + 38, 64, 60, 64, 3.0], [F1 + 48, 42, 44, 46, 2.4]]
report = {}
ROLLS = [0.0, -5.0]
OUT = [float(r) for r in range(10, 181, 20)]
for name, rows, floor, shift, sock in (
    ("A_base_shoulder_shell_4421a5d", BASE, 97.5, 0.0, 62.0),
    ("D_cage_plus38_shell_fa63d48", SHELL_38, 135.5, FW, 62.0),
    ("E_cage_plus38_socket_r93", SHELL_38, 135.5, FW, 93.0),
):
    SOCK_R = sock
    s = sets_for(False)
    report[name] = {"inward_and_zero": sweep(s, rows, floor, shift, ROLLS, PITCH), "outward": sweep(s, rows, floor, shift, OUT, PITCH)}
    for part, table in report[name]["inward_and_zero"].items():
        print(name, part, table)
    print(name, "outward worst", {p: min((v for v in t.values() if v), default=None, key=lambda v: v[0]) for p, t in report[name]["outward"].items()})
s = sets_for(False)
del s["P1"], s["roll_RS03"]
SOCK_R = 62.0
report["allowed_5deg_plus38"] = allowed(s, [-10, 55, 111, 160, 200], FW, -5.0, PITCH)
print(report["allowed_5deg_plus38"])
(HERE / "shell_gaps.json").write_text(json.dumps(report, indent=1))
