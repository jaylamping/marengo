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
MOD='''
.model DTVS D(IS=1e-12 N=1 RS=0.55 BV=64.4 IBV=1m)
.model DSCH D(IS=50u N=1.1 RS=0.01 BV=100 IBV=1u)
'''
def sim(net,probes,tend):
    out=os.path.join(tempfile.gettempdir(),'abs_sim.dat').replace('\\','/')
    if os.path.exists(out): os.remove(out)
    deck=net+MOD+f'.control\nset filetype=ascii\ntran 5n {tend} 0 5n uic\nwrdata {out} {probes}\ndestroy all\n.endc\n.end\n'
    p=os.path.join(tempfile.gettempdir(),'abs_sim.cir'); open(p,'w').write(deck)
    ng.ngSpice_Command(b'remcirc'); ng.ngSpice_Command(('source '+p.replace('\\','/')).encode())
    return [[float(x) for x in l.split()] for l in open(out) if l.strip()]
def trip(L,C,esr,t_open,t_off=2e-6,r_loop=0.1,rpre=0):
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
    r=sim(net,'v(bat) i(ll)',f'{t_open+t_off+40e-6:.3e}')
    return max(x[1] for x in r),max(x[3] for x in r)
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
