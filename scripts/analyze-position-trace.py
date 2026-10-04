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

ADR 0039 Phase 3 bench score (one joint, every-tick trace; per move):
  python scripts/analyze-position-trace.py trace.csv --score-bench \\
      --joint right_shoulder_pitch --bench-log bench-session.txt
  Pass: speed overshoot <= 20 % of the reference peak (dq_ref, else dq_traj), with speed
  from dq over 50 ms (one 0.077 rad/s feedback quantum would be 51 % of a 0.15 rad/s slew);
  <= 0.01 rad past the target at the stop; no single-tick tau_ff_cmd step > 0.05 Nm
  (retargets included); and no "MIT total torque clamped" line for the joint in the bench
  log. A criterion that cannot be scored (decimated trace, no bench log) fails the verdict.
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
# A target held for at most this many rows is a wave sample, not a hold-at move.
BENCH_WAVE_RUN_ROWS = 2
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


def _windowed_speeds(seg: list[dict[str, str]], window_ms: int) -> list[float | None]:
    """Signed speed at each row from q over the latest earlier row >= window_ms back."""
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
            out.append((_f(row, "q") - _f(seg[j], "q")) / (dt_ms / 1000.0))
    return out


def _split_moves(rows: list[dict[str, str]]) -> list[list[dict[str, str]]]:
    """One joint's rows cut at each target change. A wave moves `target` with every sample, so
    consecutive runs of at most BENCH_WAVE_RUN_ROWS rows merge into one wave move."""
    moves: list[tuple[bool, list[dict[str, str]]]] = []
    for run in _split_segments(rows):
        short = len(run) <= BENCH_WAVE_RUN_ROWS
        if short and moves and moves[-1][0]:
            moves[-1][1].extend(run)
        else:
            moves.append((short, list(run)))
    return [m for _, m in moves]


def _score_move(
    seg: list[dict[str, str]], prev: dict[str, str] | None, window_ms: int
) -> dict:
    targets = [_f(r, "target") for r in seg]
    # A wave moves its own target every tick: no stop to overshoot, only its speed.
    wave = max(targets) - min(targets) > 1e-4
    target = targets[-1]
    q_start = _f(seg[0], "q")
    distance = target - q_start
    direction = 0.0 if abs(distance) < BENCH_MIN_MOVE_RAD else math.copysign(1.0, distance)
    plan_key = "dq_ref" if _has(seg[0], "dq_ref") else "dq_traj"
    v_plan = max(abs(_f(r, plan_key)) for r in seg)

    speed_overshoot = None
    v_peak = None
    if (direction or wave) and v_plan >= BENCH_MIN_PLAN_RAD_S:
        speeds = [s for s in _windowed_speeds(seg, window_ms) if s is not None]
        along = [abs(s) for s in speeds] if wave else [s * direction for s in speeds]
        if along:
            v_peak = max(along)
            speed_overshoot = (v_peak - v_plan) / v_plan

    end_overshoot = None
    if direction and not wave:
        end_overshoot = max(0.0, max((_f(r, "q") - target) * direction for r in seg))

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

    checks = {
        "speed_overshoot_ok": speed_overshoot is None
        or speed_overshoot <= BENCH_OVERSHOOT_FRACTION,
        "end_overshoot_ok": end_overshoot is None or end_overshoot <= BENCH_END_OVERSHOOT_RAD,
        "tau_step_ok": tau_step is not None and tau_step <= BENCH_TAU_STEP_NM,
    }
    return {
        "target_rad": target,
        "wave": wave,
        "q_start": q_start,
        "t_start_ms": _i(seg[0], "t_ms"),
        "duration_s": (_i(seg[-1], "t_ms") - _i(seg[0], "t_ms")) / 1000.0,
        "plan_peak_rad_s": v_plan,
        "measured_peak_rad_s": v_peak,
        "speed_overshoot": speed_overshoot,
        "end_overshoot_rad": end_overshoot,
        "max_tau_ff_step_nm": tau_step,
        "tau_ff_wire_binding_rows": wire_binding,
        "checks": checks,
        "pass": all(checks.values()),
    }


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
    moves = []
    prev: dict[str, str] | None = None
    for seg in _split_moves(joint_rows):
        moves.append(_score_move(seg, prev, window_ms))
        prev = seg[-1]
    clamps = _count_total_torque_clamps(bench_log, joint) if bench_log is not None else None
    unscored = []
    if not moves:
        unscored.append(f"no rows for {joint}")
    if any(m["max_tau_ff_step_nm"] is None for m in moves):
        unscored.append("tau_ff step: no consecutive ticks (decimated trace?)")
    if clamps is None:
        unscored.append("total-torque clamps: no --bench-log")
    return {
        "joint": joint,
        "criteria": {
            "speed_window_ms": window_ms,
            "speed_overshoot_max": BENCH_OVERSHOOT_FRACTION,
            "end_overshoot_max_rad": BENCH_END_OVERSHOOT_RAD,
            "tau_ff_step_max_nm": BENCH_TAU_STEP_NM,
            "total_torque_clamps_max": 0,
        },
        "moves": moves,
        "total_torque_clamps": clamps,
        "unscored": unscored,
        "pass": bool(moves) and not unscored and clamps == 0 and all(m["pass"] for m in moves),
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


def _print_human(report: dict) -> None:
    print(f"trace: {report['path']} ({report['samples']} samples)")
    for i, seg in enumerate(report["segments"], 1):
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
            f"=== ADR 0039 bench score: {score['joint']} (speed over {c['speed_window_ms']} ms; "
            f"overshoot <= {c['speed_overshoot_max']:.0%}, stop <= {c['end_overshoot_max_rad']} rad, "
            f"tau_ff step <= {c['tau_ff_step_max_nm']} Nm, total-torque clamps 0) ==="
        )
        fmt = lambda v, f: "-" if v is None else format(v, f)  # noqa: E731
        for i, m in enumerate(score["moves"], 1):
            failed = [k for k, ok in m["checks"].items() if not ok]
            print(
                f"  move {i}: {m['q_start']:+.3f} -> {m['target_rad']:+.3f} rad  "
                f"plan {m['plan_peak_rad_s']:.2f} rad/s  speed overshoot "
                f"{fmt(m['speed_overshoot'], '+.0%')}  stop overshoot "
                f"{fmt(m['end_overshoot_rad'], '.4f')} rad  tau_ff step "
                f"{fmt(m['max_tau_ff_step_nm'], '.3f')} Nm  wire-binding rows "
                f"{m['tau_ff_wire_binding_rows']}  {'PASS' if not failed else 'FAIL ' + ','.join(failed)}"
            )
        clamps = score["total_torque_clamps"]
        print(f"  total-torque clamps: {'unchecked' if clamps is None else clamps}")
        for item in score["unscored"]:
            print(f"  ! not scored: {item}")
        print(f"  verdict: {'PASS' if score['pass'] else 'FAIL'}")


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
        _print_human(report)
    if args.score_bench and not report["bench_score"]["pass"]:
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
