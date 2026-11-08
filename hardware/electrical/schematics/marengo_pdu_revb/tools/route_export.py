#!/usr/bin/env python3
"""Stage 3 (own router), step 1: the board's copper as polygons for tools/grid_route.py.
Every pad, track, via and zone fill is exported per copper layer as polygons with its net; holes go to a separate list
(obstacles on every layer). The GND fills on L1, In2 and L4 are left out: they are refilled around the signals after
routing. The In1 GND fill is exported but flagged, so it joins GND pads for connectivity without blocking other nets'
vias (the fill will clear them). Coordinates in mm (KiCad page coordinates).
Run with KiCad's Python: <kicad python> tools/route_export.py <board.kicad_pcb> <copper.json>"""
import json,sys
import pcbnew
b=pcbnew.LoadBoard(sys.argv[1])
pcbnew.ZONE_FILLER(b).Fill(b.Zones())
LAYERS={pcbnew.F_Cu:0,pcbnew.In1_Cu:1,pcbnew.In2_Cu:2,pcbnew.B_Cu:3}
mm=pcbnew.ToMM
def polys(sps):
    out=[]
    for i in range(sps.OutlineCount()):
        o=sps.Outline(i); out.append([[round(mm(o.CPoint(j).x),4),round(mm(o.CPoint(j).y),4)] for j in range(o.PointCount())])
        for k in range(sps.HoleCount(i)):
            h=sps.Hole(i,k); out.append([[round(mm(h.CPoint(j).x),4),round(mm(h.CPoint(j).y),4)] for j in range(h.PointCount())]+[None])   # None marks a hole ring
    return out
items=[]; holes=[]
def shape_of(it,L):
    s=pcbnew.SHAPE_POLY_SET(); it.TransformShapeToPolygon(s,L,0,pcbnew.FromMM(0.005),pcbnew.ERROR_INSIDE); return s
for p in b.GetPads():
    for L,k in LAYERS.items():
        if p.IsOnLayer(L) and (not p.HasHole() or p.FlashLayer(L)):
            items.append(dict(net=p.GetNetname(),layer=k,kind='pad',ref=f'{p.GetParentFootprint().GetReference()}.{p.GetNumber()}',poly=polys(shape_of(p,L))))
    if p.HasHole():
        s=pcbnew.SHAPE_POLY_SET(); p.TransformHoleToPolygon(s,0,pcbnew.FromMM(0.005),pcbnew.ERROR_INSIDE); holes.append(dict(net=p.GetNetname(),poly=polys(s)))
for t in b.GetTracks():
    if t.GetClass()=='PCB_VIA':
        for L,k in LAYERS.items():
            if t.FlashLayer(L): items.append(dict(net=t.GetNetname(),layer=k,kind='via',poly=polys(shape_of(t,L))))
        c=t.GetPosition(); holes.append(dict(net=t.GetNetname(),circle=[mm(c.x),mm(c.y),mm(t.GetDrillValue())/2]))
    elif t.GetLayer() in LAYERS:
        items.append(dict(net=t.GetNetname(),layer=LAYERS[t.GetLayer()],kind='track',poly=polys(shape_of(t,t.GetLayer()))))
for z in b.Zones():
    if z.GetIsRuleArea() or not z.GetNetname(): continue
    for L in z.GetLayerSet().Seq():
        if L not in LAYERS: continue
        if z.GetNetname()=='GND' and L!=pcbnew.In1_Cu: continue
        fp=z.GetFilledPolysList(L)
        if fp.OutlineCount():
            items.append(dict(net=z.GetNetname(),layer=LAYERS[L],kind='plane' if (z.GetNetname()=='GND' and L==pcbnew.In1_Cu) else 'zone',poly=polys(fp)))
for z in b.Zones():   # board rule areas that forbid tracks (the hole keep-outs from tools/place.py)
    if z.GetIsRuleArea() and z.GetDoNotAllowTracks():
        for L,k in LAYERS.items():
            if z.IsOnLayer(L): items.append(dict(net='',layer=k,kind='keepout',poly=polys(z.Outline())))
for f in b.GetFootprints():   # footprint rule areas (keep-outs in the libraries, e.g. strain-relief holes)
    for z in f.Zones():
        if z.GetIsRuleArea() and z.GetDoNotAllowTracks():
            for L,k in LAYERS.items():
                if z.IsOnLayer(L): items.append(dict(net='',layer=k,kind='keepout',poly=polys(z.Outline())))
bb=b.GetBoardEdgesBoundingBox()
json.dump(dict(edge=[mm(bb.GetLeft()),mm(bb.GetTop()),mm(bb.GetRight()),mm(bb.GetBottom())],items=items,holes=holes),open(sys.argv[2],'w'))
print('items',len(items),'holes',len(holes))
