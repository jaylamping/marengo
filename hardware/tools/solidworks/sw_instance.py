"""Run a dedicated SolidWorks instance per session and talk to it by process ID.

    from sw_instance import launch

    with launch() as inst:      # hidden, never takes focus
        sw = inst.sw
        ...

Read RULES.md first. launch() starts a new SLDWORKS.exe and attaches through the
Running Object Table entry for that PID, so it can never reach another session's
instance. Never use GetActiveObject or Dispatch('SldWorks.Application').
"""
import msvcrt
import os
import subprocess
import sys
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
        if win32process.GetWindowThreadProcessId(h)[1] == pid and win32gui.GetClassName(h) == "#32770":
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


def hide_windows(pid):
    """Hide every visible top-level window of this PID. Works while COM is blocked by a solve. Returns what it hid."""
    hid = []

    def find(h, _):
        if win32process.GetWindowThreadProcessId(h)[1] == pid and win32gui.IsWindowVisible(h):
            win32gui.ShowWindow(h, win32con.SW_HIDE)
            hid.append((win32gui.GetClassName(h), win32gui.GetWindowText(h)))
        return True

    win32gui.EnumWindows(find, None)
    return hid


def minimize_windows(pid):
    """Fallback for anything that will not run hidden: minimize this PID's windows without taking focus."""
    hid = []

    def find(h, _):
        if win32process.GetWindowThreadProcessId(h)[1] == pid and win32gui.IsWindowVisible(h):
            win32gui.ShowWindow(h, win32con.SW_SHOWMINNOACTIVE)
            hid.append((win32gui.GetClassName(h), win32gui.GetWindowText(h)))
        return True

    win32gui.EnumWindows(find, None)
    return hid


def hider_log(pid):
    return os.path.join(tempfile.gettempdir(), f"sw_hidden_{pid}.log")


def _watch_hidden(pid):
    """Keep this PID's windows hidden until it exits. Anything hidden is logged, because a hidden modal dialog blocks the instance."""
    seen = set()
    while psutil.pid_exists(pid):
        fore = win32gui.GetForegroundWindow()
        took = fore and win32process.GetWindowThreadProcessId(fore)[1] == pid
        for item in hide_windows(pid):
            if item not in seen or took:
                seen.add(item)
                with open(hider_log(pid), "a", encoding="utf8") as f:
                    f.write(f"{time.strftime('%H:%M:%S')} {item[0]} | {item[1]}{' | HAD FOCUS' if took else ''}\n")
        time.sleep(0.1)


def _start_hider(pid):
    """A separate process, so it keeps working while this one waits on a blocking COM call and after it exits."""
    exe = sys.executable
    w = exe.replace("python.exe", "pythonw.exe")
    subprocess.Popen(
        [w if os.path.exists(w) else exe, os.path.abspath(__file__), "--hide-watch", str(pid)],
        creationflags=subprocess.CREATE_NO_WINDOW | subprocess.DETACHED_PROCESS,
        close_fds=True,
    )


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

    def hide(self):
        return hide_windows(self.pid)

    def minimize(self):
        return minimize_windows(self.pid)

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


def launch(timeout=300, visible=False):
    """Start a new SolidWorks process and return an Instance attached to it by PID.

    Hidden by default, so it never pops up or takes focus. Pass visible=True only when someone needs to see it.
    """
    if not os.path.exists(EXE):
        raise RuntimeError(f"SolidWorks not found at {EXE}")
    pythoncom.CoInitialize()
    with _LaunchLock(2 * timeout):
        si = subprocess.STARTUPINFO()
        if not visible:
            si.dwFlags |= subprocess.STARTF_USESHOWWINDOW
            si.wShowWindow = win32con.SW_HIDE
        proc = subprocess.Popen([EXE], startupinfo=si)
        pid = proc.pid
        if not visible:
            _start_hider(pid)
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
    if not visible:
        sw.UserControl = False
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


def session(state_file, visible=False):
    """Reuse this session's instance across script runs, keyed by a PID file the session owns."""
    state = str(state_file)
    if os.path.exists(state):
        pid = int(open(state).read().strip())
        if psutil.pid_exists(pid) and psutil.Process(pid).name().lower() == "sldworks.exe":
            return attach(pid)
    inst = launch(visible=visible)
    with open(state, "w") as f:
        f.write(str(inst.pid))
    return inst


if __name__ == "__main__" and len(sys.argv) == 3 and sys.argv[1] == "--hide-watch":
    _watch_hidden(int(sys.argv[2]))
