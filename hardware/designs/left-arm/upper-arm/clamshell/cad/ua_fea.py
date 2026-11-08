"""Seam FEA for the upper-arm clamshell: the db71b23b upper-arm cases (LC1A/B/C, LC2) on fea/UA_CLAM_BONDED.SLDPRT (the
clamshell geometry bonded at the seam; seam tractions are read from the saved nodal stresses), using p3_fea.run. usage: python ua_fea.py LC1A LC1B ..."""
import sys
src = open(__file__.replace('ua_fea.py', 'p3_fea.py'), encoding='utf-8').read()
argv = sys.argv[1:]
sys.argv = ['x', 'none']
NL = chr(10)
src = src.replace('    vm = s[:, 10]' + NL, '    vm = s[:, 10]' + NL + "    np.savez(str(W / 'fea' / (name + '.npz')), pos=pos, s=s)" + NL, 1)
exec(compile(src[:src.index("if sys.argv[1] == 'faces':")], 'p3_fea.py', 'exec'))
PARTS['clam'] = F / 'UA_CLAM_BONDED.SLDPRT'
SEAM = [130, 240, 3, 212, 300, 9]          # seam band, Z 3..9
# Governing case (Joey): arm straight, 5 kg at the hand about 0.5 m from the shoulder pitch plus self-weight, reduced
# to the elbow load point: 62 N along gravity and 17.2 N.m, which makes 24 N.m at the hub face (62 N x 0.110 m + 17.2).
# Arm frame: Y up the arm, Z front (flexion side). Shoulder pitch th: gravity g = (0, -cos th, -sin th); lateral: +-X.
import math as _m
FG, MG = 62.0, 17.2
for th in (0, 45, 90, 135, 180):
    g = (0.0, -_m.cos(_m.radians(th)), -_m.sin(_m.radians(th)))
    CASES[f'G{th}'] = dict(CASES['LC1A'], remote=(None, GOLD, tuple(round(FG * v, 2) for v in g), (round(MG * _m.sin(_m.radians(th)), 2), 0.0, 0.0)))
for tag, sx in (('Lp', 1.0), ('Lm', -1.0)):
    CASES['G' + tag] = dict(CASES['LC1A'], remote=(None, GOLD, (FG * sx, 0.0, 0.0), (0.0, 0.0, MG * sx)))
HUB_BOX = [125, 296, -40, 215, 321, 50]
RING_BOX = [150, 165, -40, 167, 256, 50]
for n in argv:
    c0 = CASES[n]
    c = dict(c0, part='clam', remote=(cyl(1.65, [151, 170, -50, 166, 250, 60]),) + tuple(c0['remote'][1:]), fix=[('fixed', pl((150.0, 319.86, 6.0)))],   # the hub face clamped on the yaw RS02 output
             regions={'seam_band': SEAM, 'columns_lo': [134, 255, -22, 208, 296, 6], 'columns_up': [134, 255, 6, 208, 296, 34], 'hub': HUB_BOX, 'ring': RING_BOX})
    run('clam_' + n, c)
