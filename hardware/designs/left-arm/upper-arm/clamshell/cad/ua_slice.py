"""Slice the upper-arm clamshell parts with the P2 process from build.py (6 walls, PETG, 0.2 mm) and report grams.
Halves at 50% infill with 100% modifiers on the seam columns, the lug recesses, the spigot seat band and the ring capture;
4 walls outside those zones; ring and loop clip at 100%. usage: python ua_slice.py [upper lower ring hub carrier]"""
import json, re, shutil, subprocess, sys, zipfile
from pathlib import Path
import numpy as np

HERE = Path(__file__).resolve().parent
src = (HERE / 'build.py').read_text(encoding='utf-8')
exec(compile(src[:src.index('for n in (sys.argv[1:] or JOBS):')], 'build.py', 'exec'))

U = HERE / 'stl' / 'ua'
CY, CZ = 209.860668, 5.985875
TUBE_COLS = [(147.5, 262.0), (147.5, 278.0), (193.5, 262.0), (193.5, 278.0)]   # seam posts (scripts/ua_parts.py)
HUB_COLS = [(147.5, 292.0), (193.5, 292.0)]
RING_PINS = [(158.5, 246.5)]

import trimesh
_CLIP_R = trimesh.geometry.align_vectors([0, -0.70710678, -0.70710678], [0, 0, 1])[:3, :3]
ORIENT = {   # arm coordinates -> plate coordinates (z up), all proper rotations
    'upper': lambda v: v,                                   # seam face (Z 6) on the bed
    'lower': lambda v: np.c_[v[:, 0], -v[:, 1], -v[:, 2]],  # seam face on the bed
    'ring': UP_X,
    'hub': lambda v: np.c_[v[:, 0], v[:, 2], -v[:, 1]],     # RS02 face (Y 319.9) on the bed
    'clip': lambda v: v @ _CLIP_R.T,                       # stands on its flat end, groove direction up
}
FILES = {'upper': 'ua_upper', 'lower': 'ua_lower', 'ring': 'ua_ring', 'hub': 'ua_hub', 'clip': 'ua_loop_clip'}


def half_zones():
    """100% zones in arm coordinates: column discs, lug recesses, spigot-seat band, ring capture about the elbow axis."""
    z = [disc(x, y, 6.0, -40.0, 55.0) for x, y in TUBE_COLS + HUB_COLS + RING_PINS]
    z.append(box_mesh((100.0, 286.0, -40.0), (240.0, 303.0, 55.0)))
    ring = disc(CY, CZ, 52.0, 149.0, 168.0, 96)       # built as (Y, Z, X)
    z.append(ring[:, [2, 0, 1]])
    return np.vstack(z)


for k in ('upper', 'lower'):
    MOD[k] = (lambda lo, hi, k=k: ORIENT[k](half_zones()), {'sparse_infill_density': '100%', 'wall_loops': '6'})
    EXTRA[k] = {'sparse_infill_density': '50%', 'wall_loops': '4'}
HUB_HOLES = [(159.4, 10.5), (161.0, -1.4), (172.1, -5.9), (181.6, 1.5), (180.0, 13.4), (168.8, 17.9)]


def hub_zones():
    """Hub 100% zones: the clamped RS02 plate (Y 313.9-320), the M4 bosses, the spigot band and the seam lugs."""
    z = [disc(x, zz, 7.0, 286.0, 321.0)[:, [0, 2, 1]] for x, zz in HUB_HOLES]   # built as (X, Z, Y)
    z += [box_mesh((100.0, 313.5, -60.0), (240.0, 321.0, 70.0)), box_mesh((100.0, 286.0, -60.0), (240.0, 303.5, 70.0))]
    return np.vstack(z)


MOD['hub'] = (lambda lo, hi: ORIENT['hub'](hub_zones()), {'sparse_infill_density': '100%'})
EXTRA['hub'] = {'sparse_infill_density': '50%'}
BLOCK.pop('carrier', None)
EXTRA['clip'] = {'enable_support': '0'}   # no support anywhere: short spigot cantilevers and a 2 mm tunnel bridge only


def slice_part(name):
    out = HERE / 'out' / ('ua_' + name)
    if out.exists():
        shutil.rmtree(out)
    (out / 'profile').mkdir(parents=True)
    tri = ORIENT[name](read_stl(U / (FILES[name] + '.STL')))
    srcf, size, nf = project(name, tri, out)
    final = f'{name}.3mf'
    args = [EXE, '--datadir', str(out / 'profile'), '--debug', '1', '--slice', '0', '--export-3mf', final, '--outputdir', str(out), str(srcf)]
    r = subprocess.run(args, capture_output=True, text=True, timeout=1800, creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS | subprocess.CREATE_NO_WINDOW)
    (out / 'log.txt').write_text(r.stdout[-4000:] + r.stderr[-2000:], errors='replace')
    res = dict(name=name, exit=r.returncode, size_mm=[round(float(v), 1) for v in size])
    f = out / final
    if f.exists():
        s = zipfile.ZipFile(f).read('Metadata/slice_info.config').decode(errors='replace')
        res.update({k: v for k, v in re.findall(r'key="(prediction|weight)" value="([^"]*)"', s)})
        res['warnings'] = re.findall(r'<warning[^>]*>', s)
        ps = json.loads(zipfile.ZipFile(f).read('Metadata/project_settings.config'))
        res['walls'], res['infill'] = ps.get('wall_loops'), ps.get('sparse_infill_density')
        res['support_g'] = re.findall(r'key="support_used" value="([^"]*)"', s)
        res['support_on'] = ps.get('enable_support')
    (out / 'result.json').write_text(json.dumps(res, indent=1))
    print(json.dumps(res), flush=True)
    return res


if __name__ == '__main__':
    rs = [slice_part(n) for n in (sys.argv[1:] or ['upper', 'lower', 'ring', 'hub', 'clip'])]
    print('total g', round(sum(float(r.get('weight', 0)) for r in rs if r['name'] != 'clip'), 1))
