#!/usr/bin/env python3
"""Hand-finishing setup for the rev B5 board in KiCad: net classes and the E-stop DRC rule, so KiCad's interactive
router and DRC hold the rules tools/route_checks.py tests (tools/domains.py).
  - net classes: ESTOP (the E-stop safety list), HV54 (the 54 V list), P12 (12 V distribution, 0.6 mm), and GND / 54 V
    taps at 0.25 mm; everything else Default 0.2/0.2;
  - E-stop rule in the .kicad_dru: 1.5 mm from ESTOP to HV54 copper on every layer, ahead of the fine-pitch courtyard
    rules (a later rule wins in KiCad, so inside U1/U2/U7's courtyards their 0.15 mm still applies);
  - the 54 V domain rule (0.6 mm outer, uncoated) stays in tools/route_checks.py: coated it is the 0.2 mm default.
Run with plain Python: python tools/handfinish_prep.py <project.kicad_pro> <rules.kicad_dru> <netlist.net>"""
import json,os,re,sys
sys.path.insert(0,os.path.dirname(os.path.abspath(__file__))); import domains
pro,dru,netf=sys.argv[1:4]
nets=sorted(set(re.findall(r'\(name "(/[^"]*)"\)',open(netf,encoding='utf-8').read())))
P12=['/+12V_RAW','/+12V','/+12V_JET','/+12V_PI','/SPARE_SW','/SPARE_ON','/SP_SW1']
d=json.load(open(pro)); ns=d['net_settings']
base=dict(ns['classes'][0])
def cls(name,w,cl,color):
    c=dict(base); c.update(name=name,track_width=w,clearance=cl,pcb_color=color,priority=len(ns['classes'])); return c
ns['classes']=[ns['classes'][0]]+[cls('ESTOP',0.2,0.2,'rgb(255, 0, 255)'),cls('HV54',0.25,0.2,'rgb(255, 64, 0)'),cls('P12',0.6,0.2,'rgb(255, 200, 0)')]
pat=[]
for n in nets:
    if domains.is_estop(n): pat.append(dict(netclass='ESTOP',pattern=n))
    elif domains.is_hv(n): pat.append(dict(netclass='HV54',pattern=n))
    elif n in P12: pat.append(dict(netclass='P12',pattern=n))
ns['netclass_patterns']=pat; ns['netclass_assignments']=None
json.dump(d,open(pro,'w'),indent=2)
s=open(dru).read()
RULE="""# E-stop safety nets keep 1.5 mm to 54 V copper on every layer (LAYOUT_RULES_REVB.md, tools/domains.py); the fine-pitch
# rules below come later in this file, so they win inside U1/U2/U7's courtyards
(rule estop_to_54v
  (constraint clearance (min 1.5mm))
  (condition "A.hasNetclass('ESTOP') && B.hasNetclass('HV54')"))
"""
if 'estop_to_54v' not in s:
    k=s.find('(rule '); s=s[:k]+RULE+s[k:]
open(dru,'w').write(s)
print('classes',[c['name'] for c in ns['classes']],'patterns',len(pat))
