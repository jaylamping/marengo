#!/usr/bin/env python3
"""RS03 test stand print plates, sliced offline with the Bambu Studio CLI on Joey-PC (Windows).

Reuses the torso prints helpers (Joey's P2S machine + filament presets flattened over the system profiles,
different_settings_to_system flagging). Every part passes hardware/tools/mesh_check.py before its plate is sliced.
All PETG on the Cool Plate SuperTack, no glue. No supports needed in these orientations (checked: every overhang is a
hole under 8 mm, a 1.5 mm counterbore step or the 20 mm channel bridge); Joey's default is tree if one is ever added.

Settings by purpose (Joey 2026-11-07): the adapter carries the lever torque, so 6 walls 100 %. Everything else is a
rig fixture loaded far below its strength (see README load check), so 4 walls 25 % gyroid.
Run on Joey-PC: python build_prints.py [plate ...]
"""
import importlib.util
import json
import re
import shutil
import subprocess
import sys
import zipfile
from pathlib import Path

import numpy as np
import trimesh

HERE = Path(__file__).resolve().parent
OUT = HERE / "out"
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / "hardware" / "tools"))
import mesh_check  # noqa: E402

_spec = importlib.util.spec_from_file_location("torso_prints", REPO / "hardware/designs/torso-frame/prints/build_prints.py")
tp = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(tp)

R = trimesh.transformations.rotation_matrix
FRONT_DOWN = R(-np.pi / 2, [1, 0, 0])   # stand +Y face on the bed
X_SIDE = R(np.pi / 2, [0, 1, 0])        # profile parts on their X side
P = "marengo_teststand_"

FIXTURE = {"layer_height": "0.2", "wall_loops": "4", "sparse_infill_density": "25%", "sparse_infill_pattern": "gyroid",
           "enable_support": "0", "brim_type": "outer_only", "brim_width": "5"}
LOADED = {"layer_height": "0.2", "wall_loops": "6", "sparse_infill_density": "100%", "sparse_infill_pattern": "zig-zag",
          "top_shell_layers": "6", "bottom_shell_layers": "6", "enable_support": "0"}

PLATES = {
    "teststand_rs03_upright": ([(P + "upright_rs03", FRONT_DOWN)], FIXTURE),
    "teststand_rs03_adapter": ([(P + "adapter_rs03", FRONT_DOWN)], LOADED),
    "teststand_fixtures": ([(P + "clamp_cap", FRONT_DOWN), (P + "load_block", FRONT_DOWN), (P + "lock_post", X_SIDE), (P + "anchor", X_SIDE)], FIXTURE),
}


def run(name):
    items, over = PLATES[name]
    work = tp.WORK.parent / "marengo_teststand" / name
    work.mkdir(parents=True, exist_ok=True)
    for part, _ in items:  # surface gate on the CAD-frame STL; blocks the 3MF on failure
        mesh_check.gate(part, None, HERE, src=OUT / f"{part}.stl")
    m = tp.write("machine", tp.USER / "machine" / "Bambu Lab P2S 0.4 nozzle ventoBox 20251215.json", work,
                 {"name": "Bambu Lab P2S 0.4 nozzle", "printer_settings_id": "Bambu Lab P2S 0.4 nozzle"})
    s = tp.write("process", tp.SYS / "process" / "0.20mm Standard @BBL P2S.json", work, over)
    f = tp.filament(work, tp.PETG)
    stls = []
    for i, (part, tf) in enumerate(items):
        mesh = trimesh.load(OUT / f"{part}.stl", force="mesh")
        mesh.apply_transform(tf)
        mesh.apply_translation(-mesh.bounds[0])
        stls.append(work / f"{name}_{i}.stl")
        mesh.export(stls[-1])
    final = HERE / f"{name}.3mf"
    cli = [tp.EXE, "--datadir", str(work / "profile")]
    flags = dict(capture_output=True, text=True, timeout=1500, creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS | subprocess.CREATE_NO_WINDOW)
    r = subprocess.run([*cli, "--load-settings", f"{m};{s}", "--load-filaments", str(f), "--curr-bed-type", tp.BED, "--arrange", "1",
                        "--slice", "1", "--export-3mf", final.name, "--outputdir", str(work), *map(str, stls)], **flags)
    if (work / final.name).exists():
        shutil.copy(work / final.name, final)
        tp.mark_changed(final, over)
    (work / "log.txt").write_text(r.stdout[-6000:] + r.stderr[-3000:], errors="replace")
    res = {"plate": name, "exit": r.returncode}
    if final.exists():
        info = zipfile.ZipFile(final).read("Metadata/slice_info.config").decode(errors="replace")
        pred = dict(re.findall(r'key="(prediction|weight)" value="([^"]*)"', info))
        res.update(minutes=round(float(pred.get("prediction", 0)) / 60, 1), grams=float(pred.get("weight", 0)), warnings=re.findall(r"<warning[^>]*>", info))
    print(json.dumps(res), flush=True)


if __name__ == "__main__":
    for plate in sys.argv[1:] or PLATES:
        run(plate)
