#!/usr/bin/env python3
"""Fit test and board build. Run with KiCad's Python from marengo_pdu:
  <kicad python> tools/pcb_place.py <net.net> pcb
Fixed parts are hand-placed in robot coordinates. Everything else goes to the nearest free spot,
courtyard to courtyard, with no routing room. That is the best case, so an unplaced part is a hard no-fit."""
import json, os, re, sys
sys.path.insert(0, os.path.dirname(__file__))
import pcb_gen as G
import pcbnew

HOLE_R = 3.6   # frame thread's 6 mm pad plus the hole's courtyard
TERM = 13.2
T_ROW = 333.0
FIXED = {'hp': {
    # top edge: lugs point up into Y 340 to 353.6
    'J1': (9.4, T_ROW, 0, 0), 'J3': (-9.4, T_ROW, 0, 0),
    'J501': (22.6, T_ROW, 0, 0), 'J502': (35.8, T_ROW, 0, 0),
    'J601': (-22.6, T_ROW, 0, 0), 'J602': (-35.8, T_ROW, 0, 0),
    # side columns, -X
    'J201': (-43.4, 319.6, 0, 0), 'J202': (-43.4, 306.2, 0, 0),
    'J701': (-43.4, 292.8, 0, 0), 'J702': (-43.4, 279.4, 0, 0),
    'J901': (-33, 266, 0, 0), 'J902': (-43.4, 252.6, 0, 0),
    # side columns, +X
    'J1001': (43.4, 319.6, 0, 0), 'J1002': (43.4, 306.2, 0, 0),
    'J401': (43.4, 292.8, 0, 0), 'J402': (43.4, 279.4, 0, 0),
    'J801': (33, 266, 0, 0), 'J802': (43.4, 252.6, 0, 0),
    # ribbons to LP on the bottom edge
    'J101': (-17.6, 249.6, 0, 0), 'J102': (17.6, 249.6, 0, 0),
    # return path: pack minus terminal straight into the shunt, AFE beside it
    'RS1': (-9.4, 322, 0, 0),
    # FET banks in the airflow, current runs top to bottom
    'Q201': (-21.15, 309, 0, 0), 'Q202': (-7.05, 309, 0, 0), 'Q203': (7.05, 309, 0, 0), 'Q204': (21.15, 309, 0, 0),
    'Q205': (-21.15, 298, 0, 0), 'Q206': (-7.05, 298, 0, 0), 'Q207': (7.05, 298, 0, 0), 'Q208': (21.15, 298, 0, 0),
    'Q301': (-14.1, 287, 0, 0), 'Q302': (0, 287, 0, 0), 'Q303': (14.1, 287, 0, 0),
    'Q304': (-14.1, 276, 0, 0), 'Q305': (0, 276, 0, 0), 'Q306': (14.1, 276, 0, 0),
}, 'lp': {
    # ribbons from HP on the top edge
    'J2301': (-17.6, 238.6, 0, 0), 'J2302': (17.6, 238.6, 0, 0),
}}
TARGET = {
    'hp': [('pack_input', -25, 318), ('bms_fets', 0, 303), ('estop', 0, 282), ('vmot_bulk', 0, 262),
           ('L_ARM', 34, 312), ('R_ARM', -34, 312), ('WAIST', -30, 262), ('R_LEG', -30, 262),
           ('L_LEG', 30, 262), ('AUX', 30, 262), ('', 0, 256)],
    'lp': [('front_end', -25, 218), ('pol_rails', -10, 202), ('rail_switches', 15, 205), ('supervisor', 30, 228),
           ('can_bridges', 18, 228), ('can_phy', 42, 205), ('io_fans', 40, 197), ('', 0, 232)],
}
def target(key, names):
    for k, x, y in TARGET[key]:
        if k in names: return x, y
    return 0, 0
def is_tht(fp):
    """Any drilled pad, thermal vias included, also takes the rear face."""
    return any(p.HasHole() for p in fp.Pads())

def fit(key, items, skip=lambda c: False):
    cfg = G.BOARDS[key]; Y0, Y1 = cfg['Y0'], cfg['Y1']
    f = G.Grid(-49.5, 49.5, Y0 + .5, Y1 - .5); b = G.Grid(-49.5, 49.5, Y0 + .5, Y1 - .5)
    for X, Y in cfg['holes']:
        for g in (f, b): g.block(X - HOLE_R, Y - HOLE_R, X + HOLE_R, Y + HOLE_R)
    res = {}; fixed = FIXED[key]
    def rect(bb, rot):
        w, h = bb[2] - bb[0], bb[3] - bb[1]
        return (w, h) if rot == 0 else (h, w)
    # corner keep-outs for parts over 5 mm
    corners = []
    if key == 'hp': corners = [(s, 325, s + 4, 340) if s > 0 else (s - 4, 325, s, 340) for s in (46, -46)]
    if key == 'lp': corners = [(s, 190, s + 4, 203) if s > 0 else (s - 4, 190, s, 203) for s in (46, -46)]
    live = [t for t in items if not skip(t[0])]
    for c, fp, bb in live:
        if c['ref'] in fixed:
            X, Y, rot, back = fixed[c['ref']]
            cx, cy = (bb[0] + bb[2]) / 2, (bb[1] + bb[3]) / 2
            w, h = rect(bb, rot)
            f.block(X + cx - w / 2, Y - cy - h / 2, X + cx + w / 2, Y - cy + h / 2)
            if is_tht(fp): b.block(X + cx - w / 2, Y - cy - h / 2, X + cx + w / 2, Y - cy + h / 2)
            res[c['ref']] = (X, Y, rot, back)
    rest = [t for t in live if t[0]['ref'] not in fixed]
    def area(t): return (t[2][2] - t[2][0]) * (t[2][3] - t[2][1])
    one_side = lambda t: G.FRONT_ONLY.search(t[0]['fp']) or is_tht(t[1])
    front = sorted((t for t in rest if one_side(t)), key=lambda t: (-area(t), t[0]['ref']))
    other = sorted((t for t in rest if not one_side(t)), key=lambda t: (-area(t), t[0]['ref']))
    def put(t, grids):
        c, fp, bb = t
        X, Y = target(key, c['names'])
        tall = G.height(c['fp']) > 5
        for g, back in grids:
            saved = None
            if tall and g is f:
                saved = list(f.rows)
                for x0, y0, x1, y1 in corners: f.block(x0, y0, x1, y1)
            best = None
            for rot in (0, 90):
                ww, hh = rect(bb, rot)
                p = g.find(X, Y, ww, hh)
                if p and (best is None or p[1] < best[0][1]): best = (p, rot)
            if best:
                (p, _, cell), rot = best
                if saved is not None: f.rows = saved
                g.take(*cell)
                res[c['ref']] = (p[0], p[1], rot, back, True)
                return True
            if saved is not None: f.rows = saved
        return False
    un = []
    for t in front:
        if put(t, [(f, 0)]):
            X, Y = res[t[0]['ref']][:2]
            if is_tht(t[1]):
                bb = t[2]; rot = res[t[0]['ref']][2]; w, h = rect(bb, rot)
                b.block(X - w / 2, Y - h / 2, X + w / 2, Y + h / 2)
        else: un.append(t)
    for t in other:
        if not put(t, [(f, 0), (b, 1)]): un.append(t)
    return res, un

def project(key, out):
    oz = G.BOARDS[key]['oz']
    if key == 'hp':
        cls = [('Default', .2, .25), ('HV_Signal', .2, .25), ('HV_Port', .2, 6.1), ('HV_Main', .2, 16.0)]
        pat = [('HV_Main', p) for p in ('VMOT', 'VSYS', '*PACK_TOP*', '*DSG_OUT*', '*BAT-*', '*CHG_IN*', 'MID')] + \
              [('HV_Port', p) for p in ('*V_PORT*', '*V_OUT*')] + \
              [('HV_Signal', p) for p in ('*pack_input_shunt_afe/B*', '*pack_input_shunt_afe/VC*', '*_SENS*')]
        rules = dict(min_clearance=.2, min_track_width=.2, min_via_diameter=.6, min_through_hole_diameter=.3, min_copper_edge_clearance=.5)
    else:
        cls = [('Default', .1, .15), ('HV_Signal', .1, .25), ('HV_Main', .1, 4.0), ('Rail12', .1, 6.4), ('Rail6', .1, 2.3)]
        pat = [('HV_Main', p) for p in ('VSYS', '*VSYS_OR*')] + [('Rail12', p) for p in ('+12V', '+24V')] + \
              [('Rail6', p) for p in ('RAW_*',)]
        rules = dict(min_clearance=.1, min_track_width=.1, min_via_diameter=.45, min_through_hole_diameter=.2, min_copper_edge_clearance=.3)
    pro = dict(board=dict(design_settings=dict(rules=rules, defaults={}), layer_presets=[], viewports=[]),
               meta=dict(filename='marengo_%s.kicad_pro' % key, version=3),
               net_settings=dict(classes=[dict(name=n, clearance=c, track_width=w, via_diameter=.6, via_drill=.3,
                                               microvia_diameter=.3, microvia_drill=.1, diff_pair_width=.2, diff_pair_gap=.25,
                                               diff_pair_via_gap=.25, schematic_color='rgba(0, 0, 0, 0.000)',
                                               pcb_color='rgba(0, 0, 0, 0.000)', wire_width=6, bus_width=12, line_style=0, priority=i)
                                          for i, (n, c, w) in enumerate(cls)],
                                 netclass_assignments=None, netclass_patterns=[dict(netclass=n, pattern=p) for n, p in pat],
                                 meta=dict(version=4)))
    json.dump(pro, open(os.path.join(out, 'marengo_%s.kicad_pro' % key), 'w'), indent=2)
    hv = ' || '.join("A.NetClass == '%s'" % n for n, _, _ in cls if n.startswith('HV'))
    dru = """(version 1)
# 51 to 100 V, IPC-2221 B2 external uncoated 0.6 mm, B1 internal 0.1 mm (held at 0.2). Stack peak about 88 V.
(rule hv_outer (layer outer) (condition "(%s) && A.NetClass != B.NetClass") (constraint clearance (min 0.6mm)))
(rule hv_inner (layer inner) (condition "(%s) && A.NetClass != B.NetClass") (constraint clearance (min 0.2mm)))
# Fine-pitch IC pins cannot meet 0.6 mm; the part's own pin spacing governs inside its footprint.
(rule hv_ic_pins (condition "A.memberOfFootprint('U*') && B.memberOfFootprint('U*')") (constraint clearance (min 0.1mm)))
(rule hole_to_copper (condition "A.Type == 'Hole' || B.Type == 'Hole'") (constraint hole_clearance (min 0.3mm)))
""" % (hv, hv)
    open(os.path.join(out, 'marengo_%s.kicad_dru' % key), 'w').write(dru)
    open(os.path.join(out, 'fp-lib-table'), 'w').write('(fp_lib_table\n\t(version 7)\n\t(lib (name "marengo")(type "KiCad")(uri "${KIPRJMOD}/../marengo.pretty")(options "")(descr "marengo project footprints"))\n)\n')

def main():
    comps, nets = G.load_netlist(sys.argv[1]); out = sys.argv[2]
    summary = {}
    legs = re.compile('LEG')
    for key in ('hp', 'lp'):
        board, items = G.build(key, comps, nets, None, out)
        A = lambda ts: sum((t[2][2] - t[2][0]) * (t[2][3] - t[2][1]) for t in ts)
        cases = {'as captured': lambda c: False,
                 'no DNP parts': lambda c: c['dnp'],
                 'no DNP, no bulk or hold-up caps': lambda c: c['dnp'] or 'CP_Elec' in c['fp']}
        for name, sk in cases.items():
            res, un = fit(key, items, sk)
            summary[key + ': ' + name] = dict(parts=sum(1 for t in items if not sk(t[0])), unplaced=len(un),
                area=round(A([t for t in items if not sk(t[0])])), unplaced_area=round(A(un)),
                unplaced_big=sorted({t[0]['fp'].split(':')[1][:26] for t in un if A([t]) > 40})[:14])
            if name == 'as captured': keep = res, un
        res, un = keep
        cfg = G.BOARDS[key]
        px, py = 58, cfg['Y1']; rowh = 0
        for c, fp, bb in items:
            if c['ref'] in res:
                r = res[c['ref']]; G.fp_place(fp, r[0], r[1], r[2], r[3])
                if len(r) == 5:   # centre the box on the found spot
                    cx, cy = (bb[0] + bb[2]) / 2, (bb[1] + bb[3]) / 2
                    if r[2] == 90: cx, cy = cy, -cx
                    if r[3]: cy = -cy
                    fp.SetPosition(G.vec(G.kx(r[0] - cx), G.ky(r[1] + cy)))
        for c, fp, bb in sorted(un, key=lambda t: t[0]['ref']):
            w, h = bb[2] - bb[0], bb[3] - bb[1]
            if px + w > 128: px = 58; py -= rowh + 1; rowh = 0
            G.fp_place(fp, px - bb[0], py + bb[1], 0, 0); px += w + 1; rowh = max(rowh, h)
        G.note_text(board, 58, cfg['Y1'] + 3, 'NOT ON BOARD: %d parts do not fit' % len(un))
        for i, (X, Y) in enumerate(cfg['holes']): G.add_hole(board, X, Y, 'H%d' % (i + 1))
        Y0, Y1 = cfg['Y0'], cfg['Y1']
        for s in (-1, 1):
            if key == 'hp': G.note_rect(board, min(s * 46, s * 50), 325, max(s * 46, s * 50), 340)
            else: G.note_rect(board, min(s * 46, s * 50), 190, max(s * 46, s * 50), 203)
        if key == 'hp': G.note_rect(board, -35, 340, 30, 353.6); G.note_text(board, -33, 347, 'lug and cable envelope to Y 353.6, Z 43.5-60')
        else: G.note_rect(board, -25.5, 190, 25.5, 192); G.note_text(board, -24, 188, 'brake resistor below, |X| <= 25.5')
        G.save(board, key, out)
        project(key, out)
        summary[key + ' items'] = {'front': sum(1 for r in res.values() if not r[3]), 'rear': sum(1 for r in res.values() if r[3])}
    json.dump(summary, open(os.path.join(out, 'fit.json'), 'w'), indent=1, sort_keys=True)
    print(json.dumps(summary, indent=1, sort_keys=True))
main()
