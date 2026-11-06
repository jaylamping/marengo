from swlib import *
from params import *
import json,inspect,win32com.client as wc
K=lambda x:x() if inspect.ismethod(x) else x
NUL=VARIANT(pythoncom.VT_DISPATCH,None);m=1e-3
TL=([-1,0,0, 0,0,1, 0,1,0],(97,Y0,Z0));ID=([1,0,0, 0,1,0, 0,0,1],(0,0,0))
EL=([0,0,-1, 0,1,0, 1,0,0],(AX+18.7,EY,AZ));FA=([1,0,0, 0,0,-1, 0,1,0],(AX,EY-94.2-RS00_OFFSET,AZ))
PARTS=[('parts/TORSO.SLDPRT',ID,1),('parts/P1_L01.SLDPRT',TL,1),('parts/ROLL_RS03.SLDPRT',TL,1),('parts/SPACER.SLDPRT',TL,1),('parts/P2_L04B.SLDPRT',TL,1),('parts/YAW_RS02.SLDPRT',TL,1),('parts/ELBOW_RS02.SLDPRT',EL,1),('parts/FOREARM_RS00.SLDPRT',FA,1),('LEFT_ARM_SKELETON.SLDPRT',ID,1)]
sw=app();log('docs at start',sw.GetDocumentCount)
MOD=wc.gencache.EnsureModule('{83A33D31-27C5-11CE-BFD4-00400513BB57}',0,34,0)
RB=lambda d:MOD.IModelDoc2(d._oleobj_).EditRebuild3()
from closemine import closemine
closeminex=closemine
closeminex(sw)
e,w=L(),L();sw.OpenDoc6(str(W/'LEFT_ARM_SKELETON.SLDPRT'),1,1,'',e,w);log('skeleton',e.value)
for rel,_,t in PARTS[:-1]:
    e,w=L(),L();sw.OpenDoc6(str(W/rel),t,1,'',e,w);log('open',rel,e.value)
a=sw.NewDocument(sw.GetUserPreferenceStringValue(9),0,0,0);out={}
tgt=W/'LEFT_ARM_LAYOUT.SLDASM';e,w=L(),L();log('asm created',a.Extension.SaveAs(str(tgt),0,1,NUL,e,w),e.value)
for rel,(R,t),_ in PARTS:
    c=a.AddComponent5(str(W/rel),0,'',False,'',0,0,0)
    if c is None: raise RuntimeError('add failed '+rel)
    x=c.Transform2;x.ArrayData=VARIANT(pythoncom.VT_ARRAY|pythoncom.VT_R8,[*map(float,R),t[0]*m,t[1]*m,t[2]*m,1.0,0.0,0.0,0.0]);c.Transform2=x
    a.ClearSelection2(True);c.Select4(False,NUL,False);K(a.FixComponent);a.ClearSelection2(True)
    b=c.GetBox(False,False);out[rel]=[round(v*1e3,2) for v in b] if b else None;log('placed',c.Name2,out[rel],c.IsFixed);e,w=L(),L();log('saved',a.Extension.SaveAs(str(tgt),0,1,NUL,e,w),e.value)
tgt=W/'LEFT_ARM_LAYOUT.SLDASM'
e,w=L(),L();log('asm saved',a.Extension.SaveAs(str(tgt),0,1,NUL,e,w),e.value,w.value)
(W/'component_boxes_mm.json').write_text(json.dumps(out,indent=1))
t=wc.gencache.EnsureModule('{83A33D31-27C5-11CE-BFD4-00400513BB57}',0,34,0).IModelDoc2(a._oleobj_)
for name,nv,vid in (('LEFT_ARM_LAYOUT_ISO','*Isometric',7),('LEFT_ARM_LAYOUT_FRONT','*Front',1)):
    t.ShowNamedView2(nv,vid);t.ViewZoomtofit2();t.GraphicsRedraw2();time.sleep(2)
    e,w=L(),L();log('png',name,a.Extension.SaveAs(str(W/'views'/(name+'.png')),0,1,NUL,e,w),e.value,w.value)
