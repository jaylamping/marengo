#!/usr/bin/env python3
"""To-scale placement sketch for rev B. Usage: floorplan.py <out.png>
Block sizes come from the netlist courtyards at 60 % fill (numbers below, from tools/area.py, grouped by sheet block).
Terminals, the converter pigtail connector and bus caps are drawn at true courtyard size."""
import sys
import matplotlib; matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.patches import Rectangle,Circle
A=dict(bank=866,soft=487+15,out=614+67,limb=678,latch=440+62,clamp=173,banka=529+26-180,spare=358+25)   # courtyard mm2, fitted plus legs DNP, rev B4 fixes (2026-11-07): rev B2 blocks plus the B4 parts from area.py
MIDM=180   # U17 MID meter, its clamp and the CS RC placeholder, moved to the bottom free zone
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
ax.text(8,10,'Power board rev B4: placement sketch (to scale)',fontsize=23,fontweight='bold',color=INK)
ax.text(8,17,"One board, 100 × 150 mm, all parts on the front face. Blocks are each block's courtyards ÷ 0.6. Terminals, pigtail connector and bus caps are true size. Front view, robot's left on the right (+X). Blocks only: no routing yet.",fontsize=10.5,color='#37474f')
ax.text(px(-50),Y0-3,'Front view',fontsize=16,fontweight='bold',color=INK,va='bottom')
# board
box(-50,50,190,340,'#f4f7f4',ec='black',lw=3.5,z=1)
# fan zones
for a,b in ((264,304),(193,233)):
    ax.add_patch(Rectangle((px(-40),py(b)),80,b-a,fc='#e3f2fd',ec='#1565c0',lw=1.6,ls=(0,(5,3)),zorder=1.5))
# keep-outs and standoffs
for sx_ in (-1,1):
    for a,b in ((190,203),(325,340)):
        X0,X1=sorted((sx_*46,sx_*50)); ax.add_patch(Rectangle((px(X0),py(b)),4,b-a,fc='none',ec='#9e9e9e',lw=.8,hatch='////',zorder=2))
    for Y in (194,265,336): ax.add_patch(Circle((px(sx_*46),py(Y)),3.6,fc='#b0bec5',ec='#37474f',lw=.8,zorder=4))
# terminal columns, true size. HS50 A (+X gusset) leads to J3/J4; HS50 B (-X gusset) crosses under the board to J25/J26 on the top edge, +X half.
T=13.16
L=[(291.2,'BAT +'),(277.8,'R arm +'),(252.6,'aux +'),(239.2,'R leg +'),(225.8,'GND stud 2\naux, legs')]
R=[(318.0,'HS50 A +'),(304.6,'HS50 A −'),(291.2,'BAT −'),(277.8,'L arm +'),(252.6,'waist +'),(239.2,'L leg +'),(225.8,'GND stud 1\narms, waist')]
for X,col in ((-43.4,L),(43.4,R)):
    for Y,t in col:
        dnp='leg +' in t
        box(X-T/2,X+T/2,Y-T/2,Y+T/2,'#f7f7f7' if dnp else COL['conn'],t+('\n(DNP)' if dnp else ''),fs=7.2,ec='black' if dnp else '#37474f',lw=1.4 if dnp else .9,ls=(0,(4,2)) if dnp else '-',z=5)
# J25/J26 for HS50 B, true size, along the top edge inboard of J3
# B4 absorber cans C44/C45 (10x12.6, courtyard about 11.4 x 13): row 2 has no room. One fits the top-strip pocket under J26; the other goes on the back face under bank B, or the pocket grows if bank A shrinks
box(7.2,18.6,309.5,322.5,COL['cap'],'C44\n47 µF',fs=7,z=5)
box(-36.5,-25.1,268.5,281.5,'none','C45 back face\n(under bank B)',fs=6.5,ec=INK,lw=1.2,ls=(0,(3,2)),z=6)
for X,t in ((30.2,'HS50 B +\nJ25'),(17.0,'HS50 B −\nJ26')): box(X-T/2,X+T/2,332.4-T/2,332.4+T/2,COL['conn'],t,fs=7.2,z=5)
# converter pigtail J24, Micro-Fit 2x6, true courtyard about 21.6 x 10.6
box(-48.4,-37.8,303.4,325,COL['conn'],'J24\n12 V\npigtail\n2×6',fs=7.2,z=5)
# top strip, Y 305 to 339
yt0,yt1=305,339; ht=yt1-yt0
w=lambda a:a/.6/ht
x=-36.5; box(x,x+w(A['soft']),yt0,yt1,COL['fet'],f"Converter soft\nstart, F2, ON/OFF\n{A['soft']/.6/100:.1f} cm²",fs=8); x+=w(A['soft'])
# latch moved up here so bank A sits in the fan zone (Joey, 2026-11-07); its overflow spills into the pocket beside C44
lx1=10.4; box(x,lx1,yt0,yt1,COL['logic'],f"E-stop latch, control\nrail, interlock, loop\nconnector, pressed input\n{A['latch']/.6/100:.1f} cm²",fs=8)
spill=A['latch']/.6-(lx1-x)*ht; sw=spill/(325.8-yt0); box(18.6,18.6+sw,yt0,325.8,COL['logic'],'',fs=6); x=lx1
print('latch spill mm2',round(spill),'width',round(sw,1))
cw=A['clamp']/.6/(325.8-yt0); box(36.5-cw,36.5,yt0,325.8,COL['pow'],f"Clamp\n{A['clamp']/.6/100:.1f}\ncm²",fs=7.5)
print('top strip ends at X',round(x,1),'J26 starts at',round(17.0-T/2,1),'clamp starts at',round(36.5-cw,1))
# second row, Y 268 to 302
y0,y1=268,302
w=lambda a:a/.6/(y1-y0)
x=-36.5
for k,name,c in (('bank','E-stop bank B\nLM5069-1, 2 FETs, shunt,\nabsorber (C45 on the back face)','fet'),('banka','E-stop bank A\nTPS48100, 2 FETs\n(in the fan zone)','fet')):
    box(x,x+w(A[k]),y0,y1,COL[c],f'{name}\n{A[k]/.6/100:.1f} cm²',fs=8); x+=w(A[k])
print('second row ends at X',round(x,1),'limit 36.5')
# bus caps in the strip between the fans, true size
for k in range(4):
    X=-18.3+k*12.2; box(X-6.1,X+6.1,238,250.2,COL['cap'],'68 µF',fs=7.5)
# meters
mh=A['limb']/2/.6/11.9
for X0,X1,t in ((-36.5,-24.6,'R side meters'),(24.6,36.5,'L side meters')):
    box(X0,X1,264-mh,264,COL['port'],f'{t}: shunts, INA228\n{A["limb"]/2/.6/100:.1f} cm²',fs=8,rot=90)
# spares above the caps, 12 V caps and outputs below them
h=A['spare']/.6/48.2
box(-24.1,24.1,263.5-h,263.5,COL['pow'],f'SPARE_SW eFuse, shunt, meter\nSPARE_ON eFuse, 2 outputs, {A["spare"]/.6/100:.1f} cm²',fs=7.5)
h=A['out']/.6/48.2
box(-24.1,24.1,236.8-h,236.8,COL['pow'],f'12 V caps (330 µF), meter, 3 outputs, Pi header\n{A["out"]/.6/100:.1f} cm²',fs=8.5)
print('12 V block bottom',round(236.8-h,1))
box(-24.1,-24.1+MIDM/.6/21.5,191.5,213,COL['fet'],'MID meter\n(U17, clamp,\nCS RC)',fs=7)
box(-24.1+MIDM/.6/21.5,24.1,191.5,213,'none','Free: about 8 cm²',fs=8,ec='#9e9e9e',ls=(0,(4,2)))
tot=sum(A.values())+MIDM+298+606+14*173.4+229.5
ax.text(px(-50),py(186),f'Board edge: X ±50, Y 190 to 340. Bay is 110 mm between posts.\nCourtyards {tot:.0f} mm²: {tot/.6/150:.0f} % of the one-sided 60 % budget with legs fitted.',fontsize=8.5,color=INK,va='center')
ax.text(px(0),py(266),'Fan zone, Y 264 to 304',ha='center',va='center',fontsize=8,color='#1565c0',zorder=9)
ax.text(px(0),py(195),'Fan zone, Y 193 to 233',ha='center',va='center',fontsize=8,color='#1565c0',zorder=9)
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
sbox(49.1,65.6,239,249,COL['cap'],lw=1.2,z=5)
sbox(78,92.8,297.5,340,'#d7ccc8',lw=1.2,z=5,ls=(0,(4,2)))
tx=sx(105)
def call(z,Y,ty,t,c=INK): ax.annotate(t,xy=(sx(z),sy(Y)),xytext=(tx,ty),fontsize=9.5,color=c,va='center',arrowprops=dict(arrowstyle='-',color=c,lw=.8),zorder=10)
call(92.8,320,oy+12,'HS50 A on the +X gusset (B mirrored on -X),\nZ 78 to 92.8, Y 297.5 to 346.6,\nbeside the board (X 45 to 75)')
call(61.5,284,oy+56,'Z 61.5 under the fans','#1565c0')
call(65.6,244,oy+95,'Bus caps top Z 65.6:\nneed the Z 67 strip')
call(48.3,270,oy+76,'Board from Z 47.5')
ax.text(sx(105),oy+135,'Rear on the left, front on the right.\nSame scale as the board.',fontsize=9.5,color='#37474f',va='center')
# off-board notes
vy=226
ax.text(150,vy,'Off the board (frame main 79500b9, resistors 72a3889)',fontsize=13,fontweight='bold',color=INK)
ax.text(150,vy+3.5,"12 V converter Murata DVE0125V2NBC on the top front rail, X ±29.2, Y 473.7 to 496.4: about 150 mm of twisted pigtail to J24 on the top edge, -X half.\n"
 "At the converter: 47 µF 100 V aluminium and 1 µF across the input pins, strain relief (rail zone X -40 to -29). F2, 20 A, is on the board.\n"
 "Brake resistors 2 × HS50 20R, one per gusset (frame 72a3889):\n+X about 50 mm to J3/J4; -X about 200 mm, under the board, to J25/J26. Both on the top edge, +X half.\n"
 "No Z 67 or heatsink question on the board any more: the brick's 2,200 mm² went to the top strip.",fontsize=9.5,color=INK,va='top',linespacing=1.35)
lx=8; ly=H-8
for lab,c in (('Switch FETs',COL['fet']),('Capacitors',COL['cap']),('Terminals',COL['conn']),('Meters',COL['port']),('Latch',COL['logic']),('Power, clamp',COL['pow'])):
    ax.add_patch(Rectangle((lx,ly),6,4,fc=c,ec='#37474f',lw=.8)); ax.text(lx+7.5,ly+2,lab,fontsize=9.5,va='center'); lx+=12+len(lab)*1.75
ax.add_patch(Rectangle((lx,ly),6,4,fc='none',ec='black',lw=1.4,ls=(0,(4,2)))); ax.text(lx+7.5,ly+2,'DNP: legs',fontsize=9.5,va='center'); lx+=28
ax.add_patch(Rectangle((lx,ly),6,4,fc='#e3f2fd',ec='#1565c0',lw=1.4,ls=(0,(5,3)))); ax.text(lx+7.5,ly+2,'Fan zone (shown |X| ≤ 40)',fontsize=9.5,va='center')
fig.savefig(sys.argv[1],dpi=100,facecolor='white')
