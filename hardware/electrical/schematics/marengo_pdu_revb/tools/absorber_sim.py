#!/usr/bin/env python3
"""Battery-side transient checks for rev B3 (B2 absorber, M3 BMS-close surge). KiCad's ngspice.dll, shared API.
Usage: python tools/absorber_sim.py [ngspice.dll]
Model: pack EMF 54.6 V, pack + BMS + wiring resistance R_LOOP, lead inductance L, then BAT with D1 (5 kW TVS) and the
absorber (D7 into C38 with ESR, optional low-side precharge resistor). A hard bus short sits behind the banks; the banks
are a conductance that falls to zero over T_OFF starting at T_OPEN, the time the trip logic needs after the short starts.
No pack exists yet, so L and R_LOOP are assumptions: 0.5 uH is the planning value, 1 uH the pessimistic one."""
import ctypes,os,sys,tempfile
DLL=sys.argv[1] if len(sys.argv)>1 else 'C:/Program Files/KiCad/10.0/bin/ngspice.dll'
os.add_dll_directory(os.path.dirname(DLL)); ng=ctypes.CDLL(DLL)
OUT=ctypes.CFUNCTYPE(ctypes.c_int,ctypes.c_char_p,ctypes.c_int,ctypes.c_void_p)
NUL=ctypes.CFUNCTYPE(ctypes.c_int,ctypes.c_int,ctypes.c_bool,ctypes.c_bool,ctypes.c_int,ctypes.c_void_p)
pf=OUT(lambda s,i,u:0); ef=NUL(lambda *a:0); ng.ngSpice_Init(pf,pf,ef,None,None,None,None)
MODT='''
.model DTVS D(IS=1e-12 N=1 RS=0.55 BV={bv} IBV=1m)
.model DSCH D(IS=50u N=1.1 RS={rd} BV=100 IBV=1u)
.model DAVL D(IS=1e-12 N=1 RS=0.02 BV=110 IBV=1m)
'''
MOD=MODT.format(bv=64.4,rd=0.01)
def sim(net,probes,tend):
    out=os.path.join(tempfile.gettempdir(),'abs_sim.dat').replace('\\','/')
    if os.path.exists(out): os.remove(out)
    deck=net+MOD+f'.control\nset filetype=ascii\ntran 5n {tend} 0 5n uic\nwrdata {out} {probes}\ndestroy all\n.endc\n.end\n'
    p=os.path.join(tempfile.gettempdir(),'abs_sim.cir'); open(p,'w').write(deck)
    ng.ngSpice_Command(b'remcirc'); ng.ngSpice_Command(('source '+p.replace('\\','/')).encode())
    return [[float(x) for x in l.split()] for l in open(out) if l.strip()]
def trip(L,C,esr,t_open,t_off=2e-6,r_loop=0.1,rpre=0,bv=64.4,rd=0.01,conv=False):
    global MOD; MOD=MODT.format(bv=bv,rd=rd)
    # pre-charged absorber (IC) and a short on the bus at t=0
    pre=f'RPRE CN 0 {rpre}\n' if rpre else 'VPRE CN 0 0\n'
    net=f'''trip
VB EMF 0 54.6
RL EMF L1 {r_loop}
LL L1 BAT {L} IC=0
D1 0 BAT DTVS
D7 BAT ABS DSCH
RESR ABS C1 {esr}
C38 C1 CN {C} IC=54.4
{pre}BSW BAT BUS I=V(BAT,BUS)*500*max(0,min(1,1-(time-{t_open})/{t_off}))
RSH BUS 0 1m
'''
    if conv: net+=CONV
    r=sim(net,'v(bat) i(ll)'+(' v(brk_in) v(vin)' if conv else ''),f'{t_open+t_off+60e-6:.3e}'); MOD=MODT.format(bv=64.4,rd=0.01)
    if conv: return max(x[1] for x in r),max(x[3] for x in r),max(x[5] for x in r),max(x[7] for x in r)
    return max(x[1] for x in r),max(x[3] for x in r)
# rev B5: the 12 V buck's input, ahead of the E-stop: U7 path (RS9 + Q7), BRK_IN caps (2 x 2.2 uF derated to 2.2 uF), F2,
# the J24 pigtail (150 nH, 15 mOhm both ways), the buck board's 1 uF, 2.2 uH filter, 47 uF aluminium (0.42 Ohm) and
# 5 x 2.2 uF ceramics (about 5 uF at 55 V), loaded by the buck as about 158 W of constant power (144 W at 91 %).
CONV='''RU7 BAT BRK_IN 7m
CBI BRK_IN 0 2.2u IC=54.4
RF2 BRK_IN P1 18m
LPIG P1 VIN_P 150n IC=2.9
CVP VIN_P 0 1u IC=54.4
LF VIN_P V2 2.2u IC=2.9
RLF V2 VIN 6m
CCER VIN 0 5u IC=54.4
RAL VIN A1 0.42
CAL A1 0 47u IC=54.4
BLOAD VIN 0 I=158/max(V(VIN),20)
'''
def close(L,C,esr,rpre=0,r_loop=0.1):
    # BMS closes onto a discharged absorber: peak current through D7
    pre=f'RPRE CN 0 {rpre}\n' if rpre else 'VPRE CN 0 0\n'
    net=f'''close
VB EMF 0 PWL(0 0 100n 54.6)
RL EMF L1 {r_loop}
LL L1 BAT {L} IC=0
D1 0 BAT DTVS
D7 BAT ABS DSCH
RESR ABS C1 {esr}
C38 C1 CN {C} IC=0
{pre}'''
    r=sim(net,'v(bat) i(ll)','200u')
    return max(x[1] for x in r),max(x[3] for x in r)
print('Bank trip into a hard bus short (absorber pre-charged). T_OPEN: A with C29 off and 22R is about 8.8 us; B breaker about 10 us.')
print(f"{'L':>6} {'C':>6} {'ESR':>5} {'t_open':>7} | {'peak BAT':>9} {'I cut':>7}")
for L in (0.5e-6,1e-6):
    for C,esr in ((47e-6,0.05),(94e-6,0.025)):
        for t in (8.8e-6,10e-6):
            v,i=trip(L,C,esr,t); print(f'{L*1e6:5.1f}u {C*1e6:5.0f}u {esr*1e3:4.0f}m {t*1e6:6.1f}u | {v:8.1f}V {i:6.0f}A')
print('\nBMS close onto a discharged absorber (peak through D7):')
for L in (0.5e-6,1e-6):
    for C,esr in ((47e-6,0.05),(94e-6,0.025)):
        for rp in (0,0.5,2.0):
            v,i=close(L,C,esr,rp); print(f'L {L*1e6:.1f}u C {C*1e6:.0f}u series R {rp:.1f} ohm: peak {i:6.0f} A, BAT {v:5.1f} V')
print('\nSame trip with that series R left in the branch (what a plain R or cold NTC costs):')
for rp in (0.5,2.0):
    v,i=trip(0.5e-6,47e-6,0.05,8.8e-6,rpre=rp); print(f'L 0.5u C 47u series R {rp} ohm: peak BAT {v:.1f} V')
print('\nBuck input during a bank trip (rev B5, LM5146 VIN and FETs 100 V abs max), 3 cans -20 %, D1 at 71.2 V, D7 RS 10 and 30 mOhm:')
print(f"{'L':>5} {'Rloop':>5} {'t_open':>6} {'D7 RS':>6} | {'BAT':>6} {'BRK_IN':>7} {'buck VIN':>8}")
for L in (0.5e-6,1e-6):
    for rl in (0.1,0.05):
        for t in (8.8e-6,17.5e-6):
            for rd in (0.01,0.03):
                v,i,vb,vi=trip(L,112.8e-6,0.0167,t,r_loop=rl,bv=71.2,rd=rd,conv=True)
                print(f'{L*1e6:4.1f}u {rl:5.2f} {t*1e6:5.1f}u {rd*1e3:4.0f}m | {v:5.1f}V {vb:6.1f}V {vi:7.1f}V')
print('\nCorners (rev B4 review): pulse loop R 0.05 Ohm (cell AC resistance), D1 at max BV 71.2 V, C38 -20 %, D7 RS 3 mOhm..10 mOhm,')
print('T_OPEN 8.8 / 13 / 17.5 us (U8 delay counts from threshold crossing, which itself takes 2-8 us).')
print(f"{'L':>5} {'Rloop':>5} {'C':>6} {'BV':>5} {'t_open':>6} | {'peak BAT':>8} {'I cut':>6}")
for L in (0.5e-6,1e-6):
    for rl in (0.1,0.05):
        for C,esr,lab in ((37.6e-6,0.05,'1x-20%'),(75.2e-6,0.025,'2x-20%'),(112.8e-6,0.0167,'3x-20%')):
            for t in (8.8e-6,13e-6,17.5e-6):
                v,i=trip(L,C,esr,t,r_loop=rl,bv=71.2)
                print(f'{L*1e6:4.1f}u {rl:5.2f} {lab:>6} {71.2:5.1f} {t*1e6:5.1f}u | {v:7.1f}V {i:5.0f}A')
print('\nD7 series resistance sweep (r5 review: the SK810L curve at hundreds of amps is unread), 3 cans -20 %, D1 71.2 V, 0.05 Ohm:')
for L in (0.5e-6,1e-6):
    for t in (8.8e-6,17.5e-6):
        print(f'{L*1e6:4.1f}u t_open {t*1e6:4.1f}u | '+'  '.join(f'RS {rd*1e3:.0f}m: {trip(L,112.8e-6,0.0167,t,r_loop=0.05,bv=71.2,rd=rd)[0]:5.1f}V' for rd in (0.01,0.02,0.03)))

def worst(C,esr,rd):
    w=0
    for L in (0.5e-6,1e-6):
        for rl in (0.1,0.05):
            for t in (8.8e-6,13e-6,17.5e-6):
                w=max(w,trip(L,C,esr,t,r_loop=rl,bv=71.2,rd=rd)[0])
    return w
print('\nRev B5 absorber choice (Joey 16:46Z: commodity parts, 2+ sources, JLC-placeable). Worst of all corners (L 0.5/1 uH, R 0.05/0.1,')
print('t_open 8.8/13/17.5 us, D1 at 71.2 V), two D7 in parallel at 5 and 15 mOhm:')
print('  APZ1012470M100R x3 (baseline, obsolete), 47 uF 100 V -20 %, 50 mOhm:',' '.join(f'{worst(112.8e-6,0.05/3,rd):5.1f}V' for rd in (0.005,0.015)))
for n in (4,5,6,8):
    print(f'  EEH-ZC1K470P x{n} (fitted: 8), 47 uF 80 V -20 %, 45 mOhm aged:',' '.join(f'{worst(n*37.6e-6,0.045/n,rd):5.1f}V' for rd in (0.005,0.015)))
for n in (80,120):
    print(f'  {n} x 1210 4.7 uF 100 V X7R, about 1.0 uF each at 55-90 V bias, 5 mOhm:',f'{worst(n*1.0e-6,0.005/n,0.005):5.1f}V')
for n in (4,8):
    print(f'  {n} x 100 uF 100 V low-ESR aluminium (150 mOhm, 12.5 mm cans):',f'{worst(n*80e-6,0.15/n,0.005):5.1f}V')
print('BMS close onto eight discharged cans (the 10b decision rides on this bench test):')
for L in (0.5e-6,1e-6): print(f'  L {L*1e6:.1f}u: peak {close(L,300.8e-6,0.0056)[1]:5.0f} A')

print('\nRev B5 BMS swap (JK -> JBD/Daly): how long the BMS close onto 8 cans stays above short-circuit levels (JBD delay 200-800 us, Daly 200-500 us):')
for C,esr in ((8*37.6e-6,0.045/8),(8*47e-6,0.036/8)):
    for L in (0.5e-6,1e-6):
        for rl in (0.05,0.1):
            r=sim(f'''close
VB EMF 0 PWL(0 0 100n 54.6)
RL EMF L1 {rl}
LL L1 BAT {L} IC=0
D1 0 BAT DTVS
D7 BAT ABS DSCH
RESR ABS C1 {esr}
C38 C1 CN {C} IC=0
VPRE CN 0 0
''','v(bat) i(ll)','200u')
            def dur(th):
                ts=[x[0] for x in r if x[3]>th]
                return (max(ts)-min(ts))*1e6 if ts else 0
            print(f'  C {C*1e6:3.0f}u L {L*1e6:.1f}u R {rl:.2f}: peak {max(x[3] for x in r):4.0f} A; us above 180/300 A: {dur(180):4.1f} {dur(300):4.1f}')

# rev B5 layout stage 2: a real turn-off. The bank's current falls linearly over tf (50-200 ns) instead of a conductance
# ramp, the absorber branch has its own inductance LABS (D7/D21 bodies, can ESL, comb copper: 3-5 nH estimated from
# the routed board), LBAR is the copper from the absorber node to the bank A drain, CBAT the pack-entry ceramics
# (4 x 4.7 uF 100 V, about 8.8 uF at 54 V bias) and CDR the bank A drain ceramics (C87/C88, about 4.4 uF). The bank
# FETs avalanche at about 110 V (DAV).
def rnet(L,C,esr,rl,lbar,labs,cdr,cbat,sw):
    return f"""t
VB EMF 0 54.6
RL EMF L1 {rl}
LL L1 BAT {L} IC=0
D1 0 BAT DTVS
D7 BAT ABS DSCH
RESR ABS CA {esr}
LABS CA C1 {labs} IC=0
C38 C1 0 {C} IC=54.4
LCB BAT CB1 1n IC=0
CBAT CB1 0 {cbat} IC=54.6
LBAR BAT BX {lbar} IC=0
LDR BX DX 0.5n IC=0
CDR DX 0 {cdr} IC=54.6
DAV BUS BX DAVL
{sw}
RSH BUS 0 1m
"""
def rtrip(L,C,esr,t_open,tf,rl,lbar,labs,cdr,cbat):
    global MOD; MOD=MODT.format(bv=71.2,rd=0.005)
    r=sim(rnet(L,C,esr,rl,lbar,labs,cdr,cbat,'BSW BX BUS I=V(BX,BUS)*500'),'i(lbar)',f'{t_open:.3e}'); i0=r[-1][1]
    sw=f'BSW BX BUS I=min(V(BX,BUS)*500,max(0,{i0*1.02}*(1-(time-{t_open})/{tf})))'
    r=sim(rnet(L,C,esr,rl,lbar,labs,cdr,cbat,sw),'v(bat) i(ll) v(bx)',f'{t_open+tf+60e-6:.3e}')
    MOD=MODT.format(bv=64.4,rd=0.01)
    return max(x[1] for x in r),max(x[5] for x in r)
print();print('Rev B5 layout: real turn-off (tf), absorber loop LABS, pack-entry ceramics CBAT, bank A drain ceramics CDR; 8 cans, all corners:')
for labs in (3e-9,5e-9):
    for cbat,cdr in ((1e-12,4.4e-6),(8.8e-6,4.4e-6),(8.8e-6,8.8e-6)):
        w=wd=0
        for tf in (50e-9,200e-9):
            for L in (0.5e-6,1e-6):
                for rl in (0.1,0.05):
                    for t_open in (8.8e-6,17.5e-6):
                        try: v,vd=rtrip(L,8*37.6e-6,0.045/8,t_open,tf,rl,2e-9,labs,cdr,cbat); w=max(w,v); wd=max(wd,vd)
                        except Exception: pass
        print(f'  LABS {labs*1e9:.0f} nH, CBAT {cbat*1e6:3.1f} uF, CDR {cdr*1e6:3.1f} uF: BAT {w:5.1f} V, bank A drain {wd:5.1f} V'+('   <- fitted' if (cbat,cdr)==(8.8e-6,4.4e-6) else ''))
# extracted from the routed copper by tools/absorber_loop.py (pass LABS_NH / LBAR_NH to override)
import os
labs=float(os.environ.get('LABS_NH','5.4'))*1e-9; lbar=float(os.environ.get('LBAR_NH','2.0'))*1e-9
w=wd=0
for tf in (50e-9,200e-9):
    for L in (0.5e-6,1e-6):
        for rl in (0.1,0.05):
            for t_open in (8.8e-6,17.5e-6):
                try: v,vd=rtrip(L,8*37.6e-6,0.045/8,t_open,tf,rl,lbar,labs,4.4e-6,8.8e-6); w=max(w,v); wd=max(wd,vd)
                except Exception: pass
print(f'\nExtracted loop: LABS {labs*1e9:.2f} nH, LBAR {lbar*1e9:.2f} nH, fitted ceramics, all corners: BAT {w:5.1f} V, bank A drain {wd:5.1f} V')
