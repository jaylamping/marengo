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
sheet([(crop('hero.png', (560, 0, 2640, 2400)), 'Torso shell, arms, placeholder head')], 'Marengo body shell, concept A', sub, 'SHELL_CONCEPT_HERO.png')
sheet([(crop(n + '.png', (900, 0, 2300, 2400), w=900), l) for n, l in [('front', 'Front'), ('side', 'Side (robot left)'), ('back', 'Back')]],
      'Body shell, concept A: front, side, back', sub, 'SHELL_CONCEPT_VIEWS.png')
sheet([(crop('fullA.png', (1100, 0, 2100, 2400), w=760), 'Frame as built: shoulder 1165 mm'),
       (crop('fullB.png', (1100, 0, 2100, 2400), w=760), '+70 mm waist: shoulder 1235 mm'),
       (crop('fullBiso.png', (900, 0, 2300, 2400), w=1060), '+70 mm waist, 3/4')],
      'Longer stomach: +70 mm waist section', sub, 'SHELL_CONCEPT_PROPORTIONS.png',
      ['Legs are grey placeholders at the draft lengths in kinematics.md (5 ft total). The extension goes between the waist yaw and the frame,',
       'which closes the ~70 mm shoulder-height shortfall and gives the longer belly. Arms at 215 + 195 + 90 mm read short against a 5 ft body.'])
sheet([(crop('xray.png', (800, 0, 2600, 2400), w=1100), 'X-ray: frame, pitch motors, proposed mount points (orange)'),
       (crop('explode.png', (500, 250, 2700, 2300), w=1300), 'Print panels, exploded')],
      'Body shell, concept A: mounting and P2S panel split', sub, 'SHELL_CONCEPT_PANELS.png',
      ['Largest panel (chest upper front/back) is 216 x 224 x 110 mm, inside the P2S 256 x 256 x 256 mm volume. 13 panels in all:',
       'chest upper front, upper back, lower front, lower back (service hatch), chest shield, 3 abdomen bands split front/back, 2 deltoid caps on the arms.',
       'Panels hang on printed brackets with M5 T-nuts in the outer frame slots; seams are lapped; the back hatch is magnet-held for Pi access.'])
