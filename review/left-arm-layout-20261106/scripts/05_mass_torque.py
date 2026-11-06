import json,numpy as np
from pathlib import Path
W=Path(r'C:\Users\joeyl\Documents\Codex\2026-10-29\task\LEFT_ARM_V1_LAYOUT_20261106')
import sys;sys.path.insert(0,str(Path(__file__).parent))
from params import *
S=lambda p:np.array([97-p[0],Y0+p[2],Z0+p[1]])   # left source -> STEP
RHO=1.27e-3 # g/mm3 PETG, as in build_layout
rows=[('roll RS03',880,'task supplied',S((-73.54,4.03,.06))),
('upper-arm yaw RS02',405,'task supplied',S((-73.51,6.0,-82.32))),
('elbow RS02',405,'task supplied',np.array([AX+18.7-18.67,EY-.02,AZ])),
('lower-arm yaw RS00',310,'task supplied',np.array([AX,EY-94.2-RS00_OFFSET+20.71,AZ])),
('P1 L01',133724.76*RHO,'CAD volume x PETG 1.27',S((-47.34,19.1,-.2))),
('P2 L04B',140422.79*RHO,'CAD volume x PETG 1.27',S((-70.15,38.47,-40.8))),
('spacer',19642.65*RHO,'CAD volume x PETG 1.27',S((-73.49,40.65,0)))]
M=np.array([r[1] for r in rows]);X=np.array([r[3] for r in rows])
def tq(m,x):
    pitch=np.hypot(x[:,1]-Y0,x[:,2]-Z0);roll=np.hypot(x[:,0]-AX,x[:,1]-Y0)
    com=(m[:,None]*x).sum(0)/m.sum();g=9.80665e-6
    return dict(mass_g=float(m.sum()),com_mm=com.round(2).tolist(),
      pitch_Nm=float(m.sum()*np.hypot(com[1]-Y0,com[2]-Z0)*g),roll_Nm=float(m.sum()*np.hypot(com[0]-AX,com[1]-Y0)*g),
      pitch_lever_mm=float(np.hypot(com[1]-Y0,com[2]-Z0)))
a=tq(M,X)
sens=lambda m,r:m*r*9.80665e-6
out=dict(basis='Rigid arm swung so its COM is horizontal from the axis; no payload, no links, hand, fasteners, cables',rows=[dict(item=r[0],mass_g=round(r[1],1),source=r[2],com_step_mm=r[3].round(2).tolist()) for r in rows],actuators_plus_shoulder=a,
 sensitivity_Nm_per_100g=dict(mid_upper_link=sens(100,L_UPPER/2),mid_forearm=sens(100,L_UPPER+L_LOWER/2),hand_stub_mid=sens(100,L_UPPER+L_LOWER+L_HAND/2),hand_end=sens(100,L_UPPER+L_LOWER+L_HAND)),
 repo_RS03_peak_Nm=60)
(W/'mass_torque.json').write_text(json.dumps(out,indent=1));print(json.dumps(out,indent=1))
