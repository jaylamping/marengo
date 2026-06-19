#!/usr/bin/env python3
"""Golden tests for scripts/analyze-position-trace.py Layer 2 gate."""

from __future__ import annotations

import csv
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
ANALYZER = REPO / "scripts" / "analyze-position-trace.py"
FIXTURES = REPO / "scripts" / "fixtures" / "position-trace"

HEADER_NEW = (
    "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,target_raw,q_env_lo,q_env_hi,"
    "lead,lead_sat,settle_error,phase,friction_mode,tau_p,tau_g,tau_f,tau_d,"
    "tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event"
)
HEADER_OLD = (
    "tick,t_ms,joint,q,dq,q_traj,dq_traj,q_des,target,lead,lead_sat,settle_error,"
    "phase,friction_mode,tau_p,tau_g,tau_f,tau_d,tau_ff_cmd,tau_meas,dq_mit,kp,kd,"
    "joint_stuck,planner_frozen"
)


def _run_analyzer(path: Path, *extra: str) -> dict:
    import json

    cmd = [sys.executable, str(ANALYZER), str(path), "--json", *extra]
    out = subprocess.check_output(cmd, text=True)
    return json.loads(out)


def _write_rows(path: Path, header: str, rows: list[list]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as f:
        w = csv.writer(f)
        w.writerow(header.split(","))
        w.writerows(rows)


def _smooth_segment(
    target: float,
    q_start: float,
    *,
    t0_ms: int = 0,
    dt_ms: int = 5,
    steps: int = 80,
    dq: float = 0.08,
    tau_ff_step: float = 0.2,
) -> list[list]:
    rows: list[list] = []
    q = q_start
    sign = 1.0 if target >= q_start else -1.0
    tau_ff = 1.0
    for i in range(steps):
        t_ms = t0_ms + i * dt_ms
        q = q + sign * dq * (dt_ms / 1000.0)
        if sign > 0:
            q = min(q, target)
        else:
            q = max(q, target)
        tau_ff += tau_ff_step
        rows.append(
            [
                i,
                t_ms,
                "right_shoulder_pitch",
                f"{q:.6f}",
                f"{sign * dq:.6f}",
                f"{q:.6f}",
                f"{sign * dq:.6f}",
                f"{q:.6f}",
                f"{target:.6f}",
                f"{target:.6f}",
                "-0.850000",
                "3.140000",
                "0.010000",
                "0",
                f"{target - q:.6f}",
                "Cruise",
                "traj_vel",
                "0.120000",
                "1.200000",
                "0.250000",
                "0.010000",
                f"{tau_ff:.6f}",
                "1.100000",
                f"{sign * dq:.6f}",
                "12.000",
                "1.000",
                "0",
                "0",
                str(i * dt_ms),
                "tick",
            ]
        )
    return rows


def test_layer2_pass_fixture(tmp_path: Path) -> None:
    path = FIXTURES / "layer2_pass.csv"
    if not path.exists():
        rows = _smooth_segment(0.1, 0.0, steps=100)
        rows += _smooth_segment(0.0, 0.1, t0_ms=500, steps=100)
        _write_rows(path, HEADER_NEW, rows)
    report = _run_analyzer(path, "--gate", "layer2", "--require-home-start")
    assert report["layer2_gate"]["analyzer_pass"] is True


def test_jerk_fail_detected(tmp_path: Path) -> None:
    path = tmp_path / "jerk_fail.csv"
    rows = _smooth_segment(0.1, 0.0, dq=0.08, steps=40)
    rows += _smooth_segment(0.0, 0.1, t0_ms=300, steps=40)
    # Inject acceleration spikes on measured dq to fail jerk_rms gate.
    for idx in (10, 11, 12, 50, 51, 52):
        if idx < len(rows):
            rows[idx][4] = "3.500000"
    _write_rows(path, HEADER_NEW, rows)
    report = _run_analyzer(path, "--gate", "layer2")
    assert report["layer2_gate"]["analyzer_pass"] is False
    approach = next(s for s in report["segments"] if abs(s["target_rad"] - 0.1) < 1e-3)
    assert approach["gate_checks"]["jerk_ok"] is False


def test_tau_ff_slew_fail(tmp_path: Path) -> None:
    path = tmp_path / "tau_ff_slew_fail.csv"
    rows = _smooth_segment(0.1, 0.0, tau_ff_step=8.0, steps=40)
    rows += _smooth_segment(0.0, 0.1, t0_ms=300, steps=40)
    _write_rows(path, HEADER_NEW, rows)
    report = _run_analyzer(path, "--gate", "layer2", "--tau-ff-rate-limit", "60")
    assert report["layer2_gate"]["analyzer_pass"] is False


def test_missing_approach_segment(tmp_path: Path) -> None:
    path = tmp_path / "missing_approach.csv"
    rows = _smooth_segment(0.0, 0.05, steps=40)
    _write_rows(path, HEADER_NEW, rows)
    report = _run_analyzer(path, "--gate", "layer2")
    assert "approach segment" in report["layer2_gate"]["missing_segments"][0]


def test_missing_return_segment(tmp_path: Path) -> None:
    path = tmp_path / "missing_return.csv"
    rows = _smooth_segment(0.1, 0.0, steps=40)
    _write_rows(path, HEADER_NEW, rows)
    report = _run_analyzer(path, "--gate", "layer2")
    assert "return segment" in report["layer2_gate"]["missing_segments"][0]


def test_require_home_start_fail(tmp_path: Path) -> None:
    path = tmp_path / "home_start_fail.csv"
    rows = _smooth_segment(0.1, 0.03, steps=40)
    rows += _smooth_segment(0.0, 0.1, t0_ms=300, steps=40)
    _write_rows(path, HEADER_NEW, rows)
    report = _run_analyzer(path, "--gate", "layer2", "--require-home-start")
    assert report["layer2_gate"]["approach_checks"]["home_start_ok"] is False


def test_old_schema_still_analyzes(tmp_path: Path) -> None:
    path = FIXTURES / "old_schema.csv"
    if not path.exists():
        rows = []
        for i in range(40):
            q = i * 0.002
            rows.append(
                [
                    i,
                    i * 5,
                    "shoulder_pitch",
                    f"{q:.6f}",
                    "0.080000",
                    f"{q:.6f}",
                    "0.080000",
                    f"{q:.6f}",
                    "0.100000",
                    "0.010000",
                    "0",
                    f"{0.1 - q:.6f}",
                    "Cruise",
                    "traj_vel",
                    "0.120000",
                    "1.200000",
                    "0.250000",
                    "0.010000",
                    "1.460000",
                    "1.100000",
                    "0.080000",
                    "12.000",
                    "1.000",
                    "0",
                    "0",
                ]
            )
        _write_rows(path, HEADER_OLD, rows)
    report = _run_analyzer(path)
    assert report["samples"] > 0


def test_planner_event_counts_in_segment(tmp_path: Path) -> None:
    path = tmp_path / "planner_events.csv"
    rows = _smooth_segment(0.1, 0.0, steps=20)
    rows[5][-1] = "reset"
    rows[10][-1] = "latch"
    _write_rows(path, HEADER_NEW, rows)
    report = _run_analyzer(path)
    seg = report["segments"][0]
    assert seg["planner_event_counts"].get("reset") == 1
    assert seg["planner_event_counts"].get("tick", 0) >= 1
