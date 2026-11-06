#!/usr/bin/env python3
"""FRAME_OPTIONS_D.png: two FlexVolt-class packs, three placements (D1 inside, D2 lumbar backpack, D3 through the back) + fixed pelvis cells.
Run: python3 variants.py; node shoot.js <views>; python3 options_sheet.py"""
from PIL import Image
import compose
sheet, crop, f, INK, BG = compose.sheet, compose.crop, compose.f, compose.INK, compose.BG
import json
from PIL import ImageDraw
def col(k, title, lines):
    a = crop(Image.open(f'out/opt{k}_iso_rear.png').convert('RGB'), (250, 150, 2550, 2650), 900)
    b = crop(Image.open(f'out/opt{k}_side.png').convert('RGB'), (700, 150, 2100, 2650), 500)
    c = crop(Image.open(f'out/opt{k}_front.png').convert('RGB'), (450, 150, 2350, 2650), 500)
    W = a.width + b.width + c.width + 40; H = a.height + 80 + len(lines) * 36 + 20
    im = Image.new('RGB', (W, H), BG); d = ImageDraw.Draw(im)
    im.paste(a, (0, 0)); im.paste(b, (a.width + 20, 0)); im.paste(c, (a.width + b.width + 40, 0))
    y = a.height + 10; d.text((10, y), title, font=f(40, True), fill=INK); y += 60
    for ln in lines: d.text((10, y), ln, font=f(27), fill=(40, 44, 50)); y += 36
    return im
P = [
 (col('D1', 'D1  Inside the torso: spine stack', [
   'Lower box widened to 226 (one straight box), 20 mm taller (bottom Y 135).',
   'Two packs flat across the back, Y 159 to 355, slide in from the side.',
   'Power board stands in front of the packs. Compute above, unchanged.',
   'Cost: slim waist gone (frame 226 at the hips, shell ~240).',
   'Arm thread: inward forearm yaw across the belly roughly halves.']), ''),
 (col('D2', 'D2  Lumbar backpack: packs outside', [
   'Spec stepped frame unchanged. Printed cradle on the H3/H4/H5 rear points.',
   'Packs sit 84 to 186 mm behind the frame centre, Y 178 to 374.',
   'Slim waist kept. Packs reach the back shell line by ~90 mm.',
   'CoG moves back ~15 mm for 3 kg of packs. Doubles as the crush pad.',
   'Side-slide docks. The lumbar bump is the look to judge.']), ''),
 (col('D3', 'D3  Through the back: packs lengthwise', [
   'Spec stepped frame, 20 mm taller (bottom Y 135). Lower box stays 150 wide.',
   'Packs end-on, 102 wide between the posts, stacked, slide in from the BACK.',
   'About 65 mm of each pack stands proud of the frame rear; ~16 mm past the',
   'present shell back line. Docks on the front rail plane, board on the front face.',
   'Slim waist kept, rear access, hot-swap without a door in the side.']), ''),
]
notes = [
 'Pack envelope 160 x 102 x 95 mm, 1.5 kg each (DeWalt DCB6112 class; any pack of that size works). Yellow ghost blocks: fixed pelvis cells (placeholder 60 x 80 x 90 each, flanking the waist RS03) for a lower centre of gravity.',
 'Thin grey plane: approximate inner face of the present shell back (Z -97). Grey ring: waist RS03. Electrical note: a 15S "60 V" pack is 63 V full, over the RS03 60 V maximum; run the two packs in 20 V mode in series (10S, 42 V full) or regulate.',
 'Renders are envelopes for a decision, not CAD. Y is torso height from the pelvis bottom; Z forward; the camera for the left view is at the rear left.',
]
sheet(P, 'FRAME OPTIONS D: two FlexVolt-class 60 V packs, three placements', 'FRAME_OPTIONS_D.png', notes=notes, sub='Battery and power board packaging for the internal torso frame. Torso coords: X lateral, Y up from the pelvis bottom, Z forward. 2026-11-06')
