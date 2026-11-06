# Composes the head concept line-up sheets from out/i<N>.png (3/4) and out/f<N>.png (front), rendered with
# shoot.js at DPR=2: i<N> = head=N&ty=190&d=1500&az=32&el=8, f<N> = head=N&view=front&ty=170&span=600.
import sys
from PIL import Image, ImageDraw, ImageFont
F = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'; FB = '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'
f = lambda s, b=False: ImageFont.truetype(FB if b else F, s)
BG = (201, 206, 214); INK = (30, 33, 38); SUB = (70, 75, 82)
OUT = sys.argv[1] if len(sys.argv) > 1 else '../../../head/concepts/'
H = {
 1: ('Sentinel', 'Wraparound visor, eyes glow through the tint, ear mics'),
 2: ('Crested arrow', 'Faceted helmet, arrow-notched visor, centre crest'),
 3: ('Cyclops lens', 'One big depth-camera eye with a light ring'),
 4: ('Twin optics', 'Two real camera eyes in barrels under a brow'),
 5: ('Scanner bar', 'Dark face, one light bar that sweeps to look'),
 6: ('Brow guard', 'Heavy brow, deep-set amber eyes, chin plate'),
 7: ('Chevron', 'V visor split by a centre seam, slanted eyes'),
 8: ('Listener', 'Big ear mic arrays with status rings, oval visor'),
 9: ('Fins', 'Swept fins that move to show mood'),
 10: ('Screen face', 'Display panel draws the eyes and mouth'),
 11: ('Grille knight', 'Faceted helm, vertical grille, eye slot, crest'),
 12: ('Hunter', 'Low swept wedge, slanted red eye slits'),
 13: ('Sensor stack', 'Exposed depth camera bar and lidar puck'),
 14: ('Owl', 'Wide facial disc, two big lens eyes'),
 15: ('Companion', 'Small soft head, tall oval eyes'),
 16: ('Crown', 'Ring of cameras round the brow, visor below'),
 17: ('Talking jaw', 'Separate jaw plate with a speech light'),
 18: ('Prism', 'Hard low-poly facets, light seam on the brow'),
 19: ('Hood', 'Cowl over a recessed dark face, eyes inside'),
 20: ('Stereo brow', 'Visible stereo camera above the visor, chin mic'),
}
TW, PAD = 720, 36
def tile(i):
    a = Image.open(f'out/i{i}.png').convert('RGB').crop((700, 260, 2500, 2060)).resize((TW, TW), Image.LANCZOS)
    b = Image.open(f'out/f{i}.png').convert('RGB').crop((1050, 330, 2150, 1430)).resize((250, 250), Image.LANCZOS)
    d = ImageDraw.Draw(a); x0, y0 = TW - 250 - 12, TW - 250 - 12
    d.rectangle((x0 - 3, y0 - 3, x0 + 252, y0 + 252), fill=(150, 156, 165)); a.paste(b, (x0, y0))
    d.text((x0 + 6, y0 + 4), 'front', font=f(20), fill=SUB)
    t = Image.new('RGB', (TW, TW + 100), BG); t.paste(a, (0, 0)); d = ImageDraw.Draw(t)
    name, desc = H[i]; d.text((4, TW + 12), f'{i}. {name}', font=f(34, True), fill=INK); d.text((4, TW + 56), desc, font=f(24), fill=SUB)
    return t
def sheet(ids, out, title):
    cols = 5; rows = (len(ids) + cols - 1) // cols; head = 170
    S = Image.new('RGB', (cols * TW + (cols + 1) * PAD, head + rows * (TW + 100 + PAD) + 20), BG); d = ImageDraw.Draw(S)
    d.text((PAD, 30), title, font=f(60, True), fill=INK)
    d.text((PAD, 108), 'Original concepts on the body baseline (rev B shell). Parametric mock-ups to pick a direction, not parts. Same head size on all. 2026-11-06', font=f(30), fill=SUB)
    for k, i in enumerate(ids):
        S.paste(tile(i), (PAD + (k % cols) * (TW + PAD), head + (k // cols) * (TW + 100 + PAD)))
    S.save(OUT + out, optimize=True); print(out, S.size)
sheet(range(1, 11), 'HEAD_LINEUP_01-10.png', 'Marengo head line-up, 1 to 10')
sheet(range(11, 21), 'HEAD_LINEUP_11-20.png', 'Marengo head line-up, 11 to 20')
