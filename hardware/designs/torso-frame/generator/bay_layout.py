import json,sys,matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle, FancyBboxPatch
P=json.load(open(sys.argv[1]))['parts']
def boxes(p):
    if p['type']=='box': return [(p['lo'],p['hi'])]
    if p['type']=='multibox': return p['boxes']
    if p['type']=='cyl':
        a='xyz'.index(p['axis']);c=p['c'];r=p['r'];h=p['h'];lo=[c[k]-r for k in range(3)];hi=[c[k]+r for k in range(3)];lo[a]=c[a]-h/2;hi[a]=c[a]+h/2;return [(lo,hi)]
    return []
SHOW={'pi5':('Pi 5 + CAN HAT','#3aa655'),'jetson':('Jetson Orin Nano','#26b3c9'),'pi_stub':('Pi power stub','#a35bb5'),'imu_old':('BNO085 IMU','#8fd36b'),
 'shelf':('shelf','#9b87d8'),'fan_exhaust_1':('exhaust fan','#7fd0dd'),'fan_exhaust_2':('exhaust fan','#7fd0dd'),'pdb':('power board','#a35bb5'),'pack':('pack','#e0c23a')}
def draw(ax,u,v,ylim,xlim,cut,title):
    # cut: (axis, lo, hi) slab to include
    a,c0,c1=cut
    for p in P:
        g=p['group']; pid=p['id']
        for lo,hi in boxes(p):
            if hi[a]<c0 or lo[a]>c1: continue
            if g in ('extrusion',): fc,ec,lab='#b9bec6','#666',None
            elif g=='bracket': fc,ec,lab='#d0d3d8','#888',None
            elif g=='gusset': continue
            elif pid.startswith('rs03_pitch') or pid.startswith('rs03_boss'): fc,ec,lab='#6b6f75','#333','RS03' if pid.startswith('rs03_pitch_1') else None
            elif pid.startswith('pitch_bracket') or pid.startswith('pitch_wing'): fc,ec,lab='#cfe8e0','#2a8a6a',None
            elif pid.startswith('shelf_slot'): fc,ec,lab='#ffd84a','#b08a00','slot'
            elif pid in SHOW: fc,ec=SHOW[pid][1],'#222'; lab=SHOW[pid][0]
            elif pid.startswith('harness_shoulder'): fc,ec,lab='#d8b84a','#8a6a00',None
            else: continue
            x0,x1,y0,y1=lo[u],hi[u],lo[v],hi[v]
            z=3 if pid.startswith('shelf_slot') else (2 if pid in SHOW and pid!='shelf' else 1)
            ax.add_patch(Rectangle((x0,y0),x1-x0,y1-y0,fc=fc,ec=ec,lw=0.8,alpha=0.85 if z>1 else 0.6,zorder=z))
            if pid=='bno085': continue
            if lab and (x1-x0)>12 and xlim[0]<(x0+x1)/2<xlim[1] and ylim[0]<(y0+y1)/2<ylim[1]: ax.text((x0+x1)/2,(y0+y1)/2,lab,ha='center',va='center',fontsize=7,zorder=5)
    ax.set_xlim(*xlim); ax.set_ylim(*ylim); ax.set_aspect('equal'); ax.set_title(title,fontsize=10); ax.grid(alpha=0.25)
fig,axs=plt.subplots(1,3,figsize=(18,7.5))
draw(axs[0],0,2,(-80,80),(-120,120),(1,354,400),'Shelf, top view (looking down), Y 354-400\nX right = robot left, Z up = front')
draw(axs[1],0,1,(340,500),(-140,140),(2,-75,75),'Upper bay, front view (all depths)\nX right, Y up')
draw(axs[2],2,1,(340,500),(-90,90),(0,-60,60),'Upper bay, side view (|X| < 60)\nZ right = front, Y up')
for ax in axs[1:]:
    ax.axhline(373.62,color='r',lw=0.6,ls='--'); ax.text(ax.get_xlim()[0]+3,375,'shelf top',color='r',fontsize=7)
fig.suptitle('Torso compute bay as modelled (main). Yellow = proposed oval cable slots in the shelf.',fontsize=12)
plt.tight_layout(); plt.savefig(sys.argv[2],dpi=110)
