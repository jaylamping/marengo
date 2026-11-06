# Composes the labelled frame sheets from out/*.png + out/*.json (see shoot.js). Writes ../*.png
import json, math, textwrap
from PIL import Image, ImageDraw, ImageFont
F = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'; FB = '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'
f = lambda s, b=False: ImageFont.truetype(FB if b else F, s)
BG = (201, 206, 214); INK = (30, 33, 38); DPR = 2
BOM = json.load(open('bom.json')); NAMES = {b['item']: b for b in BOM}
SUB = 'Internal torso frame rev A, reconciled from the arm, shell and manufacturing threads. Torso coords: X lateral, Y up from the pelvis bottom, Z forward. 2026-11-06'

def load(name): return Image.open(f'out/{name}.png').convert('RGB'), json.load(open(f'out/{name}.json'))
def crop(im, box=None, w=None):
    if box: im = im.crop(box)
    if w: im = im.resize((w, int(im.height * w / im.width)), Image.LANCZOS)
    return im

def balloons(im, meta, r=26, dist=140, items=None, use_tag=False, size=30):
    d = ImageDraw.Draw(im); W, H = im.size; cx, cy = W / 2, H / 2
    labs = [l for l in meta['labels'] if (items is None or l['item'] in items)]
    seen = set(); pos = []
    for l in labs:
        key = l['tag'] if use_tag else l['item']
        if not use_tag and key in seen: continue
        seen.add(key)
        x, y = l['xy'][0] * DPR, l['xy'][1] * DPR
        (bx0, by0), (bx1, by1) = l['bbox']; half = max(abs(bx1 - bx0), abs(by1 - by0)) * DPR / 2
        ang = math.atan2(y - cy, x - cx)
        pos.append([x + math.cos(ang) * (half * 0.5 + dist), y + math.sin(ang) * (half * 0.5 + dist), x, y, str(key)])
    for _ in range(400):
        moved = False
        for i in range(len(pos)):
            for j in range(i + 1, len(pos)):
                dx, dy = pos[j][0] - pos[i][0], pos[j][1] - pos[i][1]; dd = math.hypot(dx, dy)
                if dd < 2.4 * r:
                    if dd < 1e-3: dx, dy, dd = 1, 0, 1
                    s = (2.4 * r - dd) / 2 / dd; pos[i][0] -= dx * s; pos[i][1] -= dy * s; pos[j][0] += dx * s; pos[j][1] += dy * s; moved = True
        if not moved: break
    for px, py, x, y, t in pos:
        px = min(max(px, r + 4), W - r - 4); py = min(max(py, r + 4), H - r - 4)
        d.line((x, y, px, py), fill=INK, width=3); d.ellipse((x - 5, y - 5, x + 5, y + 5), fill=INK)
        rr = r + 6 if len(t) > 2 else r
        d.ellipse((px - rr, py - rr, px + rr, py + rr), fill=(255, 255, 255), outline=INK, width=3)
        tw = d.textlength(t, font=f(size, True)); d.text((px - tw / 2, py - size * 0.62), t, font=f(size, True), fill=INK)
    return im

def dimensions(im, meta):
    d = ImageDraw.Draw(im); C = (160, 30, 30)
    for dm in meta['dims']:
        p1 = [v * DPR for v in dm['p1']]; p2 = [v * DPR for v in dm['p2']]; a = [v * DPR for v in dm['a']]; b = [v * DPR for v in dm['b']]
        d.line((*p1, *a), fill=C, width=2); d.line((*p2, *b), fill=C, width=2); d.line((*a, *b), fill=C, width=3)
        for q in (a, b): d.ellipse((q[0] - 6, q[1] - 6, q[0] + 6, q[1] + 6), fill=C)
        mx, my = (a[0] + b[0]) / 2, (a[1] + b[1]) / 2; horiz = abs(b[0] - a[0]) > abs(b[1] - a[1])
        tw = d.textlength(dm['text'], font=f(40, True))
        if horiz: d.rectangle((mx - tw / 2 - 6, my - 52, mx + tw / 2 + 6, my - 6), fill=BG); d.text((mx - tw / 2, my - 50), dm['text'], font=f(40, True), fill=C)
        else:
            tmp = Image.new('RGBA', (int(tw) + 12, 50), (0, 0, 0, 0)); ImageDraw.Draw(tmp).text((6, 2), dm['text'], font=f(40, True), fill=C)
            tmp = tmp.rotate(90, expand=True); im.paste(tmp, (int(mx) - 54, int(my - tmp.height / 2)), tmp)
    return im

def table(rows, colw, size=26, head=None):
    rowh = size + 14; h = rowh * (len(rows) + (1 if head else 0)) + 10; w = sum(colw) + 10
    im = Image.new('RGB', (w, h), BG); d = ImageDraw.Draw(im); y = 5
    def line(cells, bold):
        nonlocal y; x = 5
        for c, cw in zip(cells, colw):
            txt = str(c)
            while d.textlength(txt, font=f(size, bold)) > cw - 10 and len(txt) > 3: txt = txt[:-2]
            d.text((x + 4, y + 4), txt, font=f(size, bold), fill=INK); x += cw
        y += rowh; d.line((5, y, w - 5, y), fill=(150, 155, 162), width=1)
    if head: line(head, True)
    for r in rows: line(r, False)
    return im

def sheet(panels, title, out, notes=None, side=None, sub=SUB):
    pad = 40; head = 160; ph = max(p[0].height for p in panels)
    tw = sum(p[0].width for p in panels) + pad * (len(panels) + 1) + (side.width + pad if side else 0)
    th = head + max(ph, side.height if side else 0) + 130 + (len(notes) * 42 + 30 if notes else 0)
    S = Image.new('RGB', (tw, th), BG); d = ImageDraw.Draw(S)
    d.text((pad, 28), title, font=f(56, True), fill=INK); d.text((pad, 100), sub, font=f(30), fill=(70, 75, 82))
    x = pad
    for im, lab in panels:
        S.paste(im, (x, head)); yy = head + ph + 12
        for ln in textwrap.wrap(lab, width=max(20, int(im.width / 17))): d.text((x + 10, yy), ln, font=f(30, True), fill=INK); yy += 38
        x += im.width + pad
    if side: S.paste(side, (x, head))
    if notes:
        y = head + max(ph, side.height if side else 0) + 110
        for n in notes: d.text((pad, y), n, font=f(28), fill=(40, 44, 50)); y += 42
    S.save('../' + out, optimize=True); print(out, S.size)

def bomtable(items):
    rows = [[b['item'], b['name'], b['qty'], b['material'], ('%.0f' % b['mass_g']) if b['mass_g'] else '-'] for b in BOM if b['item'] in items]
    return table(rows, [70, 760, 60, 330, 90], size=24, head=['#', 'Part', 'Qty', 'Material', 'g'])

MAIN_ITEMS = set(range(1, 34)) - {19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 32}
# 1. exploded
im, m = load('hero_exp'); balloons(im, m, items=MAIN_ITEMS)
sheet([(crop(im, (300, 100, 3700, 2900)), 'Exploded along the frame axis: upper box up, lower box down, gussets forward and back, shoulder actuators outward, electronics stacked above the shelf')],
      'Marengo internal torso frame rev A: exploded view', 'FRAME_EXPLODED.png', side=bomtable(MAIN_ITEMS),
      notes=['Grey 2020 = existing rails reused; gold = existing posts cut to length; blue = new 2020 stock; silver = 3 mm 6061-T6 gussets; dark grey = corner brackets and RS03s; teal = printed PETG;',
             'purple = compute shelf; green = Pi 5 + CAN HAT; magenta = PDB envelope (placeholder); cyan = proposed envelopes for the Jetson, fans and a third board. Fasteners and shell parts are on the detail sheets.',
             'Masses are estimates; the extrusion uses 0.5 g/mm. Corner brackets travel with the rail they bolt to.'])
# 2. fastener detail
im, m = load('hero_fast'); balloons(im, m, items={11, 34, 35, 36, 37, 38, 39, 19, 20, 21, 22, 23, 24}, dist=120)
c1, _ = load('corner_asm'); c2, _ = load('corner_exp')
ft = table([[35, 'M5 x 10 button head + T-nut, corner brackets', 64], [34, 'Corner bracket 2020, 20 mm legs (top inner-X corners)', 4], [11, 'Corner bracket 2028', 28],
            [36, 'M5 x 10 button head + T-nut, gussets', 36], [37, 'M5 x 16 button head + T-nut, shell hardpoint brackets', 20], [39, 'M5 x 12 button head + T-nut, pitch brackets to posts (assumption)', 8],
            ['', 'RS03 rear mounting screws (vendor pattern, as built)', '2 sets'], [38, 'M4 x 10 button head, shell collars on the RS03 outboard holes', 16],
            ['', 'M2.5 standoffs, Pi 5 and Jetson carrier', '8 + 4'], ['', 'M3 heat-set inserts + M3 x 8, shell panels to hardpoints (shell thread)', '~40']],
           [70, 900, 110], size=24, head=['#', 'Fastener', 'Qty'])
sheet([(crop(im, (300, 100, 3700, 2900), w=1900), 'Full exploded view with every screw, T-nut and shell hardpoint bracket (orange)'),
       (crop(c1, w=1000), 'Step corner, assembled: lower post, shared rail, upper post, gusset, brackets'), (crop(c2, w=1000), 'Step corner, exploded: screws and T-nuts pulled off their slots')],
      'Internal torso frame rev A: fasteners and joint detail', 'FRAME_EXPLODED_DETAIL.png', side=ft,
      notes=['128 M5 screws + 128 M5 T-nuts in total (brackets 64, gussets 36, hardpoints 20, pitch brackets 8). T-nuts shown as grey blocks in the slot behind each screw. Screws are M5 button heads unless noted.',
             'The four top inner-X brackets are 2020 (20 mm legs) because a 2028 there overlaps the pitch bracket wing at Y 447-450 (arm thread interference check).'])
# 3. assembled
a1, _ = load('asm_iso'); a2, _ = load('asm_rear')
sheet([(crop(a1, (200, 0, 3000, 2400), w=1600), 'Assembled, from the front left, with hardpoint brackets and fasteners'), (crop(a2, (200, 0, 3000, 2400), w=1600), 'Assembled, from the back right')],
      'Internal torso frame rev A: assembled', 'FRAME_ASSEMBLED.png',
      notes=['Lower box 150 x 140 (Y 155 to 353.6), shared step rails (Y 353.6 to 373.6), upper box 226 x 140 (Y 373.6 to 495). Height 340, unchanged. Pitch RS03 faces at X +-135, axis Y 424.86.'])
# 4. dimensions
fr, mf = load('front'); sd, ms = load('side'); tp, mt = load('top'); dimensions(fr, mf); dimensions(sd, ms); dimensions(tp, mt)
sheet([(crop(fr, w=1500), 'Front (looking -Z)'), (crop(sd, w=1100), 'Left side (looking -X)'), (crop(tp, w=1100), 'Top (front of the robot at the top of the image)')],
      'Internal torso frame rev A: dimensions', 'FRAME_DIMENSIONS.png',
      notes=['All in mm. Inner clear: lower box 110 x 100, upper box 186 x 100 (2020 walls). Shoulder joint line (arm thread) at X +-208.49; gusset plates span Y 285 to 425. Full table in README.md.'])
# 5. inside, exploded
im, m = load('inside_exp'); balloons(im, m, items={12, 13, 14, 15, 16, 17, 18, 27, 29, 30, 32, 33, 40}, dist=120)
sheet([(crop(im, (300, 0, 2900, 2400), w=1800), 'Everything inside the frame: shoulder RS03s and pitch brackets outward, Pi and Jetson stack up, the 13S2P pack slides out the back, power and bus board forward, waist parts below')],
      'Internal torso frame rev A: inside the frame', 'FRAME_INSIDE.png', side=bomtable({12, 13, 14, 15, 16, 17, 18, 27, 29, 30, 32, 33, 40}),
      notes=['Frame ghosted. Harness (yellow): exits inboard from each RS03 top-rear connector flat (Joey: one port per motor, low-front capped), step bus to the power and bus board, two drops in the front corners to the waist.'])
# 6. packaging 2D
p1 = Image.open('out/pack2d_front.png').convert('RGB'); p2 = Image.open('out/pack2d_side.png').convert('RGB'); p3 = Image.open('out/pack2d_plan.png').convert('RGB')
fits = table([['Pi 5 + CAN HAT', '85 x 56 x 21', 'upper bay, shelf', 'yes, as built'], ['Jetson Orin Nano dev kit', '103 x 90.5 x 45 incl. fan', 'upper bay, 2nd shelf Y 410', 'yes, 26.9 mm/side, 20 above'],
              ['13S2P 21700 pack', '95 x 162 x 79, 2.2 kg', 'lower box, Z -45 to 34, out through D2', 'yes, 7.5 mm/side, 8 top/bottom'],
              ['Power and bus board (BMS on it)', '100 x 150 x 24 assumed', 'in front of the pack, Z 34-58, inside the frame', 'yes, flush with the front face'],
              ['Pitch RS03 x 2', 'D99 x 57', 'through the upper sides', 'yes (0.5 mm to depth)'], ['Waist RS03', 'D99 x 57 + 6 adapter', '40 mm waist bay', 'NO: 23 mm short (hip stage)'],
              ['Third board / USB hub', '90 x 60 x 20', 'no home yet', 'NO: pelvis or over the Pi'], ['Fixed pelvis cell (Joey)', 'placeholder', 'pelvis', 'hip stage'],
              ['Fans', '3 x 40 x 40 x 10', '2 exhaust in the D1 door plane, 1 intake on the power board', 'provisional'],
              ['Sensors', 'IMU; head cams later', 'shelf; top rails', 'yes'], ['Harness', '2 trunks + CAN', 'front corners beside the packs, step bus', 'yes, off H1-H6 slots']],
             [420, 420, 440, 420], size=24, head=['Component', 'Envelope, mm', 'Where', 'Fits?'])
sheet([(crop(p1, w=1150), 'Front section at Z 0'), (crop(p2, w=935), 'Side section at X 0'), (crop(p3, w=1150), 'Plan section through the shoulder plane')],
      'Internal torso frame rev A: internal packaging check', 'FRAME_PACKAGING.png', side=fits,
      notes=['Red dashed boxes are the bays. Upper bay 156.8 x 100 x 101.4 between the RS03 rear bosses; lower box 110 x 100 x 178.6 above the bottom rails; waist bay 150 x 140 x 40 under the frame.',
             'The 13S2P pack (yellow) fills the rear 79 mm of the lower box on printed tray rails and slides out the back through D2; the power and bus board stands in front of it inside the frame, with the intake fan in the front panel.'])
# 7. hardpoints
h1, m1 = load('hp_front'); h2, m2 = load('hp_rear'); balloons(h1, m1, items={19, 20, 21, 22, 23, 24}, use_tag=True, dist=90, size=24); balloons(h2, m2, items={19, 20, 21, 22, 23, 24}, use_tag=True, dist=90, size=24)
sheet([(crop(h1, w=1500), 'Shell hardpoint brackets H1-H6 on the frame, from the front left (frame ghosted)'), (crop(h2, w=1500), 'From the back right, with the four TPU spine pads, the H3 spine rail lip and the collars')],
      'Internal torso frame rev A: shell interfaces', 'FRAME_HARDPOINTS.png',
      notes=['20 printed PETG brackets on M5 T-nuts carry the M3 inserts; panels only have holes and pins. H6 is on the outer faces of the bottom side rails (moved off the under-face, agreed with the shell thread).',
             'H3 rear brackets stay below Y 374 for the Pi ports and carry the spine rail lip (|X| <= 60, Y 355-372) that the D2 battery door (|X| <= 60, Y 175-352, bottom hinge on H4) closes onto with magnets. D1 is Y 374-492.',
             'Gussets are not mounts. Spine pads (4): |X| 68-147 x Y 368-481 and |X| 64-116 x Y 166-295, Z -77 to -124, on standoffs to H2/H3/H4 (shell thread, main 761c62b).'])
# 8. changes
b1, _ = load('built'); b2, _ = load('new_frame')
sheet([(crop(b1, w=1500), 'As built: single 150 x 140 x 340 box, 12 rails + 2 shelf side rails, RS03 faces at X +-97'),
       (crop(b2, w=1500), 'Rev A: two-box frame, same height, upper box 226 wide, RS03 faces at X +-135. Blue = new stock, gold = recut posts')],
      'Internal torso frame rev A: what changes from the built frame', 'FRAME_CHANGES.png',
      notes=['Same scale. Cut the four 340 posts into 198.6 + 121.4; add 2 x 226, 2 x 186 and 2 x 100 rails (1024 mm new stock); move the pitch brackets and the top side rails out 38 mm;',
             'add four 3 mm 6061-T6 gussets and 16 more corner brackets. Compute shelf, Pi and waist adapter stay where they are. Everything else is reused.'])
