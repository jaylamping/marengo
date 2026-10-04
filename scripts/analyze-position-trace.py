#!/usr/bin/env python3
"""Summarize Marengo position-hold CSV traces (MARENGO_POSITION_TRACE).

Usage:
  python scripts/analyze-position-trace.py /path/to/position-trace.csv
  python scripts/analyze-position-trace.py /path/to/position-trace.csv --json

Emits per-target-move segments with jerk/lead/rate-limit indicators so tuning
changes can be compared without eyeballing thousands of rows.

Trace columns (2026-06-13+): `target_raw`, `q_env_lo`, `q_env_hi` document ADR 0009
limit envelope clamping; older CSVs without these columns still analyze.

Layer 2 gate (weighted hold-at 0.1):
  python scripts/analyze-position-trace.py trace.csv --gate layer2 --tau-ff-rate-limit 60

Onset diagnostics (first 250 ms after each retarget):
  python scripts/analyze-position-trace.py trace.csv --onset
  python scripts/analyze-position-trace.py trace.csv --onset --onset-window-ms 300

ADR 0039 Phase 3 bench score (one joint; per move, automatic):
  python scripts/analyze-position-trace.py trace.csv --score-bench \\
      --joint right_shoulder_pitch --bench-log bench-session.txt
  Moves are cut where the commanded target (target_raw) changes; runs of <= 3 rows merge into
  one wave. Kinds: move (hold-at), wave, hold (no commanded change).
  - Fast moves/waves: speed overshoot (q over 50 ms) <= 20 % of the reference peak (dq_ref for
    hold-at moves; q_ref's own slope for waves).
  - Slow moves/waves (plan <= 0.3 rad/s, or a move <= 0.1 rad): max |q - q_ref| <= 0.03 rad
    instead; speed is reported over a window where two counts are <= 5 % of the plan.
  - Every move/wave: <= 0.01 rad past the target (waves: past the band) at the stop.
  - Rest >= 8 s after arrival + 2 s settle: q drift <= 0.005 rad.
  - Moves repeated >= 3 times with the same start and target: final q spread <= 0.01 rad.
  - Every-tick trace only: no single-tick tau_ff_cmd step > 0.05 Nm (retargets included).
  - No "MIT total torque clamped" line for the joint in the bench log.
  A criterion that cannot be scored (decimated trace, no bench log) fails the verdict; record
  the swept joint every tick with MARENGO_POSITION_TRACE_FULL_RATE_JOINTS=<joint>.
  Constants and their justification: BENCH_* below.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import re
import sys
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Iterable


@dataclass
class SegmentReport:
    target_rad: float
    t_start_ms: int
    t_end_ms: int
    q_start: float
    q_end: float
    peak_q: float
    overshoot_rad: float
    final_settle_error_rad: float
    duration_s: float
    lead_sat_fraction: float
    tracking_error_rms_rad: float
    velocity_lag_rms_rad_s: float
    tau_f_sign_flips: int
    tau_ff_max_slew_nm_s: float
    dq_traj_stutter_events: int
    jerk_rms_rad_s2: float
    planner_event_counts: dict[str, int] = field(default_factory=dict)
    phase_counts: dict[str, int] = field(default_factory=dict)
    hints: list[str] = field(default_factory=list)
    gate_checks: dict[str, bool] = field(default_factory=dict)


@dataclass
class OnsetReport:
    target_rad: float
    window_ms: int
    samples: int
    first_motion_ms: int | None
    stuck_ms: int
    friction_mode_counts: dict[str, int] = field(default_factory=dict)
    first_traj_vel_ms: int | None = None
    first_tau_f_nonzero_ms: int | None = None
    max_dq_traj_at_stuck: float = 0.0
    max_tau_f_at_stuck: float = 0.0
    tau_f_mode_transitions: int = 0
    hints: list[str] = field(default_factory=list)


# Layer 2 hold-at 0.1 analyzer gate (non-SoT debug). Limb Position SoT is
# docs/commissioning/limb-playbook.md §5; these constants are local to --gate layer2.
LAYER2_APPROACH_TARGET_RAD = 0.1
LAYER2_HOME_START_MAX_RAD = 0.005
LAYER2_LEAD_SAT_MAX = 0.10
LAYER2_JERK_RMS_MAX_RAD_S2 = 800.0
LAYER2_TAU_F_FLIPS_MAX = 2
LAYER2_TAU_FF_RATE_LIMIT_MULTIPLIER = 2.0
DEFAULT_ONSET_WINDOW_MS = 250
VELOCITY_DEADBAND_RAD_S = 0.02
MOTION_DQ_RAD_S = 0.005

# ADR 0039 Phase 3 bench pass criteria (--score-bench), per move.
BENCH_SPEED_WINDOW_MS = 50
BENCH_OVERSHOOT_FRACTION = 0.20
BENCH_END_OVERSHOOT_RAD = 0.01
BENCH_TAU_STEP_NM = 0.05
# Reference peaks below this are holds, not moves: no speed overshoot is scored.
BENCH_MIN_PLAN_RAD_S = 0.05
BENCH_MIN_MOVE_RAD = 1e-3
# A target_raw held for at most this many rows is a wave sample, not a hold-at move (a wave's
# target_raw changes every tick except for a rounding tie at a turnaround).
BENCH_WAVE_RUN_ROWS = 3
# RS03 position feedback quantum: 4π over 16 bits (0.077 rad/s per count per 5 ms tick).
BENCH_ENCODER_COUNT_RAD = 4.0 * math.pi / 32767.0
# Slow moves are scored on position, not speed ratio:
# - plan ≤ 0.3 rad/s: over the 50 ms speed window, one count of jitter at each end
#   (2 × 0.38 mrad / 50 ms = 0.015 rad/s) exceeds 5 % of the plan, a quarter of the 20 % budget;
# - move ≤ 0.1 rad: the joint starts anywhere in its static dead zone fs/kp (0.37/18 ≈ 0.02 rad
#   for the pitch), and catching up 0.02 rad on a 0.1 rad move is itself 20 % of the move.
BENCH_SLOW_PLAN_RAD_S = 0.3
BENCH_SHORT_MOVE_RAD = 0.1
# Slow-move tracking: max |q − q_ref| ≤ the dead zone (≈ 0.02 rad) plus the 0.01 rad stop budget.
BENCH_SLOW_TRACK_MAX_RAD = 0.03
# Slow-move speed (reported, not scored) uses a window where 2 counts are ≤ 5 % of the plan.
BENCH_SLOW_SPEED_QUANT_SHARE = 0.05
BENCH_SLOW_SPEED_WINDOW_MAX_MS = 1000
# Drift: a hold whose rest (after the reference arrives + settle) lasts ≥ this is scored.
BENCH_SETTLE_S = 2.0
BENCH_DRIFT_MIN_REST_S = 8.0
# q may wander at most this over the rest: a quarter of the 0.02 rad integral band.
BENCH_HOLD_DRIFT_MAX_RAD = 0.005
# Repeatability: moves with the same (start, target) seen at least this often; their final q
# must agree within the stop-overshoot budget.
BENCH_REPEAT_MIN_COUNT = 3
BENCH_REPEAT_SPREAD_MAX_RAD = BENCH_END_OVERSHOOT_RAD
TOTAL_TORQUE_CLAMP_MARKER = "MIT total torque clamped"


def _has(row: dict[str, str], key: str) -> bool:
    return key in row and row[key] != ""


def _b(row: dict[str, str], key: str) -> bool:
    return _i(row, key) != 0


def _onset_rows(seg: list[dict[str, str]], window_ms: int) -> list[dict[str, str]]:
    if not seg:
        return []
    if _has(seg[0], "retarget_age_ms"):
        onset = [r for r in seg if _i(r, "retarget_age_ms") <= window_ms]
        if onset:
            return onset
    t0 = _i(seg[0], "t_ms")
    return [r for r in seg if _i(r, "t_ms") - t0 <= window_ms]


def _analyze_onset(seg: list[dict[str, str]], window_ms: int = DEFAULT_ONSET_WINDOW_MS) -> OnsetReport:
    target = _f(seg[0], "target")
    rows = _onset_rows(seg, window_ms)
    if not rows:
        return OnsetReport(
            target_rad=target,
            window_ms=window_ms,
            samples=0,
            first_motion_ms=None,
            stuck_ms=0,
        )

    t0 = _i(rows[0], "t_ms")
    age_ms = lambda r: _i(r, "retarget_age_ms") if _has(r, "retarget_age_ms") else _i(r, "t_ms") - t0

    first_motion_ms: int | None = None
    stuck_ms = 0
    mode_counts: dict[str, int] = {}
    first_traj_vel_ms: int | None = None
    first_tau_f_nonzero_ms: int | None = None
    max_dq_traj_at_stuck = 0.0
    max_tau_f_at_stuck = 0.0
    mode_transitions = 0
    prev_mode: str | None = None

    for r in rows:
        ms = age_ms(r)
        dq = _f(r, "dq")
        dq_traj = _f(r, "dq_traj")
        tau_f = _f(r, "tau_f")
        stuck = _b(r, "joint_stuck") if _has(r, "joint_stuck") else (
            abs(dq) <= VELOCITY_DEADBAND_RAD_S and abs(dq_traj) > 1e-4
        )
        if abs(dq) > MOTION_DQ_RAD_S and first_motion_ms is None:
            first_motion_ms = ms
        if stuck:
            stuck_ms += 1
            max_dq_traj_at_stuck = max(max_dq_traj_at_stuck, abs(dq_traj))
            max_tau_f_at_stuck = max(max_tau_f_at_stuck, abs(tau_f))
        mode = r.get("friction_mode", "unknown")
        mode_counts[mode] = mode_counts.get(mode, 0) + 1
        if mode == "traj_vel" and first_traj_vel_ms is None:
            first_traj_vel_ms = ms
        if abs(tau_f) > 1e-4 and first_tau_f_nonzero_ms is None:
            first_tau_f_nonzero_ms = ms
        if prev_mode is not None and mode != prev_mode:
            mode_transitions += 1
        prev_mode = mode

    hints: list[str] = []
    if stuck_ms > len(rows) // 3:
        hints.append(
            f"joint stuck {stuck_ms}/{len(rows)} onset samples — check friction_mode vs dq_traj ramp"
        )
    if first_motion_ms is not None and first_tau_f_nonzero_ms is not None:
        if first_tau_f_nonzero_ms > first_motion_ms + 30:
            hints.append(
                f"tau_f late ({first_tau_f_nonzero_ms}ms vs motion {first_motion_ms}ms): breakaway assist lag"
            )
        elif first_tau_f_nonzero_ms + 30 < first_motion_ms:
            hints.append(
                f"tau_f early ({first_tau_f_nonzero_ms}ms vs motion {first_motion_ms}ms): friction snap before breakaway"
            )
    if first_traj_vel_ms is not None and max_dq_traj_at_stuck > VELOCITY_DEADBAND_RAD_S:
        hints.append(
            f"traj_vel at {first_traj_vel_ms}ms while stuck with dq_traj={max_dq_traj_at_stuck:.3f}: moving_reference path"
        )
    if mode_transitions >= 3:
        hints.append(f"friction_mode churn x{mode_transitions} in {window_ms}ms: path switching at start")
    if first_motion_ms is not None and first_motion_ms > 80:
        hints.append(f"slow breakaway ({first_motion_ms}ms to |dq|>{MOTION_DQ_RAD_S}): stiction or low assist")

    return OnsetReport(
        target_rad=target,
        window_ms=window_ms,
        samples=len(rows),
        first_motion_ms=first_motion_ms,
        stuck_ms=stuck_ms,
        friction_mode_counts=mode_counts,
        first_traj_vel_ms=first_traj_vel_ms,
        first_tau_f_nonzero_ms=first_tau_f_nonzero_ms,
        max_dq_traj_at_stuck=max_dq_traj_at_stuck,
        max_tau_f_at_stuck=max_tau_f_at_stuck,
        tau_f_mode_transitions=mode_transitions,
        hints=hints,
    )


def _f(row: dict[str, str], key: str) -> float:
    return float(row[key])


def _i(row: dict[str, str], key: str) -> int:
    return int(float(row[key]))


def _rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="") as f:
        return list(csv.DictReader(f))


def _split_segments(rows: list[dict[str, str]]) -> list[list[dict[str, str]]]:
    if not rows:
        return []
    segments: list[list[dict[str, str]]] = []
    current: list[dict[str, str]] = [rows[0]]
    prev_target = _f(rows[0], "target")
    for row in rows[1:]:
        target = _f(row, "target")
        if abs(target - prev_target) > 1e-4:
            segments.append(current)
            current = [row]
            prev_target = target
        else:
            current.append(row)
    if current:
        segments.append(current)
    return segments


def _analyze_segment(seg: list[dict[str, str]]) -> SegmentReport:
    target = _f(seg[0], "target")
    t0 = _i(seg[0], "t_ms")
    t1 = _i(seg[-1], "t_ms")
    dt_s = max((t1 - t0) / 1000.0, 1e-6)

    qs = [_f(r, "q") for r in seg]
    dqs = [_f(r, "dq") for r in seg]
    dq_trajs = [_f(r, "dq_traj") for r in seg]
    settle = [
        _f(r, "settle_error") if _has(r, "settle_error") else _f(r, "target") - _f(r, "q")
        for r in seg
    ]
    tracking = [
        _f(r, "tracking_error") if _has(r, "tracking_error") else _f(r, "q_traj") - _f(r, "q")
        for r in seg
    ]
    lead_sat = [_i(r, "lead_sat") for r in seg]
    tau_f = [_f(r, "tau_f") for r in seg]
    tau_ff = [_f(r, "tau_ff_cmd") for r in seg]
    phases = [r["phase"] for r in seg]

    q_start = qs[0]
    q_end = qs[-1]
    peak_q = max(qs) if target >= q_start else min(qs)
    if target >= q_start:
        overshoot = max(0.0, peak_q - target)
    else:
        overshoot = max(0.0, target - peak_q)

    track_rms = math.sqrt(sum(e * e for e in tracking) / len(tracking))
    vel_lag = [dqs[i] - dq_trajs[i] for i in range(len(seg))]
    vel_lag_rms = math.sqrt(sum(v * v for v in vel_lag) / len(vel_lag))

    flips = 0
    for i in range(1, len(tau_f)):
        if tau_f[i] == 0.0 or tau_f[i - 1] == 0.0:
            continue
        if math.copysign(1.0, tau_f[i]) != math.copysign(1.0, tau_f[i - 1]):
            flips += 1

    max_tau_slew = 0.0
    for i in range(1, len(seg)):
        dt = max((_i(seg[i], "t_ms") - _i(seg[i - 1], "t_ms")) / 1000.0, 1e-6)
        slew = abs(tau_ff[i] - tau_ff[i - 1]) / dt
        max_tau_slew = max(max_tau_slew, slew)

    stutter = 0
    for i in range(1, len(dq_trajs)):
        if abs(dq_trajs[i - 1]) > 0.15 and abs(dq_trajs[i]) < 0.05:
            if abs(dq_trajs[i - 1] - dq_trajs[i]) > 0.1:
                stutter += 1

    jerks: list[float] = []
    for i in range(+2, len(seg)):
        dt1 = max((_i(seg[i - 1], "t_ms") - _i(seg[i - 2], "t_ms")) / 1000.0, 1e-6)
        dt2 = max((_i(seg[i], "t_ms") - _i(seg[i - 1], "t_ms")) / 1000.0, 1e-6)
        a1 = (dqs[i - 1] - dqs[i - 2]) / dt1
        a2 = (dqs[i] - dqs[i - 1]) / dt2
        jerks.append((a2 - a1) / dt2)
    jerk_rms = math.sqrt(sum(j * j for j in jerks) / len(jerks)) if jerks else 0.0

    phase_counts: dict[str, int] = {}
    for p in phases:
        phase_counts[p] = phase_counts.get(p, 0) + 1

    planner_event_counts: dict[str, int] = {}
    for r in seg:
        if not _has(r, "planner_event"):
            continue
        ev = r["planner_event"].strip() or "tick"
        planner_event_counts[ev] = planner_event_counts.get(ev, 0) + 1
    churn = sum(
        n for k, n in planner_event_counts.items() if k not in ("tick", "")
    )

    hints: list[str] = []
    if churn >= 8:
        hints.append(
            f"planner_event churn x{churn}: {planner_event_counts} — check reset/latch near target"
        )
    lead_frac = sum(lead_sat) / len(lead_sat)
    if lead_frac > 0.35:
        hints.append(
            f"lead_sat {lead_frac:.0%}: arm outruns planner — lower kp or raise max_lead / traj v"
        )
    if stutter >= 2:
        hints.append(
            f"dq_traj stutter x{stutter}: planner decel/lead fight — check traj v/a vs gravity assist"
        )
    if max_tau_slew > 55.0:
        hints.append(
            f"tau_ff slew peak {max_tau_slew:.0f} Nm/s: likely Davout rate limit clipping (default 20–60)"
        )
    if flips > len(seg) // 40:
        hints.append(f"tau_f sign flips x{flips}: friction fighting motion — lower fc")
    if overshoot > 0.05:
        hints.append(f"overshoot {overshoot:.3f} rad: reduce kp or traj v before speeding up")
    if abs(settle[-1]) > 0.03 and dt_s > 2.0:
        hints.append(
            f"settle error {settle[-1]:+.3f} rad at segment end: stiffness/gravity mismatch"
        )
    if vel_lag_rms > 0.25:
        hints.append(
            f"velocity lag RMS {vel_lag_rms:.2f} rad/s: measured dq vs dq_traj diverge (gravity or lead_sat)"
        )
    if jerk_rms > LAYER2_JERK_RMS_MAX_RAD_S2:
        hints.append(
            f"jerk_rms {jerk_rms:.0f} rad/s²: exceeds Layer 2 smoothness gate ({LAYER2_JERK_RMS_MAX_RAD_S2:.0f}) — likely torque clipping or friction bang-bang"
        )

    return SegmentReport(
        target_rad=target,
        t_start_ms=t0,
        t_end_ms=t1,
        q_start=q_start,
        q_end=q_end,
        peak_q=peak_q,
        overshoot_rad=overshoot,
        final_settle_error_rad=settle[-1],
        duration_s=dt_s,
        lead_sat_fraction=lead_frac,
        tracking_error_rms_rad=track_rms,
        velocity_lag_rms_rad_s=vel_lag_rms,
        tau_f_sign_flips=flips,
        tau_ff_max_slew_nm_s=max_tau_slew,
        dq_traj_stutter_events=stutter,
        jerk_rms_rad_s2=jerk_rms,
        planner_event_counts=planner_event_counts,
        phase_counts=phase_counts,
        hints=hints,
        gate_checks={},
    )


def _segment_matches_target(seg: SegmentReport, target_rad: float) -> bool:
    return abs(seg.target_rad - target_rad) < 1e-3


def _evaluate_layer2_segment(
    seg: SegmentReport,
    *,
    is_approach: bool,
    tau_ff_rate_limit_nm_s: float,
) -> dict[str, bool]:
    tau_ff_cap = tau_ff_rate_limit_nm_s * LAYER2_TAU_FF_RATE_LIMIT_MULTIPLIER
    checks = {
        "lead_sat_ok": seg.lead_sat_fraction < LAYER2_LEAD_SAT_MAX,
        "jerk_ok": seg.jerk_rms_rad_s2 < LAYER2_JERK_RMS_MAX_RAD_S2,
        "tau_ff_slew_ok": seg.tau_ff_max_slew_nm_s < tau_ff_cap,
    }
    if is_approach:
        checks["tau_f_flips_ok"] = seg.tau_f_sign_flips <= LAYER2_TAU_F_FLIPS_MAX
    return checks


def evaluate_layer2_gate(
    segments: list[SegmentReport],
    tau_ff_rate_limit_nm_s: float,
    *,
    require_home_start: bool = False,
) -> dict:
    approach = next(
        (s for s in segments if _segment_matches_target(s, LAYER2_APPROACH_TARGET_RAD)),
        None,
    )
    ret = next((s for s in segments if _segment_matches_target(s, 0.0)), None)
    missing: list[str] = []
    if approach is None:
        missing.append(f"approach segment (target={LAYER2_APPROACH_TARGET_RAD})")
    if ret is None:
        missing.append("return segment (target=0)")

    approach_checks: dict[str, bool] = {}
    return_checks: dict[str, bool] = {}
    if approach is not None:
        approach_checks = _evaluate_layer2_segment(
            approach,
            is_approach=True,
            tau_ff_rate_limit_nm_s=tau_ff_rate_limit_nm_s,
        )
        if require_home_start:
            approach_checks["home_start_ok"] = abs(approach.q_start) < LAYER2_HOME_START_MAX_RAD
        approach.gate_checks = approach_checks
    if ret is not None:
        return_checks = _evaluate_layer2_segment(
            ret,
            is_approach=False,
            tau_ff_rate_limit_nm_s=tau_ff_rate_limit_nm_s,
        )
        ret.gate_checks = return_checks

    analyzer_ok = (
        not missing
        and all(approach_checks.values())
        and all(return_checks.values())
    )
    return {
        "gate": "layer2",
        "tau_ff_rate_limit_nm_s": tau_ff_rate_limit_nm_s,
        "require_home_start": require_home_start,
        "home_start_max_rad": LAYER2_HOME_START_MAX_RAD,
        "analyzer_pass": analyzer_ok,
        "missing_segments": missing,
        "approach_checks": approach_checks,
        "return_checks": return_checks,
        "note": "Operator smoothness still required — analyzer cannot detect felt jerk alone.",
    }


def _windowed_speeds(
    seg: list[dict[str, str]], window_ms: int, key: str = "q"
) -> list[float | None]:
    """Signed speed of column `key` at each row over the latest earlier row >= window_ms back."""
    out: list[float | None] = []
    j = 0
    for i, row in enumerate(seg):
        t = _i(row, "t_ms")
        while j + 1 < i and t - _i(seg[j + 1], "t_ms") >= window_ms:
            j += 1
        dt_ms = t - _i(seg[j], "t_ms")
        if j >= i or dt_ms < window_ms:
            out.append(None)
        else:
            out.append((_f(row, key) - _f(seg[j], key)) / (dt_ms / 1000.0))
    return out


def _target_key(row: dict[str, str]) -> str:
    """The commanded target: `target_raw` (before the envelope clamp) when traced."""
    return "target_raw" if _has(row, "target_raw") else "target"


def _ref_keys(row: dict[str, str]) -> tuple[str, str]:
    """Reference position / velocity columns: ADR 0039 `q_ref,dq_ref`, else the planner's."""
    return ("q_ref", "dq_ref") if _has(row, "q_ref") else ("q_traj", "dq_traj")


def _split_moves(rows: list[dict[str, str]]) -> list[list[dict[str, str]]]:
    """One joint's rows cut wherever the commanded target changes (exact text: a hold-at holds it
    bit-identical). A wave moves its target every tick, so consecutive runs of at most
    BENCH_WAVE_RUN_ROWS rows merge into one wave."""
    runs: list[list[dict[str, str]]] = []
    for row in rows:
        key = _target_key(row)
        if runs and runs[-1][-1][key] == row[key]:
            runs[-1].append(row)
        else:
            runs.append([row])
    moves: list[tuple[bool, list[dict[str, str]]]] = []
    for run in runs:
        short = len(run) <= BENCH_WAVE_RUN_ROWS
        if short and moves and moves[-1][0]:
            moves[-1][1].extend(run)
        else:
            moves.append((short, list(run)))
    return [m for _, m in moves]


def _trace_period_ticks(rows: list[dict[str, str]]) -> int | None:
    """Smallest tick step between one joint's consecutive rows: 1 = every tick."""
    steps = [_i(b, "tick") - _i(a, "tick") for a, b in zip(rows, rows[1:])]
    positive = [s for s in steps if s > 0]
    return min(positive) if positive else None


def _slow_speed_window_ms(v_plan: float) -> int:
    """Window over which two encoder counts are at most 5 % of `v_plan` (50 ms .. 1 s)."""
    if v_plan <= 0.0:
        return BENCH_SLOW_SPEED_WINDOW_MAX_MS
    ms = math.ceil(2.0 * BENCH_ENCODER_COUNT_RAD / (BENCH_SLOW_SPEED_QUANT_SHARE * v_plan) * 1000.0)
    return max(BENCH_SPEED_WINDOW_MS, min(BENCH_SLOW_SPEED_WINDOW_MAX_MS, ms))


def _peak_speed_ratio(
    seg: list[dict[str, str]], window_ms: int, v_plan: float, direction: float
) -> tuple[float | None, float | None]:
    """(measured peak speed, overshoot vs `v_plan`) along `direction` (0 = either way)."""
    speeds = [s for s in _windowed_speeds(seg, window_ms) if s is not None]
    along = [abs(s) for s in speeds] if direction == 0 else [s * direction for s in speeds]
    if not along or v_plan <= 0.0:
        return None, None
    peak = max(along)
    return peak, (peak - v_plan) / v_plan


def _rest_drift(
    seg: list[dict[str, str]], target: float, q_key: str, dq_key: str
) -> tuple[float | None, float]:
    """(drift, rest seconds): q range after the reference has arrived and BENCH_SETTLE_S passed;
    drift is None when that rest is shorter than BENCH_DRIFT_MIN_REST_S."""
    arrived = next(
        (
            _i(r, "t_ms")
            for r in seg
            if abs(_f(r, q_key) - target) < 1e-6 and abs(_f(r, dq_key)) < 1e-9
        ),
        None,
    )
    if arrived is None:
        return None, 0.0
    rest = [r for r in seg if _i(r, "t_ms") >= arrived + BENCH_SETTLE_S * 1000.0]
    if len(rest) < 2:
        return None, 0.0
    rest_s = (_i(rest[-1], "t_ms") - _i(rest[0], "t_ms")) / 1000.0
    if rest_s < BENCH_DRIFT_MIN_REST_S:
        return None, rest_s
    qs = [_f(r, "q") for r in rest]
    return max(qs) - min(qs), rest_s


def _score_move(
    seg: list[dict[str, str]],
    prev: dict[str, str] | None,
    window_ms: int,
    per_tick: bool,
) -> dict:
    tkey = _target_key(seg[0])
    q_key, dq_key = _ref_keys(seg[0])
    commanded = [_f(r, tkey) for r in seg]
    wave = max(commanded) - min(commanded) > 1e-4
    target = _f(seg[-1], "target")
    q_start = _f(seg[0], "q")
    qs = [_f(r, "q") for r in seg]
    # The move starts from the previous commanded target (the joint may rest anywhere in its
    # dead zone around it), or from q for the first move.
    start_ref = _f(prev, "target") if prev is not None else q_start
    distance = target - start_ref
    dq_ref_peak = max(abs(_f(r, dq_key)) for r in seg)
    tracking = max(abs(_f(r, "q") - _f(r, q_key)) for r in seg)

    info: dict = {}
    if wave:
        kind = "wave"
        # The reference speed from q_ref itself, over the same window as q, so a clamped
        # dq_ref column (pre-fix wave feed-forward) cannot fake an overshoot.
        ref_speeds = [abs(s) for s in _windowed_speeds(seg, window_ms, q_key) if s is not None]
        v_plan = max(ref_speeds) if ref_speeds else dq_ref_peak
        direction = 0.0
        lo, hi = min(commanded), max(commanded)
        end_overshoot = max(0.0, max(qs) - hi, lo - min(qs))
        info.update(band_rad=[lo, hi], swept_rad=[min(qs), max(qs)], dq_ref_peak_rad_s=dq_ref_peak)
        if dq_ref_peak < 0.8 * v_plan:
            info["hint"] = (
                f"dq_ref peaked at {dq_ref_peak:.2f} rad/s while q_ref moved at {v_plan:.2f} rad/s: "
                "wave velocity feed-forward clamped (pre-1d521ae5 Berthier)"
            )
    elif abs(distance) < BENCH_MIN_MOVE_RAD:
        kind = "hold"
        v_plan = dq_ref_peak
        direction = 0.0
        end_overshoot = None
    else:
        kind = "move"
        v_plan = dq_ref_peak
        direction = math.copysign(1.0, distance)
        end_overshoot = max(0.0, max((q - target) * direction for q in qs))
    slow = kind != "hold" and (
        v_plan <= BENCH_SLOW_PLAN_RAD_S
        or (kind == "move" and abs(distance) <= BENCH_SHORT_MOVE_RAD + 1e-9)
    )

    speed_overshoot = v_peak = None
    speed_window = window_ms
    if kind != "hold" and v_plan >= BENCH_MIN_PLAN_RAD_S:
        if slow:
            speed_window = _slow_speed_window_ms(v_plan)
        v_peak, speed_overshoot = _peak_speed_ratio(seg, speed_window, v_plan, direction)

    drift = None
    rest_s = 0.0
    if kind != "wave":
        drift, rest_s = _rest_drift(seg, target, q_key, dq_key)

    tau_step = None
    if per_tick:
        pairs = list(zip(seg, seg[1:]))
        if prev is not None:
            pairs.insert(0, (prev, seg[0]))
        steps = [
            abs(_f(b, "tau_ff_cmd") - _f(a, "tau_ff_cmd"))
            for a, b in pairs
            if _i(b, "tick") - _i(a, "tick") == 1
        ]
        tau_step = max(steps) if steps else None
    wire_binding = sum(
        1
        for r in seg
        if _has(r, "tau_ff_wire") and abs(_f(r, "tau_ff_wire") - _f(r, "tau_ff_cmd")) > 1e-6
    )

    checks: dict[str, bool] = {}
    if kind != "hold":
        if slow:
            checks["tracking_ok"] = tracking <= BENCH_SLOW_TRACK_MAX_RAD
        elif speed_overshoot is not None:
            checks["speed_overshoot_ok"] = speed_overshoot <= BENCH_OVERSHOOT_FRACTION
        checks["end_overshoot_ok"] = end_overshoot <= BENCH_END_OVERSHOOT_RAD
    if drift is not None:
        checks["drift_ok"] = drift <= BENCH_HOLD_DRIFT_MAX_RAD
    if tau_step is not None:
        checks["tau_step_ok"] = tau_step <= BENCH_TAU_STEP_NM
    return {
        "kind": kind,
        "slow": slow,
        "target_rad": target,
        "wave": wave,
        "start_ref_rad": start_ref,
        "q_start": q_start,
        "q_end": qs[-1],
        "t_start_ms": _i(seg[0], "t_ms"),
        "duration_s": (_i(seg[-1], "t_ms") - _i(seg[0], "t_ms")) / 1000.0,
        "plan_peak_rad_s": v_plan,
        "measured_peak_rad_s": v_peak,
        "speed_window_ms": speed_window,
        "speed_overshoot": speed_overshoot,
        "max_tracking_error_rad": tracking,
        "end_overshoot_rad": end_overshoot,
        "final_error_rad": qs[-1] - target,
        "rest_s": rest_s,
        "drift_rad": drift,
        "max_tau_ff_step_nm": tau_step,
        "tau_ff_wire_binding_rows": wire_binding,
        **info,
        "checks": checks,
        "pass": all(checks.values()),
    }


def _repeatability(moves: list[dict]) -> list[dict]:
    """Moves repeated with the same (start, target) at least BENCH_REPEAT_MIN_COUNT times: the
    spread of their final q and of their stop overshoot."""
    groups: dict[tuple[float, float], list[int]] = {}
    for i, m in enumerate(moves):
        if m["kind"] == "move":
            key = (round(m["start_ref_rad"], 3), round(m["target_rad"], 3))
            groups.setdefault(key, []).append(i)
    out = []
    for (start, target), idx in groups.items():
        if len(idx) < BENCH_REPEAT_MIN_COUNT:
            continue
        finals = [moves[i]["q_end"] for i in idx]
        stops = [moves[i]["end_overshoot_rad"] for i in idx]
        spread = max(finals) - min(finals)
        out.append(
            {
                "start_rad": start,
                "target_rad": target,
                "moves": [i + 1 for i in idx],
                "final_q_spread_rad": spread,
                "end_overshoot_spread_rad": max(stops) - min(stops),
                "pass": spread <= BENCH_REPEAT_SPREAD_MAX_RAD,
            }
        )
    return out


def _count_total_torque_clamps(log: Path, joint: str) -> int:
    """Clamps of `joint` in a bench log: the largest cumulative `joint_clamps=N` on a clamp
    warning (Davout rate-limits the warning to 1/s), else the number of warning lines."""
    lines = 0
    cumulative = 0
    with log.open(errors="replace") as f:
        for line in f:
            if TOTAL_TORQUE_CLAMP_MARKER not in line or joint not in line:
                continue
            lines += 1
            m = re.search(r"joint_clamps=(\d+)", line)
            if m:
                cumulative = max(cumulative, int(m.group(1)))
    return max(cumulative, lines)


def score_bench(
    rows: list[dict[str, str]],
    *,
    joint: str,
    window_ms: int = BENCH_SPEED_WINDOW_MS,
    bench_log: Path | None = None,
) -> dict:
    joint_rows = [r for r in rows if r.get("joint") == joint]
    period = _trace_period_ticks(joint_rows)
    per_tick = period == 1
    moves = []
    prev: dict[str, str] | None = None
    for seg in _split_moves(joint_rows):
        moves.append(_score_move(seg, prev, window_ms, per_tick))
        prev = seg[-1]
    repeats = _repeatability(moves)
    clamps = _count_total_torque_clamps(bench_log, joint) if bench_log is not None else None
    unscored = []
    if not moves:
        unscored.append(f"no rows for {joint}")
    elif not per_tick:
        unscored.append(
            f"tau_ff step: trace decimated (every {period} ticks for {joint}); record with "
            f"MARENGO_POSITION_TRACE_FULL_RATE_JOINTS={joint}"
        )
    if clamps is None:
        unscored.append("total-torque clamps: no --bench-log")
    by_kind: dict[str, dict[str, int]] = {}
    for m in moves:
        label = f"slow {m['kind']}" if m["slow"] else m["kind"]
        tally = by_kind.setdefault(label, {"moves": 0, "passed": 0})
        tally["moves"] += 1
        tally["passed"] += int(m["pass"])
    passed = sum(1 for m in moves if m["pass"])
    return {
        "joint": joint,
        "trace_period_ticks": period,
        "decimated": not per_tick,
        "criteria": {
            "speed_window_ms": window_ms,
            "speed_overshoot_max": BENCH_OVERSHOOT_FRACTION,
            "end_overshoot_max_rad": BENCH_END_OVERSHOOT_RAD,
            "tau_ff_step_max_nm": BENCH_TAU_STEP_NM,
            "slow_plan_max_rad_s": BENCH_SLOW_PLAN_RAD_S,
            "short_move_max_rad": BENCH_SHORT_MOVE_RAD,
            "slow_tracking_max_rad": BENCH_SLOW_TRACK_MAX_RAD,
            "hold_drift_max_rad": BENCH_HOLD_DRIFT_MAX_RAD,
            "repeat_spread_max_rad": BENCH_REPEAT_SPREAD_MAX_RAD,
            "total_torque_clamps_max": 0,
        },
        "moves": moves,
        "repeatability": repeats,
        "summary": {"moves": len(moves), "passed": passed, "by_kind": by_kind},
        "total_torque_clamps": clamps,
        "unscored": unscored,
        "pass": bool(moves)
        and not unscored
        and clamps == 0
        and passed == len(moves)
        and all(r["pass"] for r in repeats),
    }


def analyze(
    path: Path,
    *,
    gate: str | None = None,
    tau_ff_rate_limit_nm_s: float = 60.0,
    onset: bool = False,
    onset_window_ms: int = DEFAULT_ONSET_WINDOW_MS,
    require_home_start: bool = False,
    bench_joint: str | None = None,
    bench_log: Path | None = None,
) -> dict:
    rows = _rows(path)
    segments = _split_segments(rows)
    segment_reports = [_analyze_segment(s) for s in segments]
    result: dict = {
        "path": str(path),
        "samples": len(rows),
        "segments": [asdict(s) for s in segment_reports],
    }
    if bench_joint is not None:
        result["bench_score"] = score_bench(rows, joint=bench_joint, bench_log=bench_log)
    if onset:
        result["onset"] = [asdict(_analyze_onset(s, onset_window_ms)) for s in segments]
        result["onset_window_ms"] = onset_window_ms
    if gate == "layer2":
        result["layer2_gate"] = evaluate_layer2_gate(
            segment_reports,
            tau_ff_rate_limit_nm_s,
            require_home_start=require_home_start,
        )
        result["segments"] = [asdict(s) for s in segment_reports]
    return result


def _print_human(report: dict, segment_detail: bool = True) -> None:
    print(f"trace: {report['path']} ({report['samples']} samples)")
    if not segment_detail:
        # Every-tick waves retarget each tick, so the per-target segment dump grows to one block
        # per row (51,369 segments, 16 MiB on a 2026-10-04 pitch sweep) and buries the verdict.
        print(
            f"{len(report['segments'])} per-target segments (detail omitted under --score-bench; "
            "rerun without it for the segment report)"
        )
    for i, seg in enumerate(report["segments"] if segment_detail else [], 1):
        print()
        print(f"--- segment {i}: target={seg['target_rad']:.4f} rad ({seg['duration_s']:.1f}s) ---")
        print(
            f"  q {seg['q_start']:.3f} -> {seg['q_end']:.3f}  peak={seg['peak_q']:.3f}  "
            f"overshoot={seg['overshoot_rad']:.3f}  settle_err={seg['final_settle_error_rad']:+.3f}"
        )
        print(
            f"  lead_sat={seg['lead_sat_fraction']:.0%}  track_rms={seg['tracking_error_rms_rad']:.3f}  "
            f"vel_lag_rms={seg['velocity_lag_rms_rad_s']:.2f}  jerk_rms={seg['jerk_rms_rad_s2']:.1f}"
        )
        print(
            f"  tau_f flips={seg['tau_f_sign_flips']}  tau_ff peak slew={seg['tau_ff_max_slew_nm_s']:.0f} Nm/s  "
            f"dq_traj stutter={seg['dq_traj_stutter_events']}"
        )
        print(f"  phases: {seg['phase_counts']}")
        if seg.get("planner_event_counts"):
            print(f"  planner_events: {seg['planner_event_counts']}")
        if seg.get("gate_checks"):
            failed = [k for k, ok in seg["gate_checks"].items() if not ok]
            status = "PASS" if not failed else f"FAIL ({', '.join(failed)})"
            print(f"  layer2_gate: {status}")
        for hint in seg["hints"]:
            print(f"  ! {hint}")

    gate = report.get("layer2_gate")
    if gate is not None:
        print()
        print("=== Layer 2 gate (analyzer) ===")
        print(f"  tau_ff_rate_limit: {gate['tau_ff_rate_limit_nm_s']} Nm/s")
        if gate["missing_segments"]:
            print(f"  missing: {', '.join(gate['missing_segments'])}")
        if gate["approach_checks"]:
            print(f"  approach: {gate['approach_checks']}")
        if gate["return_checks"]:
            print(f"  return: {gate['return_checks']}")
        print(f"  analyzer: {'PASS' if gate['analyzer_pass'] else 'FAIL'}")
        print(f"  ({gate['note']})")

    onset_reports = report.get("onset")
    if onset_reports:
        window = report.get("onset_window_ms", DEFAULT_ONSET_WINDOW_MS)
        print()
        print(f"=== Onset window ({window} ms after retarget / segment start) ===")
        for i, onset in enumerate(onset_reports, 1):
            print()
            print(
                f"--- onset {i}: target={onset['target_rad']:.4f} rad ({onset['samples']} samples) ---"
            )
            print(
                f"  stuck={onset['stuck_ms']}  first_motion={onset['first_motion_ms']}  "
                f"first_tau_f={onset['first_tau_f_nonzero_ms']}  first_traj_vel={onset['first_traj_vel_ms']}"
            )
            print(
                f"  stuck peak dq_traj={onset['max_dq_traj_at_stuck']:.3f}  "
                f"tau_f={onset['max_tau_f_at_stuck']:.3f}  mode_transitions={onset['tau_f_mode_transitions']}"
            )
            print(f"  friction_mode: {onset['friction_mode_counts']}")
            for hint in onset.get("hints", []):
                print(f"  ! {hint}")

    score = report.get("bench_score")
    if score is not None:
        c = score["criteria"]
        print()
        print(
            f"=== ADR 0039 bench score: {score['joint']} (fast moves: speed over "
            f"{c['speed_window_ms']} ms <= +{c['speed_overshoot_max']:.0%}; slow moves (plan <= "
            f"{c['slow_plan_max_rad_s']} rad/s or <= {c['short_move_max_rad']} rad): "
            f"|q - q_ref| <= {c['slow_tracking_max_rad']} rad; stop <= "
            f"{c['end_overshoot_max_rad']} rad; tau_ff step <= {c['tau_ff_step_max_nm']} Nm; "
            f"hold drift <= {c['hold_drift_max_rad']} rad; repeat spread <= "
            f"{c['repeat_spread_max_rad']} rad; total-torque clamps 0) ==="
        )
        period = score["trace_period_ticks"]
        print(
            f"  trace: {'every tick' if period == 1 else f'decimated (every {period} ticks)'}"
        )
        fmt = lambda v, f: "-" if v is None else format(v, f)  # noqa: E731
        for i, m in enumerate(score["moves"], 1):
            failed = [k for k, ok in m["checks"].items() if not ok]
            verdict = "PASS" if not failed else "FAIL " + ",".join(failed)
            label = f"slow {m['kind']}" if m["slow"] else m["kind"]
            if m["kind"] == "wave":
                lo, hi = m["band_rad"]
                q_lo, q_hi = m["swept_rad"]
                what = (
                    f"band [{lo:+.3f}, {hi:+.3f}] swept [{q_lo:+.3f}, {q_hi:+.3f}] rad  "
                    f"ref {m['plan_peak_rad_s']:.2f} rad/s"
                )
            elif m["kind"] == "hold":
                what = f"at {m['target_rad']:+.3f} rad  final err {m['final_error_rad']:+.4f} rad"
            else:
                what = (
                    f"{m['start_ref_rad']:+.3f} -> {m['target_rad']:+.3f} rad (q0 "
                    f"{m['q_start']:+.3f})  plan {m['plan_peak_rad_s']:.2f} rad/s"
                )
            parts = [f"  move {i} [{label}]: {what}"]
            if m["kind"] != "hold":
                speed = f"speed {fmt(m['speed_overshoot'], '+.0%')}"
                if m["slow"]:
                    speed += f" over {m['speed_window_ms']} ms (not scored)"
                parts += [
                    speed,
                    f"track {m['max_tracking_error_rad']:.4f} rad",
                    f"stop {fmt(m['end_overshoot_rad'], '.4f')} rad",
                ]
            if m["drift_rad"] is not None:
                parts.append(f"drift {m['drift_rad']:.4f} rad over {m['rest_s']:.1f} s")
            parts += [
                f"tau_ff step {fmt(m['max_tau_ff_step_nm'], '.3f')} Nm",
                f"wire-binding rows {m['tau_ff_wire_binding_rows']}",
                verdict,
            ]
            print("  ".join(parts))
            if "hint" in m:
                print(f"    ! {m['hint']}")
        for r in score["repeatability"]:
            print(
                f"  repeat {r['start_rad']:+.3f} -> {r['target_rad']:+.3f} rad x{len(r['moves'])} "
                f"(moves {','.join(map(str, r['moves']))}): final q spread "
                f"{r['final_q_spread_rad']:.4f} rad, stop spread {r['end_overshoot_spread_rad']:.4f} rad  "
                f"{'PASS' if r['pass'] else 'FAIL'}"
            )
        clamps = score["total_torque_clamps"]
        print(f"  total-torque clamps: {'unchecked' if clamps is None else clamps}")
        for item in score["unscored"]:
            print(f"  ! not scored: {item}")
        s = score["summary"]
        kinds = ", ".join(f"{k} {v['passed']}/{v['moves']}" for k, v in s["by_kind"].items())
        print(
            f"  verdict: {'PASS' if score['pass'] else 'FAIL'} "
            f"({s['passed']}/{s['moves']} moves pass: {kinds})"
        )


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv", type=Path)
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    parser.add_argument(
        "--gate",
        choices=("layer2",),
        help="evaluate pass/fail against documented bench gate criteria",
    )
    parser.add_argument(
        "--tau-ff-rate-limit",
        type=float,
        default=60.0,
        help="tau_ff_rate_limit_nm_per_s from control.yaml (Layer 2 gate)",
    )
    parser.add_argument(
        "--onset",
        action="store_true",
        help="summarize first motion window (accel/friction onset)",
    )
    parser.add_argument(
        "--onset-window-ms",
        type=int,
        default=DEFAULT_ONSET_WINDOW_MS,
        help="onset analysis window in ms (default 250)",
    )
    parser.add_argument(
        "--require-home-start",
        action="store_true",
        help=f"Layer 2 gate: approach segment must start with |q| < {LAYER2_HOME_START_MAX_RAD} rad",
    )
    parser.add_argument(
        "--score-bench",
        action="store_true",
        help="score each move of --joint against the ADR 0039 Phase 3 bench pass criteria "
        "(exit 2 on FAIL)",
    )
    parser.add_argument(
        "--joint",
        default="right_shoulder_pitch",
        help="joint scored by --score-bench (default right_shoulder_pitch)",
    )
    parser.add_argument(
        "--bench-log",
        type=Path,
        help="bench log of the same run (bench-session.txt / bench-latest.log): counts "
        f"'{TOTAL_TORQUE_CLAMP_MARKER}' warnings for --score-bench",
    )
    args = parser.parse_args(list(argv) if argv is not None else None)

    if not args.csv.is_file():
        print(f"error: not found: {args.csv}", file=sys.stderr)
        return 1
    if args.bench_log is not None and not args.bench_log.is_file():
        print(f"error: not found: {args.bench_log}", file=sys.stderr)
        return 1

    report = analyze(
        args.csv,
        gate=args.gate,
        tau_ff_rate_limit_nm_s=args.tau_ff_rate_limit,
        onset=args.onset,
        onset_window_ms=args.onset_window_ms,
        require_home_start=args.require_home_start,
        bench_joint=args.joint if args.score_bench else None,
        bench_log=args.bench_log,
    )
    if args.json:
        json.dump(report, sys.stdout, indent=2)
        print()
    else:
        _print_human(report, segment_detail=not args.score_bench)
    if args.score_bench and not report["bench_score"]["pass"]:
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
