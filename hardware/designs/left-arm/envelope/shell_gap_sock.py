#!/usr/bin/env python3
"""Max arm pitch against the body shell for socket radii 62, 70, 78 and 93, at fw=38, topcap 0, rim 7.

Collar: r 43 to 57, 2 mm flange with its back at X 134.5, and a 2 mm chamfer from r 55 to r 57.
Outside the collar the socket floor is the generator's FACE, X 135.5, or X 134.5 as an option.
Buttons: M4 heads on the r 49 ring, 2.2 mm proud of the collar. Output: shell_gaps_sock.json.
"""
import json

exec(open("shell_gap.py").read().split("PITCHES = list")[0])

COLLAR, HEAD = 134.5, 136.7
PARTS = {}
for n in ("P2_L04B", "SPACER", "YAW_RS02", "marengo_upper_arm_left", "ELBOW_RS02"):
    f = files[n]
    s = sample(world(n, read_stl(HERE / "stl" / f"{f}.STL"), comp, boxes), 2.0)
    PARTS[n] = s[:: max(1, len(s) // 40000)]
SETS = {"P2": (np.vstack([PARTS["P2_L04B"], PARTS["SPACER"]]), [0]),
        "yaw_RS02": (PARTS["YAW_RS02"], [0]),
        "upper_arm_with_carrier": (np.vstack([PARTS["marengo_upper_arm_left"], PARTS["ELBOW_RS02"]]), list(range(-90, 91, 15)))}
ROWS_ = ROWS


def gap(p, sock, floor):
    x, inside = wall_x(ROWS_, p[:, 1], p[:, 2])
    rad = np.hypot(p[:, 1] - Y0, p[:, 2] - Z0)
    t = np.clip((rad - sock) / 7.0, 0, 1)
    x = floor + (x - floor) * (t * t * (3 - 2 * t))
    c = rad < 57
    x[c] = COLLAR - 2.0 * np.clip((rad[c] - 55.0) / 2.0, 0, 1)
    ok = inside & (rad >= BORE_R)
    if not ok.any():
        return None, None, None
    g = (p[:, 0] - x)[ok]
    j = int(np.argmin(g))
    head = (rad >= 45.4) & (rad <= 52.6)
    h = float((p[head, 0] - HEAD).min()) if head.any() else None
    return float(g[j]), [round(float(v), 1) for v in p[ok][j]], h


PITCH = [i * 2.5 for i in range(-20, 73)]
out = {}
for floor in (135.5,):
    for sock in (62.0, 70.0, 72.0, 74.0, 76.0, 78.0, 93.0):
        for roll in (0.0, -5.0):
            key = f"floor{floor}_sock{int(sock)}_roll{int(roll)}"
            tab = {}
            for name, (cloud, yaws) in SETS.items():
                row = {}
                for pitch in PITCH:
                    best = None
                    for yaw in yaws:
                        b = yawed(cloud, yaw) if yaw else cloud
                        g, w, h = gap(posed(b, pitch, roll, FW), sock, floor)
                        if g is not None and (best is None or g < best[0]):
                            best = [round(g, 1), yaw, w, None if h is None else round(h, 1)]
                    row[pitch] = best
                tab[name] = row
                vals = [(p, row[p][0]) for p in PITCH if row[p]]
                m = 180.0
                bad = [p for p, v in vals if v < 3 and p >= 0]
                m = (min(bad) - 2.5) if bad else 180.0
                neg = [p for p, v in vals if v < 3 and p < 0]
                lo = min(vals, key=lambda kv: kv[1])
                print(key, name, "max_ok", m, "min", lo, "at", row[lo[0]][2], "under3_neg", (min(neg), max(neg)) if neg else None,
                      "headgap", min((row[p][3] for p in PITCH if row[p] and row[p][3] is not None), default=None))
            out[key] = tab
(HERE / "shell_gaps_sock.json").write_text(json.dumps(out, indent=1))
