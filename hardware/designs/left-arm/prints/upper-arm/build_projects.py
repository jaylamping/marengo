"""Upper arm print projects. Each mesh goes into the reviewed P2 L04B seed project, so every setting is the P1 and P2 process, then Bambu Studio slices it offline."""
import json
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


S = HERE / 'stl'
YZ = lambda v: np.c_[v[:, 2], v[:, 0], v[:, 1]]  # +Y up, cyclic
JOBS = {
    'upper_arm': lambda: UP_X(read_stl(S / 'UA2_ARM.STL')),
    'carrier': lambda: UP_X(read_stl(S / 'UA2_CARRIER.STL')),
    'p2': lambda: read_stl(S / 'P2_L05.STL'),
    'fit_coupons': lambda: coupons_plate(),
    'guard': lambda: YZ(read_stl(S / 'GUARD.STL')),
    'seat_ring_only': lambda: UP_X(read_stl(S / 'coupon_seat.STL')),
    'cover': lambda: YZ(read_stl(S / 'UA2_COVER.STL')),
}
SEAT_XY = [0.0, 0.0]


def coupons_plate():
    """Hoop ring, bearing seat ring, hub core and carrier sector, cut from the final parts, printed the same way up."""
    out = []
    dx = 0.0
    for n in ('coupon_hoop', 'coupon_seat', 'coupon_hub', 'coupon_carrier'):
        v = UP_X(read_stl(S / f'{n}.STL'))
        v = v - v.min(0)
        if n == 'coupon_seat':
            SEAT_XY[:] = [dx + np.ptp(v[:, 0]) / 2, np.ptp(v[:, 1]) / 2]
        if n == 'coupon_carrier':  # beside the hub core, the row is full
            out.append(v + [hx, np.ptp(hub[:, 1]) + 12, 0])
            continue
        hx, hub = dx, v
        out.append(v + [dx, 0, 0])
        dx += np.ptp(v[:, 0]) + 12
    return np.vstack(out)


# settings that differ from the P2 project, on purpose
BRIM = {'brim_type': 'outer_only', 'brim_width': '5', 'curr_bed_type': 'Supertack Plate'}
# coupons only check fits, so they don't need the structural walls and infill
FIT = {'wall_loops': '3', 'sparse_infill_density': '15%'}
EXTRA = {'fit_coupons': FIT, 'seat_ring_only': FIT}
# keeps support out of the bearing seat, r 28 about the elbow axis, carrier inboard face down
BLOCK = {'carrier': lambda lo, hi: disc((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, 28.0, lo[2] - 1, hi[2] + 1),
         'seat_ring_only': lambda lo, hi: disc((lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2, 28.0, lo[2] - 1, hi[2] + 1),
         'fit_coupons': lambda lo, hi: disc(lo[0] + SEAT_XY[0], lo[1] + SEAT_XY[1], 28.0, lo[2] - 1, hi[2] + 1)}


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
    blk = BLOCK.get(name)
    mdl = z.read('3D/3dmodel.model').decode()
    if name != 'p2':
        mdl = re.sub(r'(<item objectid="2"[^>]*transform=")[^"]*"', rf'\g<1>1 0 0 0 1 0 0 0 1 128 128 {h:.5f}"', mdl)
    cfg = z.read('Metadata/model_settings.config').decode()
    if blk:
        vs, ts, nb = mesh_xml(blk(lo, hi), center)
        obj = obj.replace('  </object>', '  </object>' + chr(10) + '  <object id="3" p:UUID="00030000-81cb-4c03-9d28-80fed5dfa1dc" type="model">' + chr(10) + '   <mesh>' + chr(10) + '    <vertices>' + chr(10) + vs + chr(10) + '    </vertices>' + chr(10) + '    <triangles>' + chr(10) + ts + chr(10) + '    </triangles>' + chr(10) + '   </mesh>' + chr(10) + '  </object>', 1)
        mdl = mdl.replace('</components>', ' <component p:path="/3D/Objects/object_1.model" objectid="3" p:UUID="00010001-b206-40ff-9872-83e8017abed1" transform="1 0 0 0 1 0 0 0 1 0 0 0"/>' + chr(10) + '   </components>', 1)
        cfg = cfg.replace('    </part>', '    </part>' + chr(10) + f'    <part id="3" subtype="support_blocker" uuid="51d17843-7a32-47a7-bb0b-71b78baf25d6">' + chr(10) + '      <metadata key="name" value="blocker" />' + chr(10) + f'      <metadata key="matrix" value="1 0 0 0 0 1 0 0 0 0 1 {h:.5f} 0 0 0 1" />' + chr(10) + f'      <mesh_stat face_count="{nb}" edges_fixed="0" degenerate_facets="0" facets_removed="0" facets_reversed="0" backwards_edges="0" />' + chr(10) + '    </part>', 1)
    cfg = cfg.replace('P2_LEFT_L04B_04HF_100R_6W_DEVELOPMENT_HOLD', name)
    cfg = cfg.replace('174726', str(len(faces)))
    cfg = cfg.replace('value="1 0 0 0 0 1 0 0 0 0 1 51 0 0 0 1"', f'value="1 0 0 0 0 1 0 0 0 0 1 {h:.5f} 0 0 0 1"')
    cfg = cfg.replace('key="source_offset_z" value="51"', f'key="source_offset_z" value="{h:.5f}"')
    ps = json.loads(z.read('Metadata/project_settings.config'))
    ps.update(BRIM)
    ps.update(EXTRA.get(name, {}))
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
