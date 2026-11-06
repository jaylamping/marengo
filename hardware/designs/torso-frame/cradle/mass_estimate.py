"""Print mass of each cradle STL: solid, and 4 walls + 15 % gyroid (voxel estimate). Also checks mesh bounds."""
from pathlib import Path

import numpy as np
import trimesh
from scipy import ndimage

RHO = 1.27e-3  # g/mm3, PETG
PITCH = 0.4  # mm
WALL = 4 * 0.42  # mm, 4 perimeters of 0.42 lines; top and bottom skins treated the same


def fill(mesh):
    """Voxel grid by parity along Y through the triangle soup, no third-party ray engine needed."""
    lo, hi = mesh.bounds
    xs = np.arange(lo[0] + PITCH / 2, hi[0], PITCH)
    ys = np.arange(lo[1] + PITCH / 2, hi[1], PITCH)
    zs = np.arange(lo[2] + PITCH / 2, hi[2], PITCH)
    hits = [[] for _ in range(len(xs) * len(zs))]
    jitter = np.array([1e-4, 0, 2e-4])  # keeps columns off shared triangle edges
    for tri in mesh.triangles + jitter:
        a, b, c = tri[:, [0, 2]]
        det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1])
        if abs(det) < 1e-12:
            continue
        ix = np.where((xs >= tri[:, 0].min()) & (xs <= tri[:, 0].max()))[0]
        iz = np.where((zs >= tri[:, 2].min()) & (zs <= tri[:, 2].max()))[0]
        for i in ix:
            for k in iz:
                u = ((b[1] - c[1]) * (xs[i] - c[0]) + (c[0] - b[0]) * (zs[k] - c[1])) / det
                v = ((c[1] - a[1]) * (xs[i] - c[0]) + (a[0] - c[0]) * (zs[k] - c[1])) / det
                if u >= 0 and v >= 0 and u + v <= 1:
                    hits[i * len(zs) + k].append(u * tri[0, 1] + v * tri[1, 1] + (1 - u - v) * tri[2, 1])
    out = np.zeros((len(xs), len(ys), len(zs)), bool)
    for i in range(len(xs)):
        for k in range(len(zs)):
            h = sorted(hits[i * len(zs) + k])
            for y0, y1 in zip(h[::2], h[1::2]):
                out[i, (ys > y0) & (ys < y1), k] = True
    return out


def main():
    for path in sorted(Path(__file__).parent.glob("*.stl")):
        mesh = trimesh.load(path)
        grid = np.pad(fill(mesh), 1)
        depth = ndimage.distance_transform_edt(grid) * PITCH
        shell = (depth > 0) & (depth <= WALL)
        core = depth > WALL
        v_vox = grid.sum() * PITCH**3
        v_print = (shell.sum() + 0.15 * core.sum()) * PITCH**3
        print(path.stem, "watertight", mesh.is_watertight, "bounds", np.round(mesh.bounds, 2).tolist())
        print(f"  volume {mesh.volume:.0f} mm3, solid {mesh.volume * RHO:.1f} g; 4 walls/15 % {v_print / v_vox * mesh.volume * RHO:.1f} g"
              f" (shell share {shell.sum() / grid.sum():.0%})")


if __name__ == "__main__":
    main()
