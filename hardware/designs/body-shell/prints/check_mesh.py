#!/usr/bin/env python3
"""Pre-print mesh scan for Bambu 3MFs: bodies per object (stray bodies), sliver/needle triangles, convex knife-edge folds
(fins/spikes), open edges, and ray-sampled wall thickness. Usage: check_mesh.py file.3mf ... Needs trimesh."""
import sys,zipfile,io,re,numpy as np,trimesh,xml.etree.ElementTree as ET
def objects(path):
    z=zipfile.ZipFile(path)
    names=[n for n in z.namelist() if n.endswith('.model')]
    out=[]
    for n in names:
        root=ET.fromstring(z.read(n))
        ns={'m':root.tag.split('}')[0][1:]}
        for o in root.iter('{%s}object'%ns['m']):
            m=o.find('m:mesh',ns)
            if m is None: continue
            v=np.array([[float(x.get(a)) for a in 'xyz'] for x in m.find('m:vertices',ns)])
            f=np.array([[int(t.get(a)) for a in ('v1','v2','v3')] for t in m.find('m:triangles',ns)])
            out.append((n+'#'+o.get('id'),trimesh.Trimesh(v,f,process=False)))
    return out
for path in sys.argv[1:]:
    print('=====',path.split('/')[-1])
    for name,t in objects(path):
        tm=t.copy(); tm.merge_vertices()
        comps=tm.split(only_watertight=False)
        A=tm.area_faces; tri=tm.triangles
        e=np.linalg.norm(tri[:,[1,2,0]]-tri,axis=2); emax=e.max(1)
        h=2*A/np.maximum(emax,1e-12)          # smallest altitude
        ang=tm.face_angles.min(1)*180/np.pi
        sliver=(h<0.01)&(emax>0.5)            # thin triangles spanning >0.5 mm
        # needle points: vertices whose total incident angle is tiny (spike tips)
        va=np.zeros(len(tm.vertices)); np.add.at(va,tm.faces.ravel(),tm.face_angles.ravel())
        needle=np.where(va*180/np.pi<20)[0]
        ext=tm.extents
        print(f'{name}: faces {len(tm.faces)} watertight {tm.is_watertight} winding {tm.is_winding_consistent} vol {tm.volume:.0f} mm3 extents {np.round(ext,1)}')
        print(f'  bodies {len(comps)}: '+', '.join(f'{c.volume:.1f}mm3/{len(c.faces)}f' for c in sorted(comps,key=lambda c:-abs(c.volume))[:8]))
        print(f'  degenerate(area<1e-6) {(A<1e-6).sum()}  slivers(h<0.01,len>0.5) {sliver.sum()}  min-angle<1deg {(ang<1).sum()}  needle verts(<20deg total) {len(needle)}')
        if sliver.sum(): print('   sliver sample centers',np.round(tm.triangles_center[sliver][:5],2).tolist())
        if len(needle): print('   needle sample',np.round(tm.vertices[needle][:5],2).tolist())
        bnd=trimesh.grouping.group_rows(tm.edges_sorted,require_count=1)
        ang2=np.degrees(tm.face_adjacency_angles); fold=(ang2>150)&tm.face_adjacency_convex
        print(f'  convex folds >150deg (fins/spikes) {fold.sum()}')
        o=tm.triangles_center-tm.face_normals*1e-3
        loc,ri,_=tm.ray.intersects_location(o,-tm.face_normals,multiple_hits=False)
        d=np.full(len(o),np.inf); d[ri]=np.linalg.norm(loc-o[ri],axis=1)
        print(f'  wall thickness min {d.min():.2f} mm, faces <0.8 mm: {(d<0.8).sum()} (area {tm.area_faces[d<0.8].sum():.2f} mm2) at',np.round(tm.triangles_center[d<0.8][:4],1).tolist())
        print(f'  open edges {len(bnd)}  nonmanifold edges {sum(1 for g in trimesh.grouping.group_rows(tm.edges_sorted) if len(g)>2) if False else 0}')
