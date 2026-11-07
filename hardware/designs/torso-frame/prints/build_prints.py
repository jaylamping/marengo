#!/usr/bin/env python3
"""Fit-check print plates for the torso cradle, hooks and board carrier, sliced offline with the Bambu Studio CLI.

Joey's P2S machine and PETG HF presets flattened over the system profiles, same route as the left arm forearm slice.
Light fit-check settings throughout (Joey 2026-11-07): 3 walls, 15 % gyroid, 0.28 mm layers, Cool Plate SuperTack, no glue.
Run: python build_prints.py [plate ...]
"""
import json
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile
from pathlib import Path

import numpy as np
import trimesh

HERE = Path(__file__).resolve().parent
CRADLE, CARRIER = HERE.parent / "cradle", HERE.parent / "carrier"
SYS = Path(r"C:\Program Files\Bambu Studio\resources\profiles\BBL")
USER = Path(r"C:\Users\joeyl\AppData\Roaming\BambuStudio\user\3687192499")
EXE = r"C:\Program Files\Bambu Studio\bambu-studio.exe"
WORK = Path(tempfile.gettempdir()) / "marengo_torso_prints"
FRONT_DOWN = trimesh.transformations.rotation_matrix(np.pi, [1, 0, 0])  # torso Z 37.5 on the bed, layers in the C section
AS_IS = np.eye(4)  # torso +Z up: hook rear face or carrier rear face on the bed
BED = "Supertack Plate"
BASE = {"layer_height": "0.28", "wall_loops": "3", "sparse_infill_density": "15%", "sparse_infill_pattern": "gyroid", "enable_support": "0"}
BRIM = {"brim_type": "outer_only", "brim_width": "5"}
# Manual tree supports, only inside ENFORCE. Auto put them in the M3 insert holes, the M5 slots, the strap slot and the window arches.
SUPPORT = {"enable_support": "1", "support_type": "tree(manual)", "support_on_build_plate_only": "1"}
# Enforcer boxes in torso coordinates, as |X|, Y, Z ranges: bottom foot tongue forward edge, thumbscrew boss underside, ledge past the lip
ENFORCE = [((54.8, 75.2), (174.8, 179.2), (21.0, 22.0)), ((51.5, 59.2), (244.0, 261.0), (-2.0, 3.5)), ((39.8, 42.0), (178.8, 183.2), (33.5, 34.5))]


def side(label):
    return CRADLE / f"marengo_torso_cradle_side_{label}.stl"


def hooks():
    """The strap STL holds both hooks. Split by body, right first."""
    return sorted(trimesh.load(CRADLE / "marengo_torso_cradle_top_strap.stl").split(), key=lambda m: m.bounds[0][0])


PLATES = {
    "marengo_torso_cradle_sides_x2": ([(side("right"), FRONT_DOWN), (side("left"), FRONT_DOWN)], {**BRIM, **SUPPORT}),  # right is object 0, left object 1
    "marengo_torso_cradle_hooks": ([(h, AS_IS) for h in hooks()], {}),
    "marengo_torso_board_carrier": ([(CARRIER / "marengo_torso_board_carrier.stl", AS_IS)], {}),
}


def find(kind, name):
    p = SYS / kind / f"{name}.json"
    if p.exists():
        return p
    hits = list((SYS / kind).rglob(f"{name}.json"))
    assert hits, (kind, name)
    return hits[0]


def flat(kind, data):
    parent = data.get("inherits")
    merged = flat(kind, json.loads(find(kind, parent).read_text(encoding="utf8"))) if parent else {}
    merged.update(data)
    return merged


def write(kind, path, out, overrides=None):
    d = flat(kind, json.loads(Path(path).read_text(encoding="utf8")))
    d.update(overrides or {})
    d.pop("inherits", None)
    d["from"] = "system"
    d.setdefault("version", "02.00.00.00")
    f = out / f"{kind}.json"
    f.write_text(json.dumps(d, indent=1), encoding="utf8")
    return f


def filament(out):
    # Joe's preset has per-extruder arrays with "nil" slots, which the CLI refuses, so apply its first slot over the system preset.
    joe = json.loads((USER / "filament" / "Joe PETG HF.json").read_text(encoding="utf8"))
    base = flat("filament", json.loads(find("filament", joe["inherits"]).read_text(encoding="utf8")))
    over = {"name": joe["name"]}
    for k, v in joe.items():
        if isinstance(v, list) and k in base and k != "compatible_printers" and not k.startswith("filament_extruder_variant"):
            first = next((x for x in v if x != "nil"), None)
            if first is not None:
                over[k] = [first] * (len(base[k]) if isinstance(base[k], list) else 1)
    tmp = out / "joe_base.json"
    tmp.write_text(json.dumps(base), encoding="utf8")
    return write("filament", tmp, out, over)


def enforcers(index):
    """Enforcer mesh for side object 0 (right) or 1 (left), in the bed-up frame of its STL (torso Z 37.5 at height 0, torso Y 353.62 at y 0)."""
    boxes = []
    for (x0, x1), (y0, y1), (z0, z1) in ENFORCE:
        a, b = sorted((75 - x0, 75 - x1)) if index == 0 else (x0 - 40, x1 - 40)
        lo, hi = np.array([a, 353.62 - y1, 37.5 - z1]), np.array([b, 353.62 - y0, 37.5 - z0])
        boxes.append(trimesh.creation.box(hi - lo, trimesh.transformations.translation_matrix((lo + hi) / 2)))
    return trimesh.util.concatenate(boxes)


def add_enforcers(src, dst):
    """Bambu has no CLI switch for enforcers, so add one to each object of a project as a support_enforcer part."""
    z = zipfile.ZipFile(src)
    model = z.read("3D/3dmodel.model").decode()
    cfg = z.read("Metadata/model_settings.config").decode()
    edits = {}
    for i in (0, 1):
        obj = next(o for o in re.finditer(r'<object id="(\d+)">.*?</object>', cfg, re.S) if f"_{i}.stl" in o[0])
        comp = re.search(rf'<object id="{obj[1]}".*?<component p:path="(/3D/Objects/[^"]+)" objectid="\d+"[^>]*transform="([^"]*)"/>', model, re.S)
        path, tf = comp[1], [float(v) for v in comp[2].split()]
        matrix = re.search(r'key="matrix" value="([^"]*)"', obj[0])[1]
        mesh = enforcers(i)
        mesh.apply_translation(-np.array(tf[9:12]))
        verts = "\n".join(f'     <vertex x="{a:.5f}" y="{b:.5f}" z="{c:.5f}"/>' for a, b, c in mesh.vertices)
        tris = "\n".join(f'     <triangle v1="{a}" v2="{b}" v3="{c}"/>' for a, b, c in mesh.faces)
        new = 100 + i
        body = f'  <object id="{new}" p:UUID="00{new:06d}-81cb-4c03-9d28-80fed5dfa1dc" type="model">\n   <mesh>\n    <vertices>\n{verts}\n    </vertices>\n    <triangles>\n{tris}\n    </triangles>\n   </mesh>\n  </object>\n </resources>'
        edits[path[1:]] = z.read(path[1:]).decode().replace("</resources>", body)
        model = model.replace(comp[0], comp[0] + f'\n    <component p:path="{path}" objectid="{new}" p:UUID="00{new:06d}-b206-40ff-9872-83e8017abed1" transform="{comp[2]}"/>')
        part = f'    <part id="{new}" subtype="support_enforcer">\n      <metadata key="name" value="enforcer"/>\n      <metadata key="matrix" value="{matrix}"/>\n      <mesh_stat face_count="{len(mesh.faces)}" edges_fixed="0" degenerate_facets="0" facets_removed="0" facets_reversed="0" backwards_edges="0"/>\n    </part>\n'
        cfg = cfg.replace(obj[0], obj[0].replace("  </object>", part + "  </object>"))
    with zipfile.ZipFile(dst, "w", zipfile.ZIP_DEFLATED) as o:
        for item in z.infolist():
            data = edits.get(item.filename)
            if item.filename == "3D/3dmodel.model":
                data = model
            elif item.filename == "Metadata/model_settings.config":
                data = cfg
            o.writestr(item.filename, data if data is not None else z.read(item.filename))


def run(name):
    items, over = PLATES[name]
    out = WORK / name
    out.mkdir(parents=True, exist_ok=True)
    m = write("machine", USER / "machine" / "Bambu Lab P2S 0.4 nozzle ventoBox 20251215.json", out, {"name": "Bambu Lab P2S 0.4 nozzle", "printer_settings_id": "Bambu Lab P2S 0.4 nozzle"})
    s = write("process", SYS / "process" / "0.20mm Standard @BBL P2S.json", out, {**BASE, **over})
    f = filament(out)
    stls = []
    for i, (src, tf) in enumerate(items):
        mesh = trimesh.load(src) if isinstance(src, Path) else src.copy()
        mesh.apply_transform(tf)
        mesh.apply_translation(-mesh.bounds[0])
        stls.append(out / f"{name}_{i}.stl")
        mesh.export(stls[-1])
    final = HERE / f"{name}.3mf"
    cli = [EXE, "--datadir", str(out / "profile")]
    flags = dict(capture_output=True, text=True, timeout=1500, creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS | subprocess.CREATE_NO_WINDOW)
    load = ["--load-settings", f"{m};{s}", "--load-filaments", str(f), "--curr-bed-type", BED, "--arrange", "1"]
    if "enable_support" in over:
        raw = out / "raw.3mf"
        r = subprocess.run([*cli, *load, "--export-3mf", raw.name, "--outputdir", str(out), *map(str, stls)], **flags)
        add_enforcers(raw, out / "input.3mf")
        r = subprocess.run([*cli, "--slice", "1", "--export-3mf", final.name, "--outputdir", str(out), str(out / "input.3mf")], **flags)
    else:
        r = subprocess.run([*cli, *load, "--slice", "1", "--export-3mf", final.name, "--outputdir", str(out), *map(str, stls)], **flags)
    if (out / final.name).exists():
        shutil.copy(out / final.name, final)
    (out / "log.txt").write_text(r.stdout[-6000:] + r.stderr[-3000:], errors="replace")
    res = {"plate": name, "exit": r.returncode}
    if final.exists():
        info = zipfile.ZipFile(final).read("Metadata/slice_info.config").decode(errors="replace")
        pred = dict(re.findall(r'key="(prediction|weight)" value="([^"]*)"', info))
        res.update(minutes=round(float(pred.get("prediction", 0)) / 60, 1), grams=float(pred.get("weight", 0)), warnings=re.findall(r"<warning[^>]*>", info), kb=final.stat().st_size // 1024)
    print(json.dumps(res), flush=True)


if __name__ == "__main__":
    for plate in sys.argv[1:] or PLATES:
        run(plate)
