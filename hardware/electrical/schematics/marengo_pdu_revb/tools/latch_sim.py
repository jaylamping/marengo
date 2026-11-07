#!/usr/bin/env python3
"""E-stop latch transient checks (rev B2 M8). Drives KiCad's ngspice.dll through its shared API, no install.
Usage: python tools/latch_sim.py [path to ngspice.dll]. Values follow gen.py blocks 2, 7, 8 and 10; edit both together.
Each case prints the RUN voltage it ends on and PASS or FAIL against what the latch must do."""
import ctypes,os,sys,tempfile
DLL=sys.argv[1] if len(sys.argv)>1 else 'C:/Program Files/KiCad/10.0/bin/ngspice.dll'
os.add_dll_directory(os.path.dirname(DLL)); ng=ctypes.CDLL(DLL)
OUT=ctypes.CFUNCTYPE(ctypes.c_int,ctypes.c_char_p,ctypes.c_int,ctypes.c_void_p)
NUL=ctypes.CFUNCTYPE(ctypes.c_int,ctypes.c_int,ctypes.c_bool,ctypes.c_bool,ctypes.c_int,ctypes.c_void_p)
log=[]
pf=OUT(lambda s,i,u:(log.append(s.decode(errors='replace')),0)[1]); sf=OUT(lambda s,i,u:0); ef=NUL(lambda *a:0)
ng.ngSpice_Init(pf,sf,ef,None,None,None,None)

MODELS='''
.model Q3904 NPN(IS=6.734f XTI=3 EG=1.11 VAF=74.03 BF={bfn} NE=1.259 ISE=6.734f IKF=66.78m XTB=1.5 BR=.7371 NC=2 RC=1 CJC=3.638p MJC=.3085 VJC=.75 FC=.5 CJE=4.493p MJE=.2593 VJE=.75 TR=239.5n TF=301.2p ITF=.4 VTF=4 XTF=2 RB=10)
.model Q3906 PNP(IS=1.41f XTI=3 EG=1.11 VAF=18.7 BF={bfp} NE=1.5 IKF=80m XTB=1.5 BR=4.977 NC=2 RC=2.5 CJC=9.728p MJC=.5776 VJC=.75 FC=.5 CJE=8.063p MJE=.3677 VJE=.75 TR=33.42n TF=179.3p ITF=.4 VTF=4 XTF=6 RB=10)
.model DZ5 D(IS=1n N=1 RS=5 BV=5.1 IBV=5m)
.model DZ10 D(IS=1n N=1 RS=5 BV=10 IBV=5m)
.model DSIG D(IS=2.5n N=1.75 RS=0.6 CJO=2p BV=120)
'''
# loop: S1 is the NC contact (closed unless pressed); S2 the reset NO. Switches as voltage-controlled, 1 mOhm / 1 GOhm.
NET='''latch
VIN VCTL 0 PWL(0 0 {tr} 12.1)
R9 VCTL LOOP_OUT 680
SLOOP LOOP_OUT LOOP_RET CL 0 SW1
VCL CL 0 PWL({loop})
R4K7 VCTL RST_SRC 4.7k
CRST RST_SRC 0 1u
VMID MID 0 DC {mid}
DZ MID_Z MID DZ10
RZ MID_Z MID_B 220k
RZB MID_B 0 220k
Q11 INT_B MID_B 0 Q3904
RINT VCTL INT_B 100k
Q10 RST_SRC INT_B 0 Q3904
VVB VBUS 0 DC {vbus}
DZB VB_Z VBUS DZ5
RZ1 VB_Z VB_B 47k
RZ2 VB_B 0 220k
Q14 RST_SRC VB_B 0 Q3904
SRST RST_SRC RST_IN CR 0 SW1
VCR CR 0 PWL({rst})
R1M RST_IN 0 10k
C2N2 RST_IN RST_C 2.2n
R1K RST_C L_B 1k
Q4 RUN L_PB LOOP_RET Q3906
Q5 L_PC L_B L_E Q3904
* rev B5 watchdog: series NPN in Q5's emitter, base from the 3.3 V watchdog rail through 22k, WDO (switch) pulls it low
VWD VWD 0 DC 3.3
RWD VWD WDO 22k
SWD WDO 0 CW 0 SW1
VCW CW 0 PWL({wdo})
QWD L_E WDO 0 Q3904
R100K LOOP_RET L_PB 100k
R10KA L_PB L_PC 10k
R1K2 RUN 0 1.2k
R10KB RUN L_B 10k
R10KC L_B 0 10k
C10N L_B 0 10n
DB RUN RUN_B DSIG
R22K RUN_B UVLO 22k
R33K UVLO 0 33k
DA RUN RUN_A DSIG
R27 RUN_A EN_A 10k
R28 EN_A 0 100k
R38 RUN SP_EN 100k
R39 SP_EN 0 68k
* rev B5: three fitted limb port breakers on RUN_P (each 22k/10k to EN, 100k to INP)
DP RUN RUN_P DSIG
RENP RUN_P EN_P 7.33k
RENG EN_P 0 3.33k
RINP RUN_P INP_P 33.3k
RINPG INP_P 0 1Meg
.model SW1 SW(VT=0.5 VH=0.1 RON=1m ROFF=1G)
'''+MODELS+'''
.control
set filetype=ascii
tran 2u {tend} 0 2u uic
wrdata {out} v(run) v(l_b) v(uvlo) v(en_a) v(en_p)
destroy all
.endc
.end
'''
def run(tr,loop,rst,tend,bfn=416.4,bfp=180.7,mid=20,vbus=0,wdo='0 0 1 0'):
    out=os.path.join(tempfile.gettempdir(),'latch_sim.dat').replace('\\','/')
    if os.path.exists(out): os.remove(out)
    net=NET.format(tr=tr,loop=loop,rst=rst,tend=tend,out=out,bfn=bfn,bfp=bfp,mid=mid,vbus=vbus,wdo=wdo)
    p=os.path.join(tempfile.gettempdir(),'latch_sim.cir'); open(p,'w').write(net)
    ng.ngSpice_Command(b'remcirc'); ng.ngSpice_Command(('source '+p.replace('\\','/')).encode())
    rows=[[float(x) for x in l.split()] for l in open(out) if l.strip()]
    return rows  # t,run,t,l_b,t,uvlo,t,en_a
def at(rows,t): return min(rows,key=lambda r:abs(r[0]-t))
H=1.0  # 'off' means RUN under 1 V: UVLO then sits under 0.4 V and EN_A under 0.4 V
fails=0
def case(name,ok,info):
    global fails; fails+=not ok; print(('PASS ' if ok else 'FAIL ')+name+': '+info)
CORN=[('hFE typ',416.4,180.7),('hFE low',80,80),('hFE high',600,300)]
for cn,bn,bp in CORN:
    # VCTL can't rise faster than U2's current limit allows: 309 mA max into about 5 uF (10 uF derated) is about 0.2 ms to 12 V.
    # 20 us is a 10x margin on that. Below about 5 us the PNP's junction capacitance kicks RUN without any reset.
    for tr in ('20u','200u','1m','10m'):
        T=float(tr[:-1])*(1e-6 if tr.endswith('u') else 1e-3)
        # 1. power-up, loop closed, reset open: stays off
        r=run(tr,'0 1 1 1','0 0 1 0',f'{T+0.02:.6f}',bn,bp); mx=max(x[1] for x in r)
        case(f'{cn}, ramp {tr}: power-up stays off',mx<H,f'RUN max {mx:.2f} V')
        # 2. power-up with reset held the whole time: stays off (M8)
        r=run(tr,'0 1 1 1','0 1 1 1',f'{T+0.02:.6f}',bn,bp); mx=max(x[1] for x in r)
        case(f'{cn}, ramp {tr}: power-up with reset held stays off',mx<H,f'RUN max {mx:.2f} V, L_B max {max(x[3] for x in r):.2f} V')
    # 3. reset press after power-up closes it; 4. loop open drops it; 5. re-closing the loop does nothing; 6. reset held through a loop cycle does nothing
    t0=0.02
    r=run('1m','0 1 0.06 1 0.0601 0 0.07 0 0.0701 1 1 1',f'0 0 {t0} 0 {t0+1e-4} 1 {t0+0.02} 1 {t0+0.0201} 0 1 0','0.1',bn,bp)
    case(f'{cn}: reset closes it',at(r,0.05)[1]>6.0 and at(r,0.05)[9]>1.25,f'RUN {at(r,0.05)[1]:.2f} V, UVLO {at(r,0.05)[5]:.2f} V, EN_A {at(r,0.05)[7]:.2f} V, port EN {at(r,0.05)[9]:.2f} V (1.2 V max threshold)')
    case(f'{cn}: loop open drops it',at(r,0.065)[1]<H,f'RUN {at(r,0.065)[1]:.2f} V')
    case(f'{cn}: re-closing the loop does nothing',at(r,0.09)[1]<H,f'RUN {at(r,0.09)[1]:.2f} V')
    r=run('1m','0 1 0.04 1 0.0401 0 0.05 0 0.0501 1 1 1','0 0 0.02 0 0.0201 1 1 1','0.1',bn,bp)
    case(f'{cn}: reset held through a loop cycle does nothing',at(r,0.09)[1]<H,f'RUN {at(r,0.09)[1]:.2f} V')
    # M1 interlock: MID under the zener (bank B shorted, bus discharged) blocks reset; MID 17 V (healthy at a 36 V pack) allows it
    for mid,vb,want in ((1,0,False),(8,0,False),(17,0,True),(17,3,True),(17,10,False),(27,27,False)):
        r=run('1m','0 1 1 1',f'0 0 {t0} 0 {t0+1e-4} 1 {t0+0.02} 1 {t0+0.0201} 0 1 0','0.06',bn,bp,mid,vb)
        v=at(r,0.05)[1]; case(f'{cn}: MID {mid} V, VBUS {vb} V, reset '+('closes it' if want else 'is blocked'),(v>6.0) if want else (v<H),f'RUN {v:.2f} V')
    # rev B5 watchdog: a missed heartbeat (WDO low 200 ms) drops a closed latch and it stays off; unfed, reset can't close it
    r=run('1m','0 1 1 1',f'0 0 {t0} 0 {t0+1e-4} 1 {t0+0.02} 1 {t0+0.0201} 0 1 0','0.5',bn,bp,wdo='0 0 0.1 0 0.1001 1 0.3 1 0.3001 0 1 0')
    case(f'{cn}: watchdog released, reset closes it',at(r,0.09)[1]>6.0,f'RUN {at(r,0.09)[1]:.2f} V')
    case(f'{cn}: missed heartbeat drops it',at(r,0.2)[1]<H,f'RUN {at(r,0.2)[1]:.2f} V')
    case(f'{cn}: stays off after WDO releases',at(r,0.45)[1]<H,f'RUN {at(r,0.45)[1]:.2f} V')
    r=run('1m','0 1 1 1',f'0 0 {t0} 0 {t0+1e-4} 1 {t0+0.02} 1 {t0+0.0201} 0 1 0','0.1',bn,bp,wdo='0 1 1 1')
    case(f'{cn}: watchdog low (Pi dead), reset is blocked',max(x[1] for x in r)<H,f'RUN max {max(x[1] for x in r):.2f} V')
print('FAILS',fails)
