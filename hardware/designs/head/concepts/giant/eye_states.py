"""Mock-up of eye display states (round 240x240 display behind each socket). 2026-11-06."""
import math, sys
from PIL import Image, ImageDraw, ImageFilter, ImageFont
F = '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf'; FB = '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf'
WARM = (255, 226, 160); RED = (255, 70, 50); S = 4  # supersample

def eye(state, r=150):
    n = 2 * r + 80; im = Image.new('RGB', (n * S, n * S), (238, 240, 243)); d = ImageDraw.Draw(im)
    c = n * S / 2; R = r * S; ri = R * 0.72
    d.ellipse((c - R, c - R, c + R, c + R), fill=(22, 24, 27))            # dark socket ring
    gl = Image.new('L', im.size, 0); g = ImageDraw.Draw(gl)
    col = RED if state == 'warning' else WARM
    k = {'idle': 1.0, 'asleep': 0.25}.get(state, 1.0)
    if state == 'focus':
        g.ellipse((c - ri * 0.28, c - ri * 0.28, c + ri * 0.28, c + ri * 0.28), fill=255)
    else:
        g.ellipse((c - ri, c - ri, c + ri, c + ri), fill=int(255 * k))
    if state == 'happy':   # lower lid sweeps up into a smile squint
        g.ellipse((c - ri * 1.25, c + ri * 0.05, c + ri * 1.25, c + ri * 2.4), fill=0)
    if state == 'sceptical':   # upper lid band drops
        g.rectangle((0, 0, im.size[0], c - ri * 0.25), fill=0)
    if state == 'asleep':
        g.rectangle((0, 0, im.size[0], c - ri * 0.12), fill=0); g.rectangle((0, c + ri * 0.12, im.size[0], im.size[1]), fill=0)
    glow = Image.new('RGB', im.size, col)
    im.paste(glow, (0, 0), gl.filter(ImageFilter.GaussianBlur(S * 3)))
    if state == 'look':   # brighter core drifts toward the target
        hl = Image.new('L', im.size, 0); ImageDraw.Draw(hl).ellipse((c + ri * 0.15 - ri * 0.4, c - ri * 0.4, c + ri * 0.15 + ri * 0.4, c + ri * 0.4), fill=200)
        im.paste(Image.new('RGB', im.size, (255, 250, 235)), (0, 0), hl.filter(ImageFilter.GaussianBlur(S * 14)))
    if state == 'thinking':  # dim centre, a bright segment chasing the rim
        dm = Image.new('L', im.size, 0); ImageDraw.Draw(dm).ellipse((c - ri * 0.86, c - ri * 0.86, c + ri * 0.86, c + ri * 0.86), fill=150)
        im.paste(Image.new('RGB', im.size, (60, 52, 38)), (0, 0), dm.filter(ImageFilter.GaussianBlur(S * 4)))
        ImageDraw.Draw(im).arc((c - ri * 0.93, c - ri * 0.93, c + ri * 0.93, c + ri * 0.93), -60, 30, fill=(255, 245, 215), width=int(ri * 0.11))
    return im.resize((n, n), Image.LANCZOS)

STATES = [('idle', 'Idle / listening', 'full warm glow'), ('look', 'Looking', 'bright core leads the gaze'), ('focus', 'Focus', 'aperture closes to a point'),
          ('happy', 'Happy', 'lower lid sweeps up'), ('sceptical', 'Sceptical / tired', 'upper lid drops'), ('thinking', 'Thinking', 'a light chases the rim'),
          ('asleep', 'Asleep / idle', 'dim slit, slow breathing'), ('warning', 'Keep back', 'red, steady')]
if __name__ == '__main__':
    tw, th = 540, 330; cols = 4
    out = Image.new('RGB', (cols * tw + 40, 120 + 2 * (th + 30)), (201, 206, 214)); d = ImageDraw.Draw(out)
    d.text((24, 22), 'Eye display states (mock-up): one round display behind each socket', font=ImageFont.truetype(FB, 34), fill=(30, 33, 38))
    d.text((24, 70), 'Combined with head tilt, nods and the moving jaw. Animated in the real thing; one frame each here.', font=ImageFont.truetype(F, 22), fill=(70, 75, 82))
    for i, (st, name, sub) in enumerate(STATES):
        x = 24 + (i % cols) * tw; y = 120 + (i // cols) * (th + 30)
        pane = Image.new('RGB', (tw - 20, th), (238, 240, 243)); e = eye(st, 80)
        pane.paste(e, (40, 40)); pane.paste(e, (tw - 20 - 40 - e.width, 40))
        pd = ImageDraw.Draw(pane); pd.polygon([((tw - 20) / 2, 120), ((tw - 20) / 2 - 9, 230), ((tw - 20) / 2 + 9, 230)], fill=(225, 228, 232))  # crest point between the eyes
        pd.text((16, th - 70), name, font=ImageFont.truetype(FB, 26), fill=(30, 33, 38)); pd.text((16, th - 36), sub, font=ImageFont.truetype(F, 20), fill=(70, 75, 82))
        out.paste(pane, (x, y))
    out.save(sys.argv[1] if len(sys.argv) > 1 else 'HEAD_EYE_STATES.png', optimize=True); print(out.size)
