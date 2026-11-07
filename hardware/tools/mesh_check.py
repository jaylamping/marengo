"""Print-mesh gate: run on every part before a 3MF is written.

Fails unless:
  - the part is one watertight body (or the allowed body count for listed multi-body parts),
  - no vertex has an angle defect above 150 deg, except whitelisted design corners,
  - no knife edge: no pair of adjacent faces folded past 170 deg along an edge longer than 1 mm,
    except whitelisted spots,
  - no sampled wall thinner than 0.6 mm over 0.5 mm2 or more, except whitelisted boxes.

Needle triangles (min angle under 1 deg, edge over 2 mm) are counted and listed, not failed:
CAD tessellation makes thousands of them on flat fans, so a needle only matters when it forms a
knife edge or a thin wall, which the two checks above catch.

Whitelists are per part: a point (or box) in the coordinates of the mesh being checked, plus a reason.
"""
import numpy as np
import trimesh

DEFECT = 150.0
FOLD = 170.0
THIN = 0.6
THIN_AREA = 0.5


def _near(p, wl, r=1.5):
    return [w for w in wl if 'pt' in w and np.linalg.norm(np.asarray(p) - w['pt']) <= w.get('r', r)]


def _inbox(lo, hi, wl):
    out = []
    for w in wl:
        if 'box' in w:
            a, b = np.asarray(w['box'][0]), np.asarray(w['box'][1])
            if np.all(lo >= a - 1e-6) and np.all(hi <= b + 1e-6):
                out.append(w)
    return out


def thickness(m, chunk=3000):
    n = m.face_normals
    c = m.triangles_center - n * 0.005
    it = trimesh.ray.ray_triangle.RayMeshIntersector(m)
    th = np.full(len(c), np.inf)
    for s in range(0, len(c), chunk):
        loc, ri, _ = it.intersects_location(c[s:s + chunk], -n[s:s + chunk], multiple_hits=False)
        th[s + ri] = np.linalg.norm(loc - c[s:s + chunk][ri], axis=1)
    return th


def check(mesh, name, bodies=1, corners=(), folds=(), thin=()):
    """mesh: trimesh.Trimesh or (vertices, faces). Returns (ok, report text)."""
    m = mesh if isinstance(mesh, trimesh.Trimesh) else trimesh.Trimesh(*mesh, process=True)
    rep, ok = [f'mesh check: {name}', f'faces {len(m.faces)}, bounds {m.bounds.round(2).tolist()}'], True
    parts = m.split(only_watertight=False)
    good = (bodies is None or len(parts) == bodies) and m.is_watertight
    rep.append(f'[{"PASS" if good else "FAIL"}] bodies {len(parts)} (allowed {bodies}), watertight {m.is_watertight}')
    ok &= good
    s = np.zeros(len(m.vertices))
    np.add.at(s, m.faces.ravel(), m.face_angles.ravel())
    defect = 360 - np.degrees(s)
    bad, known = [], []
    for i in np.where(defect > DEFECT)[0]:
        p = m.vertices[i]
        w = _near(p, corners)
        (known if w else bad).append((p.round(2).tolist(), round(float(defect[i])), w[0]['reason'] if w else ''))
    rep.append(f'[{"PASS" if not bad else "FAIL"}] sharp vertices over {DEFECT:.0f} deg: {len(bad)} unlisted, {len(known)} listed design corners')
    rep += [f'    UNLISTED {p} {d} deg' for p, d, _ in bad[:30]]
    rep += [f'    listed   {p} {d} deg: {r}' for p, d, r in known]
    ok &= not bad
    ang = np.degrees(m.face_adjacency_angles)
    el = np.linalg.norm(np.diff(m.vertices[m.face_adjacency_edges], axis=1)[:, 0], axis=1)
    fb, fk = [], []
    for j in np.where((ang > FOLD) & (el > 1.0))[0]:
        mid = m.vertices[m.face_adjacency_edges[j]].mean(0)
        w = _near(mid, folds, 2.0)
        (fk if w else fb).append((mid.round(2).tolist(), round(float(ang[j]), 1), round(float(el[j]), 1), w[0]['reason'] if w else ''))
    rep.append(f'[{"PASS" if not fb else "FAIL"}] knife edges (fold over {FOLD:.0f} deg, edge over 1 mm): {len(fb)} unlisted, {len(fk)} listed')
    rep += [f'    UNLISTED {p} fold {a} deg, edge {e} mm' for p, a, e, _ in fb[:30]]
    rep += [f'    listed   {p} fold {a} deg, edge {e} mm: {r}' for p, a, e, r in fk[:10]]
    ok &= not fb
    fl = m.edges_unique_length[m.faces_unique_edges].max(1)
    needle = (np.degrees(m.face_angles.min(1)) < 1) & (fl > 2)
    rep.append(f'[INFO] needle triangles (min angle under 1 deg, edge over 2 mm): {int(needle.sum())}, tessellation only unless a check above fails')
    th = thickness(m)
    k = np.where((th < THIN) & (m.area_faces > 0.02))[0]
    tb, tk = [], []
    if len(k):
        from scipy.sparse import coo_matrix
        from scipy.sparse.csgraph import connected_components
        adj = m.face_adjacency[np.isin(m.face_adjacency, k).all(1)]
        idx = {f: i for i, f in enumerate(k)}
        A = coo_matrix((np.ones(len(adj)), ([idx[a] for a in adj[:, 0]], [idx[b] for b in adj[:, 1]])), shape=(len(k), len(k)))
        _, lab = connected_components(A, directed=False)
        for j in np.unique(lab):
            f = k[lab == j]
            area = float(m.area_faces[f].sum())
            if area < THIN_AREA:
                continue
            v = m.triangles[f].reshape(-1, 3)
            lo, hi = v.min(0), v.max(0)
            w = _inbox(lo, hi, thin)
            (tk if w else tb).append((round(area, 1), round(float(th[f].min()), 2), lo.round(1).tolist(), hi.round(1).tolist(), w[0]['reason'] if w else ''))
    rep.append(f'[{"PASS" if not tb else "FAIL"}] walls thinner than {THIN} mm over {THIN_AREA} mm2: {len(tb)} unlisted, {len(tk)} listed')
    rep += [f'    UNLISTED {a} mm2, min {t} mm, box {lo} to {hi}' for a, t, lo, hi, _ in sorted(tb, reverse=True)[:30]]
    rep += [f'    listed   {a} mm2, min {t} mm, box {lo} to {hi}: {r}' for a, t, lo, hi, r in sorted(tk, reverse=True)]
    ok &= not tb
    rep.append(f'RESULT: {"PASS" if ok else "FAIL"}')
    return ok, '\n'.join(rep) + '\n'


def find_up(start, rel):
    """First existing path start/../rel walking up, also trying a sibling _repo checkout."""
    from pathlib import Path
    for p in [Path(start).resolve(), *Path(start).resolve().parents]:
        for base in (p, p / '_repo'):
            if (base / rel).exists():
                return base / rel
    raise FileNotFoundError(rel)


def gate(name, tri, out_dir, src=None, whitelist=None):
    """Build-script hook. tri: (n*3, 3) triangle vertices as sent to the slicer. If src (a source STL in CAD
    coordinates) is given, that is what gets checked, so whitelist coordinates stay in CAD coordinates.
    Writes <out_dir>/<name>_meshcheck.txt and raises SystemExit on FAIL."""
    from pathlib import Path
    wl = (whitelist or {}).get(name, {})
    if src is not None:
        m = trimesh.load(str(src), force='mesh')
        frame = f'source {Path(src).name}, CAD coordinates'
    else:
        t = np.asarray(tri, float)
        m = trimesh.Trimesh(t, np.arange(len(t)).reshape(-1, 3), process=True)
        frame = 'plate coordinates'
    ok, rep = check(m, f'{name} ({frame})', bodies=wl.get('bodies', 1), corners=wl.get('corners', ()), folds=wl.get('folds', ()), thin=wl.get('thin', ()))
    Path(out_dir).mkdir(parents=True, exist_ok=True)
    (Path(out_dir) / f'{name}_meshcheck.txt').write_text(rep, encoding='utf8', newline='\n')
    print(rep, flush=True)
    if not ok:
        raise SystemExit(f'mesh check FAILED for {name}; see {name}_meshcheck.txt')
    return rep
