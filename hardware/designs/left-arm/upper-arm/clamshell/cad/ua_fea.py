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
for n in argv:
    c0 = CASES[n]
    c = dict(c0, part='clam', remote=(cyl(1.65, [151, 170, -50, 166, 250, 60]),) + tuple(c0['remote'][1:]), fix=[('fixed', pl((150.0, 319.86, 6.0)))],   # the hub face clamped on the yaw RS02 output
             regions={'seam_band': SEAM, 'columns_lo': [134, 255, -16, 208, 285, 6], 'columns_up': [134, 255, 6, 208, 285, 20]})
    run('clam_' + n, c)
