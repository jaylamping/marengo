#!/usr/bin/env python3
"""Summarize Marengo position-hold CSV traces (MARENGO_POSITION_TRACE).

Usage:
  python scripts/analyze-position-trace.py /path/to/position-trace.csv
  python scripts/analyze-position-trace.py /path/to/position-trace.csv --json

Emits per-target-move segments with jerk/lead/rate-limit indicators so tuning
changes can be compared without eyeballing thousands of rows.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
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
    phase_counts: dict[str, int] = field(default_factory=dict)
    hints: list[str] = field(default_factory=list)


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
    settle = [_f(r, "settle_error") for r in seg]
    tracking = [_f(r, "tracking_error") for r in seg]
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

    hints: list[str] = []
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
        phase_counts=phase_counts,
        hints=hints,
    )


def analyze(path: Path) -> dict:
    rows = _rows(path)
    segments = [_analyze_segment(s) for s in _split_segments(rows)]
    return {
        "path": str(path),
        "samples": len(rows),
        "segments": [asdict(s) for s in segments],
    }


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
        for hint in seg["hints"]:
            print(f"  ! {hint}")


def main(argv: Iterable[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv", type=Path)
    parser.add_argument("--json", action="store_true", help="machine-readable output")
    args = parser.parse_args(list(argv) if argv is not None else None)

    if not args.csv.is_file():
        print(f"error: not found: {args.csv}", file=sys.stderr)
        return 1

    report = analyze(args.csv)
    if args.json:
        json.dump(report, sys.stdout, indent=2)
        print()
    else:
        _print_human(report)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
