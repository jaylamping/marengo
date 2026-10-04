# Marengo Pi MCP

MCP server for Marengo bench control on a Raspberry Pi over SSH. Runs on your dev machine; the Pi is only an SSH target.

## Setup

### SSH

```text
# ~/.ssh/config
Host marengo.local
  Hostname marengo.local
  User joey
  IdentityFile ~/.ssh/id_ed25519
```

Verify: `ssh joey@marengo.local 'echo ok'`

If mDNS fails, set `MARENGO_PI_HOST` in the environment before Cursor starts the MCP (or export it in your shell profile). Do not add it to `.cursor/mcp.json` — that env is hashed into Cursor’s MCP approval key and thrashing it auto-disables the server.

### Passwordless sudo (Pi)

`scripts/install-pi.sh` provisions these rules when run as root. For manual repair:

```sudoers
# Marengo MCP — passwordless sudo for bench scripts only (visudo -f /etc/sudoers.d/marengo-joey)
joey ALL=(root) NOPASSWD: /opt/marengo/scripts/can-up.sh *
joey ALL=(root) NOPASSWD: /opt/marengo/scripts/install-pi.sh
joey ALL=(root) NOPASSWD: /home/joey/marengo/scripts/install-pi.sh
```

Not full passwordless sudo. Only these paths.

Verify:

```bash
sudo -n /opt/marengo/scripts/can-up.sh can0 can1
sudo -n /home/joey/marengo/scripts/install-pi.sh   # staging → /opt (use this after deploy)
```

`sudo -n true` fails by design. After editing files under `~/marengo`, run `pi_install_staging` (MCP) or the staging install command above. Do not run `/opt/marengo/scripts/install-pi.sh` alone; it re-copies stale `/opt` content.

### Build MCP server

From repo root:

```bash
just mcp-build
```

Or:

```bash
cd tools/marengo-pi-mcp
npm install
npm run build
```

Restart the marengo-pi MCP server in Cursor after rebuilding.

If a tool still shows old behavior after `npm run build`, the Cursor MCP process is stale. Restart marengo-pi before retrying.

### Cursor `mcp.json`

Repo [`.cursor/mcp.json`](../../.cursor/mcp.json) uses `${workspaceFolder}` so the same file works on **Windows, macOS, and Linux/WSL**.

**WSL / Linux / macOS (software home):** mcp.json uses **`bash` + [`run-mcp.sh`](run-mcp.sh)**. Cursor's spawn PATH often lacks mise `node`, so bare `node` fails with `spawn node ENOENT`. The shell launcher finds mise (or `MARENGO_MCP_NODE`).

**Windows CAD session:** prefer [`run-mcp.cmd`](run-mcp.cmd) / [`run-mcp.ps1`](run-mcp.ps1) — Cursor often lacks Git `sh`/`bash`. Those wrappers exec [`dist/launch.js`](src/launch.ts) once `node` is resolved.

Profile / SSH defaults live in [`src/launch.ts`](src/launch.ts) / `run-mcp.sh` — **not** in `mcp.json` `env` (that thrash auto-disables the project MCP; see [ADR 0016](../../docs/decisions/0016-wsl-software-home.md)).

Open the marengo repo root as the Cursor workspace (WSL software session, or Windows UNC CAD session — ADR 0016).

`MARENGO_LOCAL_ROOT` is optional. The server derives the repo root from its install path. Override only for unusual clone layouts.

After clone or pull that touches MCP sources:

```bash
just mcp-build
```

Then restart the marengo-pi MCP server in Cursor.

**Stay enabled:** `.cursor/hooks.json` runs `session-start-marengo.js` on `sessionStart` (`--write --best-effort`) so disabled/unapproved state is scrubbed when a chat opens. If the server still shows as disabled after a config thrash:

```bash
# Quit Cursor first for a hard write, then:
just mcp-ensure-enabled --write
```

## Tool summary

| Class | Confirm | Examples |
|-------|---------|----------|
| Read-only | No | `pi_logs_tail`, `pi_health`, `pi_homing_status`, `pi_motor_repl_status`, `pi_protocol_inspect`, `pi_gravity_preview`, `pi_imu_probe` |
| Admin | No | `pi_can_up`, `pi_sync_main`, `pi_sync_tree`, `pi_sync_bench_config`, `pi_sync_bench_urdf`, `pi_wait_deploy`, `pi_install_staging`, `pi_git_pull`, `pi_build` |
| Admin | Yes | `pi_restart_marengo_pi`, `pi_clean_tree` |
| Motion | Yes | `pi_motor_disable`, `pi_motor_recover`, `pi_set_zero`, `pi_hold_on`, `pi_hold_off`, `pi_bench_harness`, `pi_marengo_pi_script`, `pi_gravity_calibrate`, `pi_enable_soak` |

Weighted profile (`weighted_single_arm`, `arm_attached`) needs `confirm: true` and `confirm_weighted_motion: true`.

### One CAN owner

`motor-repl status` opens SocketCAN for its probe but bypasses Davout Supervisor
construction, so it sends no startup type-24 active-reporting burst.
`gravity-preview` reads the configured URDF model locally and does not open
CAN. `disable` and `set-zero` are CAN-owning commands; the latter uses Davout's
qualified reference workflow and an independent exit stop. `protocol-inspect`
(ADR 0037) also owns CAN: it stops every drive with Disable only, turns
reporting Off and reads firmware version, MCU id and registers (including the
0x7028 drive CAN timeout) one query at a time; it never enables or writes a
parameter, so `pi_protocol_inspect` needs no confirmation and prints a per-joint
firmware / CanTimeout table. MCP's ownership guard remains conservative: while
`marengo-pi` or any `motor-repl` process runs, `pi_motor_repl_status`,
`pi_protocol_inspect`, `pi_gravity_preview` and `pi_can_up` skip rather than
compete for the bus; stop `marengo-pi` with `pi_restart_marengo_pi` first.

Reference grants live only inside the `marengo-pi` process that acquired them
(ADR 0036). `pi_health`, `pi_homing_status` and `pi_sync_bench_config` (with
`install_to_opt`) therefore:

- while `marengo-pi` runs (`pgrep -x marengo-pi`), show per-joint homing from its own `RobotState`, read from the gateway's `/snapshot/robot/state`;
- otherwise print `no live marengo-pi session: reference grants are process-local (ADR 0036)` and the latest reference journal rows from `scripts/reference-journal-tail.py`, which opens `/opt/marengo/var/calibration/reference-journal.sqlite3` read-only. Journal rows are history and never grant a reference.

`install-pi.sh` runs no homing check; it prints one line pointing at the in-process procedure in [docs/homing.md](../../docs/homing.md).

Motion tools that open CAN take sole ownership of the bus for the session. These are `pi_motor_disable`, `pi_motor_recover`, `pi_set_zero`, `pi_hold_on`, `pi_hold_off`, `pi_marengo_pi_script`, `pi_gravity_calibrate`, `pi_enable_soak` and `pi_bench_harness`. Each session:

1. Stops `marengo-pi.service` with `sudo -n /usr/local/libexec/marengo/pi-restart-marengo-pi.sh stop`, the same helper `pi_restart_marengo_pi` uses. The service runs as `marengo` with `Restart=always`, so a bare `pkill` either fails or lets systemd start a second owner within 5 s.
2. Kills leftover `marengo-pi` processes owned by the deploy user.
3. Refuses to run (exit 1) if any `marengo-pi` or `motor-repl` is still running.
4. Waits for the bus to settle before every marengo-pi launch (`pi_hold_on`, `pi_marengo_pi_script`, `pi_motor_recover`, the harness session) and before restoring the unit. No `marengo-pi`/`motor-repl` may run, and the kernel CAN error counters (`rx_errors`, `rx_over_errors`, `tx_errors`, bus errors, error-passive, bus-off) must hold still for 0.5 s. The log shows `can settle: ok … <counters>`. marengo-pi latches a persistent Transport fault on any CAN error frame, and the mcp251x reports an RX FIFO overrun as one. A motor-repl run ending right before marengo-pi started (two `motor-repl disable`s, then marengo-pi's own type-24 burst) overran it at startup on 2026-10-03. If no window settles within 2 s, the session logs `can settle: FAIL` and refuses to start marengo-pi (exit 1); the unit restore warns and restarts anyway. Bench sessions run one pre-session `motor-repl disable` before the settle. `pi_hold_on` also logs `can errors after marengo-pi: …`, and runs its trailing `motor-repl disable` only once marengo-pi has exited. If marengo-pi still owns CAN, it prints `post-session bin/motor-repl disable skipped: …` and exits 1.
5. Restarts the unit when the session ends if it was active before, the same way `install-pi.sh` restores state. The unit comes back Disabled. The restart runs on every exit path, including errors and SIGHUP/SIGTERM. `pi_bench_harness` restarts it in a final `restore_marengo_pi_service` step.

To keep control off after a session, run `pi_restart_marengo_pi` with `mode: stop`.

### Reference and zero (no Motor Studio)

A current reference is granted only inside the process that acquires it. When that process exits, the grant ends. Acquiring a reference runs SetZero at the joint's current pose, so put each joint at its mechanical reference first.

- `pi_hold_on` and `pi_bench_harness` acquire references inside their own marengo-pi session. They send `home <joints> sign-tested` on stdin and wait until marengo-pi prints `reference <joint> current pos=…` for every joint. Then they send the plain `home`, `enable` and hold lines, so dwell sleeps begin only once the references are held. If a joint prints `reference <joint> failed|skipped: …` or `home failed: …`, or the wait passes 10 s per joint, the session sends `disable` and `quit` and exits 1.
  `pi_hold_on` and `pi_gravity_calibrate` also await the plain `home` (until `homing verified`) and `enable` (until `enabled (operator=…)`). marengo-pi prints `enabled` only once enable has completed: every target's Enable is written (a just-zeroed drive's is held about 800 ms past its SetZero) and each target has fresh session feedback, after a `waiting for enable to complete` line. On `home failed:`, `enable failed|blocked|refused:` (including an Enable not complete within 2 s) or no answer within 10 s, the session sends `disable` and `quit` instead of any hold line and exits 1, and the tool result names marengo-pi's refusal line.
  `pi_marengo_pi_script` applies the reference wait to any `home <joints> sign-tested` line in its script.
- Both tools require `set_zero: true` **and** `at_mechanical_reference: true`. Without them they refuse before contacting the Pi, so a session never re-zeros at an arbitrary pose. `pi_hold_on` references only `joint` when you give one. If you omit it, it references every joint of the bench profile and holds `right_shoulder_pitch`. `pi_bench_harness` references `joints`, or every joint of the profile, once. It then runs all enable-requiring suites in **one** marengo-pi process: grants survive a clean `disable` but not the process, and re-zeroing per suite would accumulate the return-to-0 tracking error. Before each later suite it checks `$LOG` and stops if an earlier suite failed.
- `pi_set_zero` runs `motor-repl set-zero <joint> --sign-tested`, which prints its own qualified readback (`set-zero <joint> verified pos=…`), then `motor-repl disable`. The grant ends when motor-repl exits, so it doesn't let a later marengo-pi enable.
- `pi_motor_recover` never acquires a reference and never enables. After a fault the arm isn't attested at the reference, so it disables and reads `fault=` from `status` while Disabled.

```json
{
  "confirm": true,
  "joint": "right_shoulder_pitch",
  "set_zero": true,
  "at_mechanical_reference": true,
  "position_rad": 0.1
}
```

`config_dir` defaults to `/opt/marengo/config`. For a 3-DOF harness run, select the harness profile that exports `MARENGO_JOINT_SUBSET`. Don't point `MARENGO_CONFIG_DIR` at a separate bringup tree.

Profiles without a joint subset (`bare_motor`, `weighted_single_arm`, `arm_attached`) reference and gravity-gate exactly the master `config/robot.yaml` joints, `MASTER_JOINTS` in `src/bench-profiles.ts`: right shoulder pitch, shoulder roll, upper-arm yaw, elbow pitch and lower-arm yaw. Subset profiles use robot.yaml-order prefixes of that list. `test/bench-profiles.test.ts` fails when these drift from `config/robot.yaml` or `config/motors.yaml`.

### Gravity-model gate

`pi_hold_on` and `pi_bench_harness` run one shared gate (`src/gravity-gate.ts`) before anything can enable, for every bench profile. It applies the limb-playbook §4a/4b bar: a joint fails when its residual is **≥ 0.20 Nm**. On a failure the tool stops with `FAIL gravity_model_mismatch` and sends no reference, enable or hold line. `pi_hold_on` returns the report. The harness records `[FAIL] gravity_gate` and restores `marengo-pi.service`.

1. Before taking CAN, the gate reads the Pi clock and the gateway's `/snapshot/robot/state`. This read never touches CAN.
2. As sole CAN owner, it runs `motor-repl gravity-preview`. `pi_hold_on` runs it in its own `soleCanOwnerShell` session. The harness runs it as the `gravity_gate` step, after `can_up` and `motor_repl_status`. Non-zero poses use a full `robot.yaml`-order vector. Standalone `pi_gravity_preview` accepts no angles for the all-zero pose or exactly one angle per master joint in order; the harness's weighted previews also pass all five values.
3. It compares results per gated joint. Gated joints are the referenced joints, plus the hold joint for `pi_hold_on`.

| Basis | When | Residual |
|---|---|---|
| `measured` | Snapshot is at most 1 s old, and every gated joint is `drive_active` and `Verified` (marengo-pi is holding the arm) | `\|τ_meas − τ_g(q_published)\|`, where τ_meas is the gateway `effort` |
| `hanging_rest` | All other cases: no gateway, stale snapshot, or drives disabled | `\|τ_g\|` at the profile's `hangingRestRad` (every joint at its mechanical reference, arm down = 0, where physical gravity torque is ~0) |

In practice `hanging_rest` is the basis that applies. Both tools disable drives before their session, and a disabled Robstride carries no phase current: it reports ~0 Nm whatever the arm weighs, so a disabled reading is not a gravity measurement. `at_mechanical_reference: true` attests that the arm is at that rest pose. If none of the gated joints is in the gravity model, or the preview prints nothing (for example CAN is still owned), the gate fails closed with `FAIL gravity_gate_unavailable`.

Do not raise gains to get past this gate. Fix the URDF instead.

### `pi_gravity_calibrate`

Repeatable right-arm gravity-model calibration (`src/tools/gravity-calibrate.ts`). It runs as **one** marengo-pi session, like `pi_hold_on`: `home <profile joints> sign-tested` (awaited), `home`, `enable`, then `hold-at` each static pose of `sweep_joint` with a `settle_sec + measure_sec` dwell, then `hold-at <sweep_joint> 0`, `status`, `disable`, `quit`. The position trace records τ_meas at every pose for the workstation fitter.

- **Sweep.** `sweep_joint` is `right_shoulder_pitch` (default poses `[0, 0.25, 0.48, 0.8, 1.2]` rad, at least 3) or `right_elbow_pitch` (default `[0, 0.25, 0.5, 0.75]`, with the pitch held at `fixed_pitch_rad`, moved there first and returned to 0 last). Poses (2–12, distinct) are sorted. An up pass overshoots to min − `approach_offset_rad` and visits every pose from below; a down pass overshoots to max + δ and visits every pose from above, so the fit can separate friction from gravity.
- **Opt-ins.** `confirm` (+ `confirm_weighted_motion` on weighted profiles), `set_zero` and `at_mechanical_reference` as for `pi_hold_on`, plus `skip_hanging_rest_gravity_check: true`. The gravity gate runs as usual but its hanging-rest `|τ_g|` mismatch ends in `SKIPPED gravity_model_mismatch (hanging rest) for gravity calibration` instead of refusing: the calibration exists to fix that model. Residuals are still reported; `gravity_gate_unavailable` and a measured-basis mismatch still refuse.
- **Limits.** Before any motion a read-only pre-flight (no CAN) reads the Pi `config_dir` `robot.yaml`, `control.yaml`, `motors.yaml` and the URDF `robot.urdf` names. Every target (overshoots included), the fixed pitch and the return pose 0 must lie in `[max(soft, hard) lower, min(soft, hard) upper]` (control.yaml `position_soft_*_rad` ∩ motors.yaml `bench.position_*_rad`), else it refuses naming joint, value and window. The session's sleep + reference budget must be ≤ 300 s.
- **Outputs.** `var/gravity-calibration/<TS>/` on the workstation (gitignored): `plan.json` (steps = the session's `hold-at` lines in order), `position-trace.csv` (Pi trace verbatim), `pi-marengo.urdf` and `config/{robot,control,motors}.yaml` (captured before motion), `bench-session.txt`. With `run_fit` (default) it runs `cargo run --release -q -p marengo-log-cli -- gravity-fit --dir <dir> --out-dir <repo>/docs/commissioning/calibrations --repo-urdf <repo>/assets/urdf/marengo.urdf [--fit mass:<link>|com:<link> …]` using absolute workspace paths; exit 2 means the fitter refused.
- **Never applied.** Review the proposed inertial patch, apply it to `assets/urdf/marengo.urdf`, then run `pi_sync_bench_urdf` as a separate explicit step (ADR 0017).

### `pi_enable_soak`

No-motion enable reliability check (`src/tools/enable-soak.ts`). Run it after firmware, wiring or software changes, before more supervised motion.

- **Cycles.** Each of `cycles` (1–50, default 20) is a **fresh** marengo-pi process, launched after its own CAN settle, so startup type-24 sync, streams inherited from the previous process, the post-SetZero blackout and the staggered Enable are exercised every time. Stdin is exactly `home <profile joints> sign-tested` (awaited), `home` (awaited), `enable <operator>` (awaited, polled every 20 ms and timed), `sleep dwell_sec` (0.5–10, default 2), `status`, `disable`, `quit`. It never sends hold, gravity, torque, impedance or wave lines; `test/enable-soak.test.ts` checks the script and the generated remote command for motion verbs.
- **Opt-ins.** `confirm`, `set_zero` and `at_mechanical_reference`. Every cycle re-zeroes at the current pose, so the arm must hang limp at its mechanical reference throughout. `profile` (default `arm_attached`) picks the referenced joints; subset profiles export their `MARENGO_JOINT_SUBSET`. `stop_on_fault` (default false) stops after the first unclean cycle. Plans estimated over 480 s (20 s + cycles × (6 s + dwell)) are refused.
- **Why neutral.** While operational=Active and control=Disabled, Berthier sends only kp=0, kd=0, velocity=0, torque_ff=0 MIT per active joint (a status solicit at the current pose), and Davout's filter and stop path keep it neutral. Only stdin hold/gravity/impedance/torque/wave lines or a Chappe testing command (`robot/testing/mit_command_batch`, forwarded by the gateway from Consul Testing hold/wave) leave ControlMode::Disabled. So each soak marengo-pi runs under `env -u MARENGO_CHAPPE_SOCKET`: stdin is its only command source, and the gateway shows no RobotState during the soak. Every MCP-started marengo-pi (soak or not) also claims motion ownership through `MARENGO_MOTION_OWNER=stdin` in the remote preamble (`src/env.ts`): a Consul tab can then only disable/stop it, and its refused enable/Testing/Set Zero commands are published as `motion_refused` events on `robot/audit/action` (see docs/safety.md, "Single motion owner").
- **Per cycle.** References acquired (n of joints), the enable outcome line, fault or refusal lines (Transport, DriveState, homing verify, watchdog, non-zero `fault=0x…`, ERROR lines, feeder/settle refusals), enable → `enabled` ms, can0 `rx_over_errors` and `rx_errors` before and after, and the exit status. A cycle is clean when it exited 0, printed `enabled (operator=…)`, acquired every reference and logged no fault line.
 - **Outputs.** One candump covers the whole session, as for the other bench tools. Each UP interface is recorded with an error mask (`can0,#FFFFFFFF`), so kernel CAN error frames (e.g. the mcp251x RX-overflow controller report) land in the same capture with kernel timestamps; `firmware-timing` decodes them into `kernel_error_frames` (timestamp, interface, error class, controller flags) and the soak report prints a short `kernel error frames:` section (diagnostic only, never PASS/FAIL). The bench log and candump are copied to `var/enable-soak/<TS>/` (gitignored) with `soak-summary.txt`, and `cargo run --release -q -p marengo-log-cli -- firmware-timing --json candump.log` writes `firmware-timing.json` when the checkout has the analyzer.
- **Verdict.** PASS only when every requested cycle ran clean, can0 `rx_over_errors` did not increase over the session, and firmware-timing ran and reported `non_neutral_mit.count` 0. A non-neutral frame is FAIL regardless of anything else, and so is a missing, failing or unparseable analyzer result (`wire-level neutral-MIT check unavailable`): the soak never passes without the wire-level proof.

```json
{ "confirm": true, "set_zero": true, "at_mechanical_reference": true, "cycles": 20, "dwell_sec": 2 }
```

### `pi_sync_main`

1. Local `git pull --ff-only` on `main` (fails if dirty)
2. `./scripts/deploy-pi.sh joey@marengo.local`
3. Remote `install-pi.sh` → `/opt/marengo`
4. Writes `/opt/marengo/.deploy-rev`

### `pi_restart_marengo_pi`

Stops leftover `marengo-pi` processes and restarts `marengo-pi.service` so Davout reloads boot config (URDF + YAML). Numerical Set Limits Apply hot-reloads live limits and write-behind URDF without restart ([ADR 0012](../../docs/decisions/0012-config-db-overrides.md) / [ADR 0017](../../docs/decisions/0017-bench-set-limits-urdf-expand.md)); use restart for binary deploy, structural wiring, or after a failed persist left disk stale. Requires `confirm: true`. Optional `mode: "stop"` skips systemd start. Does not touch `marengo-gateway`.

Canonical remote body: [`scripts/pi-restart-marengo-pi.sh`](../../scripts/pi-restart-marengo-pi.sh) (also used by `pi-remote.sh restart-marengo-pi` / `stop-marengo-pi`, and Consul `POST /control/restart-marengo-pi`). MCP embeds the local checkout copy so the tool works before the Pi has the new script installed.

```json
{ "confirm": true }
```

### `pi_sync_tree`

Sync the Pi staging checkout (`MARENGO_PI_STAGING_ROOT`, default `~/marengo`) with `origin/main` without building or installing. Fails if that tree is dirty. `pi_git_pull`, `pi_clean_tree` and `pi_build` work in the same checkout. `/opt/marengo` is the root-owned install tree and isn't a git checkout.

1. `git fetch origin`
2. `git checkout main`
3. `git pull --ff-only origin main`

## Session logs

Motion runs tee to `$MARENGO_ROOT/var/log/bench-latest.log`. Read with `pi_logs_tail` / `pi_logs_last_fault`.

Session registration calls `/opt/marengo/bin/marengo-log-cli` directly. If that binary is unavailable or registration/archive fails, MCP leaves the hot log, trace and candump files in place; it does not prune unarchived evidence.

### Motor recover (no Motor Studio)

```json
{ "confirm": true }
```

Tool: `pi_motor_recover`. Disable drives, brief `status` with `fault=0x…`, prints `RECOVER_OK` or `RECOVER_FAIL`. Optional: `"config_dir": "/opt/marengo/config"`; omission uses the same master directory.

## Skills

- [`.cursor/skills/marengo-pi-mcp/SKILL.md`](../../.cursor/skills/marengo-pi-mcp/SKILL.md) — log-first bench workflow
- [`.cursor/skills/marengo-pi-sync/SKILL.md`](../../.cursor/skills/marengo-pi-sync/SKILL.md) — sync-with-main deploy

Also [docs/pi-commissioning.md](../../docs/pi-commissioning.md).
