"""One launch, whole neck layout build: import the servo, build parts, assemble, check, render, export, exit.

Run: python run_all.py [from_step]   steps: import parts asm
The instance stays up on failure so a rerun reuses it. Never stop this with a process-tree kill: that kills the instance.
"""
import subprocess
import sys
import time
from pathlib import Path

import pythoncom
import win32com.client as wc

HERE = Path(__file__).parent
sys.path.insert(0, r"J:\code\marengo\hardware\tools\solidworks")
from sw_instance import end_session, session

STATE = r"J:\code\marengo\cad\_wip\neck\sw_session.json"
V = r"J:\code\marengo\cad\vendor\feetech\sts3215\vendor_feetech_sts3215_vendor"
IMPORT = f"""
import sys
sys.path.insert(0, r"J:\\code\\marengo\\hardware\\tools\\solidworks")
from sw_instance import attach
import pythoncom, win32com.client as wc
sw = attach(int(open(r"{STATE}").read())).sw
e = wc.VARIANT(pythoncom.VT_BYREF | pythoncom.VT_I4, 0)
sw.LoadFile4(r"{V}.step", "r", sw.GetImportFileData(r"{V}.step"), e)
print("load returned", e.value, flush=True)
"""


def ref():
    return wc.VARIANT(pythoncom.VT_BYREF | pythoncom.VT_I4, 0)


def import_servo(inst):
    """LoadFile4 loaded the part but never returned on 2026-11-07, so give it a time limit and carry on over COM."""
    if Path(V + ".SLDPRT").exists():
        return
    try:
        subprocess.run([sys.executable, "-u", "-c", IMPORT], timeout=90, check=True)
    except subprocess.TimeoutExpired:
        print("LoadFile4 did not return, using the open document", flush=True)
    m = inst.sw.ActiveDoc
    print(m.GetTitle, m.GetType, [round(v * 1e3, 2) for v in m.GetPartBox(True)], flush=True)
    if m.SaveAs3(V + ".SLDPRT", 0, 1):
        raise RuntimeError("servo save failed")
    pm = m.Extension.CustomPropertyManager("")
    for k, v in dict(process="purchase", material="STS3215 servo (vendor)", revision="vendor", owner="Joey").items():
        pm.Add3(k, 30, v, 1)
    m.Save3(1, ref(), ref())
    inst.sw.CloseAllDocuments(True)


def main():
    t0 = time.time()
    steps = ["import", "parts", "asm"]
    steps = steps[steps.index(sys.argv[1]):] if len(sys.argv) > 1 else steps
    inst = session(STATE)
    print("pid", inst.pid, round(time.time() - t0), "s", flush=True)
    if "import" in steps:
        import_servo(inst)
    if "parts" in steps:
        subprocess.run([sys.executable, "-u", str(HERE / "build_parts.py"), STATE], cwd=HERE, check=True)
    if "asm" in steps:
        subprocess.run([sys.executable, "-u", str(HERE / "assemble_check.py"), STATE], cwd=HERE, check=True)
    end_session(STATE)
    print("done", round(time.time() - t0), "s", flush=True)


if __name__ == "__main__":
    main()
