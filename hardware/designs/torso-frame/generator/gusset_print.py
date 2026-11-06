#!/usr/bin/env python3
"""Temporary PETG gussets (Joey 2026-11-06 20:45Z): the release gusset outline (frame.py GUSSET_PTS/R/HOLES, same as gusset_plate_left.dxf)
extruded 6 mm, 2 left + 2 mirrored right, laid flat, written as STL and a plain 3MF for Bambu Studio. Bench stand-in until the 3 mm 6061 plates arrive."""
import math, re, trimesh, numpy as np
from shapely.geometry import Polygon, Point
src = open('frame.py').read()
Y_STEP_HI = 373.62
PTS = eval(re.search(r'GUSSET_PTS = (\[.*?\])', src).group(1)); R = eval(re.search(r'GUSSET_R = (\[.*?\])', src).group(1)); HOLES = eval(re.search(r'GUSSET_HOLES = (\[.*?\])', src).group(1))
T = 6.0; HOLE_D = 5.5
def unit(v):
    n = math.hypot(*v); return (v[0] / n, v[1] / n)
ring = []
n = len(PTS)
for i, (vx, vy) in enumerate(PTS):
    px, py = PTS[i - 1]; nx, ny = PTS[(i + 1) % n]
    u1 = unit((px - vx, py - vy)); u2 = unit((nx - vx, ny - vy)); r = R[i]
    half = math.acos(max(-1, min(1, u1[0] * u2[0] + u1[1] * u2[1]))) / 2
    b = unit((u1[0] + u2[0], u1[1] + u2[1])); d = r / math.sin(half); C = (vx + b[0] * d, vy + b[1] * d)
    t = r / math.tan(half); T1 = (vx + u1[0] * t, vy + u1[1] * t); T2 = (vx + u2[0] * t, vy + u2[1] * t)
    a1 = math.atan2(T1[1] - C[1], T1[0] - C[0]); a2 = math.atan2(T2[1] - C[1], T2[0] - C[0])
    da = (a2 - a1 + math.pi) % (2 * math.pi) - math.pi
    for k in range(13):
        a = a1 + da * k / 12; ring.append((C[0] + r * math.cos(a), C[1] + r * math.sin(a)))
poly = Polygon(ring)
if not poly.is_valid: poly = poly.buffer(0)
holes = [Point(x, y).buffer(HOLE_D / 2, 48) for x, y in HOLES]
for h in holes: poly = poly.difference(h)
def plate(mirror):
    p = poly if not mirror else Polygon([(-x, y) for x, y in poly.exterior.coords], [[(-x, y) for x, y in i.coords] for i in poly.interiors])
    m = trimesh.creation.extrude_polygon(p, T)
    m.apply_translation(-np.array([m.bounds[0][0], m.bounds[0][1], 0]))
    return m
parts = []
for i, mir in enumerate((False, False, True, True)):
    m = plate(mir); m.apply_translation([(i % 2) * 70, (i // 2) * 150, 0]); parts.append(m)
plate_all = trimesh.util.concatenate(parts)
import os
os.makedirs('../prints', exist_ok=True)
plate(False).export('../prints/gusset_temp_left_6mm.stl'); plate(True).export('../prints/gusset_temp_right_6mm.stl')
scene = trimesh.Scene()
for i, m in enumerate(parts): scene.add_geometry(m, node_name=f"gusset_{'L' if i < 2 else 'R'}{i % 2 + 1}")
scene.export('../prints/gusset_temp_x4_6mm.3mf')
print('bounds per plate', plate(False).extents, 'volume cm3', round(plate(False).volume / 1000, 2), 'plate set', plate_all.extents, 'watertight', plate(False).is_watertight)
