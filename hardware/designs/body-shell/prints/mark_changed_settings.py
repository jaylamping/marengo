#!/usr/bin/env python3
"""Mark a Bambu 3MF's changed settings (different_settings_to_system) so Bambu Studio keeps them on open instead of
reloading preset defaults (e.g. infill back to 15%). Compares against the BBL system profiles on GitHub.
Usage: mark_changed_settings.py in.3mf out.3mf "<system filament profile, e.g. Bambu PETG HF @BBL P2S 0.4 nozzle>"
The printer slot is left empty so Studio loads its current machine G-code."""
import json,sys,zipfile,urllib.request,urllib.parse,os,shutil
BASE='https://raw.githubusercontent.com/bambulab/BambuStudio/master/resources/profiles/BBL/'
cache={}
def prof(kind,name):
    if (kind,name) in cache: return cache[(kind,name)]
    u=BASE+kind+'/'+urllib.parse.quote(name)+'.json'
    try: d=json.load(urllib.request.urlopen(u))
    except Exception as e: print('miss',kind,name,e); return None
    b=prof(kind,d['inherits']) if d.get('inherits') else {}
    b=dict(b or {}); b.update(d); cache[(kind,name)]=b; return b
SKIP={'name','from','inherits','type','instantiation','setting_id','include','compatible_printers','compatible_printers_condition','compatible_prints','compatible_prints_condition','version','print_settings_id','filament_settings_id','printer_settings_id','inherits_group','different_settings_to_system','filament_extruder_variant','print_extruder_variant','printer_extruder_variant','renamed_from','description','filament_id'}
def n1(x):
    x=str(x)
    try: return repr(float(x.rstrip('%')))
    except ValueError: return x
def norm(v):
    if isinstance(v,list): return [n1(x) for x in v]
    return n1(v)
def diff(ps,ref):
    out=[]
    for k,v in ref.items():
        if k in SKIP or k not in ps: continue
        a,b=norm(ps[k]),norm(v)
        if isinstance(a,list) and not isinstance(b,list): b=[b]
        if isinstance(b,list) and not isinstance(a,list): a=[a]
        if isinstance(a,list) and len(b)==1 and len(a)>1: b=b*len(a)
        if a!=b: out.append(k)
    return sorted(out)
src,dst=sys.argv[1],sys.argv[2]
z=zipfile.ZipFile(src); ps=json.loads(z.read('Metadata/project_settings.config'))
P=prof('process',ps['print_settings_id']); F=prof('filament',sys.argv[3]); M=prof('machine',ps['printer_settings_id'])
dp,df,dm=diff(ps,P),diff(ps,F),diff(ps,M)
print('process:',dp);print('filament:',len(df),df[:40]);print('printer:',dm)
dp=sorted(set(dp)|{k for k in ('brim_type','brim_width','support_type','support_threshold_angle','support_on_build_plate_only','support_top_z_distance','support_interface_top_layers','support_interface_spacing','seam_position','sparse_infill_density','wall_loops','line_width','outer_wall_line_width','inner_wall_line_width','enable_support') if k in ps})
print('final process:',dp)
ps['different_settings_to_system']=[';'.join(dp),';'.join(df),'']
ps['inherits_group']=['', '', '']
with zipfile.ZipFile(dst,'w',zipfile.ZIP_DEFLATED) as o:
    for i in z.infolist():
        data=z.read(i.filename)
        if i.filename=='Metadata/project_settings.config': data=json.dumps(ps,indent=4).encode()
        o.writestr(i,data)
