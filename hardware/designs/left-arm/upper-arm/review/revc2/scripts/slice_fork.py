#!/usr/bin/env python3
"""Slice the fork, build direction +X, with Joey's P2S machine and PETG HF filament presets flattened over the system profiles."""
import json
import struct
import subprocess
from pathlib import Path
import numpy as np

HERE = Path(__file__).resolve().parent
SYS = Path(r"C:\Program Files\Bambu Studio\resources\profiles\BBL")
USER = Path(r"C:\Users\joeyl\AppData\Roaming\BambuStudio\user\3687192499")
OUT = HERE / "slice_p2"
OUT.mkdir(exist_ok=True)
EXE = r"C:\Program Files\Bambu Studio\bambu-studio.exe"


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


def write(kind, path, overrides=None):
    d = flat(kind, json.loads(Path(path).read_text(encoding="utf8")))
    d.update(overrides or {})
    d.pop("inherits", None)
    d["from"] = "system"
    d["type"] = kind if kind != "filament" else "filament"
    d.setdefault("version", "02.00.00.00")
    f = OUT / f"{kind}.json"
    f.write_text(json.dumps(d, indent=1), encoding="utf8")
    return f


m = write("machine", USER / "machine" / "Bambu Lab P2S 0.4 nozzle ventoBox 20251215.json", {"name": "Bambu Lab P2S 0.4 nozzle", "printer_settings_id": "Bambu Lab P2S 0.4 nozzle"})
s = write("process", SYS / "process" / "0.20mm Standard @BBL P2S.json", {"sparse_infill_density": "100%", "sparse_infill_pattern": "zig-zag", "enable_support": "1", "support_type": "tree(auto)", "wall_loops": "4"})
# Joe's filament preset sets per-extruder arrays with "nil" slots, which the CLI refuses, so apply its first slot over the system preset.
joe = json.loads((USER / "filament" / "Joe PETG HF.json").read_text(encoding="utf8"))
base = flat("filament", json.loads(find("filament", joe["inherits"]).read_text(encoding="utf8")))
over = {"name": joe["name"]}
for k, v in joe.items():
    if isinstance(v, list) and k in base and k != "compatible_printers" and not k.startswith("filament_extruder_variant"):
        n_ = len(base[k]) if isinstance(base[k], list) else 1
        first = next((x for x in v if x != "nil"), None)
        if first is not None:
            over[k] = [first] * n_
print("filament overrides", {k: v[0] for k, v in over.items() if k != "name"})
tmp = OUT / "joe_base.json"
tmp.write_text(json.dumps(base), encoding="utf8")
f = write("filament", tmp, over)

src = Path(r"J:\code\marengo\cad\exports\left_arm\marengo_forearm_left.STL")
b = src.read_bytes()
n = struct.unpack("<I", b[80:84])[0]
rec = np.dtype([("n", "<3f4"), ("v", "<9f4"), ("a", "<u2")])
t = np.frombuffer(b, rec, n, 84).copy()
v = t["v"].reshape(-1, 3).astype(float)
if np.ptp(v, axis=0).max() < 5:
    v *= 1000
# +X up. Cyclic axes keep handedness: model X to print Z, Y to X, Z to Y.
v = v[:, [1, 2, 0]]
v -= [v[:, 0].mean(), v[:, 1].mean(), v[:, 2].min()]
v += [128, 128, 0]
t["v"] = v.reshape(-1, 9)
t["n"] = t["n"][:, [1, 2, 0]]
stl = OUT / "fork_plusX.stl"
stl.write_bytes(b[:84] + t.tobytes())
print("size mm", np.ptp(v, axis=0).round(1))
args = [EXE, "--load-settings", f"{m};{s}", "--load-filaments", str(f), "--curr-bed-type", "Textured PEI Plate", "--slice", "1", "--min-save", "--outputdir", str(OUT), str(stl)]
r = subprocess.run(args, capture_output=True, text=True, timeout=500)
print("exit", r.returncode)
print((r.stdout + r.stderr)[-1500:])
print(sorted(x.name for x in OUT.iterdir()))


