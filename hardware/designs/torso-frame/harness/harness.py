#!/usr/bin/env python3
"""Torso harness as solid wires, for the torso assembly (Joey 2026-11-07: real cable routing in the CAD).

Torso coordinates (mm): X sideways (+X robot left), Y up, Z forward. Each run is a list of points; corners are filleted at the run's
bend radius (silicone wire ~3 x OD, 8 AWG >= 15) and a round section of the wire's OD is swept along it. Pairs are two parallel runs.

Ends at the power board are OPEN until Joey places the board in the torso (power board thread: HARNESS_INTERFACE.md). Until then the
board-bound runs stop at a named stub point and the export only covers the frame-side segments.
Run: python3 harness.py  ->  harness.step, harness.json (lengths, minimum bend radius per run)
"""
import json
from pathlib import Path

from build123d import Circle, FilletPolyline, Plane, Compound, export_step, sweep, Vector

HERE = Path(__file__).resolve().parent

# wire classes: OD includes the jacket (silicone), bend = minimum inside bend radius used at corners
WIRE = {
    '8awg': dict(od=7.0, bend=15.0),
    '14awg': dict(od=3.4, bend=8.0),
    '16awg': dict(od=2.9, bend=7.0),
}

# Runs: name, wire class, points, note. Points are torso coordinates.
RUNS = [
    # brake resistors (2 x HS50 20R, mirror images, main 72a3889): bottom tag at Y 285.8, X 60.15; lead drops down the post's inner
    # front edge (X 53, Z 72) toward the board. Board end open: stops at Y 200 for now.
    *[(f'brake_{s}_{k}', '16awg', [(s * (60.15 + d), 285.0, 82.0), (s * (60.15 + d), 280.0, 82.0), (s * (54.0 + d / 2), 272.0, 75.0), (s * (54.0 + d / 2), 200.0, 75.0)],
       'HS50 bottom tag down the post front edge; board end open') for s in (1, -1) for k, d in ((0, -2.0), (1, 2.0))],
    # 12 V brick on the top front rail (main 79500b9): input end (-X), output end (+X); both drop down the front of the upper bay.
    ('dcdc_in_pos', '14awg', [(-25.4, 488.0, 90.0), (-36.0, 488.0, 90.0), (-36.0, 470.0, 88.0), (-36.0, 380.0, 88.0)], '48 V in from the board, fuse/cap zone at -X; board end open'),
    ('dcdc_in_neg', '14awg', [(-25.4, 482.0, 90.0), (-39.0, 482.0, 90.0), (-39.0, 468.0, 88.0), (-39.0, 380.0, 88.0)], '48 V return; board end open'),
    ('dcdc_out_pos', '14awg', [(25.4, 488.0, 90.0), (36.0, 488.0, 90.0), (36.0, 470.0, 88.0), (36.0, 380.0, 88.0)], '12 V out; board end open'),
    ('dcdc_out_neg', '14awg', [(25.4, 482.0, 90.0), (39.0, 482.0, 90.0), (39.0, 468.0, 88.0), (39.0, 380.0, 88.0)], '12 V return; board end open'),
]
# NOT YET ROUTED (needs the board placement or a pack-corner decision): pack pair 8 AWG XT90S -> board; limb pigtails 14 AWG;
# signal cables. Open: the XT90S lies along X on the pack rear face, and with 8 AWG the board-side end needs ~18 mm of bend room
# beyond the connector, which |X| <= 45 and the XT60 beside it do not leave (battery thread to settle the corner).


def solid(points, od, bend):
    pts = [Vector(*p) for p in points]
    path = FilletPolyline(*pts, radius=bend + od / 2) if len(pts) > 2 else None
    if path is None:
        from build123d import Line
        path = Line(pts[0], pts[1])
    start = path @ 0
    tangent = path % 0
    prof = Plane(origin=start, z_dir=tangent) * Circle(od / 2)
    return sweep(prof, path=path), path.length


def build():
    solids, info = [], []
    for name, cls, pts, note in RUNS:
        w = WIRE[cls]
        s, length = solid(pts, w['od'], w['bend'])
        s.label = name
        solids.append(s)
        info.append(dict(run=name, wire=cls, od=w['od'], bend_r=w['bend'], length_mm=round(length, 1), note=note))
    export_step(Compound(children=solids), str(HERE / 'harness.step'))
    (HERE / 'harness.json').write_text(json.dumps(info, indent=1))
    return info


if __name__ == '__main__':
    for row in build():
        print(row['run'], row['wire'], row['length_mm'])
