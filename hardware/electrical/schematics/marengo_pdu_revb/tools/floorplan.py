#!/usr/bin/env python3
"""To-scale placement sketch for rev B. Usage: floorplan.py <out.png>
Block sizes come from the netlist courtyards at 60 % fill (numbers below, from tools/area.py).
Terminals, brick and bus caps are drawn at true courtyard size."""
import sys
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle,Circle
A=dict(bank=639,soft=330,out=507,limb=678,latch=269,clamp=173,banka=521,spare=358)   # courtyard mm2, fitted plus legs DNP
INK='#1c2733'
COL=dict(fet='#e57373',cap='#b39ddb',conn='#f2c14e',port='#c5e1a5',logic='#80deea',pow='#90caf9')
W,H=372,262
fig=plt.figure(figsize=(W/20,H/20),dpi=100); ax=fig.add_axes([0,0,1,1]); ax.set_xlim(0,W); ax.set_ylim(H,0); ax.axis('off')
Y0=36
px=lambda X:64+X
py=lambda Y:Y0+(340-Y)
def box(X0,X1,Y0_,Y1,fc,name='',fs=9,ec='#37474f',lw=.9,ls='-',z=3,rot=0,alpha=1,color=INK):
    ax.add_patch(Rectangle((px(X0),py(Y1)),X1-X0,Y1-Y0_,fc=fc,ec=ec,lw=lw,ls=ls,zorder=z,alpha=alpha))
    if name: ax.text(px((X0+X1)/2),py((Y0_+Y1)/2),name,ha='center',va='center',fontsize=fs,rotation=rot,zorder=9,color=color,linespacing=1.05)
ax.text(8,10,'Power board rev B: placement sketch (to scale)',fontsize=23,fontweight='bold',color=INK)
ax.text(8,17,"One board, 100 × 150 mm, all parts on the front face. Blocks are each block's courtyards ÷ 0.6. Terminals, brick and bus caps are true size. Front view, robot's left on the right. Blocks only: no routing yet.",fontsize=10.5,color='#37474f')
ax.text(px(-50),Y0-3,'Front view',fontsize=16,fontweight='bold',color=INK,va='bottom')
# board
box(-50,50,190,340,'#f4f7f4',ec='black',lw=3.5,z=1)
# fan zones
for a,b in ((264,304),(193,233)):
    ax.add_patch(Rectangle((px(-40),py(b)),80,b-a,fc='#e3f2fd',ec='#1565c0',lw=1.6,ls=(0,(5,3)),zorder=1.5))
ax.text(px(0),py(263.5),'',fontsize=8)
# keep-outs and standoffs
for sx_ in (-1,1):
    for a,b in ((190,203),(325,340)):
        X0,X1=sorted((sx_*46,sx_*50)); ax.add_patch(Rectangle((px(X0),py(b)),4,b-a,fc='none',ec='#9e9e9e',lw=.8,hatch='////',zorder=2))
    for Y in (194,265,336): ax.add_patch(Circle((px(sx_*46),py(Y)),3.6,fc='#b0bec5',ec='#37474f',lw=.8,zorder=4))
# brick: body Y 302.85 to 339.65
box(-28.95,28.95,302.85,339.65,COL['pow'],'12 V brick IQB150W8-12\nHeatsink on top\n57.9 × 36.8 × 12.7 mm, true size\nAhead of the E-stop switch',fs=10.5,lw=1.4)
# terminal columns, true size
T=13.16
ROWS=[318,304.6,291.2,277.8,252.6,239.2,225.8,212.4]
L=['BAT +','R arm +','R arm −','aux +','aux −','R leg +','R leg −','HS25 +']
R=['BAT −','L arm +','L arm −','waist +','waist −','L leg +','L leg −','HS25 −']
for Y,l,r in zip(ROWS,L,R):
    for X,t in ((-43.4,l),(43.4,r)):
        dnp='leg' in t
        box(X-T/2,X+T/2,Y-T/2,Y+T/2,'#f7f7f7' if dnp else COL['conn'],t+('\n(DNP)' if dnp else ''),fs=7.6,ec='black' if dnp else '#37474f',lw=1.4 if dnp else .9,ls=(0,(4,2)) if dnp else '-',z=5)
# blocks
Xl,Xr=-36.5,36.5
y0,y1=268,302
w=lambda a:a/.6/(y1-y0)
x=Xl
for k,name,c in (('banka','E-stop bank A\nTPS48100, 2 FETs,\nMID meter','fet'),('bank','E-stop bank B\nLM5069-1, 2 FETs,\nshunt','fet'),('soft','Brick soft\nstart','fet')):
    box(x,x+w(A[k]),y0,y1,COL[c],f'{name}\n{A[k]/.6/100:.1f} cm²',fs=8.5); x+=w(A[k])
print('top row ends at X',round(x,1),'limit',Xr)
# bus caps in the strip between the fans, true size
for k in range(4):
    X=-18.3+k*12.2; box(X-6.1,X+6.1,238,250.2,COL['cap'],'100 µF',fs=7.5)
# meters
mh=A['limb']/2/.6/11.9
for X0,X1,t in ((-36.5,-24.6,'R side meters'),(24.6,36.5,'L side meters')):
    box(X0,X1,264-mh,264,COL['port'],f'{t}: shunts, INA228\n{A["limb"]/2/.6/100:.1f} cm²',fs=8,rot=90)
# spares in the strip above the caps, bank A below them
h=A['spare']/.6/48.2
box(-24.1,24.1,263.5-h,263.5,COL['pow'],f'SPARE_SW eFuse, shunt, meter\nSPARE_ON eFuse, 2 outputs, {A["spare"]/.6/100:.1f} cm²',fs=7.5)
h=A['out']/.6/48.2
box(-24.1,24.1,236.8-h,236.8,COL['pow'],f'12 V caps, meter, 3 outputs, Pi header\n{A["out"]/.6/100:.1f} cm²',fs=8.5)
print('12 V block bottom',round(236.8-h,1))
# latch and clamp
h=A['latch']/.6/21.5
box(-22,-.5,191.5,191.5+h,COL['logic'],f'E-stop latch,\ncontrol rail, loop\nconnector, pressed\ninput, {A["latch"]/.6/100:.1f} cm²',fs=7)
h=A['clamp']/.6/21.5
box(.5,22,191.5,191.5+h,COL['pow'],f'Braking clamp\n{A["clamp"]/.6/100:.1f} cm²',fs=8)
# frame below the board
box(-25.5,25.5,154,182,'#d7ccc8','Brake resistor HS25, on the frame, Y 154 to 182',fs=8,ec='#6d4c41')
ax.text(px(-50),py(186),'Board edge: X ±50, Y 190 to 340. Bay is 110 mm between posts.\nBlocks fill the face: 100 % of the 60 % budget with legs fitted.',fontsize=8.5,color=INK,va='center')
ax.text(px(39),py(263.3),'',fontsize=8)
# fan labels
ax.annotate('Fan zone, Y 264 to 304',xy=(px(0),py(266)),xytext=(px(0),py(265.8)),ha='center',va='center',fontsize=8,color='#1565c0',zorder=9)
ax.text(px(0),py(215),'Fan zone, Y 193 to 233',ha='center',va='center',fontsize=8,color='#1565c0',zorder=9)
# side view
ox=150; oy=Y0
sx=lambda z:ox+(z+50)
sy=lambda Y:oy+(340-Y)
def sbox(z0,z1,y0_,y1_,fc,ec='#37474f',lw=.8,ls='-',z=3,alpha=1):
    ax.add_patch(Rectangle((sx(z0),sy(y1_)),z1-z0,y1_-y0_,fc=fc,ec=ec,lw=lw,ls=ls,zorder=z,alpha=alpha))
ax.text(ox,oy-3,'Side view in the torso',fontsize=16,fontweight='bold',color=INK,va='bottom')
sbox(-45,34,183,340,'#ffe0b2'); ax.text(sx(-5.5),sy(264),'Pack\nZ -45 to 34',ha='center',va='center',fontsize=10)
sbox(37.5,41.5,190,340,'#b0bec5')
sbox(47.5,49.1,190,340,'#1b5e20')
for Y in (336,265,194): sbox(41.5,47.5,Y-1.5,Y+1.5,'#78909c')
Zc=[(190,193,67),(193,233,61.5),(233,264,67),(264,304,61.5),(304,340,67)]
for a,b,z in Zc: sbox(49.1,z,a,b,'#e3f2fd',ec='none',z=1,alpha=.9)
xs=[];ys=[]
for a,b,z in Zc: xs+=[sx(z),sx(z)]; ys+=[sy(a),sy(b)]
ax.plot(xs,ys,color='#1565c0',lw=2,zorder=4)
for a,b in ((264,304),(193,233)):
    sbox(70,80,a,b,'#cfd8dc'); ax.text(sx(75),sy((a+b)/2),'fan',rotation=90,ha='center',va='center',fontsize=8.5,zorder=9)
sbox(49.1,61.8,302.85,339.65,COL['pow'],lw=1.2,z=5)
sbox(61.8,66.8,304,339.65,'#b0bec5',lw=1.2,z=5)
sbox(49.1,65.6,239,249,COL['cap'],lw=1.2,z=5)
sbox(33.4,48,154,182,'#d7ccc8')
tx=sx(90)
def call(z,Y,ty,t,c=INK): ax.annotate(t,xy=(sx(z),sy(Y)),xytext=(tx,ty),fontsize=9.5,color=c,va='center',arrowprops=dict(arrowstyle='-',color=c,lw=.8),zorder=10)
call(66.8,322,oy+10,'Heatsink to Z 66.8, 5 mm, needs Z 67.\nBrick top Z 61.8')
call(67,315,oy+26,'Z 67 ceiling: proposed,\nnot in the frame README','#1565c0')
call(61.5,284,oy+56,'Z 61.5 under the fans','#1565c0')
call(65.6,244,oy+95,'Bus caps top Z 65.6:\nneed the Z 67 strip')
call(48.3,270,oy+76,'Board from Z 47.5')
ax.text(sx(90),oy+135,'Rear on the left, front on the right.\nSame scale as the board.',fontsize=9.5,color='#37474f',va='center')
# verdict
vy=226
ax.text(150,vy,'Brick placement',fontsize=13,fontweight='bold',color=INK)
ax.text(150,vy+3.5,"Fits at the top of the board, long side across, Y 302.85 to 339.65. It can't go between the fans: that strip is 31 mm and the brick is 36.8 mm.\n"
 "Two small misses to clear with the frame thread: the body runs 1.15 mm into the upper fan zone in Y, and its top is 0.3 mm over Z 61.5.\n"
 "It needs a heatsink at 121 W in a warm torso, held to Y 304 and up, so Z 67 is required here too.\nIf Z 67 is refused, the caps need a shorter can and the brick moves to a frame plate on a pigtail.",fontsize=9.5,color=INK,va='top',linespacing=1.35)
lx=8; ly=H-8
for lab,c in (('Switch FETs',COL['fet']),('Capacitors',COL['cap']),('Terminals',COL['conn']),('Meters',COL['port']),('Latch',COL['logic']),('Power, clamp',COL['pow'])):
    ax.add_patch(Rectangle((lx,ly),6,4,fc=c,ec='#37474f',lw=.8)); ax.text(lx+7.5,ly+2,lab,fontsize=9.5,va='center'); lx+=12+len(lab)*1.75
ax.add_patch(Rectangle((lx,ly),6,4,fc='none',ec='black',lw=1.4,ls=(0,(4,2)))); ax.text(lx+7.5,ly+2,'DNP: legs',fontsize=9.5,va='center'); lx+=28
ax.add_patch(Rectangle((lx,ly),6,4,fc='#e3f2fd',ec='#1565c0',lw=1.4,ls=(0,(5,3)))); ax.text(lx+7.5,ly+2,'Fan zone (shown |X| ≤ 40)',fontsize=9.5,va='center')
fig.savefig(sys.argv[1],dpi=100,facecolor='white')
