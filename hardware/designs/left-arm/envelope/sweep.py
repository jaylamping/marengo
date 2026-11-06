#!/usr/bin/env python3
"""Left arm swept envelope and arm-to-chest gap, in STEP coordinates (mm, X lateral, Y up, Z depth).

Inputs: stl/*.STL from the CAD parts, cad_export.json, component_boxes_mm.json.
Outputs: left_arm_envelope.stl, gaps.json.
"""
import json
import struct
import time
from pathlib import Path

import numpy as np
from scipy import ndimage as ni

HERE = Path(__file__).resolve().parent
VOX = 5.0
CX, Y0, Z0, AZ = 170.49, 424.860667548, -0.0141253, 5.985875
EY, RY = 209.860668, 66.260668
HAND_TIP_Y = -75.14
PITCH_AXIS, ROLL_AXIS, YAW_AXIS, ELBOW_AXIS = (-1, 0, 0), (0, 0, 1), (0, 1, 0), (-1, 0, 0)
SHOULDER = np.array([CX, Y0, Z0])
YAW_O, ELBOW_O = np.array([CX, Y0, AZ]), np.array([CX, EY, AZ])
FRAME_BOTTOM = 155.0
STL_RECORD = np.dtype([("n", "<3f4"), ("v", "<9f4"), ("a", "<u2")])


def rot(axis, deg):
    x, y, z = axis
    a = np.radians(deg)
    k = np.array([[0, -z, y], [z, 0, -x], [-y, x, 0]], float)
    return np.eye(3) + np.sin(a) * k + (1 - np.cos(a)) * k @ k


def read_stl(path):
    data = path.read_bytes()
    n = struct.unpack("<I", data[80:84])[0]
    assert len(data) == 84 + 50 * n, path
    return np.frombuffer(data, STL_RECORD, n, 84)["v"].reshape(-1, 3, 3).astype(float)


def world(name, tris, comp, boxes):
    """Place a part: rotate by its saved rows, then register its bounds to the saved world box."""
    if np.ptp(tris.reshape(-1, 3), axis=0).max() < 5:
        tris = tris * 1000.0
    box = np.array(boxes[name])
    p = tris if name == "marengo_upper_arm_left" else tris @ np.array(comp[name]["rows"])
    p = p + (box[:3] - p.reshape(-1, 3).min(0))
    assert np.abs(p.reshape(-1, 3).max(0) - box[3:]).max() < 2.0, (name, p.reshape(-1, 3).max(0), box)
    return p


def sample(tris, spacing=2.0):
    rng = np.random.default_rng(1)
    e1, e2 = tris[:, 1] - tris[:, 0], tris[:, 2] - tris[:, 0]
    area = 0.5 * np.linalg.norm(np.cross(e1, e2), axis=1)
    idx = np.repeat(np.arange(len(tris)), np.maximum(1, (area / spacing**2).astype(int) + 1))
    u, v = rng.random(len(idx)), rng.random(len(idx))
    flip = u + v > 1
    u[flip], v[flip] = 1 - u[flip], 1 - v[flip]
    return tris[idx, 0] + u[:, None] * e1[idx] + v[:, None] * e2[idx]


def cyl(r, y0, y1, step=2.0):
    t = np.arange(max(int(2 * np.pi * r / step), 24)) * 2 * np.pi / max(int(2 * np.pi * r / step), 24)
    a = np.stack(np.meshgrid(t, np.arange(y0, y1 + step, step), indexing="ij"), -1).reshape(-1, 2)
    return np.c_[CX + r * np.cos(a[:, 0]), a[:, 1], AZ + r * np.sin(a[:, 0])]


def voxels(p):
    return np.unique(np.round(p / VOX).astype(np.int32), axis=0).astype(float) * VOX


def write_stl(path, occ, origin):
    total = 0
    records = []
    for ax in range(3):
        a, b = (ax + 1) % 3, (ax + 2) % 3
        for sign in (1, -1):
            idx = np.argwhere(occ & ~np.roll(occ, -sign, axis=ax))
            c0 = idx * VOX + origin - VOX / 2
            c0[:, ax] += VOX / 2 + sign * VOX / 2
            ea, eb = np.eye(3)[a] * VOX, np.eye(3)[b] * VOX
            if sign < 0:
                ea, eb = eb, ea
            rec = np.zeros(len(c0) * 2, STL_RECORD)
            rec["n"] = np.eye(3)[ax] * sign
            rec["v"][::2] = np.c_[c0, c0 + ea, c0 + ea + eb]
            rec["v"][1::2] = np.c_[c0, c0 + ea + eb, c0 + eb]
            records.append(rec)
            total += len(rec)
    with open(path, "wb") as out:
        out.write(b"marengo left arm swept envelope".ljust(80, b" "))
        out.write(struct.pack("<I", total))
        for rec in records:
            out.write(rec.tobytes())
    return total


def wall(y):
    """Chest side half-width at height y."""
    h = Y0 - y
    return np.where(h <= 170, 108.0, 108.0 - (h - 170) * (108 - 82) / (Y0 - FRAME_BOTTOM - 170))


def gap(cloud, pitch, roll):
    r = rot(PITCH_AXIS, pitch) @ rot(ROLL_AXIS, roll)
    p = (cloud - SHOULDER) @ r.T + SHOULDER
    inside = (p[:, 1] >= FRAME_BOTTOM) & (p[:, 1] <= Y0) & (p[:, 2] >= -88) & (p[:, 2] <= 110)
    if not inside.any():
        return None
    g = p[inside, 0] - wall(p[inside, 1])
    j = int(np.argmin(g))
    return round(float(g[j]), 1), p[inside][j].round(1).tolist()


def limit(cloud, target=5.0):
    table = {}
    for roll in np.arange(-15, 30.01, 0.25):
        worst = None
        for pitch in range(-50, 181, 5):
            g = gap(cloud, pitch, roll)
            if g and (worst is None or g[0] < worst[0]):
                worst = (g[0], pitch, g[1])
        table[float(roll)] = worst
    passing = [r for r in sorted(table) if table[r] is None or table[r][0] >= target]
    return (passing[0] if passing else None), table


def main():
    t0 = time.time()
    export = json.loads((HERE.parent / "cad_export.json").read_text())
    comp = export["components"]
    raw = json.loads((HERE.parent / "component_boxes_mm.json").read_text())
    boxes = {k.replace("parts/", "").replace(".SLDPRT", ""): v for k, v in raw.items() if k.startswith("parts/")}
    boxes["marengo_upper_arm_left"] = [131.24, 167.611, -36.264, 209.74, 319.86, 48.236]
    files = {"P1_L01": "P1_L01", "ROLL_RS03": "ROLL_RS03", "SPACER": "SPACER", "P2_L04B": "P2_L04B", "YAW_RS02": "YAW_RS02",
             "marengo_upper_arm_left": "UPPER_ARM", "ELBOW_RS02": "ELBOW_RS02", "FOREARM_RS00": "FOREARM_RS00"}
    pts = {n: sample(world(n, read_stl(HERE / "stl" / f"{f}.STL"), comp, boxes)) for n, f in files.items()}
    print("parts sampled", {k: len(v) for k, v in pts.items()}, round(time.time() - t0, 1))
    ua = pts["marengo_upper_arm_left"]
    # The forearm is not designed yet: proxy shell over the RS00, to the elbow and the hand stub.
    forearm = np.vstack([pts["FOREARM_RS00"], cyl(31.5, RY + 51.4, EY - 40), cyl(31.5, RY - 51.4, RY), cyl(30, HAND_TIP_Y, RY - 51.4)])

    post = [voxels(np.vstack([pts["SPACER"], pts["P2_L04B"], pts["YAW_RS02"]]))]
    yaw_link = voxels(np.vstack([ua, pts["ELBOW_RS02"]]))
    fore = voxels(forearm)
    for yaw in np.arange(-90, 91, 10):
        ry = rot(YAW_AXIS, yaw)
        post.append(voxels((yaw_link - YAW_O) @ ry.T + YAW_O))
        for elbow in np.arange(0, 136, 5):
            p = (fore - ELBOW_O) @ rot(ELBOW_AXIS, elbow).T + ELBOW_O
            post.append(voxels((p - YAW_O) @ ry.T + YAW_O))
    post = voxels(np.vstack(post))
    print("post-roll points", len(post), round(time.time() - t0, 1))

    lo, hi = np.array([-750.0, -300.0, -800.0]), np.array([900.0, 1150.0, 800.0])
    occ = np.zeros(np.ceil((hi - lo) / VOX).astype(int) + 1, bool)

    def mark(p):
        i = np.round((p - lo) / VOX).astype(np.int32)
        occ[i[:, 0], i[:, 1], i[:, 2]] = True

    pitch_set = voxels(np.vstack([pts["P1_L01"], pts["ROLL_RS03"]]))
    for pitch in np.arange(-50, 181, 2.0):
        rp = rot(PITCH_AXIS, pitch)
        mark((pitch_set - SHOULDER) @ rp.T + SHOULDER)
        for roll in np.arange(-15, 181, 2.0):
            mark((post - SHOULDER) @ (rp @ rot(ROLL_AXIS, roll)).T + SHOULDER)
    print("swept", int(occ.sum()), round(time.time() - t0, 1))
    solid = ni.binary_fill_holes(ni.binary_closing(occ, ni.generate_binary_structure(3, 1), iterations=3) | occ)
    tris = write_stl(HERE / "left_arm_envelope.stl", solid, lo)
    nz = np.argwhere(solid)
    print("solid voxels", int(solid.sum()), "tris", tris, "bbox", (nz.min(0) * VOX + lo).tolist(), (nz.max(0) * VOX + lo).tolist(), round(time.time() - t0, 1))

    tube = ua[ua[:, 1] > 254.0]
    report = {"wall": "X 108 to 170 mm below the shoulder, linear to X 82 at Y 155, Z -88 to 110, stops at the shoulder axis height",
              "roll_-15": {p: {"tube": gap(tube, p, -15), "whole_part": gap(ua, p, -15)} for p in range(-50, 181, 10)}}
    for name, cloud in (("tube", tube), ("whole_part", ua)):
        lim, table = limit(cloud)
        report[f"limit_5mm_{name}"] = {"roll_deg": lim, "at": table.get(lim), "roll_0": table[0.0], "roll_-5": table[-5.0], "roll_-10": table[-10.0], "roll_-15": table[-15.0]}
        print(name, report[f"limit_5mm_{name}"])
    (HERE / "gaps.json").write_text(json.dumps(report, indent=1))
    print("bbox check, solid volume m3", solid.sum() * VOX**3 / 1e9, "done", round(time.time() - t0, 1))


main()
