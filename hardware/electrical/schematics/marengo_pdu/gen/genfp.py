#!/usr/bin/env python3
"""Generate marengo.pretty footprints. Dimensions cited per footprint in descr."""
import math, os, sys
OUT = sys.argv[1]
VERSION = "20260206"

def n(v):
    s = f"{v:.6f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s

class FP:
    def __init__(s, name, descr, tags, attr, ref_y, val_y, cx=0):
        s.name, s.descr, s.tags, s.attr, s.ref_y, s.val_y, s.cx = name, descr, tags, attr, ref_y, val_y, cx
        s.g, s.p, s.extra = [], [], ""
    def line(s, a, b, layer, w):
        s.g.append(f"""\t(fp_line
\t\t(start {n(a[0])} {n(a[1])})
\t\t(end {n(b[0])} {n(b[1])})
\t\t(stroke
\t\t\t(width {n(w)})
\t\t\t(type solid)
\t\t)
\t\t(layer "{layer}")
\t)""")
    def poly(s, pts, layer, w, fill):
        p = "\n".join(f"\t\t\t(xy {n(x)} {n(y)})" for x, y in pts)
        s.g.append(f"""\t(fp_poly
\t\t(pts
{p}
\t\t)
\t\t(stroke
\t\t\t(width {n(w)})
\t\t\t(type solid)
\t\t)
\t\t(fill {'yes' if fill else 'no'})
\t\t(layer "{layer}")
\t)""")
    def rect(s, x0, y0, x1, y1, layer, w):
        s.g.append(f"""\t(fp_rect
\t\t(start {n(x0)} {n(y0)})
\t\t(end {n(x1)} {n(y1)})
\t\t(stroke
\t\t\t(width {n(w)})
\t\t\t(type solid)
\t\t)
\t\t(fill no)
\t\t(layer "{layer}")
\t)""")
    def silk(s, a, b): s.line(a, b, "F.SilkS", 0.12)
    def tri(s, x, tip_y, half=0.2, h=0.28):
        """Silk pad-1 triangle pointing down at the pad column, KiCad library style."""
        s.poly([(x, tip_y), (x - half, tip_y - h), (x + half, tip_y - h)], "F.SilkS", 0.12, True)
    def crtyd(s, x0, y0, x1, y1): s.rect(x0, y0, x1, y1, "F.CrtYd", 0.05)
    def fab_body(s, x0, y0, x1, y1, ch):
        s.poly([(x0 + ch, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0 + ch)], "F.Fab", 0.1, False)
    def rr(s, num, x, y, w, h, r=0.05, layers=("F.Cu", "F.Mask", "F.Paste"), mask=None):
        ratio = r / min(w, h)
        L = " ".join(f'"{l}"' for l in layers)
        m = f"\n\t\t(solder_mask_margin {n(mask)})" if mask is not None else ""
        s.p.append(f"""\t(pad "{num}" smd roundrect
\t\t(at {n(x)} {n(y)})
\t\t(size {n(w)} {n(h)})
\t\t(layers {L})
\t\t(roundrect_rratio {ratio:.6f}){m}
\t)""")
    def rc(s, num, x, y, w, h, layers=("F.Cu", "F.Mask", "F.Paste"), mask=None):
        L = " ".join(f'"{l}"' for l in layers)
        m = f"\n\t\t(solder_mask_margin {n(mask)})" if mask is not None else ""
        s.p.append(f"""\t(pad "{num}" smd rect
\t\t(at {n(x)} {n(y)})
\t\t(size {n(w)} {n(h)})
\t\t(layers {L}){m}
\t)""")
    def custom(s, num, x, y, pts, anchor, mask=None):
        p = "\n".join(f"\t\t\t\t\t(xy {n(a - x)} {n(b - y)})" for a, b in pts)
        m = f"\n\t\t(solder_mask_margin {n(mask)})" if mask is not None else ""
        s.p.append(f"""\t(pad "{num}" smd custom
\t\t(at {n(x)} {n(y)})
\t\t(size {n(anchor)} {n(anchor)})
\t\t(layers "F.Cu" "F.Mask" "F.Paste"){m}
\t\t(options
\t\t\t(clearance outline)
\t\t\t(anchor rect)
\t\t)
\t\t(primitives
\t\t\t(gr_poly
\t\t\t\t(pts
{p}
\t\t\t\t)
\t\t\t\t(width 0)
\t\t\t\t(fill yes)
\t\t\t)
\t\t)
\t)""")
    def th(s, num, x, y, shape, size, drill):
        s.p.append(f"""\t(pad "{num}" thru_hole {shape}
\t\t(at {n(x)} {n(y)})
\t\t(size {n(size)} {n(size)})
\t\t(drill {n(drill)})
\t\t(layers "*.Cu" "*.Mask")
\t\t(remove_unused_layers no)
\t)""")
    def write(s):
        font = lambda sz, t: f"""\t\t(effects
\t\t\t(font
\t\t\t\t(size {sz} {sz})
\t\t\t\t(thickness {t})
\t\t\t)
\t\t)"""
        extra = ("\n\t" + s.extra) if s.extra else ""
        txt = f"""(footprint "{s.name}"
\t(version {VERSION})
\t(generator "marengo-footprints")
\t(layer "F.Cu")
\t(descr "{s.descr}")
\t(tags "{s.tags}")
\t(property "Reference" "REF**"
\t\t(at {n(s.cx)} {n(s.ref_y)} 0)
\t\t(layer "F.SilkS")
{font(1, 0.15)}
\t)
\t(property "Value" "{s.name}"
\t\t(at {n(s.cx)} {n(s.val_y)} 0)
\t\t(layer "F.Fab")
{font(1, 0.15)}
\t)
\t(attr {s.attr}){extra}
\t(duplicate_pad_numbers_are_jumpers no)
""" + "\n".join(s.g) + f"""
\t(fp_text user "${{REFERENCE}}"
\t\t(at {n(s.cx)} {n(s.fab_y)} 0)
\t\t(layer "F.Fab")
{font(s.fab_size, n(s.fab_size * 0.15))}
\t)
""" + "\n".join(s.p) + """
\t(embedded_fonts no)
)
"""
        with open(os.path.join(OUT, s.name + ".kicad_mod"), "w") as f:
            f.write(txt)

# ---- 1. TPS48100 DGX ------------------------------------------------------
f = FP("TPS48100_DGX_VSSOP-19",
       "TI DGX0019A VSSOP-19, 5.1x3.0 mm, P0.5 mm, 20-pin frame, pin 16 absent. Land: TI example board layout, SLUSEW1C (TPS4810-Q1) p.35; stencil p.36 (1:1, 0.125 mm); outline p.34. NSMD mask 0.05 per p.35. No thermal pad in the drawing.",
       "TPS48100-Q1 TPS4810 VSSOP DGX DGX0019A", "smd", -4.0, 4.0)
f.fab_y, f.fab_size = 0, 0.6
PX, PW, PH, MASK = 2.2, 1.45, 0.3, 0.05
for i in range(1, 21):
    if i == 16: continue
    if i <= 10: f.rr(i, -PX, -2.25 + (i - 1) * 0.5, PW, PH, mask=MASK)
    else:       f.rr(i,  PX,  2.25 - (i - 11) * 0.5, PW, PH, mask=MASK)
f.fab_body(-1.5, -2.55, 1.5, 2.55, 0.75)
f.silk((-1.61, -2.66), (1.61, -2.66)); f.silk((-1.61, 2.66), (1.61, 2.66))
f.silk((-1.61, -2.66), (-1.61, -2.61)); f.silk((1.61, -2.66), (1.61, -2.61))
f.silk((-1.61, 2.66), (-1.61, 2.61)); f.silk((1.61, 2.66), (1.61, 2.61))
f.tri(-PX, -2.67)
f.crtyd(-3.2, -3.05, 3.2, 3.05)
f.write()

# ---- 2. TPS56637 RPA ------------------------------------------------------
f = FP("TPS56637_RPA_VQFN-HR-10",
       "TI RPA0010A VQFN-HR-10, 3x3 mm. Land: TI example board layout, SLVSEG1A p.29, pad coordinates cross-checked against PDF vectors; stencil p.30 (pads 6 and 9 split, 89% coverage, 0.1 mm); outline p.28. NSMD mask 0.07 per p.29.",
       "TPS56637 VQFN-HR RPA RPA0010A", "smd", -3.0, 3.0)
f.fab_y, f.fab_size = 0, 0.5
M = 0.07
for i, y in enumerate((-0.75, -0.25, 0.25, 0.75), 1):
    f.rr(i, -1.4, y, 0.6, 0.25, mask=M)
f.rr(5, -0.93, 1.4, 0.25, 0.6, mask=M)
f.rr(6, 0.37, 0.85, 0.25, 1.7, layers=("F.Cu", "F.Mask"), mask=M)
f.rr(7, 0.87, 1.4, 0.25, 0.6, mask=M)
# pad 8 comb: outline from PDF vectors, p.29; R0.05 on convex corners and on the top finger root
R = 0.05
def arc(cx, cy, a0, a1, steps=4):
    return [(cx + R * math.cos(math.radians(a0 + (a1 - a0) * k / steps)),
             cy + R * math.sin(math.radians(a0 + (a1 - a0) * k / steps))) for k in range(steps + 1)]
X0, X1, X2 = 0.745, 1.145, 1.70
pts = []
pts += arc(X0 + R, -1.70 + R, 180, 270)        # bar top-left
pts += arc(X1 - R, -1.70 + R, 270, 360)        # bar top-right
pts += arc(X1 + R, -0.925, 180, 90)            # concave root, finger 1 top
pts += arc(X2 - R, -0.875 + R, 270, 360)       # finger 1
pts += arc(X2 - R, -0.625 - R, 0, 90)
pts += [(X1, -0.625), (X1, -0.375)]
pts += arc(X2 - R, -0.375 + R, 270, 360)       # finger 2
pts += arc(X2 - R, -0.125 - R, 0, 90)
pts += [(X1, -0.125), (X1, 0.125)]
pts += arc(X2 - R, 0.125 + R, 270, 360)        # finger 3
pts += arc(X2 - R, 0.375 - R, 0, 90)
pts += [(X1, 0.375), (X1, 0.400)]
pts += arc(X0 + R, 0.400 - R, 90, 180)         # bar bottom-left
f.custom(8, 0.945, -0.65, pts, 0.3, mask=M)
f.rr(9, -0.205, -0.65, 0.4, 2.1, layers=("F.Cu", "F.Mask"), mask=M)
f.rr(10, -0.93, -1.4, 0.25, 0.6, mask=M)
for y in (0.375, 1.325): f.rr("", 0.37, y, 0.25, 0.75, layers=("F.Paste",))
for y in (-1.225, -0.075): f.rr("", -0.205, y, 0.4, 0.95, layers=("F.Paste",))
f.fab_body(-1.5, -1.5, 1.5, 1.5, 0.75)
s = 1.61
f.silk((-s, -s), (-1.31, -s)); f.silk((-s, -s), (-s, -1.13))
f.silk((s, -s), (1.41, -s));   f.silk((s, -s), (s, -1.14))
f.silk((-s, s), (-1.31, s));   f.silk((-s, s), (-s, 1.14))
f.silk((s, s), (1.26, s));     f.silk((s, s), (s, 0.66))
f.tri(-1.8, -1.2, half=0.14, h=0.24)
f.crtyd(-2.0, -2.0, 2.0, 2.0)
f.write()

# ---- 3. ISOM8710 DFF ------------------------------------------------------
f = FP("ISOM8710_DFF",
       "TI DFF0005A SOIC-5, body 4.8x3.51 mm, P2.54 mm. Land: TI example board layout HV/ISOLATION OPTION, 5X 1.2x0.6 at 6.3 span, pad-to-pad 5.1 mm, SLLSFO4E p.34; stencil p.35; outline p.33. IPC-7351 nominal alternative on p.34 is 1.5x0.6 at 6.0 span, 4.5 mm. NSMD mask 0.07 per p.34.",
       "ISOM8710 ISOM8711 DFF DFF0005A isolator optocoupler emulator", "smd", -3.2, 3.2)
f.fab_y, f.fab_size = 0, 0.8
PX, PW, PH, M = 3.15, 1.2, 0.6, 0.07
f.rr(1, -PX, -1.27, PW, PH, mask=M)
f.rr(2, -PX,  1.27, PW, PH, mask=M)
f.rr(3,  PX,  1.27, PW, PH, mask=M)
f.rr(4,  PX,  0,    PW, PH, mask=M)
f.rr(5,  PX, -1.27, PW, PH, mask=M)
f.fab_body(-2.4, -1.755, 2.4, 1.755, 0.75)
f.silk((-2.51, -1.87), (2.51, -1.87)); f.silk((-2.51, 1.87), (2.51, 1.87))
f.tri(-PX, -1.84)
f.crtyd(-4.0, -2.35, 4.0, 2.35)
f.write()

# ---- 4. WSLP3921 Kelvin ---------------------------------------------------
GROUP = '(net_tie_pad_groups "1,2" "3,4")'
f = FP("WSLP3921_Kelvin",
       "Vishay WSLP3921 3921 shunt, L10.0 W5.2 T2.0 mm, 0.2 to 4 mOhm. Current pads d=2.70 b=6.20 gap l=5.60 mm per doc 30176 rev 29-Aug-2025 p.2 solder pad table. Pads 2 and 3 are derived Kelvin tap pads on the inner pad edge at mid width, following the p.2 Typical sensing traces figure, which has no dimensions. Copper only, no paste, no mask opening, net tied to the current pad.",
       "WSLP3921 shunt Kelvin 3921 current sense", "smd", -4.6, 4.6)
f.fab_y, f.fab_size = 0, 1
f.extra = GROUP
D, B, GAP = 2.70, 6.20, 5.60
CX = GAP / 2 + D / 2
f.rc(1, -CX, 0, D, B)
TW, TH, OV = 1.0, 0.6, 0.25            # derived tap: 1.0 x 0.6, 0.25 overlap into current land
f.rc(2, -(GAP / 2 + OV - TW / 2), 0, TW, TH, layers=("F.Cu",))
f.rc(3,  (GAP / 2 + OV - TW / 2), 0, TW, TH, layers=("F.Cu",))
f.rc(4,  CX, 0, D, B)
f.fab_body(-5.0, -2.6, 5.0, 2.6, 0.75)
f.line((-3.0, -2.6), (-3.0, 2.6), "F.Fab", 0.1)
f.line((3.0, -2.6), (3.0, 2.6), "F.Fab", 0.1)
f.silk((-2.54, -2.71), (2.54, -2.71)); f.silk((-2.54, 2.71), (2.54, 2.71))
f.tri(-CX, -3.32)
f.crtyd(-5.75, -3.75, 5.75, 3.75)
f.write()

# ---- 5. NTC wire pads -----------------------------------------------------
f = FP("Wire_Pads_2_NTC",
       "2 through-hole solder pads for a leaded thermistor, no body. 2.0 mm pad, 1.0 mm drill, 2.54 mm pitch, per request. Not from a datasheet.",
       "NTC thermistor wire pads solder through hole", "through_hole", -3.0, 3.0, cx=1.27)
f.fab_y, f.fab_size = 0, 0.5
f.th(1, 0, 0, "rect", 2.0, 1.0)
f.th(2, 2.54, 0, "circle", 2.0, 1.0)
f.fab_body(-1.27, -1.27, 3.81, 1.27, 0.5)
f.rect(-1.38, -1.38, 3.92, 1.38, "F.SilkS", 0.12)
f.tri(0, -1.5, half=0.2, h=0.26)
f.crtyd(-1.75, -2.0, 4.3, 1.75)
f.write()
# ---- 6. Schurter USE 2410 fuse --------------------------------------------
f = FP("Fuse_2410",
       "Schurter USE 2410 fuse. Soldering pads 2.50 x 2.80 mm, 3.00 mm gap; body 6.1 x 2.5 x 2.2 mm. Schurter USE 2410 data sheet, 21.07.2026, p.2.",
       "fuse Schurter USE 2410 6125 SMD", "smd", -2.6, 2.6)
f.fab_y, f.fab_size = 0, 0.5
PW, PH, PGAP = 2.5, 2.8, 3.0           # datasheet soldering pads
BL, BW = 6.1, 2.5                      # datasheet body
f.rc(1, -(PGAP + PW) / 2, 0, PW, PH)
f.rc(2,  (PGAP + PW) / 2, 0, PW, PH)
f.rect(-BL / 2, -BW / 2, BL / 2, BW / 2, "F.Fab", 0.1)
f.silk((-1.2, -1.36), (1.2, -1.36)); f.silk((-1.2, 1.36), (1.2, 1.36))
f.crtyd(-4.25, -1.65, 4.25, 1.65)      # 0.25 past the land
f.write()
print("ok")
