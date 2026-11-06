#!/usr/bin/env python3
"""Build marengo_hp.kicad_pcb and marengo_lp.kicad_pcb from the KiCad netlist.

Run with KiCad's Python, from marengo_pdu:
  <kicad python> tools/pcb_gen.py <net.net> pcb
Board x = robot X + 150, board y = 400 - robot Y, component face towards the viewer.
So robot +X (the robot's left) is on the right, as in FLOORPLAN.png.
Placement is a FIT TEST, not a finished layout: see design/LAYOUT_NOTES.md.
"""
import json, math, os, re, sys, collections
sys.path.insert(0, os.path.join(os.path.dirname(__file__)))
import sx, pcbnew

FPDIR = '/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints/'
MM = 1000000
def kx(X): return 150 + X
def ky(Y): return 400 - Y
def vec(x, y): return pcbnew.VECTOR2I(int(round(x * MM)), int(round(y * MM)))

BOARDS = {
    'hp': dict(sheet='HP_power_board', Y0=245, Y1=340, layers=4, oz=2,
               holes=[(-46, 336), (46, 336), (-46, 265), (46, 265)]),
    'lp': dict(sheet='LP_logic_board', Y0=190, Y1=243, layers=6, oz=1,
               holes=[(-46, 194), (46, 194), (-46, 239), (46, 239)]),
}

# Not allowed on the rear face: over 5 mm, through-hole, or must be reached from the front.
FRONT_ONLY = re.compile(r'CP_Elec|REDCUBE|Fuseholder|Molex|PinHeader|PinSocket|FanPin|Buzzer|XAL1510|TestPoint|'
                        r'USB_C|Tag-Connect|JST_')
# Installed height above the board, mm. Sources in LAYOUT_NOTES.md.
HEIGHT = [(r'CP_Elec_10x14', 17.0), (r'CP_Elec_10x10', 10.8), (r'Fuseholder_Blade_Mini', 15.0),
          (r'Fuseholder_Blade_ATO', 20.0), (r'REDCUBE', 8.0), (r'XAL1510', 10.0), (r'XAL60', 6.1),
          (r'Micro-Fit.*Vertical', 9.4), (r'Milligrid', 7.0), (r'FanPin', 11.0), (r'PinHeader', 11.0),
          (r'Buzzer', 6.5), (r'TO-263', 4.8), (r'Infineon', 2.4)]
def height(fp):
    for k, h in HEIGHT:
        if re.search(k, fp): return h
    return 3.0

def load_netlist(path):
    d = sx.parse(open(path).read())
    comps = {}
    for c in sx.getall(sx.get(d, 'components'), 'comp'):
        ref = sx.get(c, 'ref')[1]
        sp = sx.get(c, 'sheetpath')
        comps[ref] = dict(ref=ref, value=sx.get(c, 'value')[1], fp=sx.get(c, 'footprint')[1],
                          names=sx.get(sp, 'names')[1], tstamps=sx.get(sp, 'tstamps')[1],
                          tstamp=sx.get(c, 'tstamps')[1], dnp='"dnp"' in sx.dump(c))
    nets = {}
    for n in sx.getall(sx.get(d, 'nets'), 'net'):
        name = sx.get(n, 'name')[1]
        for x in sx.getall(n, 'node'):
            nets[(sx.get(x, 'ref')[1], sx.get(x, 'pin')[1])] = name
    return comps, nets

cache = {}
def footprint(fpid):
    lib, name = fpid.split(':')
    return pcbnew.FootprintLoad('marengo.pretty' if lib == 'marengo' else FPDIR + lib + '.pretty', name)

def bbox(fp):
    cy = fp.GetCourtyard(pcbnew.F_CrtYd)
    bb = cy.BBox() if cy.OutlineCount() else fp.GetBoundingBox(False)
    p = fp.GetPosition()
    return ((bb.GetLeft() - p.x) / MM, (bb.GetTop() - p.y) / MM, (bb.GetRight() - p.x) / MM, (bb.GetBottom() - p.y) / MM)

class Grid:
    """Occupancy at 0.25 mm. One Python int per row."""
    S = 0.25
    def __init__(s, X0, X1, Y0, Y1):
        s.x0, s.y1 = X0, Y1
        s.w, s.h = int((X1 - X0) / s.S), int((Y1 - Y0) / s.S)
        s.rows = [0] * s.h
    def cell(s, X, Y): return int(round((X - s.x0) / s.S)), int(round((s.y1 - Y) / s.S))
    def free(s, c, r, w, h):
        if c < 0 or r < 0 or c + w > s.w or r + h > s.h: return False
        m = ((1 << w) - 1) << c
        return not any(s.rows[i] & m for i in range(r, r + h))
    def take(s, c, r, w, h):
        if c < 0: w += c; c = 0
        w = min(w, s.w - c)
        if w <= 0: return
        m = ((1 << w) - 1) << c
        for i in range(max(r, 0), min(r + h, s.h)): s.rows[i] |= m
    def block(s, X0, Y0, X1, Y1):
        c, r = s.cell(X0, Y1); c2, r2 = s.cell(X1, Y0); s.take(c, r, c2 - c, r2 - r)
    def find(s, X, Y, w, h):
        """Nearest free spot to (X, Y) for a w x h mm box. Returns (centre, distance, cell) without taking it."""
        W, H = int(math.ceil(w / s.S)), int(math.ceil(h / s.S))
        if W > s.w or H > s.h: return None
        full = (1 << s.w) - 1
        def erode(x, n):
            k = 1; n -= 1
            while n > 0:
                d = min(k, n); x &= x >> d; n -= d; k *= 2
            return x
        rows = [erode(~r & full, W) for r in s.rows]
        n = H - 1; k = 1
        while n > 0:
            d = min(k, n)
            rows = [rows[i] & rows[i + d] if i + d < len(rows) else 0 for i in range(len(rows))]
            n -= d; k *= 2
        cx, cy = s.cell(X, Y); cx -= W // 2; cy -= H // 2; cx = max(cx, 0)
        best = None
        for r, m in enumerate(rows):
            if not m: continue
            hi = m >> cx
            cands = []
            if hi: cands.append(cx + ((hi & -hi).bit_length() - 1))
            lo = m & ((1 << cx) - 1)
            if lo: cands.append(lo.bit_length() - 1)
            for c in cands:
                d = (c - cx) ** 2 + (r - cy) ** 2
                if best is None or d < best[0]: best = (d, c, r)
        if best is None: return None
        d, c, r = best
        return (s.x0 + (c + W / 2) * s.S, s.y1 - (r + H / 2) * s.S), d, (c, r, W, H)

def fp_place(fp, X, Y, rot, back):
    fp.SetOrientationDegrees(rot)
    fp.SetPosition(vec(kx(X), ky(Y)))
    if back: fp.Flip(fp.GetPosition(), pcbnew.FLIP_DIRECTION_TOP_BOTTOM)

def edge(board, X0, Y0, X1, Y1, r=2.0):
    s = pcbnew.PCB_SHAPE(board); s.SetShape(pcbnew.SHAPE_T_RECTANGLE)
    s.SetStart(vec(kx(X0), ky(Y1))); s.SetEnd(vec(kx(X1), ky(Y0)))
    s.SetLayer(pcbnew.Edge_Cuts); s.SetWidth(int(0.1 * MM)); board.Add(s)

def note_rect(board, X0, Y0, X1, Y1, layer=pcbnew.Dwgs_User):
    s = pcbnew.PCB_SHAPE(board); s.SetShape(pcbnew.SHAPE_T_RECTANGLE)
    s.SetStart(vec(kx(X0), ky(Y1))); s.SetEnd(vec(kx(X1), ky(Y0)))
    s.SetLayer(layer); s.SetWidth(int(0.15 * MM)); board.Add(s)

def note_text(board, X, Y, t):
    x = pcbnew.PCB_TEXT(board); x.SetText(t); x.SetPosition(vec(kx(X), ky(Y)))
    x.SetLayer(pcbnew.Dwgs_User); x.SetTextSize(vec(1.5, 1.5)); x.SetTextThickness(int(.15 * MM)); board.Add(x)

def add_hole(board, X, Y, ref):
    fp = pcbnew.FootprintLoad(FPDIR + 'MountingHole.pretty', 'MountingHole_3.2mm_M3_Pad_Via')
    fp.SetReference(ref); fp.SetValue('M3 standoff'); fp.SetPosition(vec(kx(X), ky(Y)))
    board.Add(fp)
    for p in fp.Pads(): p.SetNetCode(0)
    return fp

def stackup_text(layers, oz):
    cu = 0.035 * oz
    if layers == 4:
        rows = [('F.Cu', cu), ('dielectric 1', 0.2, 'prepreg'), ('In1.Cu', cu), ('dielectric 2', 0.0, 'core'),
                ('In2.Cu', cu), ('dielectric 3', 0.2, 'prepreg'), ('B.Cu', cu)]
    else:
        rows = [('F.Cu', cu), ('dielectric 1', 0.15, 'prepreg'), ('In1.Cu', cu), ('dielectric 2', 0.25, 'core'),
                ('In2.Cu', cu), ('dielectric 3', 0.0, 'prepreg'), ('In3.Cu', cu), ('dielectric 4', 0.25, 'core'),
                ('In4.Cu', cu), ('dielectric 5', 0.15, 'prepreg'), ('B.Cu', cu)]
    total = 1.6 - sum(r[1] for r in rows if len(r) == 2) - sum(r[1] for r in rows if len(r) == 3)
    out = ['(stackup', '(layer "F.SilkS" (type "Top Silk Screen"))', '(layer "F.Paste" (type "Top Solder Paste"))',
           '(layer "F.Mask" (type "Top Solder Mask") (thickness 0.01))']
    for r in rows:
        if len(r) == 2:
            out.append('(layer "%s" (type "copper") (thickness %.3f))' % r)
        else:
            t = r[1] if r[1] else total
            out.append('(layer "%s" (type "%s") (thickness %.3f) (material "FR4") (epsilon_r 4.5) (loss_tangent 0.02))' % (r[0], r[2], t))
    out += ['(layer "B.Mask" (type "Bottom Solder Mask") (thickness 0.01))', '(layer "B.Paste" (type "Bottom Solder Paste"))',
            '(layer "B.SilkS" (type "Bottom Silk Screen"))', '(copper_finish "ENIG")', '(dielectric_constraints no)', ')']
    return '\n'.join(out)

def build(key, comps, nets, targets, outdir, parts_filter=None):
    cfg = BOARDS[key]
    board = pcbnew.BOARD()
    board.SetCopperLayerCount(cfg['layers'])
    ds = board.GetDesignSettings()
    edge(board, -50, cfg['Y0'], 50, cfg['Y1'])
    refs = sorted((c for c in comps.values() if c['names'].split('/')[1] == cfg['sheet']), key=lambda c: c['ref'])
    netmap = {}
    def net(name):
        if name not in netmap:
            n = pcbnew.NETINFO_ITEM(board, name); board.Add(n); netmap[name] = n
        return netmap[name]
    items = []
    for c in refs:
        fp = footprint(c['fp'])
        fp.SetReference(c['ref']); fp.SetValue(c['value'])
        fp.SetFPIDAsString(c['fp'])
        fp.SetPath(pcbnew.KIID_PATH(c['tstamps'].rstrip('/').lstrip('/') + '/' + c['tstamp']))
        fp.SetSheetname(c['names'].rstrip('/').split('/')[-1]); fp.SetDNP(c['dnp'])
        board.Add(fp)
        for p in fp.Pads():
            nm = nets.get((c['ref'], p.GetNumber()))
            if nm: p.SetNet(net(nm))
        items.append((c, fp, bbox(fp)))
    return board, items

def save(board, key, outdir):
    path = os.path.join(outdir, 'marengo_%s.kicad_pcb' % key)
    pcbnew.SaveBoard(path, board)
    t = open(path).read()
    st = stackup_text(BOARDS[key]['layers'], BOARDS[key]['oz'])
    t = re.sub(r'\(setup\s*\n', lambda m: '(setup\n' + st + '\n', t, count=1)
    open(path, 'w').write(t)
    return path
