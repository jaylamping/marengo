# Handoff 2026-10-03: right-arm URDF geometry from CAD

Move to the Windows host (`J:\code\marengo`, SolidWorks running, SolidWorks MCP enabled). `git pull` first; `main` is at or after `123a433`.

## Goal

Replace the placeholder right-shoulder geometry in `assets/urdf/marengo.urdf` with values read from CAD, then re-run the bench hold.

## Why

The bench hold on 2026-10-03 failed: with the arm hanging at mechanical zero, the model predicts τ_g(right_shoulder_pitch) ≈ −1.6 Nm instead of ≈ 0. Position hold fed that torque forward, so pitch was pushed 0.036 rad away from its target and the hold-tracking/ascent fuse disabled it. The drive delivered exactly what was commanded; there was no sign or direction fault.

Since commit `2f50de4` (armee-dynamics `urdf_gravity.rs` now applies joint-origin translations), these hand-written values matter. They are placeholders, identical in every `assets/urdf/archive/seed-arm_*` URDF:

| Line (repo URDF) | Element | Current |
|---|---|---|
| ~31 | `right_shoulder_roll` link inertial COM | `0.05 0 0` |
| ~95 | `right_shoulder_roll` joint origin | `0.05 0 0` |
| ~103 | `right_upper_arm_yaw` joint origin | `0.08 0 0` |
| ~112 | `right_elbow_pitch` joint origin | `0 0 -0.24` |

Moving the three +X offsets to −Y zeroes pitch but adds about −0.95 Nm on roll at q=0, so **don't guess; read them from CAD**.

## Steps

1. Through the SolidWorks MCP, open the right-arm assembly and read, in the URDF frame convention (`hardware/docs/kinematics.md`: +X forward, ±Y width, +Z up; pitch axis Y):
   - joint origins and axes for right_shoulder_pitch → roll → upper_arm_yaw → elbow_pitch → lower_arm_yaw
   - mass, COM and inertia for each right-arm link (include actuators).
2. Start from the **Pi's** URDF, not the repo copy (ADR 0017: it carries taught limits): `pi_read_file` `/opt/marengo/assets/urdf/marengo.urdf`, diff against the repo, keep the Pi limits.
3. Edit only the right-arm origins/inertials. Run `./scripts/validate-urdf.sh` and `just check`.
4. Gravity check before any motion: `pi_gravity_preview` (or `motor-repl gravity-preview`) at pitch 0 should give |τ_g| < 0.20 Nm on every joint, and pitch τ_g should be positive and rise as pitch goes positive. Reference: the 2026-08-12 hold measured about +0.57 Nm at q_pitch ≈ 0.48.
5. `pi_sync_bench_urdf`, then confirm the arm is supported at mechanical zero and run `pi_hold_on` with `set_zero: true`, `at_mechanical_reference: true`, profile `arm_attached`. The gravity gate (`gravity_model_mismatch`) must now pass. Then check `pi_candump_summary` and `pi_logs_last_fault`.

## State at handoff

- Pi runs `123a433`; gateway healthy; `marengo-pi.service` stopped but still **enabled**. Run `sudo systemctl disable marengo-pi` once.
- Physical reference (ADR 0036) works: all 5 joints acquire a zero within 0.0002 rad of the readback. The grant lives only for that `marengo-pi` process (`home <joints> sign-tested` on stdin).
- Enable works since `edb8fb3` (strict drive-mode check starts at our Enable echo).
- `123a433`: hold-tracking fuse; a 1-count hold-on latch is classified as home.
- `04567a8`: `pi_hold_on` and the harness refuse arm profiles until the model matches. This is expected until this fix lands.
- Reconnect the `marengo-pi` MCP after pulling (`just mcp-build` first) so the new tools load.
- Never run `motor-repl` or read-only CAN tools while `marengo-pi` is running: the CAN-owner guard covers the MCP, but manual SSH doesn't.

## Not done / open

- `just check` (Docker) not run on the macOS side; run it on Windows.
- The gravity gate only checks the rest pose; elevated-pose model error is validated in step 4 by hand.
- The 2026-10-03 bench candump is `/tmp/cd-20261003T031759Z.log` on the Pi (decoded analysis in the commit history / this doc).
