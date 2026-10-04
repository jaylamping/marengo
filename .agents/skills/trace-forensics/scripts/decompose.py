#!/usr/bin/env python3
"""Decompose a Marengo position-trace.csv into the torques and errors behind a joint's motion.

Run with numpy (matplotlib only for --plot):
  uv run --no-project --with numpy --with matplotlib python3 decompose.py TRACE.csv \
      --joint right_shoulder_pitch [--plot OUT_DIR] [--json]

Sections:
  coverage   rows, sample period, every-tick share (acceleration and per-tick checks need it)
  tracking   |q - q_ref| and |q - target| overall and per trapezoid phase
  drive      host's model of the applied torque, kp*(q_des-q) + kd_mit*(dq_mit-dq) + tau_ff_wire,
             against the drive-reported tau_meas
  shaping    rows where Davout or the envelope changed what the law asked for
  residual   least-squares fit of (torque - tau_g) = J*a + Fc*sign(v) + B*v + c on moving rows,
             once with tau_meas and once with the commanded (model) torque
  up/down    per 0.1 rad bin of q on moving rows: gravity error = mean of up and down residuals,
             friction = half their difference (the calibration-sweep method)
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from pathlib import Path

import numpy as np

NUM = [
    "tick", "t_ms", "q", "dq", "q_traj", "dq_traj", "q_des", "target", "target_raw",
    "q_env_lo", "q_env_hi", "lead", "settle_error", "tau_p", "tau_g", "tau_f", "tau_d",
    "tau_ff_cmd", "tau_meas", "dq_mit", "kp", "kd", "q_ref", "dq_ref", "time_scale", "tau_i",
    "kd_mit", "tau_ff_wire",
]
DT = 0.005
ACCEL_WINDOW = 10  # ticks: central difference of dq over 50 ms
MOVING_RAD_S = 0.05
BIN_RAD = 0.1


def load(path: Path, joint: str) -> tuple[dict[str, np.ndarray], list[dict[str, str]]]:
    with path.open(newline="") as f:
        rows = [r for r in csv.DictReader(f) if r.get("joint") == joint]
    if not rows:
        sys.exit(f"no rows for joint {joint} in {path}")
    cols: dict[str, np.ndarray] = {}
    for name in NUM:
        if name in rows[0]:
            cols[name] = np.array([float(r[name]) if r[name] not in ("", "nan", "NaN") else math.nan for r in rows])
    return cols, rows


def stats(x: np.ndarray) -> dict[str, float]:
    x = x[np.isfinite(x)]
    if x.size == 0:
        return {"n": 0}
    return {
        "n": int(x.size),
        "rms": float(np.sqrt(np.mean(x**2))),
        "mean": float(np.mean(x)),
        "p95_abs": float(np.percentile(np.abs(x), 95)),
        "max_abs": float(np.max(np.abs(x))),
    }


def fit_residual(r: np.ndarray, a: np.ndarray, v: np.ndarray) -> dict[str, float] | None:
    m = np.isfinite(r) & np.isfinite(a) & np.isfinite(v) & (np.abs(v) > MOVING_RAD_S)
    if m.sum() < 50:
        return None
    A = np.column_stack([a[m], np.sign(v[m]), v[m], np.ones(m.sum())])
    coef, *_ = np.linalg.lstsq(A, r[m], rcond=None)
    pred = A @ coef
    ss_res = float(np.sum((r[m] - pred) ** 2))
    ss_tot = float(np.sum((r[m] - r[m].mean()) ** 2)) or math.nan
    return {
        "rows": int(m.sum()),
        "J_kg_m2": float(coef[0]),
        "Fc_Nm": float(coef[1]),
        "B_Nm_s_per_rad": float(coef[2]),
        "offset_Nm": float(coef[3]),
        "r2": 1.0 - ss_res / ss_tot,
        "rms_unexplained_Nm": math.sqrt(ss_res / m.sum()),
    }


def up_down(q: np.ndarray, v: np.ndarray, r: np.ndarray) -> list[dict[str, float]]:
    out = []
    m = np.isfinite(q) & np.isfinite(v) & np.isfinite(r) & (np.abs(v) > MOVING_RAD_S)
    if not m.any():
        return out
    lo = math.floor(q[m].min() / BIN_RAD) * BIN_RAD
    hi = q[m].max()
    edge = lo
    while edge <= hi:
        b = m & (q >= edge) & (q < edge + BIN_RAD)
        up, down = b & (v > 0), b & (v < 0)
        if up.sum() >= 5 and down.sum() >= 5:
            ru, rd = float(r[up].mean()), float(r[down].mean())
            out.append({
                "q_bin_rad": round(edge + BIN_RAD / 2, 3),
                "n_up": int(up.sum()),
                "n_down": int(down.sum()),
                "gravity_error_Nm": (ru + rd) / 2,
                "friction_Nm": (ru - rd) / 2,
            })
        edge += BIN_RAD
    return out


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("trace", type=Path)
    ap.add_argument("--joint", required=True)
    ap.add_argument("--plot", type=Path, help="write PNG plots into this directory")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    c, rows = load(args.trace, args.joint)
    n = len(rows)
    tick = c["tick"]
    dtick = np.diff(tick)
    every_tick = float(np.mean(dtick == 1)) if n > 1 else 0.0
    report: dict[str, object] = {
        "trace": str(args.trace),
        "joint": args.joint,
        "coverage": {
            "rows": n,
            "duration_s": float((c["t_ms"][-1] - c["t_ms"][0]) / 1000.0),
            "every_tick_share": every_tick,
            "laws": sorted({r.get("law", "") for r in rows}),
        },
    }

    q, dq = c["q"], c["dq"]
    q_ref = c.get("q_ref", c["q_traj"])
    report["tracking"] = {
        "q_minus_q_ref": stats(q - q_ref),
        "q_minus_target": stats(q - c["target"]),
        "by_phase": {
            ph: stats((q - q_ref)[[r["phase"] == ph for r in rows]])
            for ph in sorted({r["phase"] for r in rows})
        },
    }

    tau_wire = c.get("tau_ff_wire", c["tau_ff_cmd"])
    kd_wire = c.get("kd_mit", c["kd"])
    tau_model = c["kp"] * (c["q_des"] - q) + kd_wire * (c["dq_mit"] - dq) + tau_wire
    report["drive"] = {
        "tau_model_minus_tau_meas": stats(tau_model - c["tau_meas"]),
        "corr_model_meas": float(np.corrcoef(
            tau_model[np.isfinite(tau_model)], c["tau_meas"][np.isfinite(tau_model)])[0, 1])
        if np.isfinite(tau_model).sum() > 2 else math.nan,
    }

    ff_reshaped = np.isfinite(tau_wire) & (np.abs(tau_wire - c["tau_ff_cmd"]) > 1e-3)
    env_clamped = np.abs(c["q_des"] - q_ref) > 1e-6
    events: dict[str, int] = {}
    for r in rows:
        events[r.get("planner_event", "")] = events.get(r.get("planner_event", ""), 0) + 1
    report["shaping"] = {
        "tau_ff_wire_ne_cmd_rows": int(ff_reshaped.sum()),
        "tau_ff_wire_ne_cmd_max_Nm": float(np.nanmax(np.abs(tau_wire - c["tau_ff_cmd"]))) if np.isfinite(tau_wire).any() else math.nan,
        "envelope_clamped_rows": int(env_clamped.sum()),
        "lead_sat_rows": int(sum(r.get("lead_sat") == "1" for r in rows)),
        "planner_events": events,
    }

    if every_tick >= 0.95:
        a = np.full(n, math.nan)
        h = ACCEL_WINDOW // 2
        a[h:-h] = (dq[2 * h:] - dq[:-2 * h]) / (2 * h * DT)
        r_meas = c["tau_meas"] - c["tau_g"]
        r_model = tau_model - c["tau_g"]
        report["residual_fit"] = {
            "with_tau_meas": fit_residual(r_meas, a, dq),
            "with_commanded_torque": fit_residual(r_model, a, dq),
            "note": "tau_meas is the drive's own estimate; on pitch it under-reads inertia vs the commanded torque (bench_replay.rs)",
        }
        report["up_down_bins"] = {
            "with_tau_meas": up_down(q, dq, r_meas),
            "with_commanded_torque": up_down(q, dq, r_model),
        }
    else:
        report["residual_fit"] = f"skipped: every-tick share {every_tick:.2f} < 0.95 (set MARENGO_POSITION_TRACE_FULL_RATE_JOINTS)"

    if args.plot:
        import matplotlib
        matplotlib.use("Agg")
        import matplotlib.pyplot as plt

        args.plot.mkdir(parents=True, exist_ok=True)
        t = (c["t_ms"] - c["t_ms"][0]) / 1000.0
        fig, ax = plt.subplots(3, 1, figsize=(14, 10), sharex=True)
        ax[0].plot(t, q, label="q")
        ax[0].plot(t, q_ref, label="q_ref", lw=0.8)
        ax[0].plot(t, c["target"], label="target", lw=0.8, ls="--")
        ax[0].set_ylabel("rad"); ax[0].legend(loc="upper right")
        ax[1].plot(t, q - q_ref, lw=0.8)
        ax[1].set_ylabel("q - q_ref (rad)")
        for name in ("tau_g", "tau_ff_cmd", "tau_p", "tau_meas"):
            ax[2].plot(t, c[name], label=name, lw=0.8)
        ax[2].plot(t, tau_model, label="tau_model", lw=0.8, ls=":")
        ax[2].set_ylabel("Nm"); ax[2].set_xlabel("s"); ax[2].legend(loc="upper right")
        fig.tight_layout(); fig.savefig(args.plot / f"{args.joint}-timeseries.png", dpi=110)
        if isinstance(report.get("residual_fit"), dict):
            fig, ax = plt.subplots(1, 2, figsize=(14, 5))
            ax[0].scatter(dq, c["tau_meas"] - c["tau_g"], s=1, alpha=0.3, label="tau_meas - tau_g")
            ax[0].scatter(dq, tau_model - c["tau_g"], s=1, alpha=0.3, label="tau_model - tau_g")
            ax[0].set_xlabel("dq (rad/s)"); ax[0].set_ylabel("Nm"); ax[0].legend()
            ax[1].scatter(q, c["tau_meas"] - c["tau_g"], s=1, alpha=0.3, c=np.sign(dq), cmap="coolwarm")
            ax[1].set_xlabel("q (rad)"); ax[1].set_ylabel("tau_meas - tau_g (Nm), colour = sign(dq)")
            fig.tight_layout(); fig.savefig(args.plot / f"{args.joint}-residuals.png", dpi=110)
        report["plots"] = str(args.plot)

    if args.json:
        print(json.dumps(report, indent=2, default=float))
        return
    print_text(report)


def fmt(d: object) -> str:
    if isinstance(d, dict) and "n" in d:
        if d["n"] == 0:
            return "n=0"
        return f"n={d['n']} rms={d['rms']:.4f} mean={d['mean']:+.4f} p95|x|={d['p95_abs']:.4f} max|x|={d['max_abs']:.4f}"
    return json.dumps(d, default=float)


def print_text(r: dict) -> None:
    cov = r["coverage"]
    print(f"trace {r['trace']}  joint {r['joint']}")
    print(f"coverage: {cov['rows']} rows, {cov['duration_s']:.1f} s, every-tick {cov['every_tick_share']:.0%}, law {cov['laws']}")
    tr = r["tracking"]
    print(f"tracking q-q_ref (rad):  {fmt(tr['q_minus_q_ref'])}")
    print(f"tracking q-target (rad): {fmt(tr['q_minus_target'])}")
    for ph, s in tr["by_phase"].items():
        print(f"  phase {ph:<11} {fmt(s)}")
    dr = r["drive"]
    print(f"drive tau_model - tau_meas (Nm): {fmt(dr['tau_model_minus_tau_meas'])}  corr={dr['corr_model_meas']:.3f}")
    sh = r["shaping"]
    print(f"shaping: tau_ff_wire!=cmd rows {sh['tau_ff_wire_ne_cmd_rows']} (max {sh['tau_ff_wire_ne_cmd_max_Nm']:.3f} Nm), "
          f"envelope-clamped rows {sh['envelope_clamped_rows']}, lead_sat rows {sh['lead_sat_rows']}, events {sh['planner_events']}")
    rf = r["residual_fit"]
    if isinstance(rf, str):
        print(f"residual fit: {rf}")
    else:
        for key in ("with_tau_meas", "with_commanded_torque"):
            f = rf[key]
            if f is None:
                print(f"residual fit {key}: too few moving rows")
                continue
            print(f"residual fit {key}: J={f['J_kg_m2']:.4f} kg·m²  Fc={f['Fc_Nm']:+.3f} Nm  B={f['B_Nm_s_per_rad']:+.3f} Nm·s/rad  "
                  f"offset={f['offset_Nm']:+.3f} Nm  R²={f['r2']:.3f}  unexplained rms={f['rms_unexplained_Nm']:.3f} Nm  ({f['rows']} rows)")
        print(f"  note: {rf['note']}")
        for key, bins in r["up_down_bins"].items():
            if not bins:
                print(f"up/down bins {key}: no bin has both directions")
                continue
            print(f"up/down bins {key} (gravity error = mean, friction = half difference):")
            for b in bins:
                print(f"  q≈{b['q_bin_rad']:+.2f}  gravity_err={b['gravity_error_Nm']:+.3f} Nm  friction={b['friction_Nm']:+.3f} Nm  (up {b['n_up']}, down {b['n_down']})")
    if "plots" in r:
        print(f"plots: {r['plots']}")


if __name__ == "__main__":
    main()
