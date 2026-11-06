# 2D packaging drawings from parts.json: front (X-Y), side (Z-Y), plan (X-Z). Writes out/pack_*.png
import json
from PIL import Image, ImageDraw, ImageFont
F = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'; FB = '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'
f = lambda s, b=False: ImageFont.truetype(FB if b else F, s)
D = json.load(open('parts.json')); P = D['parts']
S = 4.0  # px per mm
LABEL = {'rs03_pitch': 'RS03 pitch', 'rs03_boss': '', 'rs03_pitch_out': '', 'pitch_bracket': 'pitch bracket', 'pitch_wing': '', 'shelf': 'shelf', 'pi5': 'Pi 5 + HAT', 'pi_stub': 'Pi stub',
         'imu_old': 'IMU', 'pdb_logic': 'logic board', 'pdb_carrier': '', 'pdb_sheet': '', 'pdb_m3': '', 'pdb': 'power board', 'brake_res': 'brake R', 'rs03_waist': 'RS03 waist', 'waist_adapter': 'waist adapter', 'jetson': 'Jetson Orin Nano', 'fan_exhaust': 'fan', 'fan_intake': 'fan',
         'compute3': '', 'battery': 'battery option', 'packrail': '', 'pack_sense': 'sense', 'pack': '13S2P pack', 'bno085': '', 'harness': 'harness'}
def lab(p):
    for k, v in LABEL.items():
        if p['id'].startswith(k): return v
    return ''
def rect_of(p, ax):  # returns (u0,u1,v0,v1) in mm for projection ax in {'front':(0,1),'side':(2,1),'plan':(0,2)}
    i, j = {'front': (0, 1), 'side': (2, 1), 'plan': (0, 2)}[ax]
    if p['type'] == 'box': return p['lo'][i], p['hi'][i], p['lo'][j], p['hi'][j], 'rect'
    if p['type'] == 'multibox': return None
    if p['type'] == 'cyl':
        c, r, h = p['c'], p['r'], p['h']; a = 'xyz'.index(p['axis'])
        lo = [c[k] - (h / 2 if k == a else r) for k in range(3)]; hi = [c[k] + (h / 2 if k == a else r) for k in range(3)]
        shape = 'ell' if a not in (i, j) else 'rect'
        return lo[i], hi[i], lo[j], hi[j], shape
    if p['type'] == 'poly':
        if ax == 'front': return p['pts'], None, None, None, 'poly'
        xs = [q[0] for q in p['pts']]; ys = [q[1] for q in p['pts']]
        if ax == 'side': return p['z0'], p['z0'] + p['t'], min(ys), max(ys), 'rect'
        return min(xs), max(xs), p['z0'], p['z0'] + p['t'], 'rect'
def draw(ax, out, title, bays):
    i, j = {'front': (0, 1), 'side': (2, 1), 'plan': (0, 2)}[ax]
    umin, umax, vmin, vmax = {'front': (-150, 150, 80, 520), 'side': (-140, 100, 80, 520), 'plan': (-150, 150, -100, 100)}[ax]
    W, H = int((umax - umin) * S) + 80, int((vmax - vmin) * S) + 120
    im = Image.new('RGBA', (W, H), (255, 255, 255, 255)); d = ImageDraw.Draw(im, 'RGBA')
    flipv = ax != 'plan'
    def X(u): return 40 + (u - umin) * S
    def Y(v): return (40 + (vmax - v) * S) if flipv else (40 + (v - vmin) * S)
    def to(lo, hi): return [X(lo[0]), Y(hi[1]) if flipv else Y(lo[1]), X(hi[0]), Y(lo[1]) if flipv else Y(hi[1])]
    order = {'extrusion': 0, 'gusset': 1, 'bracket': 2, 'actuator': 3, 'waist': 3, 'electronics': 4, 'proposed': 5, 'harness': 7}
    for p in sorted([p for p in P if p['group'] in order], key=lambda p: order[p['group']]):
        r = rect_of(p, ax)
        if r is None: continue
        if p['group'] == 'extrusion':
            if ax == 'side' and p['id'].startswith(('rail_shared', 'rail_top', 'rail_bot')) and r[0] < 0 < r[1]: continue  # hidden behind
            d.rectangle(to((r[0], r[2]), (r[1], r[3])), fill=(150, 154, 160, 255), outline=(40, 40, 44, 255), width=2); continue
        if p['group'] == 'gusset':
            if r[4] == 'poly': d.polygon([(X(q[0]), Y(q[1])) for q in r[0]], fill=(225, 225, 230, 160), outline=(60, 60, 66, 255))
            else: d.rectangle(to((r[0], r[2]), (r[1], r[3])), fill=(225, 225, 230, 200), outline=(60, 60, 66, 255))
            continue
        col = tuple(int(p['color'][k:k + 2], 16) for k in (1, 3, 5)) + (120 if p['group'] in ('proposed', 'option') else 170,)
        box = to((r[0], r[2]), (r[1], r[3]))
        (d.ellipse if r[4] == 'ell' else d.rectangle)(box, fill=col, outline=(30, 30, 34, 255), width=2)
        t = lab(p)
        if t and (box[2] - box[0] > 60):
            tw = d.textlength(t, font=f(22, True)); cx, cy = (box[0] + box[2]) / 2, (box[1] + box[3]) / 2
            d.rectangle((cx - tw / 2 - 4, cy - 14, cx + tw / 2 + 4, cy + 14), fill=(255, 255, 255, 200)); d.text((cx - tw / 2, cy - 13), t, font=f(22, True), fill=(20, 20, 20, 255))
    for (lo, hi, text) in bays:  # free bays: dashed boxes
        b = to(lo, hi); x0, y0, x1, y1 = b
        for k in range(0, int(x1 - x0), 14): d.line((x0 + k, y0, min(x0 + k + 7, x1), y0), fill=(200, 30, 30, 255), width=2); d.line((x0 + k, y1, min(x0 + k + 7, x1), y1), fill=(200, 30, 30, 255), width=2)
        for k in range(0, int(y1 - y0), 14): d.line((x0, y0 + k, x0, min(y0 + k + 7, y1)), fill=(200, 30, 30, 255), width=2); d.line((x1, y0 + k, x1, min(y0 + k + 7, y1)), fill=(200, 30, 30, 255), width=2)
        d.text((x0 + 6, y1 - 28), text, font=f(20, True), fill=(200, 30, 30, 255))
    d.text((40, H - 60), title, font=f(26, True), fill=(20, 20, 20, 255))
    # axis note
    axn = {'front': 'X (robot left +) right, Y up. Looking from the front.', 'side': 'Z (front +) right, Y up. Looking from the robot left.', 'plan': 'X right, Z (front) up. Looking down.'}[ax]
    d.text((40, H - 30), axn, font=f(20), fill=(90, 90, 96, 255))
    im.convert('RGB').save(out); print(out, im.size)
draw('front', 'out/pack2d_front.png', 'Front elevation (section at Z 0)',
     [((-78.4, 373.62), (78.4, 475), 'upper bay 156.8 x 101.4 (x 100 deep)'), ((-55, 175), (55, 353.62), 'lower box 110 x 178.6 (x 100 deep)'), ((-75, 115), (75, 155), 'waist bay 150 x 40')])
draw('side', 'out/pack2d_side.png', 'Side elevation (section at X 0)',
     [((-50, 373.62), (50, 475), 'upper bay 100 deep'), ((-50, 175), (50, 353.62), 'lower box 100 deep'), ((-124, 166), (-77, 481), 'spine pads, outside the shell')])
draw('plan', 'out/pack2d_plan.png', 'Plan (section at Y 430, through the RS03s and the Jetson)',
     [((-78.4, -50), (78.4, 50), 'upper bay 156.8 x 100')])
