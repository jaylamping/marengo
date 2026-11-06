"""Render the cradle with the pack box and frame extrusions from parts.json, in a throwaway assembly under cad/_wip."""
import json
import sys
from pathlib import Path

import pythoncom
import win32com.client as wc
from PIL import Image

sys.path.insert(0, r"J:\code\marengo\hardware\tools\solidworks")
from build_cradle import CAD, NULL, OUT, Part
from sw_instance import session

WIP = Path(r"J:\code\marengo\cad\_wip\cradle_render")
PARTS = json.loads((OUT.parent / "generator" / "parts.json").read_text())["parts"]
GREY, BLUE, ORANGE, GREEN = (0.62, 0.64, 0.68), (0.20, 0.42, 0.75), (0.95, 0.50, 0.12), (0.25, 0.65, 0.40)


def material(rgb, clear=0.0):
    return wc.VARIANT(pythoncom.VT_ARRAY | pythoncom.VT_R8, [*rgb, 1, 1, 0.5, 0.4, clear, 0])


def build_context(sw):
    p = Part(sw, WIP / "context.SLDPRT")
    boxes = [(q["lo"], q["hi"], f"{q['id']}".replace("-", "m"), GREY) for q in PARTS if q["group"] == "extrusion" and q["lo"][1] < 354]
    boxes += [(q["lo"], q["hi"], "pack", BLUE) for q in PARTS if q["id"] == "pack"]
    for lo, hi, name, _ in boxes:
        p.sketch("front")
        p.sm.CreateCornerRectangle(lo[0] * 1e-3, lo[1] * 1e-3, 0, hi[0] * 1e-3, hi[1] * 1e-3, 0)
        p.extrude(lo[2], hi[2], name, merge=False)
    for body in p.m.GetBodies2(0, True):
        box = body.GetBodyBox()
        is_pack = box[3] - box[0] > 0.09 and box[4] - box[1] > 0.15
        body.MaterialPropertyValues2 = material(BLUE, 0.45) if is_pack else material(GREY, 0.65)
    p.save()
    sw.CloseDoc(p.m.GetTitle)


def main():
    WIP.mkdir(parents=True, exist_ok=True)
    inst = session(sys.argv[1])
    sw = inst.sw
    sw.CloseAllDocuments(True)
    build_context(sw)
    asm = sw.NewDocument(sw.GetUserPreferenceStringValue(9), 0, 0, 0)
    err = wc.VARIANT(pythoncom.VT_BYREF | pythoncom.VT_I4, 0)
    warn = wc.VARIANT(pythoncom.VT_BYREF | pythoncom.VT_I4, 0)
    title = asm.GetTitle
    # AddComponent5 places the part's bounding box centre at x, y, z, so pass each part's own centre (m)
    items = [(WIP / "context.SLDPRT", None, (0, 0.26431, 0))]
    items += [(CAD / "marengo_torso_cradle_side_right.SLDPRT", ORANGE, (-0.0575, 0.26431, -0.00625))]
    items += [(CAD / "marengo_torso_cradle_side_left.SLDPRT", ORANGE, (0.0575, 0.26431, -0.00625))]
    items += [(CAD / "marengo_torso_cradle_top_strap.SLDPRT", GREEN, (0, 0.3445, -0.005))]
    for path, rgb, centre in items:
        sw.OpenDoc6(str(path), 1, 1, "", err, warn)
        asm = sw.ActivateDoc3(title, False, 0, err)
        comp = asm.AddComponent5(str(path), 0, "", False, "", *centre)
        if comp is None:
            raise RuntimeError(f"could not add {path}")
        print(path.name, comp.Name2, [round(v * 1e3, 2) for v in comp.GetBox(False, False)], flush=True)
        if rgb:
            comp.MaterialPropertyValues = material(rgb)
    asm.SaveAs3(str(WIP / "render.SLDASM"), 0, 1)
    asm.ClearSelection2(True)
    for name, view in (("cradle_render", "*Isometric"), ("check_right", "*Right"), ("check_front", "*Front")):
        asm.ShowNamedView2(view, -1)
        asm.ViewZoomtofit2()
        bmp = WIP / f"{name}.bmp"
        print(name, asm.SaveBMP(str(bmp), 2400, 1800), flush=True)
        target = OUT / "CRADLE_ASSEMBLED.png" if name == "cradle_render" else WIP / f"{name}.png"
        Image.open(bmp).convert("RGB").save(target)
    sw.CloseAllDocuments(True)


if __name__ == "__main__":
    main()
