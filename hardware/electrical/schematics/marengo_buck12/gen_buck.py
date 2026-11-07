"""12 V, 12 A buck board (rev B5): replaces the Murata DVE on the -X rail and plugs into the same J24 pigtail.
Edit this, not the .kicad_sch. Run: python -X utf8 gen_buck.py <this folder>, then ERC (tools/regen_buck.sh).
Starting point: TI LM5146 datasheet SNVSBV0A, application circuit 2 (48 V to 12 V, 8 A, 400 kHz): same L, C and
compensation, uprated FETs and inductor for 12 A, our UVLO, an input filter and the J24 interface."""
import os,sys,textwrap
sys.path.insert(0,os.path.join(os.path.dirname(os.path.abspath(__file__)),'..','marengo_pdu_revb','gen'))
import kit
kit.PROJ='marengo_buck12'; kit.LIBNAME='marengo_buck12'
from kit import *
LIBN=kit.LIBNAME
wfile(OUT+'/sym-lib-table').write('(sym_lib_table\n\t(version 7)\n\t(lib (name "marengo_buck12")(type "KiCad")(uri "${KIPRJMOD}/marengo_buck12.kicad_sym")(options "")(descr "buck board symbols"))\n)\n')
s=Sheet('buck12','marengo 12 V buck board (rev B5)','A2',comment='Replaces the DVE0125V2NBC on J24. Main board: hardware/electrical/schematics/marengo_pdu_revb.')
s.file='marengo_buck12.kicad_sch'
def head(t,at): s.text(t,at,size=2,bold=True)
def wnote(t,at,w): s.text('\n'.join(textwrap.fill(l,w) for l in t.split('\n')),at,size=1.27)
def Q(lid,val,at,b,e,c): r=s.nref('Q'); nic(s,lid,r,val,at,SOT23,{'1':b,'2':e,'3':c}); return r
mkic('LM5146',[('1','EN/UVLO','input'),('2','RT','passive'),('3','SS/TRK','passive'),('4','COMP','output'),('5','FB','input'),('6','AGND','power_in'),('7','SYNCOUT','output'),('8','SYNCIN','input'),('9','NC','passive'),('10','PGOOD','open_collector'),('11','ILIM','passive'),('12','PGND','power_in'),('13','LO','output'),('14','VCC','passive'),('15','EPC','passive'),('16','NC','passive'),('17','BST','passive'),('18','HO','output'),('19','SW','passive'),('20','VIN','power_in'),('21','EP','passive')],
     'Package_DFN_QFN:Texas_RGY_R-PVQFN-N20_EP2.05x3.05mm_ThermalVias','https://www.ti.com/lit/ds/symlink/lm5146.pdf','100 V synchronous buck controller, voltage mode with line feed-forward, 7.5 V drivers')
head('12 V buck: LM5146, 48 V to 12 V, 12 A, 400 kHz',(20,20))
nic(s,'Connector_Generic:Conn_02x06_Odd_Even','J1','J24 mate: 1-2 +VIN, 3-4 GND, 5 ON/OFF (low = on), 6 GND, 7-9 GND, 10-12 +12 V',(40,70),'Connector_Molex:Molex_Micro-Fit_3.0_43045-1212_2x06_P3.00mm_Vertical',
    {'1':'VIN_P','2':'VIN_P','3':'GND','4':'GND','5':'ON_N','6':'GND','7':'GND','8':'GND','9':'GND','10':'VOUT','11':'VOUT','12':'VOUT'})
g=Grid(s,80,50,17,30,300)
g.D('SK810L-TP','VIN_P','GND',SMC); g.C('1uF 100V X7R','VIN_P','GND',C1210); g.L('2.2uH XAL7030-222MEB','VIN_P','VIN','Inductor_SMD:L_Coilcraft_XAL7030-222')
g.CP('47uF 100V EEE-TK2A470AQ','VIN','GND','Capacitor_SMD:CP_Elec_10x10'); g.C('2.2uF 100V X7R','VIN','GND',C1210,k=5); g.C('100nF 100V X7R','VIN','GND',C603)
g.F('VIN_P'); g.F('VIN'); g.F('GND')
nic(s,LIBN+':LM5146','U1','LM5146RGYR',(120,140),'Package_DFN_QFN:Texas_RGY_R-PVQFN-N20_EP2.05x3.05mm_ThermalVias',
    {'1':'EN','2':'RT','3':'SS','4':'COMP','5':'FB','6':'GND','7':None,'8':'GND','9':None,'10':None,'11':'ILIM','12':'GND','13':'LO','14':'VCC','15':'GND','16':None,'17':'BST','18':'HO','19':'SW','20':'VIN_C','21':'GND'},ds='https://www.ti.com/lit/ds/symlink/lm5146.pdf')
nfet5(s,'Q1','BSC070N10NS5ATMA1',(200,120),'VIN','HO','SW')
nfet5(s,'Q2','BSC040N10NS5ATMA1',(200,160),'SW','LO','GND'); s.refc['Q']=2
g=Grid(s,230,100,17,30,330)
g.L('6.8uH XAL1010-682MED','SW','VOUT','Inductor_SMD:L_Coilcraft_XAL1010-XXX'); g.C('22uF 25V X7R','VOUT','GND',C1210,k=5); g.F('VOUT')
g=Grid(s,160,190,15,30,330)
g.R('2.2R','VIN','VIN_C',R603); g.C('100nF 100V X7R','VIN_C','GND',C603); g.F('VIN_C'); g.R('300k 1%','VIN','EN',R1206); g.R('12.4k 1%','EN','GND',R603)
g.R('24.9k 1%','RT','GND'); g.C('47nF X7R','SS','GND'); g.C('2.2uF 25V X7R','VCC','GND',C805); g.D('BAT46W','VCC','VOUT')
g.nl(); g.R('2.2R','SW','BST_R',R603); g.C('100nF 25V X7R','BST','BST_R',C603); g.R('402R 1%','ILIM','SW'); g.C('100pF C0G','ILIM','GND')
g.R('0R (loop injection)','VOUT','VOUT_S',R603); g.R('21k 1%','VOUT_S','FB'); g.R('1.5k 1%','FB','GND')
g.nl(); g.R('100R','VOUT_S','FB_Z'); g.C('1nF C0G','FB_Z','FB'); g.R('8.06k 1%','COMP','COMP_Z'); g.C('4.7nF C0G','COMP_Z','FB',C603); g.C('10pF C0G','COMP','FB')
g.R('220k','VIN','ON_N',R1206); g.R('10k','ON_N','ON_B')
Q('Transistor_BJT:Q_NPN_BEC','MMBT3904',(320,250),'ON_B','GND','EN')
wnote('''Input: SK810L crowbar (a reversed pigtail clamps at about 1 V; U7 on the main board limits and times out), 1 uF at the connector, 2.2 uH filter, 47 uF aluminium (0.42 Ohm ESR damps the filter and the pigtail), 5 x 2.2 uF hot-loop ceramics at Q1. tools/absorber_sim.py (main board) runs the bank-trip spike through U7, F2, the pigtail and this filter.
UVLO: 300k / 12.4k: on at 30.2 V, off at 27.2 V (10 uA hysteresis current). Below the 30.5 V worst sag at 90 A from a 38 V pack, so motor load can't brown out compute before the BMS does; the Pi still shuts down on its load-aware threshold.
ON/OFF: the main board's 2N7002 pulls pin 5 low through 220R when U7's PGD is good. Pin 5 open: 220k from VIN biases the NPN, which holds EN low (off). Pin 5 low: NPN off, EN rises: on. Pin 5 idles at about 2.7 V, like the DVE's.
Power train (TI design 2 uprated): 400 kHz (RT 24.9k), 6.8 uH 8.9 mOhm 21.8 A Isat, 3.3 A ripple; Q1 BSC070N10NS5 (low Qgd), Q2 BSC040N10NS5. Valley limit by Q2 RDS(on): 200 uA x 402R / 4.5 mOhm = 18 A cold, about 12-13 A hot: over the 11.6 A ceiling, under the 19-21 A of the inductor. Hiccup on overload.
Output: 12.0 V (0.8 V x (1 + 21k / 1.5k)), 5 x 22 uF here, plus 330 uF + 2 x 22 uF on the main board at the far end of the pigtail. Compensation is TI's for this L and C (40 kHz crossover). The pigtail and 330 uF resonate near 20 kHz: measure the loop with an injection resistor in place of the 0R before trusting it. VCC from VOUT through BAT46W (TI's D1) cuts bias loss.
Loss at 12 A, 48 V (est): Q1 0.4 W conduction + 1.2 W switching, Q2 0.7 W, inductor 1.3 W, controller 0.3 W: about 4 W, 97 %. Board: 2 layers, 2 oz, about 45 x 35 mm, bolted to the rail with a thermal pad like the DVE.''',(20,280),170)
writelib()
s.write([('/'+kit.ROOT,'1')])
print('ok',{k:v for k,v in s.refc.items() if not k.startswith('#')})
