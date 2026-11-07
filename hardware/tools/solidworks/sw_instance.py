"""Run a dedicated SolidWorks instance per session and talk to it by process ID.

    from sw_instance import launch

    with launch() as inst:      # hidden, never takes focus
        sw = inst.sw
        ...

Read RULES.md first. launch() starts a new SLDWORKS.exe and attaches through the
Running Object Table entry for that PID, so it can never reach another session's
instance. Never use GetActiveObject or Dispatch('SldWorks.Application').
"""
import ctypes
import ctypes.wintypes as wt
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
    """Hide windows of this PID and its child processes the moment they show, and hand focus back if one takes it.

    Runs on window events rather than polling, so a window has no time to paint. Child processes matter:
    sldexitapp.exe opens the SOLIDWORKS Error Report after an exit. Anything hidden is logged, because a hidden
    modal dialog blocks the instance.
    """
    user = ctypes.windll.user32
    ours = {pid: _started(pid)}  # pid to start time, because Windows reuses pids
    state = {"prev": None}
    seen = set()

    def live(p):
        return p in ours and ours[p] is not None and ours[p] == _started(p)

    def is_ours(p):
        if live(p):
            return True
        try:
            q = psutil.Process(p)
            crash = q.name().lower() == "sldexitapp.exe" and f"/SWPID={pid}" in " ".join(q.cmdline()).upper()
            if crash or live(q.ppid()):
                ours[p] = q.create_time()
                return True
        except psutil.Error:
            pass
        return False

    def log(h, took):
        item = (win32gui.GetClassName(h), win32gui.GetWindowText(h))
        if item not in seen or took:
            seen.add(item)
            with open(hider_log(pid), "a", encoding="utf8") as f:
                f.write(f"{time.strftime('%H:%M:%S')} {item[0]} | {item[1]}{' | HAD FOCUS' if took else ''}\n")

    def event(_hook, ev, h, obj, child, _thread, _time):
        try:
            if obj != 0 or child != 0 or not h or user.GetAncestor(h, 2) != h:
                return
            p = win32process.GetWindowThreadProcessId(h)[1]
            if not is_ours(p):
                if ev == 3:
                    state["prev"] = h
                return
            if win32gui.IsWindowVisible(h):
                win32gui.ShowWindow(h, win32con.SW_HIDE)
            log(h, ev == 3)
            if ev == 3 and state["prev"] and win32gui.IsWindow(state["prev"]):
                user.SwitchToThisWindow(state["prev"], True)
        except Exception:
            pass

    proc = ctypes.WINFUNCTYPE(None, wt.HANDLE, wt.DWORD, wt.HWND, wt.LONG, wt.LONG, wt.DWORD, wt.DWORD)(event)
    user.SetWinEventHook.restype = wt.HANDLE
    user.SetWinEventHook(0x8002, 0x8002, 0, proc, 0, 0, 0)  # EVENT_OBJECT_SHOW
    user.SetWinEventHook(3, 3, 0, proc, 0, 0, 0)  # EVENT_SYSTEM_FOREGROUND
    fore = win32gui.GetForegroundWindow()
    if fore and win32process.GetWindowThreadProcessId(fore)[1] not in ours:
        state["prev"] = fore
    msg = wt.MSG()
    sweep = 0
    gone = None
    while True:
        user.MsgWaitForMultipleObjectsEx(0, None, 500, 0x04FF, 0)
        while user.PeekMessageW(ctypes.byref(msg), 0, 0, 0, 1):
            user.TranslateMessage(ctypes.byref(msg))
            user.DispatchMessageW(ctypes.byref(msg))
        if time.time() - sweep > 2:
            sweep = time.time()
            alive = [q for q in ours if live(q)]
            for q in alive:
                try:
                    for c in psutil.Process(q).children(recursive=True):
                        ours.setdefault(c.pid, c.create_time())
                except psutil.Error:
                    pass
            for q in [q for q in ours if not live(q)]:
                del ours[q]
            alive = list(ours)
            # A crash report can appear after the instance has gone, so stay a minute longer.
            gone = (gone or time.time()) if not alive else None
            if gone and time.time() - gone > 60:
                return
            for q in alive:
                hide_windows(q)


def _started(pid):
    try:
        return psutil.Process(pid).create_time()
    except psutil.Error:
        return None


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
        # ExitApp can start sldexitapp.exe, which opens a visible Error Report. Only this PID's own child.
        for p in psutil.process_iter(["name", "ppid"]):
            if p.info["ppid"] == self.pid and (p.info["name"] or "").lower() == "sldexitapp.exe":
                p.kill()


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
        journal = False
        while time.time() - start < timeout:
            if proc.poll() is not None:
                raise RuntimeError(f"SolidWorks exited during start, code {proc.returncode}")
            journal = _clear_journal_prompt(pid) or journal
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


def end_session(state_file):
    """Close this session's instance if it is running. Never starts one."""
    state = str(state_file)
    if os.path.exists(state):
        pid = int(open(state).read().strip())
        if psutil.pid_exists(pid) and psutil.Process(pid).name().lower() == "sldworks.exe":
            attach(pid).close()
        os.remove(state)


if __name__ == "__main__" and len(sys.argv) == 3 and sys.argv[1] == "--hide-watch":
    _watch_hidden(int(sys.argv[2]))
