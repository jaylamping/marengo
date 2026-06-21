HANDOFF CONTEXT
===============

USER REQUESTS (AS-IS)
---------------------
- Session started from previous handoff doc: "lets continue robot testing, check your handoff doc"
- "ready" (load attached at mechanical zero)
- "something went wrong" (T6 Part B velocity trip on fast retarget)
- "save a handoff"

GOAL
----
Complete T7 ladder rungs (-0.5, -0.7, -0.85), T8 recovery protocol, T9 disable-drop, T10 full suite verdict, and address T6 watchdog protocol mode-scope issue.

WORK COMPLETED
--------------
- Reverted config: direction=-1 restored in motors.yaml, velocity limits at original 2.0/2.5 rad/s
- Re-zeroed arm at mechanical home with 700g load (pos=0.0000 rad)
- T4 friction ID sweeps at 0.1, 0.5, 1.0 rad/s - all 3 clean, fault=0x0000
- Restored position_trajectory_velocity_rad_s to 1.45 rad/s
- T5 mode-switch transient test - GravityComp->Position transition captured
- T6 Part A: inverted direction test - watchdog didn't fire because hold-at 0.5 transitions from GravityComp to Position mode, and the watchdog only fires in GravityComp mode (known protocol gap)
- T6 Part B: grace period test - fast retarget 0.5->-0.3 caused Davout velocity-limit trip at 8 rad/s (exceeds 2.5 limit). No WrongSignWatchdog fired (expected - only applies in GravityComp)
- T7 gentle gate: hold-at -0.3 rad from home - successful, fault=0x0000, peak sanitized dq ~1.6 rad/s

CURRENT STATE
-------------
- Pi rev 203ab0b, CAN up, homing Verified, motor Disabled, fault=0x0000
- Arm at home (pos=-0.0019 rad) with 700g load attached
- Config clean (no dirty files): direction=-1, velocity_max_rad_s=2.5, velocity_limit_rad_s=2.0, position_trajectory_velocity_rad_s=1.45
- Latest run was T7 gentle gate hold-at -0.3, returned home clean
- muJoCo sim: not involved in this test session

PENDING TASKS
-------------
- T6 Part A re-run: need GravityComp-only test (without hold-at switching to Position mode) to trigger WrongSignWatchdog. Approach: enable with gravity-on at elevated pose, let inverted tau_g cause wrong-sign fall
- T7 ladder: -0.5, -0.7, -0.85 rad holds from home (each as separate pi_hold_on with return_home_sec=6)
- T8: Recovery protocol - deliberate disable at elevated pose, confirm clean re-enable
- T9: Disable-drop - measure drop distance on disable at 0.5 rad
- T10: Full suite re-run and written verdict in bench-gravity-comp-test-suite-results.md
- Layer 2 gate analysis: run analyze-position-trace.py on collected traces to verify dq peak, tau_g error band, friction model fit

KEY FILES
---------
- config/bringup/shoulder_pitch_right_only/ - bench profile config (clean, direction=-1)
- config/bringup/shoulder_pitch_right_only/control.yaml - velocity limits and gains
- config/bringup/shoulder_pitch_right_only/motors.yaml - direction and bench limits
- docs/bench-gravity-comp-test-suite.md - test protocol document
- docs/bench-gravity-comp-test-suite-results.md - results document (needs verdict update)
- scripts/check.sh - CI-parity check to run before PR
- .omo/session-handoff-2026-06-21.md - this handoff

IMPORTANT DECISIONS
-------------------
- T5 used gravity-on/gravity-off commands (not non-existent "mode position/gravity") to trigger mode transitions
- T6 Part A protocol has a mode-scope gap: hold-at 0.5 transitions GravityComp->Position, watchdog is GravityComp-only. Need dedicated GravityComp-only run
- T7 gentle gate at -0.3 rad succeeded with Davout sanitizing velocity spikes
- Davout velocity spike sanitization is working correctly - all "ignored uncorroborated feedback velocity spike" events showed sanitized velocity << 2.5 limit
- The velocity limit trip during T6 Part B fast retarget (8 rad/s) is expected behavior for a 0.8 rad delta descent under gravity

EXPLICIT CONSTRAINTS
--------------------
- None specific to this session beyond project AGENTS.md

CONTEXT FOR CONTINUATION
------------------------
- Pi MCP tools are available for all bench operations
- Use pi_hold_on for position holds (supports return_home_sec for auto-return)
- Use pi_marengo_pi_script for scripted sequences (enable, gravity-on, hold-at, sleep, disable, quit)
- Use pi_motor_recover after velocity-limit or watchdog trips
- Arm has 700g load attached at all times
- The position-trace CSV is at /opt/marengo/var/log/position-trace-<TS>.csv on Pi after each session
- Candump logs at /opt/marengo/var/log/candump-<TS>.log
- Position traces can be fetched via pi_read_file for local analysis
- Velocity spike sanitation at 2.5 rad/s limit is active in Davout
- homing-preflight.sh Permission denied is a pre-existing non-blocking issue (can be fixed by chmod +x)

---

TO CONTINUE IN A NEW SESSION:

1. Press 'n' in OpenCode TUI to open a new session, or run 'opencode' in a new terminal
2. Paste the HANDOFF CONTEXT above as your first message
3. Add your request: "Continue from the handoff context above. Resume T7 ladder test."