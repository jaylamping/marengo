#!/usr/bin/env python3
"""Writes ../gusset_plate_left.dxf: the release gusset outline from frame.py (GUSSET_PTS, GUSSET_R, GUSSET_HOLES) as true LINE/ARC/CIRCLE
entities, mm, torso X-Y plane, left-front plate. The 2026-11-06 gate reviews found the earlier file in torso-stepped/ was R8 at every corner
while the spec says R15 at X 113 / Y 335. Mirror X for the right-hand plates. Run after frame.py."""
import math, re
src = open('frame.py').read()
Y_STEP_HI = 373.62
PTS = eval(re.search(r'GUSSET_PTS = (\[.*?\])', src).group(1)); R = eval(re.search(r'GUSSET_R = (\[.*?\])', src).group(1)); HOLES = eval(re.search(r'GUSSET_HOLES = (\[.*?\])', src).group(1))
HOLE_D = 5.5; T = 3.0
def unit(v):
    n = math.hypot(*v); return (v[0] / n, v[1] / n)
ents = []
n = len(PTS); tang = []
for i, (vx, vy) in enumerate(PTS):
    px, py = PTS[i - 1]; nx, ny = PTS[(i + 1) % n]
    u1 = unit((px - vx, py - vy)); u2 = unit((nx - vx, ny - vy)); r = R[i]
    cosang = max(-1, min(1, u1[0] * u2[0] + u1[1] * u2[1])); half = math.acos(cosang) / 2
    t = r / math.tan(half); T1 = (vx + u1[0] * t, vy + u1[1] * t); T2 = (vx + u2[0] * t, vy + u2[1] * t)
    b = unit((u1[0] + u2[0], u1[1] + u2[1])); d = r / math.sin(half); C = (vx + b[0] * d, vy + b[1] * d)
    a1 = math.degrees(math.atan2(T1[1] - C[1], T1[0] - C[0])) % 360; a2 = math.degrees(math.atan2(T2[1] - C[1], T2[0] - C[0])) % 360
    if (a2 - a1) % 360 > 180: a1, a2 = a2, a1
    ents.append(('ARC', C, r, a1, a2)); tang.append((T1, T2))
for i in range(n):
    ents.append(('LINE', tang[i][1], tang[(i + 1) % n][0]))
for hx, hy in HOLES: ents.append(('CIRCLE', (hx, hy), HOLE_D / 2))
out = ['0', 'SECTION', '2', 'HEADER', '9', '$INSUNITS', '70', '4', '0', 'ENDSEC', '0', 'SECTION', '2', 'ENTITIES']
for e in ents:
    if e[0] == 'LINE': out += ['0', 'LINE', '8', 'OUTLINE', '10', f'{e[1][0]:.4f}', '20', f'{e[1][1]:.4f}', '30', '0', '11', f'{e[2][0]:.4f}', '21', f'{e[2][1]:.4f}', '31', '0']
    elif e[0] == 'ARC': out += ['0', 'ARC', '8', 'OUTLINE', '10', f'{e[1][0]:.4f}', '20', f'{e[1][1]:.4f}', '30', '0', '40', f'{e[2]:.4f}', '50', f'{e[3]:.4f}', '51', f'{e[4]:.4f}']
    else: out += ['0', 'CIRCLE', '8', 'HOLES', '10', f'{e[1][0]:.4f}', '20', f'{e[1][1]:.4f}', '30', '0', '40', f'{e[2]:.4f}']
out += ['0', 'ENDSEC', '0', 'EOF']
open('../gusset_plate_left.dxf', 'w').write('\n'.join(out) + '\n')
# area and mass for the BOM check (shoelace minus fillet corner losses, minus holes)
A = 0.5 * abs(sum(PTS[i][0] * PTS[(i + 1) % n][1] - PTS[(i + 1) % n][0] * PTS[i][1] for i in range(n)))
loss = 0
for i in range(n):
    px, py = PTS[i - 1]; vx, vy = PTS[i]; nx, ny = PTS[(i + 1) % n]
    u1 = unit((px - vx, py - vy)); u2 = unit((nx - vx, ny - vy)); half = math.acos(max(-1, min(1, u1[0] * u2[0] + u1[1] * u2[1]))) / 2
    r = R[i]; t = r / math.tan(half); loss += r * t - r * r * (math.pi - 2 * half) / 2
A_net = A - loss - len(HOLES) * math.pi * (HOLE_D / 2) ** 2
print(f'gusset_plate_left.dxf: {len(ents)} entities, R={R}, net area {A_net:.0f} mm2, mass {A_net * T * 2.70e-3:.1f} g per plate')
