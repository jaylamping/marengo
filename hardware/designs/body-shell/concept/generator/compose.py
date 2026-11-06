# Composes the labelled concept sheets from the raw renders in out/ (see shoot.js).
from PIL import Image, ImageDraw, ImageFont
F = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'; FB = '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'
f = lambda s, b=False: ImageFont.truetype(FB if b else F, s)
BG = (201, 206, 214)
def crop(p, box=None, w=None):
    im = Image.open('out/' + p).convert('RGB')
    if box: im = im.crop(box)
    if w: im = im.resize((w, int(im.height * w / im.width)), Image.LANCZOS)
    return im
def sheet(panels, title, sub, out, notes=None):
    pad = 40; head = 150; ph = max(p[0].height for p in panels)
    tw = sum(p[0].width for p in panels) + pad * (len(panels) + 1); th = ph + head + 80 + (len(notes) * 44 + 40 if notes else 0)
    S = Image.new('RGB', (tw, th), BG); d = ImageDraw.Draw(S)
    d.text((pad, 30), title, font=f(54, True), fill=(30, 33, 38)); d.text((pad, 98), sub, font=f(30), fill=(70, 75, 82))
    x = pad
    for im, lab in panels:
        S.paste(im, (x, head)); d.text((x + 10, head + ph + 14), lab, font=f(32, True), fill=(30, 33, 38)); x += im.width + pad
    if notes:
        y = head + ph + 80
        for n in notes: d.text((pad, y), n, font=f(30), fill=(40, 44, 50)); y += 44
    S.save('final/' + out, optimize=True); print(out, S.size)
sub = 'Concept only, not a part. Original design; shell bolts to the 2020 torso frame. 2026-11-06'
sheet([(crop('hero.png', (560, 0, 2640, 2400)), 'Square shoulders, V-taper, slim dark waist; arm mechanics exposed')], 'Marengo body shell, concept B', sub, 'SHELL_CONCEPT_HERO.png')
sheet([(crop(n + '.png', (900, 0, 2300, 2400), w=900), l) for n, l in [('front', 'Front'), ('side', 'Side (robot left)'), ('back', 'Back')]],
      'Body shell, concept B: front, side, back', sub, 'SHELL_CONCEPT_VIEWS.png')
sheet([(crop('fullA.png', (1100, 0, 2100, 2400), w=760), 'Frame as built: shoulder 1165 mm'),
       (crop('fullB.png', (1100, 0, 2100, 2400), w=760), '+70 mm waist: shoulder 1235 mm'),
       (crop('fullBiso.png', (900, 0, 2300, 2400), w=1060), '+70 mm waist, 3/4')],
      'Longer stomach: +70 mm waist section', sub, 'SHELL_CONCEPT_PROPORTIONS.png',
      ['Legs are grey placeholders at the draft lengths in kinematics.md (5 ft total). The extension goes between the waist yaw and the frame,',
       'which closes the ~70 mm shoulder-height shortfall and gives the longer belly. Arms at 215 + 195 + 90 mm read short against a 5 ft body.'])
sheet([(crop('shoulder.png', (0, 0, 3200, 2400), w=1300), 'Shoulder socket: shell steps in to the RS03 face around the joint'),
       (crop('explode.png', (600, 350, 2600, 2250), w=1000), 'Print panels, exploded')],
      'Body shell, concept B: shoulder interface and P2S panel split', sub, 'SHELL_CONCEPT_PANELS.png',
      ['Shoulders 242 mm across, sides pulled to +-90 where the arm hangs, 158 mm at the chest bottom, 136 mm dark waist. Around the pitch joint the shell steps in to the RS03 face (Y +-97) inside a 62 mm',
       'radius socket, clear of the ~57 mm P1/roll sweep. The collar (114 mm OD) sits in the socket under the RS03 outer-ring bolts; those bolts also',
       'hold the motor to its bracket, so the collar waits on the arm thread. Socket edge is jagged only in this mock-up render.',
       '13 panels, largest 242 x 228 x 108 mm (chest upper front), all inside the P2S 256 mm cube. Shoulder parts shown are stand-ins.'])
sheet([(crop('w0.png', (700, 300, 2500, 2300), w=900), 'Shoulders as built: roll axes 341 mm apart'),
       (crop('w25.png', (700, 300, 2500, 2300), w=900), 'P1 +25 mm per side: 391 mm apart'),
       (crop('w25iso.png', (700, 200, 2500, 2400), w=900), '+25 mm, three-quarter')],
      'Body shell, concept B: chest plate and shoulder width', sub, 'SHELL_CONCEPT_WIDTH.png',
      ['Head removed until it is designed. Chest has pec masses, a sternum groove and a crisp lower edge; the black yoke panel is gone.',
       'The +25 mm option lengthens P1 (arm thread); the shell shoulder line widens with it. Shell upper panels then need a centre split for the P2S.'])
sheet([(crop('m25.png', (600, 300, 2600, 2300), w=800), 'Previous: shoulders +25 mm per side'),
       (crop('w38.png', (600, 300, 2600, 2300), w=800), 'Latest: shoulders +38, waist +10 per side'),
       (crop('w38iso.png', (700, 200, 2500, 2400), w=800), 'Latest, three-quarter'),
       (crop('w38x.png', (600, 300, 2600, 2300), w=800), 'Latest, x-ray: wider 2020 frame')],
      'Body shell: broader shoulders (wider frame) and a wider waist', sub, 'SHELL_CONCEPT_BLEND.png',
      ['Same side line as before, but the whole shoulder moves out another 13 mm per side: longer left-right 2020 rails (150 -> 226 mm),',
       'pitch RS03 faces at Y +-135.5 (were +-97.5), P1 unchanged. Widening only the shell there would hit the hanging upper arm, so the arm moves out with it.',
       'Waist +10 mm per side: chest bottom 178 mm, dark bands 156 mm. Upper panels 324 mm and lower front 279 mm wide: both split at the centre for the P2S.'])
