# ADR 0015: Config-driven wrong-sign watchdog for GravityComp

**Status:** Accepted
**Date:** 2026-06-19

## Context

Gravity compensation with a wrong sign causes runaway — the motor accelerates the
arm in the direction of gravity instead of opposing it. The manual Phase 0b sign
check (see `docs/safety.md`) catches this at bench start, but not during runtime
(e.g. if the URDF joint axis or `motors.yaml` `direction` is edited incorrectly
after the bench was commissioned).

A sustained opposition between `torque_ff` and measured velocity `dq_meas` is a
reliable runtime indicator: in correct GravityComp, `torque_ff` drives the joint
toward its commanded pose, so it should not oppose the measured motion for more
than a few ticks. Sustained opposition over ~50 ms (10 ticks at 200 Hz) with
`|dq| > 0.05` rad/s indicates the sign is wrong and the arm is accelerating
away from the command.

## Decision

Add a **config-driven sign table** to Davout. In `GravityComp` mode only,
`filter_mit_command_at_tick` checks whether `sign(torque_ff)` opposes the
expected sign sustained over `min_opposition_ticks`. On trip, Davout returns
`DavoutError::WrongSignWatchdog { joint, ticks }`, which disables the drive.

Davout does **not** recompute `tau_g` — that would break the crate boundary
(`crates/davout/src/lib.rs:22`: "Does not compute gravity, impedance targets, or
trajectories"). It also does not change the Chappe proto wire types. Instead,
the expected sign is config-driven: `expected_sign_at_positive_q` tells the
watchdog what sign the motor torque should have when `q > 0`. The sign for
`q < 0` is the negation. This captures the URDF joint axis + motor `direction`
convention in one operator-tunable field.

### Config schema (`control.yaml`)

```yaml
wrong_sign_watchdog:
  enabled: true
  expected_sign_at_positive_q: -1   # direction=-1 flips positive tau_g to negative motor torque
  min_velocity_rad_s: 0.05          # below this, sign is undefined — do not trip
  min_opposition_ticks: 10          # 10 ticks = 50 ms at 200 Hz
  grace_period_ticks: 20            # no trip during first 20 ticks after enable
```

### Trip conditions (all must hold)

1. `control_mode == GravityComp` (not Impedance/Position/TorqueOnly where `tau_ff` is composed).
2. `ticks_since_enable > grace_period_ticks` (settling window after enable).
3. `|dq_meas| > min_velocity_rad_s` (do not trip on zero / near-zero velocity).
4. `sign(torque_ff) != expected_sign(q)` for `min_opposition_ticks` consecutive ticks.

### Reset

`wrong_sign_state` is cleared on `request_enable(true)` and `disable_all()`.

## Consequences

- New `DavoutError::WrongSignWatchdog { joint, ticks }` variant.
- New `WrongSignWatchdogConfig` in `marengo-config` (`control.wrong_sign_watchdog`).
- Watchdog applies **only** in `GravityComp` mode. In Impedance/Position modes
  `tau_ff` is composed with `kp`/`kd` terms, so its sign is not a clean gravity
  indicator.
- Does not trip on a single tick — requires sustained opposition.
- Does not trip during the grace period or when `|dq|` is below threshold.
- No `tau_g` recomputation in Davout; no proto wire-type changes.
