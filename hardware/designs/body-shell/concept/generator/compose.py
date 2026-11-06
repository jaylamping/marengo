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
sheet([(crop('hero.png', (560, 0, 2640, 2400)), 'Baseline: heroic shoulders on a wider frame, rounded shoulder and chest, slim dark waist; arm mechanics exposed')], 'Marengo body shell, baseline', sub, 'SHELL_CONCEPT_HERO.png')
sheet([(crop(n + '.png', (900, 0, 2300, 2400), w=900), l) for n, l in [('front', 'Front'), ('side', 'Side (robot left)'), ('back', 'Back')]],
      'Body shell baseline: front, side, back', sub, 'SHELL_CONCEPT_VIEWS.png')
sheet([(crop('fullA.png', (1100, 0, 2100, 2400), w=760), 'Frame as built: shoulder 1165 mm'),
       (crop('fullB.png', (1100, 0, 2100, 2400), w=760), '+70 mm waist: shoulder 1235 mm'),
       (crop('fullBiso.png', (900, 0, 2300, 2400), w=1060), '+70 mm waist, 3/4')],
      'Longer stomach: +70 mm waist section', sub, 'SHELL_CONCEPT_PROPORTIONS.png',
      ['Legs are grey placeholders at the draft lengths in kinematics.md (5 ft total). The extension goes between the waist yaw and the frame and brings',
       'shoulder height within 1% of a 5 ft man. Arms at 215 + 195 + 90 mm are 24% / 12% short (see PROPORTIONS.md); the arm thread has a length card open.'])
sheet([(crop('shoulder.png', (0, 0, 3200, 2400), w=1300), 'Shoulder cavity (r 78): shell steps in to the RS03 face around the joint'),
       (crop('explode.png', (600, 350, 2600, 2250), w=1000), 'Print panels, exploded')],
      'Body shell baseline: shoulder cavity and P2S panel split', sub, 'SHELL_CONCEPT_PANELS.png',
      ['Frame widened 38 mm per side (wide upper box), shoulder joints at Y +-208.5. Shell 324 mm across the shoulders, 178 mm at the chest bottom,',
       '156 mm dark waist. Around each pitch joint the shell steps in to the RS03 face (Y +-135) inside a 78 mm cavity: the arm thread\'s sweep',
       'needs r 76 for full pitch (-50..180) with >= 3 mm. The 2 mm collar sits in the cavity on the RS03\'s unused outboard M4 holes.',
       'Upper panels ~310 mm and lower ~279 mm wide split at the centre for the P2S. Exploded frame is the single-box stand-in; the two-box frame is in progress.'])
sheet([(crop('w0.png', (700, 300, 2500, 2300), w=900), 'Shoulders as built: roll axes 341 mm apart'),
       (crop('w25.png', (700, 300, 2500, 2300), w=900), 'P1 +25 mm per side: 391 mm apart'),
       (crop('w25iso.png', (700, 200, 2500, 2400), w=900), '+25 mm, three-quarter')],
      'Body shell, concept B: chest plate and shoulder width', sub, 'SHELL_CONCEPT_WIDTH.png',
      ['Head removed until it is designed. Chest has pec masses, a sternum groove and a crisp lower edge; the black yoke panel is gone.',
       'The +25 mm option lengthens P1 (arm thread); the shell shoulder line widens with it. Shell upper panels then need a centre split for the P2S.'])
sheet([(crop('r38iso.png', (700, 200, 2500, 2400), w=760), 'Previous: flatter chest front'),
       (crop('c38iso.png', (700, 200, 2500, 2400), w=760), 'Latest: chest rolls back to the shoulders'),
       (crop('c38.png', (600, 200, 2600, 2300), w=760), 'Latest, front'),
       (crop('c38top.png', (600, 200, 2600, 2200), w=760), 'Latest, from above')],
      'Body shell: rounded shoulder top and chest front', sub, 'SHELL_CONCEPT_BLEND.png',
      ['Shoulder point is a quarter-round cap (about 40 mm radius), the top slopes up to the neck like a trapezius, and the chest front rolls back',
       'toward each shoulder (up to 30% of its depth at the outer edge), like a pec wrapping into the shoulder. Side line unchanged.',
       'Width from a wider 2020 frame: +38 mm per side (rails 150 -> 226 mm), pitch RS03 faces at Y +-135.5, P1 unchanged. Waist +10 mm per side.'])
sheet([(crop('c38.png', (600, 200, 2600, 2300), w=760), 'r 62: P2 hits at every pitch'),
       (crop('front.png', (600, 200, 2600, 2300), w=760), 'r 78 (baseline): full pitch, >= 3 mm'),
       (crop('k93.png', (600, 200, 2600, 2300), w=760), 'r 93: full pitch, >= 5 mm'),
       (crop('hero.png', (700, 200, 2500, 2400), w=760), 'r 78, three-quarter')],
      'Body shell: shoulder cavity size vs arm clearance', sub, 'SHELL_CONCEPT_CLEARANCE.png',
      ['Arm-thread sweep, pitch -50..180 at 0 and 5 deg inward roll, 2 mm collar. r 76 is the smallest cavity with >= 3 mm everywhere; r 78 gives 3.2 / 3.8 mm.',
       'r 70 and below fail in bands a pitch stop cannot avoid. Collar button heads stand 2.2 mm proud; P2 gets a local relief in its next rev.'])
sheet([(crop('x1.png', (600, 200, 2600, 2300), w=860), 'Front, x-ray'),
       (crop('x2.png', (600, 200, 2600, 2300), w=860), 'Three-quarter, x-ray'),
       (crop('x3.png', (700, 200, 2500, 2300), w=760), 'Side, x-ray')],
      'Body shell over the two-box frame', sub, 'SHELL_CONCEPT_FRAME.png',
      ['Frame from the arm thread (hardware/designs/torso-stepped/, de5a006): upper box half-width 113 from 71 mm below the shoulder axis to the top,',
       'lower box 75; 3 mm aluminium gusset plates (outline simplified here) bridge the step down to 140 mm below the axis, outer faces at X +-73.',
       'Shell clearance to the frame: about 40 mm at the sides of the step, 35 mm in front and 15 mm behind the plates. Orange dots: proposed panel mounts.'])
