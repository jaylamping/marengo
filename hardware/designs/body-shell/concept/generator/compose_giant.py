# Sheet for the Giant-inspired head variants (renders from shoot.js at DPR=2, see head/concepts/giant/README.md).
from PIL import Image, ImageDraw, ImageFont
F = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'; FB = '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'
f = lambda s, b=False: ImageFont.truetype(FB if b else F, s)
BG = (201, 206, 214); INK = (30, 33, 38); SUB = (70, 75, 82)
V = [('A_rounded', 'A. Rounded', 'White dome and jaw, rounded jaw edges, white crest'),
     ('B_angular', 'B. Angular', 'White dome, gunmetal jaw with chamfered flat planes, taller gunmetal crest'),
     ('C_gunmetal', 'C. Gunmetal', 'Gunmetal head, white crest, bezels and hinge caps, slightly bigger eyes')]
T = 780; PAD = 40; HEAD = 230; LAB = 70
S = Image.new('RGB', (3 * T + 4 * PAD, HEAD + len(V) * (T + LAB + PAD)), BG); d = ImageDraw.Draw(S)
d.text((PAD, 30), 'Marengo head: Giant-inspired, first CAD pass', font=f(64, True), fill=INK)
d.text((PAD, 112), 'Clean solids (build123d), rendered on the rev B body. Original restyle, not a replica. Each eye: round display + camera; stereo cameras in the nose bridge;', font=f(28), fill=SUB)
d.text((PAD, 152), 'speaker and mic slots in the jaw; hinge bosses at the sides. Shape and proportion only: panel lines and fasteners come in SolidWorks. 2026-11-06', font=f(28), fill=SUB)
for r, (v, name, desc) in enumerate(V):
    y = HEAD + r * (T + LAB + PAD)
    ims = [Image.open(f'out/g{v}.png').crop((700, 120, 2500, 1920)), Image.open(f'out/g{v}f.png').crop((800, 120, 2400, 1720)), Image.open(f'out/g{v}s.png').crop((800, 120, 2400, 1720))]
    for k, im in enumerate(ims):
        S.paste(im.resize((T, T), Image.LANCZOS), (PAD + k * (T + PAD), y))
    d.text((PAD, y + T + 10), name, font=f(38, True), fill=INK); d.text((PAD + 300, y + T + 16), desc + '   (3/4, front, side)', font=f(30), fill=SUB)
S.save('../../../head/concepts/giant/HEAD_GIANT_VARIANTS.png', optimize=True); print(S.size)
