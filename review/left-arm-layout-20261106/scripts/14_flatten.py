"""Flat single-product STEPs so SolidWorks imports one self-contained multibody part.
Torso: all leaf BREPs from marengo-reference except the layout reference block, world coordinates."""
import sys,csv,time
from pathlib import Path
T=Path(r'C:\Users\joeyl\Documents\Codex\2026-10-29\task');O=T/'LEFT_ARM_V1_LAYOUT_20261106/inputs';R=T/'marengo-reference'
sys.path.insert(0,str(R/'cad-reader'))
from OCP.BRep import BRep_Builder
from OCP.BRepTools import BRepTools
from OCP.TopoDS import TopoDS_Shape,TopoDS_Compound,TopoDS_Iterator
from OCP.TopAbs import TopAbs_COMPOUND
from OCP.STEPControl import STEPControl_Writer,STEPControl_AsIs
from OCP.Interface import Interface_Static
def rb(p):
    s=TopoDS_Shape();assert BRepTools.Read_s(s,str(p),BRep_Builder());return s
def leaves(s):
    if s.ShapeType()==TopAbs_COMPOUND:
        i=TopoDS_Iterator(s)
        while i.More():
            yield from leaves(i.Value());i.Next()
    else: yield s
def wr(ss,p):
    c=TopoDS_Compound();b=BRep_Builder();b.MakeCompound(c);n=0
    for s in ss:
        for q in leaves(s): b.Add(c,q);n+=1
    w=STEPControl_Writer();w.Transfer(c,STEPControl_AsIs);assert w.Write(str(p))==1;print(p.name,'items',n,p.stat().st_size,flush=True)
lay={int(r['node_id']) for r in csv.DictReader(open(R/'component_inventory.csv')) if any(k in r['name'].lower() for k in ('layout','pi','raspberry','compute')) and 'pitch' not in r['name'].lower()}
print('layout nodes',lay)
fs=[p for p in sorted(R.glob('shape_*.brep')) if int(p.stem[6:]) not in lay];print('leaf files',len(fs))
wr([rb(p) for p in fs],O/'TORSO_SIMPLE.step')
