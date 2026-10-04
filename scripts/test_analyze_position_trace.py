#!/usr/bin/env python3
"""Golden tests for scripts/analyze-position-trace.py Layer 2 gate."""

from __future__ import annotations

import csv
import math
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


# --- slow moves, waves, drift, repeatability, decimation ---


def _analyzer_module():
    import importlib.util

    spec = importlib.util.spec_from_file_location("analyze_position_trace", ANALYZER)
    module = importlib.util.module_from_spec(spec)
    assert spec.loader is not None
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def _row(
    tick: int,
    *,
    q: float,
    target: float,
    q_ref: float | None = None,
    dq_ref: float = 0.0,
    tau_ff: float = 1.0,
) -> list:
    """`_bench_row` with an independent reference (q_ref may differ from q)."""
    row = _bench_row(tick, q, 0.0, target, dq_ref, tau_ff)
    q_ref = q if q_ref is None else q_ref
    row[5] = f"{q_ref:.6f}"  # q_traj
    row[31] = f"{q_ref:.6f}"  # q_ref
    return row


def _slow_move(start: float, target: float, v: float, catch_up: float) -> list[list]:
    """A slew move: the reference runs at `v`; the joint sticks at `start` until the reference is
    halfway (lagging half the move), then catches up at `catch_up` × v, then holds 1 s."""
    rows, tick = [], 0
    for _ in range(100):
        rows.append(_row(tick, q=start, target=start))
        tick += 1
    sign = 1.0 if target >= start else -1.0
    ref, q = start, start
    while (target - ref) * sign > 1e-9 or abs(q - target) > 1e-9:
        ref = ref + sign * v * 0.005 if (target - ref) * sign > v * 0.005 else target
        if abs(ref - start) < abs(target - start) / 2:
            q = start
        else:
            step = sign * v * catch_up * 0.005
            q = q + step if (ref - q) * sign > abs(step) else ref
        dq_ref = 0.0 if ref == target else sign * v
        rows.append(_row(tick, q=q, target=target, q_ref=ref, dq_ref=dq_ref))
        tick += 1
    for _ in range(200):
        rows.append(_row(tick, q=target, target=target, q_ref=target))
        tick += 1
    return rows


def test_thresholds_follow_the_encoder_quantum() -> None:
    m = _analyzer_module()
    # Two counts over the 50 ms window are 5 % of the slow-plan threshold (0.3 rad/s).
    two_counts_rad_s = 2 * m.BENCH_ENCODER_COUNT_RAD / (m.BENCH_SPEED_WINDOW_MS / 1000)
    assert abs(two_counts_rad_s / m.BENCH_SLOW_PLAN_RAD_S - 0.05) < 0.003
    # Slow speed windows keep two counts at 5 % of the plan.
    assert m._slow_speed_window_ms(0.15) == 103
    assert m._slow_speed_window_ms(1.0) == m.BENCH_SPEED_WINDOW_MS
    assert m._slow_speed_window_ms(0.001) == m.BENCH_SLOW_SPEED_WINDOW_MAX_MS


def test_slow_move_scores_position_not_speed_ratio(tmp_path: Path) -> None:
    # A 0.05 rad slew move that sticks 0.025 rad behind, then catches up at 2× plan: +100 %
    # "speed overshoot" from the dead zone, yet it tracks within 0.03 rad and stops on target.
    rows = _slow_move(0.5, 0.55, 0.15, 2.0)
    code, score = _score(tmp_path, rows)
    move = next(m for m in score["moves"] if m["kind"] == "move")
    assert move["slow"] is True
    assert move["speed_overshoot"] > 0.5
    assert "speed_overshoot_ok" not in move["checks"]
    assert move["checks"]["tracking_ok"] is True
    assert code == 0, score
    # Lagging 0.05 rad behind fails on tracking.
    code, score = _score(tmp_path, _slow_move(0.0, 0.1, 0.15, 2.0))
    move = next(m for m in score["moves"] if m["kind"] == "move")
    assert move["checks"]["tracking_ok"] is False
    assert code == 2


def test_fast_long_move_keeps_the_speed_ratio(tmp_path: Path) -> None:
    code, score = _score(tmp_path, _bench_trace([(0.75, 1.0)], speed_scale=1.3))
    move = score["moves"][0]
    assert move["slow"] is False
    assert move["checks"]["speed_overshoot_ok"] is False
    assert code == 2


def test_tau_step_scored_per_tick_and_reported_decimated_otherwise(tmp_path: Path) -> None:
    code, score = _score(tmp_path, _bench_trace(TRIAL))
    assert score["decimated"] is False
    assert all(m["max_tau_ff_step_nm"] is not None for m in score["moves"])
    code, score = _score(tmp_path, _bench_trace(TRIAL)[::4])
    assert code == 2
    assert score["decimated"] is True and score["trace_period_ticks"] == 4
    assert all(m["max_tau_ff_step_nm"] is None for m in score["moves"])
    assert any("decimated" in u for u in score["unscored"]), score


def _wave_rows(lo: float, hi: float, half_period_s: float, dq_ref_clamp: float | None) -> list[list]:
    """Park 0.5 s at lo, one raised-cosine cycle (q tracks the reference), park 0.5 s."""
    rows, tick = [], 0
    for _ in range(100):
        rows.append(_row(tick, q=lo, target=lo))
        tick += 1
    mid, amp = (lo + hi) / 2, (hi - lo) / 2
    ticks = round(2 * half_period_s / 0.005)
    omega = math.pi / half_period_s
    for k in range(1, ticks):
        t = k * 0.005
        ref = mid - amp * math.cos(omega * t)
        v = amp * omega * math.sin(omega * t)
        if dq_ref_clamp is not None:
            v = max(-dq_ref_clamp, min(dq_ref_clamp, v))
        rows.append(_row(tick, q=ref, target=ref, q_ref=ref, dq_ref=v))
        tick += 1
    for _ in range(100):
        rows.append(_row(tick, q=lo, target=lo))
        tick += 1
    return rows


def test_wave_is_one_move_scored_against_its_reference_speed(tmp_path: Path) -> None:
    # Bench 2026-10-04: dq_ref clamped at 0.15 while q_ref swept at 0.79 rad/s read as +437 %.
    code, score = _score(tmp_path, _wave_rows(-0.5, 0.5, 2.0, dq_ref_clamp=0.15))
    waves = [m for m in score["moves"] if m["kind"] == "wave"]
    assert len(waves) == 1, [m["kind"] for m in score["moves"]]
    wave = waves[0]
    assert wave["swept_rad"][0] < -0.49 and wave["swept_rad"][1] > 0.49
    assert abs(wave["plan_peak_rad_s"] - math.pi / 4) < 0.02
    assert abs(wave["speed_overshoot"]) < 0.05
    assert "clamped" in wave["hint"]
    assert code == 0, score


def test_long_hold_scores_drift(tmp_path: Path) -> None:
    def hold(drift: float) -> list[list]:
        rows = [_row(t, q=0.0, target=0.0) for t in range(10)]
        for t in range(10, 10 + 2400):  # 12 s at 0.4: arrives at once, then drifts linearly
            q = 0.4 + drift * (t - 10) / 2400
            rows.append(_row(t, q=q, target=0.4, q_ref=0.4))
        return rows

    code, score = _score(tmp_path, hold(0.002))
    held = score["moves"][-1]
    assert held["drift_rad"] is not None and held["checks"]["drift_ok"] is True
    code, score = _score(tmp_path, hold(0.01))
    assert score["moves"][-1]["checks"]["drift_ok"] is False
    assert code == 2


def test_repeated_moves_score_their_spread(tmp_path: Path) -> None:
    def repeats(spread: float) -> list[list]:
        """5 × (0 → 0.6 at 1 rad/s, landing short by up to `spread`, then back to 0)."""
        rows = [_bench_row(t, 0.0, 0.0, 0.0, 0.0, 1.0) for t in range(100)]
        tick, q = 100, 0.0
        for i in range(5):
            for target, land in ((0.6, 0.6 - spread * i / 4), (0.0, 0.0)):
                sign = 1.0 if target >= q else -1.0
                while (land - q) * sign > 1e-9:
                    q = min(q + 0.005, land) if sign > 0 else max(q - 0.005, land)
                    rows.append(_bench_row(tick, q, sign, target, sign, 1.0))
                    tick += 1
                for _ in range(100):
                    rows.append(_bench_row(tick, q, 0.0, target, 0.0, 1.0))
                    tick += 1
        return rows

    code, score = _score(tmp_path, repeats(0.004))
    out = next(r for r in score["repeatability"] if r["target_rad"] == 0.6)
    assert len(out["moves"]) == 5 and out["pass"] is True
    assert abs(out["final_q_spread_rad"] - 0.004) < 1e-5
    assert code == 0, score
    code, score = _score(tmp_path, repeats(0.02))
    out = next(r for r in score["repeatability"] if r["target_rad"] == 0.6)
    assert out["pass"] is False
    assert code == 2
