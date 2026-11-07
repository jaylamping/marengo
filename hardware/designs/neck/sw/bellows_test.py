"""TPU bellows test section: 3 folds of the chosen profile with hand flanges. Builds in SolidWorks, then slices.

Run: python bellows_test.py build   (launches one hidden instance, exports STEP/STL, exits)
     python bellows_test.py slice   (Bambu Studio CLI, no window)
     python bellows_test.py report  (numbers and BELLOWS_PROFILE.png)
"""
import json
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from math import cos, pi, sin
from pathlib import Path

import numpy as np

import bellows as b
import neck_geom as g

HERE = Path(__file__).resolve().parent
OUT = HERE.parent / "bellows"
NAME = "marengo_neck_bellows_test"

# chosen profile (see BELLOWS.md)
FOLDS_FULL, FOLDS_TEST = 5, 3
PITCH, ROOT_R, ARC_R, WALL = 12.5, 33.0, 2.5, 1.2
CREST_R = b.OD_R - WALL / 2
FLANGE_R, FLANGE_T, CUFF, FLARE, RIM = 48.0, 3.0, 5.0, 5.0, 1.5
BED = "Textured PEI Plate"   # the TPU 95A HF system profile sets Super Tack to 0 degC: Bambu does not support that pairing


def centreline(n, z0):
    """(r, z, nr, nz) along the wall centreline from crest to crest. (nr, nz) is the unit normal pointing out of the crest."""
    psi, f = b.fold(PITCH, ROOT_R, CREST_R, ARC_R)
    pts = []
    z = z0
    STEPS = 8

    def arc(cr, cz, sign, a0, a1):
        for a in np.linspace(a0, a1, STEPS + 1):
            pts.append((cr + sign * ARC_R * cos(a), cz + ARC_R * sin(a), cos(a), sign * sin(a)))

    for k in range(n):
        zc = z0 + k * PITCH
        # crest second half: centre inside the crest; root: centre outside it
        arc(CREST_R - ARC_R, zc, +1, 0, psi)
        zr = zc + PITCH / 2
        arc(ROOT_R + ARC_R, zr, -1, -psi, psi)
        arc(CREST_R - ARC_R, zc + PITCH, +1, -psi, 0)
    return pts


def outline():
    """Closed (r, z) polygon of the test section."""
    z0 = FLANGE_T + CUFF
    line = centreline(FOLDS_TEST, z0)
    zt = z0 + FOLDS_TEST * PITCH
    top = zt + CUFF + FLARE + RIM
    ro, ri = b.OD_R, b.OD_R - WALL
    inner = [(r - nr * WALL / 2, z - nz * WALL / 2) for r, z, nr, nz in line]
    outer = [(r + nr * WALL / 2, z + nz * WALL / 2) for r, z, nr, nz in line]
    poly = [(ri, 0.0), (ri, z0)] + inner + [(ri, top), (ro + FLARE, top), (ro + FLARE, top - RIM), (ro, zt + CUFF), (ro, zt)]
    poly += outer[::-1] + [(ro, z0), (ro, FLANGE_T), (FLANGE_R, FLANGE_T), (FLANGE_R, 0.0)]
    # drop repeats where the arcs meet
    out = [poly[0]]
    for p in poly[1:]:
        if np.hypot(p[0] - out[-1][0], p[1] - out[-1][1]) > 1e-4:
            out.append(p)
    return out, top


def build():
    sys.path.insert(0, r"J:\code\marengo\hardware\tools\solidworks")
    from sw_instance import launch

    import build_parts as P
    OUT.mkdir(exist_ok=True)
    poly, top = outline()
    inst = launch()
    print("instance up", inst.pid, flush=True)
    try:
        p = P.Part(inst.sw, "bellows_test", "print", "TPU 95A")
        p.sketch("front")
        sm = p.sm
        sm.CreateCenterLine(0, 0, 0, 0, top * P.MM, 0)
        for (r0, z0), (r1, z1) in zip(poly, poly[1:] + poly[:1]):
            sm.CreateLine(r0 * P.MM, z0 * P.MM, 0, r1 * P.MM, z1 * P.MM, 0)
        sm.AddToDB = False
        f = p.fm.FeatureRevolve2(True, True, False, False, False, False, 0, 0, 2 * pi, 0, False, False, 0, 0, 0, 0, 0, True, True, True)
        if f is None:
            raise RuntimeError("revolve failed")
        f.Name = "wall"
        p.m.ClearSelection2(True)
        p.save()
        mp = p.m.Extension.CreateMassProperty
        print("volume mm3", round(mp.Volume * 1e9), "box", [round(v * 1e3, 2) for v in p.m.GetPartBox(True)], flush=True)
        for ext in ("step", "stl"):
            print(ext, p.m.SaveAs3(str(OUT / f"{NAME}.{ext}"), 0, 2 | 1), flush=True)
        p.m.ShowNamedView2("*Isometric", 7)
        p.m.ViewZoomtofit2()
        bmp = Path(r"J:\code\marengo\cad\_wip\neck\bellows_test.bmp")
        print("bmp", p.m.SaveBMP(str(bmp), 1600, 1200), flush=True)
        from PIL import Image
        Image.open(bmp).convert("RGB").save(OUT / "BELLOWS_TEST.png")
        inst.sw.CloseAllDocuments(True)
    finally:
        inst.close()
    print("closed", flush=True)


def slice_it():
    import trimesh
    sys.path.insert(0, str(HERE.parents[1] / "torso-frame" / "prints"))
    import build_prints as P   # same flattening route as the torso plates
    mesh = trimesh.load(OUT / f"{NAME}.stl")
    print("stl", mesh.is_watertight, mesh.extents.round(2), len(mesh.faces), flush=True)
    mesh.apply_transform(trimesh.transformations.rotation_matrix(pi / 2, [1, 0, 0]))   # part +Y becomes +Z
    mesh.apply_translation(-mesh.bounds[0])
    work = Path(tempfile.gettempdir()) / NAME
    work.mkdir(exist_ok=True)
    stl = work / f"{NAME}.stl"
    mesh.export(stl)
    m = P.write("machine", P.USER / "machine" / "Bambu Lab P2S 0.4 nozzle ventoBox 20251215.json", work, {"name": "Bambu Lab P2S 0.4 nozzle", "printer_settings_id": "Bambu Lab P2S 0.4 nozzle"})
    s = P.write("process", P.SYS / "process" / "0.12mm High Quality @BBL P2S.json", work, {
        "wall_loops": "8", "sparse_infill_density": "0%", "enable_support": "0", "brim_type": "no_brim", "wall_generator": "arachne"})
    f = P.write("filament", P.find("filament", "Bambu TPU 95A HF @BBL P2S"), work)
    final = OUT / f"{NAME}.3mf"
    cli = [P.EXE, "--datadir", str(work / "profile"), "--load-settings", f"{m};{s}", "--load-filaments", str(f), "--curr-bed-type", BED, "--arrange", "1",
           "--slice", "1", "--export-3mf", final.name, "--outputdir", str(work), str(stl)]
    r = subprocess.run(cli, capture_output=True, text=True, timeout=1500, creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS | subprocess.CREATE_NO_WINDOW)
    (work / "log.txt").write_text(r.stdout[-6000:] + r.stderr[-3000:], errors="replace")
    print("exit", r.returncode, flush=True)
    if (work / final.name).exists():
        shutil.copy(work / final.name, final)
        info = zipfile.ZipFile(final).read("Metadata/slice_info.config").decode(errors="replace")
        print(dict(re.findall(r'key="(prediction|weight)" value="([^"]*)"', info)), re.findall(r"<warning[^>]*>", info), final.stat().st_size // 1024, "kB")
    else:
        print((work / "log.txt").read_text(errors="replace")[-1500:])


def report():
    """Strain and clearance numbers for the chosen profile, per fold and at the corners."""
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    psi0, f = b.fold(PITCH, ROOT_R, CREST_R, ARC_R)
    a = 2 * psi0 * ARC_R
    Lmin, Lmax = b.lengths(CREST_R)
    n = FOLDS_FULL
    free = b.HEAD_H - b.FOOT_H
    rm = (ROOT_R + CREST_R) / 2
    res = dict(moulded_pitch=PITCH, installed_pitch=free / n, moulded_free_length=PITCH * n, neutral_psi_deg=round(float(np.degrees(psi0)), 1), flank=round(f, 2), crest_range=[round(Lmin, 1), round(Lmax, 1)])
    rows = {}
    for k, L in (("squashed", Lmin), ("installed", free), ("moulded", PITCH * n), ("stretched", Lmax)):
        p = L / n
        e, d, r, m = b.strain(p, psi0, a, f, ARC_R, WALL)
        rows[k] = dict(fold_pitch=round(p, 2), bending_strain_pct=round(e * 100, 1), depth=round(d, 2), arc_R=round(r, 2), OD_r=round(rm + d / 2 + WALL / 2, 2), bore_r=round(rm - d / 2 - WALL / 2, 2),
                       hoop_strain_pct=round((d - (CREST_R - ROOT_R)) / 2 / CREST_R * 100, 1), test_gap_flange_to_rim=round(p * FOLDS_TEST + FLANGE_T + 2 * CUFF + FLARE + RIM, 1))
    res["states"] = rows
    bore = min(v["bore_r"] for v in rows.values())
    # rod radius from the bellows centreline, a cubic from foot to head, measured in the plane of the nearest fold ring
    worst, shear = 0, 0
    for p, q in list(g.corners()) + [(g.PITCH_DOWN, 0), (-g.PITCH_UP, 0), (0, g.ROLL_MAX), (0, -g.ROLL_MAX)]:
        Rh = g.head_rot(p, q)
        c0, c3 = np.array([0, 0, b.FOOT_H]), g.UJ + Rh @ (np.array([0, 0, b.HEAD_H]) - g.UJ)
        n3 = Rh @ np.array([0, 0, 1.0])
        h = np.linalg.norm(c3 - c0) / 3
        c1, c2 = c0 + [0, 0, h], c3 - n3 * h
        ss = np.linspace(0, 1, 80)
        cs = np.array([(1 - s) ** 3 * c0 + 3 * (1 - s) ** 2 * s * c1 + 3 * (1 - s) * s ** 2 * c2 + s ** 3 * c3 for s in ss])
        ts = np.gradient(cs, axis=0)
        ts /= np.linalg.norm(ts, axis=1)[:, None]
        shear = max(shear, float(np.hypot(*c3[:2])))
        for k in g.SERVOS:
            t, lo, hi = g.rod_pose(k, p, q)
            for w in np.linspace(0, 1, 160):
                d = lo + (hi - lo) * w - cs
                ax = np.einsum("ij,ij->i", d, ts)
                i = int(np.argmin(np.abs(ax)))
                if 0 < i < 79:
                    worst = max(worst, float(np.linalg.norm(d[i] - ts[i] * ax[i])) + g.ROD_R)
    res["bore_min_r"] = bore
    res["head_end_offset_mm"] = round(shear, 1)
    res["rod_outer_radius_in_bellows"] = round(worst, 1)
    res["rod_gap_to_fold_root_mm"] = round(bore - worst, 1)
    res["sleeve_note"] = "tube sleeve r 33 up to h 48 does not fit this bore; stop the tube at the spigot (h 20) in the full-bellows pass"
    print(json.dumps(res, indent=1))
    (OUT / "bellows_numbers.json").write_text(json.dumps(res, indent=1))

    fig, ax = plt.subplots(figsize=(11, 5.2))
    x0 = 0
    for k, L in (("squashed\n(worst side)", Lmin), ("installed", free), ("as printed", PITCH * n), ("stretched\n(worst side)", Lmax)):
        p = L / n
        e, d, r, m = b.strain(p, psi0, a, f, ARC_R, WALL)
        zs, rs = [], []
        for j in range(n):
            for u in np.linspace(0, 1, 41):
                zs.append((j + u) * p)
                rs.append(rm + d / 2 * cos(2 * pi * u))
        ax.plot(np.array(rs) - rm + x0, zs, color="#26282c", lw=3, solid_capstyle="round")
        ax.text(x0, -9, k, ha="center", va="top", fontsize=9)
        ax.text(x0, L + 3, f"{L:.0f} mm", ha="center", fontsize=9)
        x0 += 22
    ax.set_aspect("equal")
    ax.set_ylim(-20, 85)
    ax.axis("off")
    ax.set_title("Wall section of the 5-fold bellows (fold shape simplified), to scale", fontsize=10, loc="left")
    fig.tight_layout()
    fig.savefig(OUT / "BELLOWS_PROFILE.png", dpi=130)


if __name__ == "__main__":
    {"build": build, "slice": slice_it, "report": report}[sys.argv[1]]()
