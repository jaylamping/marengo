"""Net classes for the rev B5 spacing rules (LAYOUT_RULES_REVB.md), shared by tools/grid_route.py (plain Python) and
tools/route_checks.py / hv_spacing.py / gnd_fanout.py (KiCad's Python), so the router routes to the rules the checks test.
  - 54 V copper (HV): LAYOUT_RULES_REVB.md's list (BAT, SW_D, MID, VBUS, ABS, CL_D, BRK_*, SH*, V_L*) plus ABS_N and the
    port drains D_L*. The latch's own supply chain (BAT_F, BAT_R, BAT_CTL) and the divider taps are not in it.
  - E-stop safety nets (ESTOP): keep >= 1.5 mm to 54 V copper (the HV list) on every layer, 0.6 mm (outer) to copper
    riding on 54 V (0.44 MOhm of leakage into UVLO enables bank
    B by itself).
  - Voltage domains: copper that rides on one 54 V node (a port's gate and bootstrap on its V_L, sense taps on VBUS, bank
    B's gate on MID, ...). Pairs inside one domain are tens of volts apart at most, so the 0.6 mm rule (outer layers,
    IPC-2221B B2, 51-100 V) applies only between different domains where one side is 54 V.
"""
import re
ESTOP_NAMES='RUN RUN_A RUN_B RUN_P UVLO EN_A FLT_A FLT_D L_B L_E L_PB L_PC MID_B VB_B INT_B WDO_N VWD'.split()
def is_estop(n): return n in {'/'+x for x in ESTOP_NAMES} or bool(re.match(r'/(LOOP_|RST_)',n))
def is_hv(n):
    return bool(re.match(r'/(BAT|SW_D|MID|VBUS|ABS|ABS_N|CL_D)$|/(BRK_|SH|V_L|D_L)',n)) and not is_estop(n) and not n.startswith('/SH12')
DOMRE=[('BAT',r'/(BAT|SW_D|SH_[PN]|SH9_[PN]|BRK_D|CSP_A|CSN_A|VS_A)$'),('MID',r'/(MID|GA1|GA2|GATE_A|BST_A|G1PD_A)$'),
       ('VBUS',r'/(VBUS|D_L\d|CSP_L\d|INP_L\d|SHL\d_[PN]|G1|G2|GATE|SHSP_[PN])$'),('ABS',r'/ABS$'),('CLD',r'/CL_D$'),
       ('BRK',r'/BRK_(F|P|T|PG|IN|GATE|G1)$')]+[(f'VL{k}',rf'/(V|G|GPD|GPU|BST|GC)_L{k}$') for k in range(1,7)]
def dom(n):
    """domain name of a net, or None for ground-referenced copper"""
    for name,rx in DOMRE:
        if re.match(rx,n): return name
    return None
def rides(n):
    """54 V copper, or copper riding on a 54 V node (a gate or bootstrap on its source, a sense tap on VBUS)"""
    return is_hv(n) or dom(n) is not None
import os
# The board gets an acrylic conformal coat after bring-up (Joey, 2026-11-09; IPC-2221B Table 6-1, 51-100 V: A5/A7 0.13 mm,
# B4 0.13 mm, against B2 0.6 mm and A6 0.5 mm uncoated), so the 0.6 mm domain rule is the 0.2 mm default. The E-stop
# 1.5 mm rule and the hole keep-outs don't change. MARENGO_COATED=0 checks the bare-board rule.
COATED=os.environ.get('MARENGO_COATED','1')=='1'
def clearance(a,b,outer):
    """required copper clearance (mm) between nets a and b on an outer (True) or inner layer"""
    if a==b: return 0.0
    if (is_estop(a) and is_hv(b)) or (is_estop(b) and is_hv(a)): return 1.5   # the rule's net list; 54 V-riding nets get 0.6 below
    if (rides(a) or rides(b)) and not (dom(a) and dom(a)==dom(b)): return 0.6 if (outer and not COATED) else 0.2
    return 0.2
