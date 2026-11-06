"""Run a dedicated SolidWorks instance per session and talk to it by process ID.

    from sw_instance import launch

    with launch() as inst:
        sw = inst.sw
        ...

Read RULES.md first. launch() starts a new SLDWORKS.exe and attaches through the
Running Object Table entry for that PID, so it can never reach another session's
instance. Never use GetActiveObject or Dispatch('SldWorks.Application').
"""
import msvcrt
import os
import subprocess
import tempfile
import time

import psutil
import pythoncom
import win32com.client as wc
import win32con
import win32gui
import win32process

EXE = r"C:\Program Files\SOLIDWORKS Corp\SOLIDWORKS\SLDWORKS.exe"
SIM_DLL = r"C:\Program Files\SOLIDWORKS Corp\SOLIDWORKS\Simulation\cosworks.dll"
LOCK = os.path.join(tempfile.gettempdir(), "sw_instance_launch.lock")


class _LaunchLock:
    """Two SolidWorks processes started together can hang at startup, so launches queue across sessions."""

    def __init__(self, timeout):
        self.timeout = timeout

    def __enter__(self):
        self.f = open(LOCK, "a+b")
        start = time.time()
        while True:
            try:
                self.f.seek(0)
                msvcrt.locking(self.f.fileno(), msvcrt.LK_NBLCK, 1)
                return self
            except OSError:
                if time.time() - start > self.timeout:
                    self.f.close()
                    raise RuntimeError("timed out waiting for another SolidWorks launch to finish")
                time.sleep(1)

    def __exit__(self, *exc):
        self.f.seek(0)
        msvcrt.locking(self.f.fileno(), msvcrt.LK_UNLCK, 1)
        self.f.close()


def _clear_journal_prompt(pid):
    """A third SolidWorks process stops at a modal 'journal file could not be created' prompt. Click OK on it.

    Only that exact prompt, only in this PID. That instance has no Auto Recover, so save after each step.
    """
    tops = []

    def find(h, _):
        if win32process.GetWindowThreadProcessId(h)[1] == pid and win32gui.IsWindowVisible(h) and win32gui.GetClassName(h) == "#32770":
            tops.append(h)
        return True

    win32gui.EnumWindows(find, None)
    for h in tops:
        kids = []
        win32gui.EnumChildWindows(h, lambda c, _: kids.append(c) or True, None)
        texts = {win32gui.GetWindowText(c): c for c in kids}
        if any("journal file could not be created" in s for s in texts) and "OK" in texts:
            win32gui.PostMessage(texts["OK"], win32con.BM_CLICK, 0, 0)
            return True
    return False


def _attach_by_pid(pid):
    """Return the IDispatch registered in the ROT for this PID, or None."""
    rot = pythoncom.GetRunningObjectTable()
    ctx = pythoncom.CreateBindCtx(0)
    tag = f"SolidWorks_PID_{pid}".lower()
    for moniker in rot.EnumRunning():
        name = moniker.GetDisplayName(ctx, None).lower()
        if name.lstrip("!") == tag:
            return rot.GetObject(moniker).QueryInterface(pythoncom.IID_IDispatch)
    return None


class Instance:
    def __init__(self, pid, sw, owned=True):
        self.pid = pid
        self.sw = sw
        self.owned = owned
        self.no_auto_recover = False

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    def rss_mb(self):
        return round(psutil.Process(self.pid).memory_info().rss / 1e6)

    def load_simulation(self):
        """Load the Simulation add-in in this instance. Returns its CosmosWorks object."""
        add = None
        for prog in ("SldWorks.Simulation", "CosmosWorks.CosmosWorks"):
            try:
                add = self.sw.GetAddInObject(prog)
            except Exception:
                add = None
            if add is not None:
                break
        if add is None:
            rc = self.sw.LoadAddIn(SIM_DLL)
            if rc != 0:
                raise RuntimeError(f"Simulation add-in did not load, LoadAddIn returned {rc}; 7 means a license error")
            add = self.sw.GetAddInObject("SldWorks.Simulation")
        if add is None:
            raise RuntimeError("Simulation add-in object not available")
        return wc.dynamic.Dispatch(add.CosmosWorks._oleobj_)

    def close(self, timeout=60):
        """Close this instance's documents and exit it. Only ever touches self.pid."""
        if not self.owned:
            raise RuntimeError("this session does not own that instance")
        if self.sw is not None:
            if int(self.sw.GetProcessID) != self.pid:
                raise RuntimeError("handle does not match this instance's PID")
            self.sw.CloseAllDocuments(True)
            self.sw.ExitApp()
            self.sw = None
        pythoncom.CoFreeUnusedLibraries()
        try:
            psutil.Process(self.pid).wait(timeout)
        except psutil.TimeoutExpired:
            # ExitApp can leave a half-closed process with no documents and no Simulation license.
            psutil.Process(self.pid).kill()
            psutil.Process(self.pid).wait(30)
        except psutil.NoSuchProcess:
            pass


def launch(timeout=300, visible=True):
    """Start a new SolidWorks process and return an Instance attached to it by PID."""
    if not os.path.exists(EXE):
        raise RuntimeError(f"SolidWorks not found at {EXE}")
    pythoncom.CoInitialize()
    with _LaunchLock(2 * timeout):
        proc = subprocess.Popen([EXE])
        pid = proc.pid
        start = time.time()
        disp = None
        while time.time() - start < timeout:
            if proc.poll() is not None:
                raise RuntimeError(f"SolidWorks exited during start, code {proc.returncode}")
            try:
                disp = _attach_by_pid(pid)
            except pythoncom.com_error:
                disp = None
            if disp is not None:
                break
            time.sleep(1)
        if disp is None:
            proc.kill()
            raise RuntimeError(f"SolidWorks PID {pid} never registered in the ROT")
        sw = wc.dynamic.Dispatch(disp)
        journal = False
        while time.time() - start < timeout:
            journal = _clear_journal_prompt(pid) or journal
            try:
                if sw.StartupProcessCompleted:
                    break
            except pythoncom.com_error:
                pass
            time.sleep(1)
        else:
            proc.kill()
            raise RuntimeError(f"SolidWorks PID {pid} did not finish starting")
    got = int(sw.GetProcessID)
    if got != pid:
        raise RuntimeError(f"attached to PID {got}, expected {pid}")
    sw.Visible = visible
    inst = Instance(pid, sw)
    inst.no_auto_recover = journal
    return inst


def attach(pid):
    """Attach to an instance this session already launched, for example from an earlier script run."""
    pythoncom.CoInitialize()
    disp = _attach_by_pid(pid)
    if disp is None:
        raise RuntimeError(f"no SolidWorks PID {pid} in the ROT")
    sw = wc.dynamic.Dispatch(disp)
    got = int(sw.GetProcessID)
    if got != pid:
        raise RuntimeError(f"attached to PID {got}, expected {pid}")
    return Instance(pid, sw)


def session(state_file):
    """Reuse this session's instance across script runs, keyed by a PID file the session owns."""
    state = str(state_file)
    if os.path.exists(state):
        pid = int(open(state).read().strip())
        if psutil.pid_exists(pid) and psutil.Process(pid).name().lower() == "sldworks.exe":
            return attach(pid)
    inst = launch()
    with open(state, "w") as f:
        f.write(str(inst.pid))
    return inst
