from swlib import *
from params import *
NUL=VARIANT(pythoncom.VT_DISPATCH,None);m=1e-3
import inspect,atexit
C=lambda x:x() if inspect.ismethod(x) else x
from closemine import closemine
sw=app();closemine(sw)
atexit.register(lambda:sw.SetUserPreferenceToggle(10,True));sw.SetUserPreferenceToggle(10,False)
d=sw.NewDocument(sw.GetUserPreferenceStringValue(8),0,0,0);sm=d.SketchManager;ext=d.Extension
eq=C(d.GetEquationMgr)
for i,(n,v) in enumerate((('L_UPPER',L_UPPER),('L_LOWER',L_LOWER),('L_HAND',L_HAND),('RS00_OFFSET',RS00_OFFSET))): log('eq',eq.Add2(-1,f'"{n}" = {v}mm',True))
# fixed torso-side axes
sm.Insert3DSketch(True);sm.AddToDB=True
sm.CreateLine(97*m,Y0*m,Z0*m,245*m,Y0*m,Z0*m);sm.CreateLine(AX*m,Y0*m,(Z0-75)*m,AX*m,Y0*m,(Z0+85)*m);sm.CreateLine(AX*m,(Y0+45)*m,AZ*m,AX*m,Y0*m,AZ*m)
sm.AddToDB=False;sm.Insert3DSketch(True);d.FeatureByPositionReverse(0).Name='AXES_PITCH_ROLL_YAWTOP'
# arm plane through yaw axis
d.ClearSelection2(True);log('sel',ext.SelectByID2('Front Plane','PLANE',0,0,0,False,0,NUL,0))
p=d.FeatureManager.InsertRefPlane(8,AZ*m,0,0,0,0);log('plane',p is not None)
p.Name='ARM_PLANE';tf=C(p.GetSpecificFeature2).Transform.ArrayData;log('plane tf',[round(x,5) for x in tf[:12]])
if tf[11]<0: raise SystemExit('plane flipped')
d.ClearSelection2(True);ext.SelectByID2('ARM_PLANE','PLANE',0,0,0,False,0,NUL,0)
sm.InsertSketch(True);sm.AddToDB=True
def ln(a,b): return sm.CreateLine(a[0]*m,a[1]*m,0,b[0]*m,b[1]*m,0)
u=ln((AX,Y0),(AX,EY));f=ln((AX,EY),(AX,WY));h=ln((AX,WY),(AX,HY));e1=ln((AX,EY),(AX-60,EY));e2=ln((AX,EY),(AX+60,EY));g=ln((AX+60,EY),(AX+60,EY-48.2-RS00_OFFSET));tk=ln((AX+60,EY-48.2-RS00_OFFSET),(AX,EY-48.2-RS00_OFFSET))
sm.AddToDB=False
def con(c,*ents):
    d.ClearSelection2(True)
    for i,x in enumerate(ents): x.Select4(i>0,NUL)
    d.SketchAddConstraints(c);d.ClearSelection2(True)
for s in (u,f,h,g): con('sgVERTICAL2D',s)
for s in (e1,e2,tk): con('sgHORIZONTAL2D',s)
con('sgFIXED',C(u.GetStartPoint2))
con('sgMERGEPOINTS',C(u.GetEndPoint2),C(f.GetStartPoint2));con('sgMERGEPOINTS',C(f.GetEndPoint2),C(h.GetStartPoint2))
con('sgMERGEPOINTS',C(u.GetEndPoint2),C(e1.GetStartPoint2));con('sgMERGEPOINTS',C(u.GetEndPoint2),C(e2.GetStartPoint2))
con('sgMERGEPOINTS',C(e2.GetEndPoint2),C(g.GetStartPoint2));con('sgMERGEPOINTS',C(g.GetEndPoint2),C(tk.GetStartPoint2));con('sgCOINCIDENT',C(tk.GetEndPoint2),f)
names=[]
for s,v,mx in ((u,'L_UPPER',0.02),(f,'L_LOWER',0.02),(h,'L_HAND',0.02),(e1,None,0),(e2,None,0),(g,'RS00',0.04)):
    d.ClearSelection2(True);s.Select4(False,NUL)
    a=C(s.GetStartPoint2);b=C(s.GetEndPoint2)
    dd=d.AddDimension2((a.X+b.X)/2+mx+.01,(a.Y+b.Y)/2+.01,0);dm=dd.GetDimension2(0);names.append((dm.FullName,v));d.ClearSelection2(True)
sm.InsertSketch(True);f0=d.FeatureByPositionReverse(0);f0.Name='LINKS_AND_ELBOW_AXIS';log('dims',names)
for nm,v in names:
    if v:
        short=nm.split('@')[0]
        rhs='"RS00_OFFSET" + 48.2mm' if v=='RS00' else f'"{v}"'
        log('eq',v,eq.Add2(-1,f'"{short}@LINKS_AND_ELBOW_AXIS" = {rhs}',True))
log('rebuild',d.ForceRebuild3(False),'eqs',C(eq.GetCount))
for i in range(C(eq.GetCount)): log(eq.Equation(i),eq.Value(i))
e,w=L(),L();log('save',ext.SaveAs(str(W/'LEFT_ARM_SKELETON.SLDPRT'),0,1,NUL,e,w),e.value,w.value)

