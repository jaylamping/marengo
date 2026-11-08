"""Support-contact check for a print orientation: which faces need support (overhang steeper than the limit, not on the bed),
and which of those are visible outer faces (a ray from the face along its normal escapes the part).
usage: python support_check.py MESH.stl ux uy uz [limit_deg=45] [png]"""
import sys, numpy as np, trimesh


def check(m, up, limit=45.0, bed_tol=0.3, ref=None):
    """ref: the assembled outer body (or several parts concatenated); visibility is judged against it, so cavity faces of a
    split half that face the open cut do not count as outer."""
    up = np.asarray(up, float); up /= np.linalg.norm(up)
    n = m.face_normals; c = m.triangles_center; a = m.area_faces
    h = c @ up; h0 = (m.vertices @ up).min()
    down = n @ up < -np.cos(np.radians(limit))          # face points down more than the limit from vertical
    on_bed = h - h0 < bed_tol
    need = down & ~on_bed
    # visible from outside: a ray from just off the face along its normal does not hit the part again
    o = c[need] + n[need] * 0.05
    R = m if ref is None else ref
    hit = R.ray.intersects_any(o, n[need]) if need.any() else np.zeros(0, bool)
    outer = np.zeros(len(n), bool); outer[np.flatnonzero(need)[~hit]] = True
    inner = need & ~outer
    return dict(support_mm2=round(float(a[need].sum()), 1), outer_mm2=round(float(a[outer].sum()), 1), inner_mm2=round(float(a[inner].sum()), 1)), outer, inner


if __name__ == '__main__':
    m = trimesh.load(sys.argv[1], force='mesh')
    up = [float(v) for v in sys.argv[2:5]]
    lim = float(sys.argv[5]) if len(sys.argv) > 5 else 45.0
    r, outer, inner = check(m, up, lim)
    print(r)
    if len(sys.argv) > 6:
        import matplotlib; matplotlib.use('Agg'); import matplotlib.pyplot as plt
        exec(open('raster.py').read().split("if __name__")[0])
        U = np.asarray(up, float); H = np.cross(U, [1, 0, 0] if abs(U[0]) < 0.9 else [0, 1, 0]); H /= np.linalg.norm(H); H2 = np.cross(U, H)
        fig, ax = plt.subplots(1, 2, figsize=(20, 10))
        for a_, d in zip(ax, (1.1 * U + H - H2, 1.1 * U - H + H2)):
            ms = m.copy(); col = np.tile([235, 233, 225, 255], (len(m.faces), 1)); col[inner] = [80, 140, 230, 255]; col[outer] = [220, 40, 40, 255]
            img = view(m, d, U, 900)
            a_.imshow(img, cmap='gray'); a_.axis('off')
        plt.savefig(sys.argv[6], dpi=60)
