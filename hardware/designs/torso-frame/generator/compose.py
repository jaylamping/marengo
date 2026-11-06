# Composes the labelled frame sheets from out/*.png + out/*.json (see shoot.js). Writes ../*.png
import json, math, textwrap
from PIL import Image, ImageDraw, ImageFont
F = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'; FB = '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'
f = lambda s, b=False: ImageFont.truetype(FB if b else F, s)
BG = (201, 206, 214); INK = (30, 33, 38); DPR = 2
BOM = json.load(open('bom.json'))
NAMES = {b['item']: b for b in BOM}

def load(name):
    return Image.open(f'out/{name}.png').convert('RGB'), json.load(open(f'out/{name}.json'))

def balloons(im, meta, r=26, dist=150, items=None):
    d = ImageDraw.Draw(im); W, H = im.size; cx, cy = W / 2, H / 2
    labs = [l for l in meta['labels'] if items is None or l['item'] in items]
    pos = []
    for l in labs:
        x, y = l['xy'][0] * DPR, l['xy'][1] * DPR
        bx0, by0 = l['bbox'][0]; bx1, by1 = l['bbox'][1]
        half = max(abs(bx1 - bx0), abs(by1 - by0)) * DPR / 2
        ang = math.atan2(y - cy, x - cx)
        px, py = x + math.cos(ang) * (half * 0.6 + dist), y + math.sin(ang) * (half * 0.6 + dist)
        pos.append([px, py, x, y, l['item']])
    for _ in range(300):  # push apart overlapping balloons
        moved = False
        for i in range(len(pos)):
            for j in range(i + 1, len(pos)):
                dx, dy = pos[j][0] - pos[i][0], pos[j][1] - pos[i][1]; dd = math.hypot(dx, dy)
                if dd < 2.3 * r:
                    if dd < 1e-3: dx, dy, dd = 1, 0, 1
                    s = (2.3 * r - dd) / 2 / dd; pos[i][0] -= dx * s; pos[i][1] -= dy * s; pos[j][0] += dx * s; pos[j][1] += dy * s; moved = True
        if not moved: break
    for px, py, x, y, it in pos:
        px = min(max(px, r + 4), W - r - 4); py = min(max(py, r + 4), H - r - 4)
        d.line((x, y, px, py), fill=INK, width=3); d.ellipse((x - 5, y - 5, x + 5, y + 5), fill=INK)
        d.ellipse((px - r, py - r, px + r, py + r), fill=(255, 255, 255), outline=INK, width=3)
        t = str(it); tw = d.textlength(t, font=f(30, True)); d.text((px - tw / 2, py - 19), t, font=f(30, True), fill=INK)
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

def table(rows, colw, size=26, head=None, rowh=None):
    rowh = rowh or size + 14; h = rowh * (len(rows) + (1 if head else 0)) + 10; w = sum(colw) + 10
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

def sheet(panels, title, sub, out, notes=None, side=None):
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

def crop(im, box=None, w=None):
    if box: im = im.crop(box)
    if w: im = im.resize((w, int(im.height * w / im.width)), Image.LANCZOS)
    return im

sub = 'Internal torso frame rev A, reconciled from the arm, shell and manufacturing threads. Torso coords: X lateral, Y up from the pelvis bottom, Z forward. 2026-11-06'
bomrows = [[b['item'], b['name'], b['qty'], b['material'], ('%.0f' % b['mass_g']) if b['mass_g'] else '-'] for b in BOM]
bomtab = table(bomrows, [70, 760, 60, 330, 90], size=24, head=['#', 'Part', 'Qty', 'Material', 'g'])

# 1. hero exploded
im, m = load('hero_exp'); balloons(im, m)
sheet([(im, 'Full exploded view: every frame member, gusset, bracket, actuator, shelf, electronics envelope and shell hardpoint, numbered to the table. Cyan = proposed packaging envelopes, yellow blocks = in-torso battery option')],
      'Marengo internal torso frame: exploded view', sub, 'FRAME_EXPLODED.png', side=bomtab,
      notes=['Colours: grey 2020 = existing rails reused as is; gold = existing posts cut to length; blue = new 2020 stock; silver = 3 mm 6061-T6 gussets; dark grey = 2028 corner brackets and RS03s;',
             'teal = printed PETG (pitch brackets, waist adapter); purple = compute shelf; green = Pi 5 + CAN HAT + IMU; magenta = PDB envelope (placeholder); orange = shell hardpoint brackets;',
             'yellow = harness trunks; charcoal = TPU spine pads (outside the shell); white discs = shell collars on the RS03s. Masses are estimates; the extrusion uses 0.5 g/mm.'])
# 2. assembled
a1, m1 = load('asm_iso'); a2, m2 = load('asm_rear')
sheet([(crop(a1, w=1700), 'Assembled, from the front left'), (crop(a2, w=1700), 'Assembled, from the back right (spine pads and PDB envelope behind the doors)')],
      'Internal torso frame: assembled', sub, 'FRAME_ASSEMBLED.png',
      notes=['Two-box 2020 frame: lower box 150 wide x 140 deep (Y 155 to 353.6), shared step rails (Y 353.6 to 373.6), upper box 226 wide x 140 deep (Y 373.6 to 495). Total height 340, unchanged from the built frame.',
             'Shoulder pitch RS03s sit through the upper box sides, output faces at X +-135, axis at Y 424.86. Four 3 mm aluminium gussets bridge the step on the front and rear faces.'])
# 3. dimensions
fr, mf = load('front'); sd, ms = load('side'); tp, mt = load('top')
dimensions(fr, mf); dimensions(sd, ms); dimensions(tp, mt)
sheet([(crop(fr, w=1500), 'Front (looking -Z)'), (crop(sd, w=1100), 'Left side (looking -X)'), (crop(tp, w=1100), 'Top (front of the robot at the top of the image)')],
      'Internal torso frame: dimensions', sub, 'FRAME_DIMENSIONS.png',
      notes=['All in mm. Inner clear: lower box 110 x 100, upper box 186 x 100 (2020 walls). Shoulder joint line (arm thread) at X +-208.49; gusset plates span Y 285 to 425.',
             'Hardpoints and spine pads hidden for clarity. Full dimension table in README.md.'])
# 4. subsystems
s1, n1 = load('sub_frame'); s2, n2 = load('sub_inner'); s3, n3 = load('sub_shell')
balloons(s1, n1, dist=110); balloons(s2, n2, dist=120); balloons(s3, n3, dist=120)
sheet([(crop(s1, w=1300), 'A. Structure: 22 extrusions, 4 gussets, 32 corner brackets'), (crop(s2, w=1300), 'B. Inside: actuators, pitch brackets, shelf, Pi, PDB envelope, harness, waist'),
       (crop(s3, w=1300), 'C. Shell interfaces: 20 hardpoint brackets, collars, TPU spine pads (from the back)')],
      'Internal torso frame: exploded by subsystem', sub, 'FRAME_SUBSYSTEMS.png',
      notes=['Numbers match the table on FRAME_EXPLODED.png. In B and C the frame is ghosted for reference.'])
# 5. changes: built vs new, same scale
b1, _ = load('built'); b2, _ = load('new_frame')
sheet([(crop(b1, w=1500), 'As built: single 150 x 140 x 340 box, 12 rails + 2 shelf side rails, RS03 faces at X +-97'),
       (crop(b2, w=1500), 'Rev A: two-box frame, same height, upper box 226 wide, RS03 faces at X +-135. Blue = new stock, gold = recut posts')],
      'Internal torso frame: what changes from the built frame', sub, 'FRAME_CHANGES.png',
      notes=['Same scale. Cut the four 340 posts into 198.6 + 121.4; add 2 x 226, 2 x 186 and 2 x 100 rails (1024 mm new stock); move the pitch brackets and the top side rails out 38 mm;',
             'add four 3 mm 6061-T6 gussets and 16 more corner brackets. Compute shelf, Pi and waist adapter stay where they are. Everything else is reused.'])
# 6. packaging
p1, q1 = load('pack_top'); p2, q2 = load('pack_front'); p3, q3 = load('pack_side')
sheet([(crop(p1, w=1300), 'Plan view at the shoulder plane: RS03s, Pi stack between them, harness to each shoulder'),
       (crop(p2, w=1300), 'Front: free volumes above the Pi stack and in the lower box'),
       (crop(p3, w=1300), 'Side: Pi ports at the rear face, PDB behind the D2 door, spine pads outside the shell')],
      'Internal torso frame: internal packaging check', sub, 'FRAME_PACKAGING.png',
      notes=['Upper bay (between the RS03 rears): 156.8 wide x 100 deep x 101.4 tall. Pi 5 + CAN HAT stack 85 x 56 x 21 on the shelf; Jetson Orin Nano dev kit 103 x 90.5 x 35 on a second shelf above it (Y 410-445);',
             '30 mm left under the top rails for two 40 mm exhaust fans. Lower box: 110 x 100 x 198.6. PDB envelope 100 x 90 x 28 at the rear (D2 door), third board / USB hub at the front, harness trunks in the front corners,',
             'intake fan low at the front. In-torso batteries: two 6S packs 155 x 50 x 45 fit standing in the lower box centre (yellow), but the baseline keeps the main 48 V pack in the pelvis. See README section 6.'])
