#!/usr/bin/env python3
"""Frame options A, B, C for the battery + power board requirement (Joey 2026-11-06 04:59Z). Writes parts_variants.json.
Torso coords. Battery envelopes: 20V-class drill pack 117 x 76 x 73 (DeWalt DCB205 class, two in series = 10S);
FlexVolt/60V-class pack 140 x 95 x 95 (15S, 63 V full: over the RS03 60 V limit without a regulator).
Power board envelope 170 x 90 x 35 (A), 140 x 90 x 35 (B/C), with a 40 mm fan."""
import json
E = 20.0; Y_BOT, Y_TOP, HZ = 155.0, 495.0, 70.0; Y_S0, Y_S1 = 353.62, 373.62; Y_SH = 424.86
RS03_D, RS03_REAR, RS03_FACE, RS03_OUT = 99.0, 78.4, 132.5, 135.0
C = dict(alu='#8c9096', new='#3f8fd2', gusset='#d9d9de', motor='#6f7378', printed='#1f9c8f', pi='#3aa655', jet='#26b3c9', bat='#e0c23a', psu='#b05fa0', fan='#26b3c9', shelf='#9a7bd1')
def mk():
    P = []
    def box(id, name, lo, hi, color, opacity=1.0, balloon=False, tag=None):
        P.append(dict(id=id, item=0, name=name, type='box', lo=list(lo), hi=list(hi), color=color, group='v', explode=[0, 0, 0], balloon=balloon, opacity=opacity, tag=tag))
    def cyl(id, name, axis, c, r, h, color):
        P.append(dict(id=id, item=0, name=name, type='cyl', axis=axis, c=list(c), r=r, h=h, color=color, group='v', explode=[0, 0, 0], balloon=False, opacity=1.0))
    return P, box, cyl
def rails_box(box, hx, y0, y1, color_rail, color_post, prefix, posts=True):
    """A 2020 box: posts at X +-(hx-20..hx), Z +-(50..70); front/rear rails between posts; side rails between front/rear rails."""
    if posts:
        for sx in (1, -1):
            for sz in (1, -1):
                box(f'{prefix}post{sx}{sz}', 'post', (min(sx * hx, sx * (hx - E)), y0, min(sz * HZ, sz * (HZ - E))), (max(sx * hx, sx * (hx - E)), y1, max(sz * HZ, sz * (HZ - E))), color_post)
    for yy in (y0, y1 - E):
        for sz in (1, -1):
            box(f'{prefix}rail{yy}{sz}', 'rail', (-(hx - E), yy, min(sz * HZ, sz * (HZ - E))), (hx - E, yy + E, max(sz * HZ, sz * (HZ - E))), color_rail)
        for sx in (1, -1):
            box(f'{prefix}side{yy}{sx}', 'rail', (min(sx * hx, sx * (hx - E)), yy, -(HZ - E)), (max(sx * hx, sx * (hx - E)), yy + E, HZ - E), C['alu'])
def common(box, cyl, hx_up):
    for sx in (1, -1):
        cyl(f'rs03{sx}', 'RS03', 'x', (sx * (RS03_REAR + (RS03_FACE - RS03_REAR) / 2), Y_SH, 0), RS03_D / 2, RS03_FACE - RS03_REAR, C['motor'])
        cyl(f'rs03o{sx}', 'flange', 'x', (sx * (RS03_FACE + 1.25), Y_SH, 0), 40, 2.5, '#9a9ea4')
        box(f'pb{sx}', 'pitch bracket', (min(sx * RS03_REAR, sx * hx_up), Y_S1, -50), (max(sx * RS03_REAR, sx * hx_up), Y_TOP, 50), C['printed'], 0.85)
    box('shelf', 'shelf', (-55, 354.6, -50), (55, 372.6, 50), C['shelf'])
    box('pi', 'Pi 5 + HAT', (-37.5, 376, -71.8), (19.7, 397, 13.2), C['pi'])
    box('jet', 'Jetson', (-51.5, 410, -45.5), (51.5, 445, 45), C['jet'], 0.8)
    for sx in (1, -1): box(f'fanx{sx}', 'fan', (min(sx * 15, sx * 55), 448, -50), (max(sx * 15, sx * 55), 488, -40), C['fan'], 0.8)
    cyl('rs03w', 'RS03 waist', 'y', (0, Y_BOT - 6 - 28.5, 0), RS03_D / 2, 57, C['motor'])
    box('wad', 'waist adapter', (-75, Y_BOT - 6, -HZ), (75, Y_BOT, HZ), C['printed'])
out = {}
# ---- A: one 226 box, mid rails at the shelf level --------------------------------------------
P, box, cyl = mk(); rails_box(box, 113, Y_BOT, Y_TOP, C['new'], C['alu'], 'A')
for sz in (1, -1): box(f'Amid{sz}', 'mid rail 186 (new)', (-93, Y_S0, min(sz * HZ, sz * (HZ - E))), (93, Y_S1, max(sz * HZ, sz * (HZ - E))), C['new'])
for sx in (1, -1): box(f'Ashelf{sx}', 'shelf rail 100 (new)', (min(sx * 75, sx * 55), Y_S0, -(HZ - E)), (max(sx * 75, sx * 55), Y_S1, HZ - E), C['new'])
common(box, cyl, 113)
box('Apsu', 'power board 170 x 90 x 35 + fan', (-85, 177, -45), (85, 212, 45), C['psu'], 0.85)
for sx in (1, -1): box(f'Abat{sx}', '20V pack 76 x 73 x 117', (min(sx * 4, sx * 80), 216, -36.5), (max(sx * 4, sx * 80), 333, 36.5), C['bat'], 0.9)
box('Afan', 'intake fan 40', (-20, 250, 40), (20, 290, 50), C['fan'], 0.8)
out['A'] = P
# ---- B: stepped, lower box 190 ---------------------------------------------------------------
P, box, cyl = mk(); rails_box(box, 95, Y_BOT, Y_S0 + E, C['alu'], C['alu'], 'Blo'); rails_box(box, 113, Y_S1, Y_TOP, C['new'], C['alu'], 'Bup')
# the shared rails replace the lower box top rails and upper box bottom rails: overwrite by drawing shared rails over them
for sz in (1, -1): box(f'Bsh{sz}', 'shared rail 226', (-113, Y_S0, min(sz * HZ, sz * (HZ - E))), (113, Y_S1, max(sz * HZ, sz * (HZ - E))), C['new'])
for sx in (1, -1):
    for sz in (1, -1):
        pts = [(55 + 20, 285), (95, 285), (113, 335), (113, 425), (93, 425), (93, Y_S1), (75, Y_S1)]
        P.append(dict(id=f'Bg{sx}{sz}', item=0, name='gusset', type='poly', pts=[(sx * x, y) for x, y in pts], radii=[8] * 7, holes=[], z0=HZ if sz > 0 else -HZ - 3, t=3, color=C['gusset'], group='v', explode=[0, 0, 0], balloon=False, opacity=1.0))
common(box, cyl, 113)
box('Bpsu', 'power board 140 x 90 x 35 + fan', (-70, 177, -45), (70, 212, 45), C['psu'], 0.85)
for sx in (1, -1): box(f'Bbat{sx}', '20V pack 73 x 76 x 117', (min(sx * 2, sx * 75), 216, -38), (max(sx * 2, sx * 75), 333, 38), C['bat'], 0.9)
box('Bfan', 'intake fan 40', (-20, 250, 40), (20, 290, 50), C['fan'], 0.8)
out['B'] = P
# ---- C: spec frame + 70 mm belt bay below -----------------------------------------------------
P, box, cyl = mk(); rails_box(box, 75, Y_BOT, Y_S0 + E, C['alu'], C['alu'], 'Clo'); rails_box(box, 113, Y_S1, Y_TOP, C['new'], C['alu'], 'Cup')
for sz in (1, -1): box(f'Csh{sz}', 'shared rail 226', (-113, Y_S0, min(sz * HZ, sz * (HZ - E))), (113, Y_S1, max(sz * HZ, sz * (HZ - E))), C['new'])
for sx in (1, -1):
    for sz in (1, -1):
        pts = [(55, 285), (75, 285), (113, 335), (113, 425), (93, 425), (93, Y_S1), (55, Y_S1)]
        P.append(dict(id=f'Cg{sx}{sz}', item=0, name='gusset', type='poly', pts=[(sx * x, y) for x, y in pts], radii=[8] * 7, holes=[], z0=HZ if sz > 0 else -HZ - 3, t=3, color=C['gusset'], group='v', explode=[0, 0, 0], balloon=False, opacity=1.0))
common(box, cyl, 113)
# belt bay 226 x 140 x 70 under the frame (Y 85 to 155); waist adapter and RS03 move down 70
P[:] = [p for p in P if p['id'] not in ('rs03w', 'wad')]
rails_box(box, 113, 85, 155, C['new'], C['new'], 'Cbelt')
box('Cpack', 'flat custom pack 180 x 90 x 40 (drill packs do not fit a 50 mm bay)', (-90, 90, -45), (90, 130, 45), C['bat'], 0.9)
box('Cpsu', 'power board 100 x 90 x 28', (-50, 200, -48), (50, 290, -20), C['psu'], 0.85)
cyl('rs03w', 'RS03 waist', 'y', (0, 85 - 6 - 28.5, 0), RS03_D / 2, 57, C['motor']); box('wad', 'waist adapter', (-113, 79, -HZ), (113, 85, HZ), C['printed'])
out['C'] = P

# ---- D family: two FlexVolt-class 60 V packs (Joey 2026-11-06 05:05Z, "spine" stack; 05:13Z backpack / partly proud) --------
# Pack envelope 160 x 102 x 95, 1.5 kg (DeWalt DCB6112 class: 159 x 102 x ~95; any similar pack). Rail/insert axis along the 160.
PK_L, PK_W, PK_H = 160.0, 102.0, 95.0
PK_C = C['bat']; DOCK = '#c9a227'
def pelvis_cells(box):
    # Joey 05:11Z: a fixed, non-removable cell low in the groin for CoG. Pelvis is not designed; placeholder cells flank the waist RS03.
    for sx in (1, -1): box(f'pelv{sx}', 'fixed pelvis cell (placeholder 60 x 80 x 90)', (min(sx * 55, sx * 115), 70, -45), (max(sx * 55, sx * 115), 150, 45), PK_C, 0.45)
def shell_back(box, y0=160, y1=490):
    box('shellback', 'shell back inner face (approx.)', (-95, y0, -99), (95, y1, -96), '#e8e8ec', 0.35)
def stepped(box, P, y_bot):
    rails_box(box, 75, y_bot, Y_S0 + E, C['alu'], C['alu'], 'Dlo'); rails_box(box, 113, Y_S1, Y_TOP, C['new'], C['alu'], 'Dup')
    for sz in (1, -1): box(f'Dsh{sz}', 'shared rail 226', (-113, Y_S0, min(sz * HZ, sz * (HZ - E))), (113, Y_S1, max(sz * HZ, sz * (HZ - E))), C['new'])
    for sx in (1, -1):
        for sz in (1, -1):
            pts = [(55, 285), (75, 285), (113, 335), (113, 425), (93, 425), (93, Y_S1), (55, Y_S1)]
            P.append(dict(id=f'Dg{sx}{sz}', item=0, name='gusset', type='poly', pts=[(sx * x, y) for x, y in pts], radii=[8] * 7, holes=[], z0=HZ if sz > 0 else -HZ - 3, t=3, color=C['gusset'], group='v', explode=[0, 0, 0], balloon=False, opacity=1.0))
def waist(box, cyl, y_bot, hx=75):
    cyl('rs03w', 'RS03 waist', 'y', (0, y_bot - 6 - 28.5, 0), RS03_D / 2, 57, C['motor']); box('wad', 'waist adapter', (-hx, y_bot - 6, -HZ), (hx, y_bot, HZ), C['printed'])
def psu(box, lo, hi, label='power board + fan'):
    box('Dpsu', label, lo, hi, C['psu'], 0.85)
def compute(box):
    box('shelf', 'shelf', (-55, 354.6, -50), (55, 372.6, 50), C['shelf'])
    box('pi', 'Pi 5 + HAT', (-37.5, 376, -71.8), (19.7, 397, 13.2), C['pi'])
    box('jet', 'Jetson', (-51.5, 410, -45.5), (51.5, 445, 45), C['jet'], 0.8)
    for sx in (1, -1): box(f'fanx{sx}', 'fan', (min(sx * 15, sx * 55), 448, -50), (max(sx * 15, sx * 55), 488, -40), C['fan'], 0.8)
def motors(box, cyl, hx_up=113):
    for sx in (1, -1):
        cyl(f'rs03{sx}', 'RS03', 'x', (sx * (RS03_REAR + (RS03_FACE - RS03_REAR) / 2), Y_SH, 0), RS03_D / 2, RS03_FACE - RS03_REAR, C['motor'])
        cyl(f'rs03o{sx}', 'flange', 'x', (sx * (RS03_FACE + 1.25), Y_SH, 0), 40, 2.5, '#9a9ea4')
        box(f'pb{sx}', 'pitch bracket', (min(sx * RS03_REAR, sx * hx_up), Y_S1, -50), (max(sx * RS03_REAR, sx * hx_up), Y_TOP, 50), C['printed'], 0.85)

# D1: inside the torso. Lower box widened to 226 (one straight box) and 20 mm taller (Y_BOT 135) so two packs stack flat across the back.
YB = 135.0
P, box, cyl = mk(); rails_box(box, 113, YB, Y_TOP, C['new'], C['alu'], 'D1')
for sz in (1, -1): box(f'D1mid{sz}', 'mid rail 186 (new)', (-93, Y_S0, min(sz * HZ, sz * (HZ - E))), (93, Y_S1, max(sz * HZ, sz * (HZ - E))), C['new'])
for sx in (1, -1): box(f'D1shelf{sx}', 'shelf rail 100 (new)', (min(sx * 75, sx * 55), Y_S0, -(HZ - E)), (max(sx * 75, sx * 55), Y_S1, HZ - E), C['new'])
motors(box, cyl); compute(box); waist(box, cyl, YB, 113)
for i, y0 in enumerate((YB + 24, YB + 24 + PK_H + 6)):
    box(f'D1pk{i}', 'pack 160 x 102 x 95, slides in from the side', (-80, y0, -58), (80, y0 + PK_H, -58 + PK_W), PK_C, 0.9)
    box(f'D1dk{i}', 'dock', (-93, y0 - 2, -60), (-80, y0 + PK_H + 2, 46), DOCK)
psu(box, (-85, YB + 24, 46), (85, YB + 24 + 2 * PK_H + 6, 46 + 24), 'power board 170 x 196 x 24 standing in front of the packs, fan below')
box('D1fan', 'intake fan 40', (-20, 250, 60), (20, 290, 70), C['fan'], 0.8)
pelvis_cells(box); shell_back(box, YB + 25)
out['D1'] = P

# D2: lumbar backpack. Spec stepped frame unchanged. Both packs outside the frame behind the lower back in a printed cradle on H3/H4/H5 rear points.
P, box, cyl = mk(); stepped(box, P, Y_BOT); motors(box, cyl); compute(box); waist(box, cyl, Y_BOT)
for i, y0 in enumerate((178, 178 + PK_H + 6)):
    box(f'D2pk{i}', 'pack 160 x 102 x 95, outside the frame', (-80, y0, -84 - PK_W), (80, y0 + PK_H, -84), PK_C, 0.9)
    box(f'D2dk{i}', 'dock', (-93, y0 - 2, -84 - PK_W - 2), (-80, y0 + PK_H + 2, -82), DOCK)
box('D2cr', 'printed cradle, 4 mm walls', (-96, 170, -84 - PK_W - 6), (96, 178 + 2 * PK_H + 10, -80), C['printed'], 0.35)
psu(box, (-50, 180, -48), (50, 330, -20), 'power board 100 x 150 x 28 inside, rear')
box('D2fan', 'intake fan 40', (-20, 250, 60), (20, 290, 70), C['fan'], 0.8)
pelvis_cells(box); shell_back(box)
out['D2'] = P

# D3: through the back. Stepped frame, lower box 20 mm taller (Y_BOT 135). Packs lengthwise (160 along Z), stacked, 102 wide between the 110 mm posts,
# slide in from the back onto docks at the front rail; about 65 mm of each pack stands proud of the frame rear face.
P, box, cyl = mk(); stepped(box, P, YB); motors(box, cyl); compute(box); waist(box, cyl, YB)
for i, y0 in enumerate((YB + 24, YB + 24 + PK_H + 6)):
    box(f'D3pk{i}', 'pack 160 x 102 x 95, slides in from the back', (-51, y0, -115), (51, y0 + PK_H, -115 + PK_L), PK_C, 0.9)
    box(f'D3dk{i}', 'dock on the front rail plane', (-54, y0 - 2, 45), (54, y0 + PK_H + 2, 58), DOCK)
psu(box, (-52, 190, 60), (52, 340, 84), 'power board 104 x 150 x 24 on the front face, fan beside')
box('D3fan', 'intake fan 40', (-20, 200, 62), (20, 240, 72), C['fan'], 0.8)
pelvis_cells(box); shell_back(box, YB + 25)
out['D3'] = P
json.dump(dict(**out, dims={}, meta={}), open('parts_variants.json', 'w'))
print({k: len(v) for k, v in out.items()})
