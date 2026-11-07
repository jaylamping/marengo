# SolidWorks instance rules

Every session runs its own SolidWorks instance, so sessions do not take turns. Joey made this the standard on 2026-11-06.

```python
import sys
sys.path.insert(0, r"J:\code\marengo\hardware\tools\solidworks")
from sw_instance import launch

with launch() as inst:          # new SLDWORKS.exe, hidden, attached by PID
    sw = inst.sw                # ordinary SldWorks COM object
    cw = inst.load_simulation() # only if the session needs FEA
    ...
# leaving the block closes this instance's documents and exits only this PID
```

`inst.pid` is the session's PID. If `inst.no_auto_recover` is True, that instance has no Auto Recover.

To reuse one instance across separate script runs, use `session(state_file)` with a PID file the session owns, and call `inst.close()` when finished.

## Hidden by default

Joey uses the PC while sessions work, so SolidWorks must never pop up or take focus. `launch()` and `session()` now start hidden. Pass `visible=True` only when someone needs to see the window.

A hidden launch does three things:

- Starts SLDWORKS.exe with its window hidden.
- Starts a small separate process that hides every window of that PID and its child processes the moment it shows, using window events, until they all exit. If one takes focus, it hands focus back. It keeps working during a blocking solve, when COM can't be called.
- Sets `Visible` and `UserControl` to False.

Setting `sw.Visible = False` alone is not enough. It read back True and the window stayed up.

Anything hidden is logged to `sw_hidden_<pid>.log` in the temp folder, and `HAD FOCUS` marks a window that took focus. **A hidden modal dialog blocks the instance, so if a script hangs, read that log first.**

**What was popping up on 2026-11-07:** a watcher logged every window for a real run. Two things showed.

- `sldexitapp.exe`, started by SolidWorks on exit, opened a visible **SOLIDWORKS Error Report** that stayed on screen and took focus. It is a child process, so hiding the SolidWorks PID didn't cover it. `close()` now also stops that child, and the hider covers child processes.
- Each launch briefly took focus for the splash, the main window and the welcome screen. Polling at 10 Hz was too slow for that.

No console windows showed, from Python, PowerShell, git or Bambu Studio.

Use `end_session(state_file)` to close an instance. Calling `session(state_file).close()` when the instance is already gone starts a new one just to close it.

If something won't run hidden, use `inst.minimize()`, which never takes focus. `inst.hide()` hides on demand. From another process, or for a stuck instance of your own, call `hide_windows(pid)`.

For Bambu Studio, use the CLI (`--slice`), which opens no window.

solidworks-mcp servers start their instance through their own code, so they don't get this until that build is changed.

## Rules

1. Each session starts its own instance with `launch()` and uses that handle. Keep it hidden.
2. Never use `GetActiveObject`, `Dispatch("SldWorks.Application")` or anything else that reaches "the running SolidWorks". It can land in another session's instance.
3. Never call `ExitApp`, close documents in, or kill a PID the session did not start.
4. One file belongs to one session. Other sessions open a copy, read-only. Agree ownership before writing.
5. Save after each step. A third instance has no Auto Recover, and an instance can crash.
6. Concurrent Simulation solves are allowed, up to the two tested so far. Keep large solves to one at a time until a larger pair is tested.
7. Exit the instance when the work is done. Each one holds about 1 GB idle and about 1.6 GB after a solve.
8. solidworks-mcp follows these rules from branch `feat/owned-instance`: each MCP server launches and owns one instance, attached by PID. **A server started before that build still attaches to the running SolidWorks until it restarts**, so do not use its tools alongside other sessions. Running servers are not restarted for you.

## What was tested

Joey-PC, SolidWorks 2026 SP0, 2026-11-06, with another session's instance open throughout and untouched.

| Check | Result |
| --- | --- |
| New instance attached by PID, `GetProcessID` matches | Pass, 13 to 17 s to attach |
| Part, sketch, extrude, save, close | Pass |
| Simulation add-in in a second instance | Pass, no license prompt |
| Mesh and solve, Ø40 × 100 mm cylinder, 12,220 nodes | Pass, 0.7 s and 1.0 s |
| Two instances solving at once, 476,266 nodes each, started within 0.1 s | Pass, 19.1 and 19.4 s against 15.4 to 15.8 s alone, same peak displacement |
| Exit own instance, other instance still responding | Pass |
| RAM | About 950 MB idle, 1.3 GB with Simulation, 1.6 GB after the solve |

Hidden mode, Joey-PC, 2026-11-07:

| Check | Result |
| --- | --- |
| Launch to ready | Pass. Splash and welcome screen hidden within 0.1 s of appearing. No window took focus |
| Open, modify, save and STL export | Pass |
| Simulation add-in, mesh and solve, 307,835 nodes | Pass, 29 to 35 s. No new windows and no focus change during the run |
| Exit | Pass, hider process stops by itself |

Two startup problems came up. `launch()` handles both:

- Two SolidWorks processes starting at the same moment can stall, so launches queue on a lock file.
- A third process stops at a modal prompt, "A journal file could not be created. Auto recover will not work". `launch()` clicks OK on exactly that prompt, in its own PID only, even when it is hidden, and sets `no_auto_recover`.

Four instances ran at once in the solidworks-mcp test, each in its own PID, with no modeling load on all four.

`ExitApp` can leave a half-closed process with no documents and no Simulation license, so `close()` force-stops its own PID if it has not gone after 60 seconds.
