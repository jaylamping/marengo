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
    "tau_ff_cmd,tau_meas,dq_mit,kp,kd,joint_stuck,planner_frozen,retarget_age_ms,planner_event,"
    "law,q_ref,dq_ref,time_scale,tau_i,kd_mit,tau_ff_wire"
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
                "legacy",
                f"{q:.6f}",
                f"{sign * dq:.6f}",
                "1.000000",
                "0.000000",
                "1.000",
                f"{tau_ff:.6f}",
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
    pe = HEADER_NEW.split(",").index("planner_event")
    rows[5][pe] = "reset"
    rows[10][pe] = "latch"
    _write_rows(path, HEADER_NEW, rows)
    report = _run_analyzer(path)
    seg = report["segments"][0]
    assert seg["planner_event_counts"].get("reset") == 1
    assert seg["planner_event_counts"].get("tick", 0) >= 1


# --- ADR 0039 Phase 3 bench score (--score-bench) ---


def _bench_row(tick: int, q: float, dq: float, target: float, dq_ref: float, tau_ff: float) -> list:
    """One every-tick (5 ms) pitch row in the current schema."""
    return [
        tick,
        tick * 5,
        "right_shoulder_pitch",
        f"{q:.6f}",
        f"{dq:.6f}",
        f"{q:.6f}",
        f"{dq_ref:.6f}",
        f"{q:.6f}",
        f"{target:.6f}",
        f"{target:.6f}",
        "-1.088000",
        "2.980000",
        "0.000000",
        "0",
        f"{target - q:.6f}",
        "Cruise",
        "reference",
        "0.000000",
        "1.000000",
        "0.000000",
        "0.000000",
        f"{tau_ff:.6f}",
        f"{tau_ff:.6f}",
        f"{dq_ref:.6f}",
        "18.000",
        "3.000",
        "0",
        "0",
        "0",
        "tick",
        "scaled_pd",
        f"{q:.6f}",
        f"{dq_ref:.6f}",
        "1.000000",
        "0.000000",
        "3.000",
        f"{tau_ff:.6f}",
    ]


def _bench_trace(
    moves: list[tuple[float, float]],
    *,
    speed_scale: float = 1.0,
    stop_overshoot: float = 0.0,
    tau_kick_nm: float = 0.0,
) -> list[list]:
    """Moves (target rad, plan rad/s) from 0: the reference runs at the plan speed, the joint at
    `speed_scale` × plan, overshoots each stop by `stop_overshoot`, then rests 0.5 s there.
    `tau_kick_nm` steps tau_ff once mid-move."""
    rows: list[list] = []
    tick, q, tau = 0, 0.0, 1.0
    for target, v in moves:
        sign = 1.0 if target >= q else -1.0
        stop = target + sign * stop_overshoot
        kicked = False
        while (stop - q) * sign > 1e-9:
            dq = sign * v * speed_scale
            q = min(q + dq * 0.005, stop) if sign > 0 else max(q + dq * 0.005, stop)
            tau += 0.004
            if tau_kick_nm and not kicked and abs(q - target) < abs(target) / 2:
                tau += tau_kick_nm
                kicked = True
            rows.append(_bench_row(tick, q, dq, target, sign * v, tau))
            tick += 1
        for _ in range(100):
            rows.append(_bench_row(tick, q, 0.0, target, 0.0, tau))
            tick += 1
    return rows


def _score(tmp_path: Path, rows: list[list], log_text: str | None = "") -> tuple[int, dict]:
    import json

    trace = tmp_path / "trace.csv"
    _write_rows(trace, HEADER_NEW, rows)
    cmd = [sys.executable, str(ANALYZER), str(trace), "--json", "--score-bench"]
    if log_text is not None:
        log = tmp_path / "bench-session.txt"
        log.write_text(log_text)
        cmd += ["--bench-log", str(log)]
    proc = subprocess.run(cmd, text=True, capture_output=True, check=False)
    return proc.returncode, json.loads(proc.stdout)["bench_score"]


TRIAL = [(0.75, 1.0), (0.70, 0.15), (0.0, 1.0)]


def test_score_bench_passes_a_clean_trial(tmp_path: Path) -> None:
    code, score = _score(tmp_path, _bench_trace(TRIAL))
    assert code == 0, score
    assert score["pass"] is True
    assert [round(m["target_rad"], 2) for m in score["moves"]] == [0.75, 0.70, 0.0]
    assert score["total_torque_clamps"] == 0


def test_score_bench_fails_each_criterion(tmp_path: Path) -> None:
    cases = {
        "speed_overshoot_ok": _bench_trace(TRIAL, speed_scale=1.3),
        "end_overshoot_ok": _bench_trace(TRIAL, stop_overshoot=0.02),
        "tau_step_ok": _bench_trace(TRIAL, tau_kick_nm=0.1),
    }
    for check, rows in cases.items():
        code, score = _score(tmp_path, rows)
        assert code == 2, check
        assert any(not m["checks"][check] for m in score["moves"]), (check, score)


def test_score_bench_reads_total_torque_clamps_from_the_bench_log(tmp_path: Path) -> None:
    log = (
        "WARN davout::total_torque: MIT total torque clamped: setpoints moved toward feedback "
        'joint="right_shoulder_pitch" predicted_nm=5.4 joint_clamps=3 clamps_since_last_warning=3\n'
        'WARN davout::total_torque: MIT total torque clamped joint="right_elbow_pitch" joint_clamps=9\n'
    )
    code, score = _score(tmp_path, _bench_trace(TRIAL), log)
    assert code == 2
    assert score["total_torque_clamps"] == 3


def test_score_bench_fails_when_a_criterion_is_unscored(tmp_path: Path) -> None:
    code, score = _score(tmp_path, _bench_trace(TRIAL), None)
    assert code == 2
    assert score["total_torque_clamps"] is None
    decimated = _bench_trace(TRIAL)[::4]
    code, score = _score(tmp_path, decimated)
    assert code == 2
    assert any("tau_ff step" in u for u in score["unscored"]), score
