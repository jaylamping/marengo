"""Read joint frames, mass properties and actuator placement from the saved CAD. Shares no code with the placer."""
from swlib import *  # SolidWorks COM helpers in the layout folder
import json,win32com.client as wc
sw=app();out={'source':'LEFT_ARM_SKELETON.SLDPRT / LEFT_ARM_LAYOUT.SLDASM','units':'metres, kg, kg m2, STEP frame'}
e,w=L(),L();a=sw.OpenDoc6(str(W/'LEFT_ARM_LAYOUT.SLDASM'),2,1,'',e,w)
s=sw.GetOpenDocumentByName(str(W/'LEFT_ARM_SKELETON.SLDPRT'))
NAMES=['F_TORSO','F_L_SHOULDER_PITCH','F_L_SHOULDER_ROLL','F_L_UPPER_ARM_YAW','F_L_ELBOW_PITCH','F_L_LOWER_ARM_YAW','F_L_HAND','F_L_HAND_TIP']
fr={}
for n in NAMES:
    x=s.Extension.GetCoordinateSystemTransformByName(n).ArrayData;fr[n]=dict(axes=[list(x[0:3]),list(x[3:6]),list(x[6:9])],origin=list(x[9:12]))
out['frames']=fr
eq=s.GetEquationMgr;out['globals']={eq.Equation(i).split('"')[1]:round(eq.Value(i)*25.4,4) for i in range(4)}
comps={}
for c in a.GetComponents(True):
    n=c.Name2.rsplit('-',1)[0]
    if n in('LEFT_ARM_SKELETON','TORSO'):continue
    x=c.Transform2.ArrayData
    e,w=L(),L();p=sw.OpenDoc6(c.GetPathName,1,1,'',e,w)
    m=p.Extension.CreateMassProperty
    m.UseSystemUnits=True
    comps[n]=dict(rows=[list(x[0:3]),list(x[3:6]),list(x[6:9])],t=list(x[9:12]),sw_mass=m.Mass,volume=m.Volume,com=list(m.CenterOfMass),moi_com=list(m.GetMomentOfInertia(0)))
    log(n,'vol mm3',round(m.Volume*1e9),'com mm',[round(v*1e3,2) for v in m.CenterOfMass])
out['components']=comps
(W/'cad_export.json').write_text(json.dumps(out,indent=1))
