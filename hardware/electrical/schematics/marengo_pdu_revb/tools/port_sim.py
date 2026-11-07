#!/usr/bin/env python3
"""Limb port breaker coordination (rev B5). KiCad's ngspice.dll, shared API. Usage: python tools/port_sim.py [ngspice.dll]
Question: for a hard short on one limb port, does that port's TPS48111 open before U1's breaker (160 A min, 0.44 us min
response) or U8's comparator (158 A min) sees its threshold on RS1? And what does the port FET take?
Model: pack EMF 54.6 V, loop R, lead L into BAT; RS1 + both banks (on) into VBUS; the bus caps with ESR and ESL; the
other limbs as a constant load (their drive caps left out, which is pessimistic: they would also feed the short);
the faulted port: copper, 2 mOhm shunt, FET as a switch, SK810L clamp to GND on the port output, the 20 A pigtail
fuse, then wiring L and R into a dead short at t = 0.
Port trip: TPS48111-Q1 short-circuit comparator at the top of its band (RISCP 4.64k: 35-45 A on 2 mOhm), tSC max
1.6 us to PD low, then PD (4 A peak) empties the gate to the plateau and through Miller (gate charge from the FET
datasheets, max values; the slew cap is behind a diode, so PD doesn't see it). The 'slow' rows use a 0.25 A sink
to show how much the strong pull-down matters.
Two passes: the first finds when the port current crosses 87 A, the second opens the switch on that schedule."""
import ctypes,os,sys,tempfile
DLL=sys.argv[1] if len(sys.argv)>1 else 'C:/Program Files/KiCad/10.0/bin/ngspice.dll'
if hasattr(os,'add_dll_directory'): os.add_dll_directory(os.path.dirname(DLL))
ng=ctypes.CDLL(DLL)
OUT=ctypes.CFUNCTYPE(ctypes.c_int,ctypes.c_char_p,ctypes.c_int,ctypes.c_void_p)
NUL=ctypes.CFUNCTYPE(ctypes.c_int,ctypes.c_int,ctypes.c_bool,ctypes.c_bool,ctypes.c_int,ctypes.c_void_p)
pf=OUT(lambda s,i,u:0); ef=NUL(lambda *a:0); ng.ngSpice_Init(pf,pf,ef,None,None,None,None)
MOD='.model DSCH D(IS=50u N=1.1 RS=10m BV=100 IBV=1u)\n'
def sim(net,probes,tend):
    out=os.path.join(tempfile.gettempdir(),'port_sim.dat').replace('\\','/')
    if os.path.exists(out): os.remove(out)
    deck=net+MOD+f'.control\nset filetype=ascii\ntran 2n {tend} 0 2n uic\nwrdata {out} {probes}\ndestroy all\n.endc\n.end\n'
    p=os.path.join(tempfile.gettempdir(),'port_sim.cir'); open(p,'w').write(deck)
    ng.ngSpice_Command(b'remcirc'); ng.ngSpice_Command(('source '+p.replace('\\','/')).encode())
    return [[float(x) for x in l.split()] for l in open(out) if l.strip()]
NCAP,CAP,ESR,ESL=5,47e-6,0.045,3e-9   # 5 x EEH-ZC1K470P, ESR 36 mOhm + 25 % for ageing and temperature; port shunt 2 mOhm (rev B5)
I_CB=0.0906/2e-3; T_CB=1.6e-6
FET={'IPB017N10N5LF':dict(ron=1.7e-3*1.75,qpl=120e-9,qgd=51e-9),'BSC040N10NS5':dict(ron=4.0e-3*1.75,qpl=46e-9,qgd=17e-9)}
def run(fet,lw,rw,ioth,lp,rl,helper,ts=1.0):
    f=FET[fet]; isink=4.0 if helper else 0.25
    tpl=f['qpl']/isink; tm=max(f['qgd']/isink,30e-9)
    v0=54.6-ioth*(rl+2.4e-3)
    caps=''.join(f'RE{k} BUS E{k} {ESR}\nLE{k} E{k} F{k} {ESL} IC=0\nC{k} F{k} 0 {CAP} IC={v0}\n' for k in range(NCAP))
    net=f'''port
VB EMF 0 54.6
RL EMF L1 {rl}
LL L1 BAT {lp} IC={ioth}
RRS1 BAT B1 0.5m
RBK B1 B2 1.9m
LCU B2 BUS 5n IC={ioth}
IOTH BUS 0 {ioth}
{caps}LPC BUS P0 10n IC=0
RPC P0 P1 0.3m
RSH P1 P2 2m
BFET P2 VL I=V(P2,VL)*{1/f['ron']}*max(1e-7,min(1,1-(time-{ts})/{tm}))
DCL 0 VL DSCH
RFU VL W1 3.2m
LW W1 W2 {lw} IC=0
RW W2 0 {rw}
'''
    r=sim(net,'i(ll) v(p1) v(p2) v(vl) i(lw)','60u')
    t=[x[0] for x in r]; ipk=[x[1] for x in r]; iport=[(x[3]-x[5])/2e-3 for x in r]
    return t,ipk,iport,[x[5]-x[7] for x in r],tpl,tm,r
def case(fet,lw,rw,ioth,lp,rl,helper):
    t,ipk,iport,_,tpl,tm,_=run(fet,lw,rw,ioth,lp,rl,helper)
    t1=next((a for a,b in zip(t,iport) if b>I_CB),None)
    if t1 is None: return None
    ts=t1+T_CB+tpl
    t,ipk,iport,vds,_,_,_=run(fet,lw,rw,ioth,lp,rl,helper,ts)
    e=sum((t[k+1]-t[k])*vds[k]*iport[k] for k in range(len(t)-1) if t[k]>=ts)
    return t1,ts+tm,max(ipk),max(iport),e
print(__doc__.split('\n')[1]); print(f'Bus caps {NCAP} x {CAP*1e6:.0f} uF, ESR {ESR*1e3:.0f} mOhm. U1 breaker 160 A min, U8 comparator 158 A min. Port trip {I_CB:.0f} A + {T_CB*1e6:.1f} us.')
print(f"{'FET':14} {'sink':4} {'Lwire':>6} {'Rwire':>6} {'other':>5} {'Lpack':>5} {'Rloop':>5} | {'cross':>6} {'off':>6} {'I(RS1)':>7} {'I port':>7} {'E FET':>6} verdict")
fails=0
for fet in FET:
    for helper in (False,True):
        for lw,rw in ((50e-9,2e-3),(200e-9,8e-3),(1e-6,30e-3)):
            for ioth in (0,50):
                for lp,rl in ((0.5e-6,0.05),(1e-6,0.1)):
                    c=case(fet,lw,rw,ioth,lp,rl,helper)
                    if c is None: print(fet,'no crossing'); continue
                    t1,toff,irs,ip,e=c; ok=irs<158
                    fails+=not ok
                    print(f"{fet:14} {'4A' if helper else 'slow':4} {lw*1e9:5.0f}n {rw*1e3:5.0f}m {ioth:4.0f}A {lp*1e6:4.1f}u {rl:5.2f} | {t1*1e6:5.2f}u {toff*1e6:5.2f}u {irs:6.0f}A {ip:6.0f}A {e*1e3:5.1f}mJ {'OK' if ok else 'BANK TRIPS'}")
print('cases where a bank threshold is reached:',fails)
