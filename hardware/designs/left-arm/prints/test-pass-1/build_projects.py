"""Test pass 1 print projects. Each mesh goes into the reviewed P2 L04B seed project, so every setting is the P1 and P2 process, then Bambu Studio slices it offline."""
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import zipfile
from pathlib import Path
import numpy as np

HERE = Path(__file__).resolve().parent
TASK = HERE.parent.parent
SEED = TASK / 'LEFT_P2_L04B_PRINT_REVIEW_20261105' / 'inputs' / 'P2_LEFT_L04B_04HF_CURRENT_SCHEMA_INPUT_HOLD.3mf'
HOLD = TASK / 'LEFT_P2_L04B_PRINT_REVIEW_20261105' / 'print-HOLD' / 'LEFT_P2_L04B_04HF_100R_6W.3mf'
PARTS = Path('J:/code/marengo/cad/parts/left_arm/test-pass-1')
EXE = 'C:/Program Files/Bambu Studio/bambu-studio.exe'
BED = 256.0
UP_X = lambda v: v[:, [1, 2, 0]]  # +X up, cyclic, keeps handedness
DOWN_X = lambda v: np.c_[v[:, 2], v[:, 1], -v[:, 0]]  # -X up, outboard face on the bed


def read_stl(p):
    b = Path(p).read_bytes()
    n = struct.unpack('<I', b[80:84])[0]
    t = np.frombuffer(b, np.dtype([('n', '<3f4'), ('v', '<9f4'), ('a', '<u2')]), n, 84)['v'].reshape(-1, 3).astype(float)
    return t * 1000 if np.ptp(t, axis=0).max() < 5 else t



PLA = 'Bambu PLA Basic @BBL P2S'
FDIR = Path(os.environ.get('APPDATA', '')) / 'BambuStudio/system/BBL/filament'
SKIP = {'name', 'from', 'inherits', 'type', 'instantiation', 'setting_id', 'include', 'compatible_printers', 'filament_extruder_variant'}


def filament(n):
    """A Bambu system filament profile with its inherits chain resolved."""
    p = FDIR / f'{n}.json'
    if not p.exists():
        return {}
    d = json.loads(p.read_text(encoding='utf8'))
    b = filament(d['inherits']) if 'inherits' in d else {}
    b.update(d)
    return b


def to_pla(ps):
    """Swap the project's PETG for Bambu PLA Basic, keeping the process (walls, infill, supports) as it is."""
    f = filament(PLA)
    if not f:
        raise SystemExit(f'no {PLA} profile in {FDIR}')
    for k, v in f.items():
        if k in SKIP or k not in ps:
            continue
        cur = ps[k]
        if isinstance(cur, list) and isinstance(v, list) and len(v) != len(cur):
            v = (v * len(cur))[:len(cur)] if len(v) == 1 else v[:len(cur)] if len(v) > len(cur) else v + [v[-1]] * (len(cur) - len(v))
        ps[k] = v
    ps['filament_settings_id'] = [PLA]
    if 'filament_ids' in ps:
        ps['filament_ids'] = [f.get('filament_id', '')]
    return ps


PLA_JOBS = ('dummy_housing', 'dummy_output')  # rig fixtures


def clamp_zone(lo, hi):
    """100% infill round the six carrier screws and under the clamp face, so tests 7 and 8 load the joint, not sparse infill. UP_X coordinates."""
    cy, cz = 209.860668, 5.985875
    parts = [disc(cy - 36.5 * np.cos(np.radians(a)), cz + 36.5 * np.sin(np.radians(a)), 6.0, 151.6, 172.1) for a in (63.9, 103.9, 143.9, 223.9, 263.9, 303.9)]
    parts.append(disc(cy, cz, 44.0, 151.6, 154.8, 96))
    return np.vstack(parts)


MOD = {'dummy_housing': (clamp_zone, {'sparse_infill_density': '100%'})}

def box(lo, hi):
    (x0, y0, z0), (x1, y1, z1) = lo, hi
    c = np.array([[x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0], [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1]], float)
    f = [(0, 2, 1), (0, 3, 2), (4, 5, 6), (4, 6, 7), (0, 1, 5), (0, 5, 4), (1, 2, 6), (1, 6, 5), (2, 3, 7), (2, 7, 6), (3, 0, 4), (3, 4, 7)]
    return c[np.array(f)].reshape(-1, 3)


def coupons():
    """Five bars standing and five lying flat, 80 x 12 x 6, on one plate."""
    v = []
    for i in range(5):
        x = 24.0 * i
        v.append(box((x, 0, 0), (x + 12, 6, 80)))
    for i in range(5):
        y = 26 + 20.0 * i
        v.append(box((5, y, 0), (85, y + 12, 6)))
    return np.vstack(v)


JOBS = {
    'PF03_roll_spacer': lambda: read_stl(TASK / 'FINAL_PRINT_REVIEW_20261104_HOLD/spacer-process/SPACER_PF03_AXIS_VERTICAL_HOLD_mm.stl'),
    'layer_coupons': coupons,
    'elbow_carrier': lambda: DOWN_X(read_stl(PARTS / 'test_carrier.STL')),
    'elbow_carrier_strength': lambda: UP_X(read_stl(PARTS / 'test_carrier.STL')),
    'dummy_housing': lambda: UP_X(read_stl(PARTS / 'test_dummy_housing.STL')),
    'dummy_output': lambda: output_plate(),
    'fork_sector': lambda: DOWN_X(read_stl(PARTS / 'test_fork_sector.STL')),
}
# settings that differ from the P2 project, on purpose
BRIM = {'brim_type': 'outer_only', 'brim_width': '5', 'curr_bed_type': 'Supertack Plate'}
EXTRA = {'dummy_housing': {'sparse_infill_density': '25%'}}


def output_plate():
    """Dummy output with its cheek face on the bed, and the M8 spacer beside it."""
    o = DOWN_X(read_stl(PARTS / 'test_dummy_output.STL'))
    s = UP_X(read_stl(PARTS / 'test_m8_spacer.STL'))
    o = o - [o[:, 0].min(), o[:, 1].min(), o[:, 2].min()]
    s = s - [s[:, 0].min(), s[:, 1].min(), s[:, 2].min()] + [45.0, 6.0, 0]
    return np.vstack([o, s])


def disc(cx, cy, r, z0, z1, n=48):
    """Solid cylinder, as triangles, for a support blocker. Plate-up coordinates of the job."""
    a = np.linspace(0, 2 * np.pi, n, endpoint=False)
    p = np.c_[cx + r * np.cos(a), cy + r * np.sin(a)]
    v = []
    for i in range(n):
        j = (i + 1) % n
        A, B = p[i], p[j]
        v += [[cx, cy, z0], [B[0], B[1], z0], [A[0], A[1], z0]]
        v += [[cx, cy, z1], [A[0], A[1], z1], [B[0], B[1], z1]]
        v += [[A[0], A[1], z0], [B[0], B[1], z0], [B[0], B[1], z1]]
        v += [[A[0], A[1], z0], [B[0], B[1], z1], [A[0], A[1], z1]]
    return np.array(v, float)


def mesh_xml(tri, shift):
    key = np.round(tri, 4)
    uniq, inv = np.unique(key, axis=0, return_inverse=True)
    faces = inv.reshape(-1, 3)
    faces = faces[(faces[:, 0] != faces[:, 1]) & (faces[:, 1] != faces[:, 2]) & (faces[:, 0] != faces[:, 2])]
    uniq = uniq - shift
    vs = chr(10).join(f'     <vertex x="{x:.5f}" y="{y:.5f}" z="{w:.5f}"/>' for x, y, w in uniq)
    ts = chr(10).join(f'     <triangle v1="{a}" v2="{b}" v3="{c}"/>' for a, b, c in faces)
    return vs, ts, len(faces)


def project(name, tri, out):
    key = np.round(tri, 4)
    uniq, inv = np.unique(key, axis=0, return_inverse=True)
    faces = inv.reshape(-1, 3)
    faces = faces[(faces[:, 0] != faces[:, 1]) & (faces[:, 1] != faces[:, 2]) & (faces[:, 0] != faces[:, 2])]
    lo, hi = uniq.min(0), uniq.max(0)
    size = hi - lo
    center = (lo + hi) / 2
    uniq = uniq - center
    if size[0] > BED - 12 or size[1] > BED - 12 or size[2] > BED:
        raise SystemExit(f'{name} does not fit {size}')
    h = size[2] / 2
    z = zipfile.ZipFile(SEED)
    verts = '\n'.join(f'     <vertex x="{x:.5f}" y="{y:.5f}" z="{w:.5f}"/>' for x, y, w in uniq)
    tris = '\n'.join(f'     <triangle v1="{a}" v2="{b}" v3="{c}"/>' for a, b, c in faces)
    obj = z.read('3D/Objects/object_1.model').decode()
    obj = obj[:obj.index('<vertices>')] + '<vertices>\n' + verts + '\n    </vertices>\n    <triangles>\n' + tris + '\n    </triangles>' + obj[obj.index('</triangles>') + len('</triangles>'):]
    mdl = z.read('3D/3dmodel.model').decode()
    mdl = re.sub(r'(<item objectid="2"[^>]*transform=")[^"]*"', rf'\g<1>1 0 0 0 1 0 0 0 1 128 128 {h:.5f}"', mdl)
    cfg = z.read('Metadata/model_settings.config').decode()
    mod = MOD.get(name)
    if mod:
        vs, ts, nm_ = mesh_xml(mod[0](lo, hi), center)
        obj = obj.replace('  </object>', '  </object>' + chr(10) + '  <object id="4" p:UUID="00040000-81cb-4c03-9d28-80fed5dfa1dc" type="model">' + chr(10) + '   <mesh>' + chr(10) + '    <vertices>' + chr(10) + vs + chr(10) + '    </vertices>' + chr(10) + '    <triangles>' + chr(10) + ts + chr(10) + '    </triangles>' + chr(10) + '   </mesh>' + chr(10) + '  </object>', 1)
        mdl = mdl.replace('</components>', ' <component p:path="/3D/Objects/object_1.model" objectid="4" p:UUID="00010002-b206-40ff-9872-83e8017abed1" transform="1 0 0 0 1 0 0 0 1 0 0 0"/>' + chr(10) + '   </components>', 1)
        meta = ''.join(f'      <metadata key="{k}" value="{v}" />' + chr(10) for k, v in mod[1].items())
        cfg = cfg.replace('    </part>', '    </part>' + chr(10) + '    <part id="4" subtype="modifier_part" uuid="51d17843-7a32-47a7-bb0b-71b78baf25d7">' + chr(10) + '      <metadata key="name" value="clamp_solid" />' + chr(10) + f'      <metadata key="matrix" value="1 0 0 0 0 1 0 0 0 0 1 {h:.5f} 0 0 0 1" />' + chr(10) + meta + f'      <mesh_stat face_count="{nm_}" edges_fixed="0" degenerate_facets="0" facets_removed="0" facets_reversed="0" backwards_edges="0" />' + chr(10) + '    </part>', 1)
    cfg = cfg.replace('P2_LEFT_L04B_04HF_100R_6W_DEVELOPMENT_HOLD', name)
    cfg = cfg.replace('174726', str(len(faces)))
    cfg = cfg.replace('value="1 0 0 0 0 1 0 0 0 0 1 51 0 0 0 1"', f'value="1 0 0 0 0 1 0 0 0 0 1 {h:.5f} 0 0 0 1"')
    cfg = cfg.replace('key="source_offset_z" value="51"', f'key="source_offset_z" value="{h:.5f}"')
    ps = json.loads(z.read('Metadata/project_settings.config'))
    ps.update(BRIM)
    ps.update(EXTRA.get(name, {}))
    if name in PLA_JOBS:
        to_pla(ps)
    src = out / f'{name}_input.3mf'
    with zipfile.ZipFile(src, 'w', zipfile.ZIP_DEFLATED) as o:
        for i in z.infolist():
            d = z.read(i.filename)
            if i.filename == '3D/Objects/object_1.model':
                d = obj.encode()
            elif i.filename == '3D/3dmodel.model':
                d = mdl.encode()
            elif i.filename == 'Metadata/model_settings.config':
                d = cfg.encode()
            elif i.filename == 'Metadata/project_settings.config':
                d = json.dumps(ps, indent=4).encode()
            elif i.filename == 'Metadata/plate_1.json' or i.filename.endswith('.png'):
                continue
            o.writestr(i.filename, d)
    return src, size, len(faces)


def run(name):
    out = HERE / 'out' / name
    if out.exists():
        shutil.rmtree(out)
    (out / 'profile').mkdir(parents=True)
    tri = JOBS[name]()
    src, size, nf = project(name, tri, out)
    final = f'{name}.3mf'
    args = [EXE, '--datadir', str(out / 'profile'), '--debug', '1', '--slice', '0', '--export-3mf', final, '--outputdir', str(out), str(src)]
    r = subprocess.run(args, capture_output=True, text=True, timeout=1800, creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS | subprocess.CREATE_NO_WINDOW)
    (out / 'log.txt').write_text(r.stdout[-4000:] + r.stderr[-2000:], errors='replace')
    res = dict(name=name, exit=r.returncode, size_mm=[round(float(v), 1) for v in size], faces=int(nf))
    f = out / final
    if f.exists():
        z = zipfile.ZipFile(f)
        s = z.read('Metadata/slice_info.config').decode(errors='replace')
        res['info'] = re.findall(r'key="(prediction|weight)" value="([^"]*)"', s)
        res['warnings'] = re.findall(r'<warning[^>]*>', s)
        p = json.loads(z.read('Metadata/plate_1.json'))
        res['bbox'] = [round(v, 1) for v in p.get('bbox_all', [])]
        cur = json.loads(z.read('Metadata/project_settings.config'))
        ref = json.loads(zipfile.ZipFile(HOLD).read('Metadata/project_settings.config'))
        res['settings_diff'] = {k: (str(cur.get(k))[:40], str(ref.get(k))[:40]) for k in ref if cur.get(k) != ref.get(k)}
    (out / 'result.json').write_text(json.dumps(res, indent=1))
    print(json.dumps(res)[:900], flush=True)


for n in (sys.argv[1:] or JOBS):
    run(n)
