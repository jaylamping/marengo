#!/usr/bin/env python3
"""Body shell split study: thicken the concept shell into solid panels, cut them on the proposed seams,
and check every print against the Bambu P2S (256 x 256 x 256 mm).

Input : out/grids.json from `node dump.js` (tagged loft grids from scene.js, robot frame, mm, Z from floor).
Output: out/pieces/<name>.stl (in robot frame, for the renders), out/pieces/<name>_print.stl (in print
        orientation, on the bed), out/split_report.json, printed table.

A concept-level check, not a slice: the shell is a constant-thickness offset of the concept surface,
with no bosses, ribs, laps or vent holes. Masses and times are estimates (see ESTIMATES below).
"""
import json, math, pathlib, sys
import numpy as np
import trimesh

HERE = pathlib.Path(__file__).parent
OUT = HERE / "out"; (OUT / "pieces").mkdir(parents=True, exist_ok=True)

WALL = 2.4             # mm, 6 perimeters at 0.4 mm: solid wall, no infill
BED = 256.0            # P2S build volume per axis (spec)
USABLE = 250.0         # keep 3 mm a side for brim/skirt and purge
RHO = 1.27e-3          # g/mm^3, PETG
# ESTIMATES (assumption): PETG on a 0.4 HF nozzle, 0.2 mm layers, outer walls slowed for finish.
# Effective volumetric rate over the whole print, incl. travel and wall slowdowns: 6 to 10 mm^3/s.
RATE_LO, RATE_HI = 6.0, 10.0
SUPPORT_FRAC = 0.25    # support volume as a fraction of supported area x 1 mm (tree supports, rough)

ZS = 740 + 424.9 + 70  # shoulder pitch axis, Z from floor (generator default ext=70)
F0, F1 = 740 + 155 + 70, 740 + 495 + 70

grids = {g["tag"]: g for g in json.load(open(OUT / "grids.json"))["grids"]}


def grid(tag):
    g = grids[tag]
    return np.array(g["pts"], float).reshape(g["nz"] + 1, g["nt"] + 1, 3)


def thicken(G, t=WALL, x_axis=4.0):
    """Closed solid from a structured surface grid G[i, j, xyz]: offset inward by t along the surface normal."""
    di = np.gradient(G, axis=0); dj = np.gradient(G, axis=1)
    n = np.cross(di, dj); n /= np.linalg.norm(n, axis=2, keepdims=True) + 1e-12
    radial = G.copy(); radial[..., 0] -= x_axis; radial[..., 2] = 0
    if np.sum(n * radial) < 0:
        n = -n                                 # make n point outward
    I = G - t * n
    ni, nj = G.shape[:2]
    V = np.vstack([G.reshape(-1, 3), I.reshape(-1, 3)])
    o = lambda i, j: i * nj + j
    k = ni * nj
    F = []
    for i in range(ni - 1):
        for j in range(nj - 1):
            a, b, c, d = o(i, j), o(i + 1, j), o(i + 1, j + 1), o(i, j + 1)
            F += [[a, b, c], [a, c, d], [k + a, k + c, k + b], [k + a, k + d, k + c]]
    def strip(idx):                            # boundary wall between outer and inner along an index path
        for p, q in zip(idx[:-1], idx[1:]):
            F.extend([[p, q, k + q], [p, k + q, k + p]])
    strip([o(0, j) for j in range(nj)])
    strip([o(i, nj - 1) for i in range(ni)])
    strip([o(ni - 1, j) for j in range(nj - 1, -1, -1)])
    strip([o(i, 0) for i in range(ni - 1, -1, -1)])
    m = trimesh.Trimesh(V, np.array(F), process=True)
    trimesh.repair.fix_normals(m)
    return m


def zi(G, z):                                   # row index nearest a height
    return int(np.argmin(np.abs(G[:, 0, 2] - z)))


def jy(G, i, ymax):                             # column range with |Y| <= ymax on row i (contiguous, centred)
    js = np.where(np.abs(G[i, :, 1]) <= ymax)[0]
    return int(js.min()), int(js.max())


def ring_half(r_in, r_out, t, side, half):
    """Shoulder collar half: annulus in the robot X-Z plane at Y = side*135.5, front (half=+1) or back half."""
    th = np.linspace(-math.pi / 2, math.pi / 2, 64) if half > 0 else np.linspace(math.pi / 2, 3 * math.pi / 2, 64)
    pts = []
    for r in (r_out, r_in):
        for a in (th if r == r_out else th[::-1]):
            pts.append([r * math.cos(a), r * math.sin(a)])
    from shapely.geometry import Polygon
    m = trimesh.creation.extrude_polygon(Polygon(pts), t)
    # polygon x -> robot X, polygon y -> robot Z offset, extrusion -> robot Y
    T = np.array([[1, 0, 0, 0], [0, 0, side, side * 135.5 - (t if side > 0 else 0) * 0], [0, 1, 0, ZS], [0, 0, 0, 1]], float)
    m.apply_transform(T)
    return m


# ---------------- proposed breakup ----------------
# unit = service unit (comes off the robot as one); print splits inside a unit are bolted on inner splice plates.
PIECES = []
def piece(name, unit, kind, mesh, note=""):
    PIECES.append(dict(name=name, unit=unit, kind=kind, mesh=mesh, note=note))

UF, UB = grid("chest upper front"), grid("chest upper back")
LF, LB = grid("chest lower front"), grid("chest lower back (service hatch)")
nt = UF.shape[1] - 1
# upper front: print split on the sternum crease (hidden under the chest shield)
piece("upper_front_L", "U1 upper front", "panel", thicken(UF[:, : nt // 2 + 1]), "pec plate, left (+Y)")
piece("upper_front_R", "U1 upper front", "panel", thicken(UF[:, nt // 2:]), "pec plate, right")
# upper back: print split on the spine; compute door spans the spine
piece("upper_back_L", "U2 upper back", "panel", thicken(UB[:, nt // 2:]), "back plate, left (+Y); door opening not cut")
piece("upper_back_R", "U2 upper back", "panel", thicken(UB[:, : nt // 2 + 1]), "back plate, right")
i0, i1 = zi(UB, F0 + 219), zi(UB, F1 - 3)   # torso Y 374-492: H3 rear below 374, fans to 488
j0, j1 = jy(UB, (i0 + i1) // 2, 62)
piece("door_compute", "D1 compute door", "door", thicken(UB[i0:i1 + 1, j0:j1 + 1]), "Pi 5, CAN HAT, Jetson ports, 2 exhaust fans; louvre/fan variants")
# shoulder-top hatches over each pitch RS03 (back half of the cap, outboard of the neck)
for s, nm in ((1, "L"), (-1, "R")):
    i0, i1 = zi(UB, F1 - 5), zi(UB, F1 + 30)
    ys = UB[(i0 + i1) // 2, :, 1]
    js = np.where((s * ys > 75) & (s * ys < 140))[0]
    piece(f"hatch_shoulder_{nm}", f"D3 shoulder hatch {nm}", "door", thicken(UB[i0:i1 + 1, js.min():js.max() + 1]),
          "pitch RS03 connectors and harness entry")
# lower front / back: split on the sternum and spine
nl = LF.shape[1] - 1
piece("lower_front_L", "U3 lower front", "panel", thicken(LF[:, : nl // 2 + 1]))
piece("lower_front_R", "U3 lower front", "panel", thicken(LF[:, nl // 2:]))
piece("lower_back_L", "U4 lower back", "panel", thicken(LB[:, nl // 2:]), "door opening not cut")
piece("lower_back_R", "U4 lower back", "panel", thicken(LB[:, : nl // 2 + 1]))
# D2 battery door (custom 13S2P pack, hardware/electrical/power/custom-21700-pack.md): pack |X| <= 47.5, torso Y 183-345,
# pulled straight out the back. Door covers torso Y 175-352 (robot Z F0+20 .. F0+197), |Y| <= 60, and spans the U2/U4 seam,
# so it is resampled across both grids. Bottom hinge on H4, magnets onto a spine rail on the H3 rear bracket (Y 355-372).
def back_patch(za, zb, ymax, nz=80, ny=60):
    rows = [G[i] for G in (LB, UB) for i in range(G.shape[0])]
    rows = sorted(rows, key=lambda r: r[0, 2])
    zs = np.array([r[0, 2] for r in rows]); out = np.zeros((nz + 1, ny + 1, 3))
    for a, z in enumerate(np.linspace(za, zb, nz + 1)):
        k = int(np.clip(np.searchsorted(zs, z), 1, len(rows) - 1)); r0, r1 = rows[k - 1], rows[k]
        t = 0 if zs[k] == zs[k - 1] else (z - zs[k - 1]) / (zs[k] - zs[k - 1])
        for b, y in enumerate(np.linspace(ymax, -ymax, ny + 1)):
            x0 = np.interp(y, r0[:, 1][::-1] if r0[0, 1] > r0[-1, 1] else r0[:, 1], (r0[:, 0][::-1] if r0[0, 1] > r0[-1, 1] else r0[:, 0]))
            x1 = np.interp(y, r1[:, 1][::-1] if r1[0, 1] > r1[-1, 1] else r1[:, 1], (r1[:, 0][::-1] if r1[0, 1] > r1[-1, 1] else r1[:, 0]))
            out[a, b] = [x0 + (x1 - x0) * t, y, z]
    return out
D2 = back_patch(F0 + 20, F0 + 197, 60)
piece("door_battery", "D2 battery door", "door", thicken(D2), "13S2P pack slides out the back; PDB/ports moved to the front power board")
# side vent grilles: straddle the front/back seam at the sides of the lower chest (cooling variants)
for s, nm in ((1, "L"), (-1, "R")):
    A = LF if True else LB
    i0, i1 = zi(LF, F0 + 25), zi(LF, F0 + 140)
    # take the side strip from both halves: front columns near +/-Y edge plus back columns, joined
    jf = slice(0, 12) if s > 0 else slice(nl - 11, nl + 1)
    jb = slice(nl - 11, nl + 1) if s > 0 else slice(0, 12)
    Gf, Gb = LF[i0:i1 + 1, jf], LB[i0:i1 + 1, jb]
    G = np.concatenate([Gb, Gf[:, 1:]], axis=1) if s > 0 else np.concatenate([Gf, Gb[:, 1:]], axis=1)
    piece(f"vent_side_{nm}", f"V1 side vent {nm}", "vent", thicken(G), "intake grille insert, swappable")
# shoulder collars, split so they come off with the arm fitted
for s, nm in ((1, "L"), (-1, "R")):
    for h, hn in ((1, "front"), (-1, "back")):
        piece(f"collar_{nm}_{hn}", f"C collar {nm}", "collar", ring_half(43, 57, 2.0, s, h), "on 4 unused outboard M4 holes")
# spine pack: TPU 95A crush pads standing 30 mm proud of the back (arm-thread tip-over recheck: ~30 mm beyond the shell),
# flanking the doors so the doors open between them. Bolted through the shell to H2/H3/H4 rear on standoff posts.
def pad(G, za, zb, ya, yb, name, unit):
    i0, i1 = zi(G, za), zi(G, zb); ys = G[(i0 + i1) // 2, :, 1]
    js = np.where((ys >= ya) & (ys <= yb))[0]
    piece(name, unit, "pad", thicken(G[i0:i1 + 1, js.min():js.max() + 1], t=-30.0), "TPU 95A gyroid, 30 mm crush")
for s_, nm in ((1, "L"), (-1, "R")):
    lo, hi = sorted((s_ * 72, s_ * 135)); pad(UB, F0 + 215, F0 + 325, lo, hi, f"pad_upper_{nm}", "P spine pack")
    lo, hi = sorted((s_ * 70, s_ * 90)); pad(LB, F0 + 15, F0 + 140, lo, hi, f"pad_lower_{nm}", "P spine pack")
for b in (1, 2, 3):
    for h in ("front", "back"):
        piece(f"abdomen_{b}_{h}", f"A{b} abdomen band {b}", "band", thicken(grid(f"abdomen band {b} {h}"), x_axis=2.0))


# ---------------- print orientation search ----------------
def rot_candidates():
    R = []
    for ax in ([1, 0, 0], [0, 1, 0], [0, 0, 1]):
        for ang in (0, 90, 180, 270):
            R.append(trimesh.transformations.rotation_matrix(math.radians(ang), ax))
    for a in (30, 45, 60):                      # tilts about robot Y and X
        for ax in ([1, 0, 0], [0, 1, 0]):
            for sg in (1, -1):
                R.append(trimesh.transformations.rotation_matrix(sg * math.radians(a), ax))
    out = []
    for A in R:
        for B in R[:12]:
            out.append(A @ B)
    return out


CANDS = rot_candidates()


def evaluate(mesh):
    """Pick the orientation that fits the bed and needs the least support under the cosmetic outer face."""
    # outer (cosmetic) faces: those whose normal points away from the torso axis
    c = mesh.triangles_center; n = mesh.face_normals; A = mesh.area_faces
    radial = c.copy(); radial[:, 0] -= 4.0; radial[:, 2] = 0
    rn = np.linalg.norm(radial, axis=1, keepdims=True) + 1e-9
    outer = np.sum(n * radial / rn, axis=1) > 0.3
    outer |= (n[:, 2] > 0.6) & (c[:, 2] > F1 - 20)          # shoulder-top cap
    best = None
    for T in CANDS:
        R = T[:3, :3]
        nz = n @ R.T
        v = mesh.vertices @ R.T
        ext = v.max(0) - v.min(0)
        if ext.max() > USABLE:
            continue
        zmin = v[:, 2].min()
        cz = (c @ R.T)[:, 2]
        over = (nz[:, 2] < -math.cos(math.radians(50))) & (cz > zmin + 0.6)   # needs support (>40 deg overhang)
        sup_outer = A[over & outer].sum()
        sup_all = A[over].sum()
        bed = A[(nz[:, 2] < -0.98) & (cz < zmin + 0.6)].sum()
        score = sup_outer * 5 + sup_all + ext[2] * 20 - bed * 0.5      # finish first, then support, then height
        if best is None or score < best[0]:
            best = (score, T, ext, sup_outer, sup_all)
    return best


rows = []
for p in PIECES:
    m = p["mesh"]
    m.export(OUT / "pieces" / f"{p['name']}.stl")
    ev = evaluate(m)
    vol = abs(m.volume)
    fits = ev is not None
    if fits:
        _, T, ext, so, sa = ev
        mp = m.copy(); mp.apply_transform(T); mp.apply_translation(-mp.bounds[0])
        mp.export(OUT / "pieces" / f"{p['name']}_print.stl")
        sup_vol = sa * 1.0 * SUPPORT_FRAC
    else:
        ext, so, sa, sup_vol = m.extents, float("nan"), float("nan"), 0
    pad_ = p["kind"] == "pad"
    dens = RHO * (0.25 if pad_ else 1.0)   # pads: TPU ~1.21 g/cm3, ~25% solid (2 walls + 10% gyroid; crush coupon C9 tunes it)
    g = (vol + sup_vol) * dens
    hrs = ((vol + sup_vol) / RATE_HI / 3600, (vol + sup_vol) / RATE_LO / 3600)
    if p["kind"] == "pad":                      # TPU 95A: ~2.5 to 4 mm^3/s, on the solid fraction only
        hrs = (vol * 0.25 / 4.0 / 3600, vol * 0.25 / 2.5 / 3600)
    rows.append(dict(name=p["name"], unit=p["unit"], kind=p["kind"], note=p["note"], fits=fits,
                     watertight=bool(m.is_watertight), extents_robot=[round(float(x), 1) for x in m.extents],
                     print_xyz=[round(float(x), 1) for x in sorted(ext[:2], reverse=True)] + [round(float(ext[2]), 1)],
                     part_g=round(vol * dens, 1), support_g=round(sup_vol * RHO, 1),
                     outer_support_cm2=round(so / 100, 1) if fits else None,
                     hours=[round(h, 1) for h in hrs]))

# doors and vent grilles fill openings cut from their parent panels (not cut in this study), so count them once
cut = sum(r["part_g"] for r in rows if r["kind"] in ("door", "vent", "pad"))
pad_g = sum(r["part_g"] for r in rows if r["kind"] == "pad")
tot_g = sum(r["part_g"] for r in rows) - cut; tot_s = sum(r["support_g"] for r in rows)
tot_h = [round(sum(r["hours"][k] for r in rows), 1) for k in (0, 1)]
json.dump(dict(wall_mm=WALL, usable_mm=USABLE, pieces=rows, total_part_g=round(tot_g), total_support_g=round(tot_s),
               total_hours=tot_h, spine_pack_g=round(pad_g)), open(OUT / "split_report.json", "w"), indent=1)
print(f"{'piece':22s} {'unit':22s} fit  print XYxZ (mm)        part g  sup g  outer-sup cm2  hours")
for r in rows:
    print(f"{r['name']:22s} {r['unit']:22s} {'yes' if r['fits'] else 'NO ':3s}  {str(r['print_xyz']):22s} {r['part_g']:7.1f} {r['support_g']:6.1f}  {str(r['outer_support_cm2']):>12s}  {r['hours'][0]}-{r['hours'][1]}")
print("watertight:", all(r["watertight"] for r in rows), [r["name"] for r in rows if not r["watertight"]])
print(f"spine pack {pad_g:.0f} g TPU"); print(f"TOTAL {len(rows)} pieces, {tot_g:.0f} g part (openings netted out) + {tot_s:.0f} g support, {tot_h[0]}-{tot_h[1]} h; all fit: {all(r['fits'] for r in rows)}")
