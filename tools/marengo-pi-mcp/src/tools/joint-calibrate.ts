/**
 * pi_joint_calibrate: one right-arm calibration session (docs/commissioning/
 * right-arm-calibration-suite.md). One joint sweeps while the others hold fixed poses.
 *
 * `method: "wave"` (default): at each pose a local raised-cosine `wave` of amplitude a at 2–3
 * peak speeds. A static hold rests anywhere inside the ±F_s stiction band, so it cannot place
 * gravity better than F_s; a wave drives through stiction both ways, and gravity-fit takes the
 * up/down mean per q bin (friction = half the difference). a = (F_s + (1.6 − 1)·max|τ_g|)/kp
 * from the Pi's control.yaml (breakaway `fs`, else `fc`; impedance `kp`) and the model τ_g at
 * the poses: the P term alone must break stiction plus the unverified model error.
 *
 * `method: "static"`: holds approached from below and from above (gravity and Coulomb
 * friction), then in-loop `wave` passes at several speeds (viscous friction).
 *
 * A pre-flight guard refuses before any motion unless every target clears the soft ∩ hard
 * window by 0.05 rad, the model gravity torque × its uncertainty factor (1.6, or
 * 1 + max(3σ_A/A, 0.15) for a fitted and applied joint: src/tau-factor.ts) stays within 80 % of
 * each joint's τ_ff cap along every commanded path, every wave stays within 80 % of the joint's
 * velocity cap, and the session fits in 300 s. Output is the v2 `var/gravity-calibration/<TS>/`
 * directory; nothing is applied to the Pi.
 */

import path from "node:path";
import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { BENCH_PROFILES, MASTER_JOINTS, profileMeta } from "../bench-profiles.js";
import { type TauFactors, UNVERIFIED_TAU_FACTOR, resolveTauFactors } from "../tau-factor.js";
import { wrapRemoteWithConfig } from "../env.js";
import { soleCanOwnerShell } from "../can-owner.js";
import { validateMotionConfirm } from "../safety.js";
import {
  type AuditMotion,
  type CalibrationDeps,
  type JointWindow,
  type LimitTarget,
  MAX_SESSION_SLEEP_SEC,
  type PreflightFiles,
  type Refusal,
  type RunRemote,
  type SweepHold,
  type YamlNode,
  checkTargetsInWindows,
  defaultCalibrationDeps,
  holdSweepSteps,
  jointWindows,
  motorEntry,
  parsePreflight,
  parseYamlLite,
  preflightReadShell,
  roundRad,
  runCalibrationSession,
  skipHangingRestRequired,
  validatePoses,
  yamlGet,
  yamlNumber,
  yamlStringList,
} from "./calibration-session.js";
import {
  BENCH_CONFIG_MASTER,
  CAN_SESSION_SLACK_MS,
  REFERENCE_OPT_IN_REQUIRED,
  SOLE_CAN_OWNER_NOTE,
  benchConfigDirForJoint,
  motionConfirmSchema,
  referenceAcquireLine,
  referenceOptInShape,
  scriptSleepTotalSec,
} from "./motion.js";

const TOOL = "pi_joint_calibrate";

/** Clearance every commanded target keeps from each soft ∩ hard window edge (rad). */
export const LIMIT_INSET_RAD = 0.05;
/** Share of the joint's τ_ff cap the factored |τ_g| may use. */
export const TAU_CAP_SHARE = 0.8;
/** Share of the joint's resolved velocity cap a wave's peak speed may use. */
export const SPEED_CAP_SHARE = 0.8;
/** Spacing of the τ_g guard's samples along each commanded path (rad). */
export const TAU_SAMPLE_STEP_RAD = 0.05;

export const AMPLITUDE_FRACTIONS = [0.25, 0.5, 0.9] as const;
export const CALIBRATION_METHODS = ["wave", "static"] as const;
export type CalibrationMethod = (typeof CALIBRATION_METHODS)[number];
const DEFAULT_POSE_COUNT = 5;
const MIN_POSES = 2;
const MAX_POSES = 12;
const DEFAULT_APPROACH_OFFSET_RAD = 0.05;
const DEFAULT_SETTLE_SEC = 2.5;
const DEFAULT_MEASURE_SEC = 1.5;
const DEFAULT_RETURN_HOME_SEC = 6;
/** Default velocity band: this wide (rad), centred in the hold-pose band. */
const DEFAULT_WAVE_SPAN_RAD = 0.3;
/** Default wave peak speeds as shares of the joint's admissible wave speed. */
const DEFAULT_WAVE_SPEED_SHARES = [0.25, 0.5, 0.75] as const;
const DEFAULT_WAVE_CYCLES = 2;
/** Narrowest wave the trace check can tell apart from hold noise (rad). */
const MIN_WAVE_SPAN_RAD = 0.05;
/** Smallest local-wave amplitude (rad): half the narrowest traceable wave. */
export const MIN_WAVE_AMPLITUDE_RAD = MIN_WAVE_SPAN_RAD / 2;
/**
 * Largest local-wave amplitude (rad). Default wave poses keep this much (plus the 0.05 rad
 * inset) inside the window; a derived amplitude above it refuses (the model or friction is
 * too far off for a local wave).
 */
export const MAX_WAVE_AMPLITUDE_RAD = 0.1;
/**
 * Slowest local-wave peak speed (rad/s). gravity-fit's centre bin (|q − pose| ≤ a/4) runs at
 * ≥ 97 % of the peak, so 0.2 keeps it clear of the fitter's 0.15 rad/s moving deadband.
 */
export const MIN_WAVE_PEAK_SPEED_RAD_S = 0.2;
/**
 * gravity-fit bins a wave in a/2-wide bins and needs ≥ 10 moving samples per direction in
 * each (marengo-log-cli `BIN_SHARE_OF_AMPLITUDE`, `MIN_BIN_SAMPLES`); cycles keep 1.5× that.
 */
const FIT_BIN_SHARE = 0.5;
const FIT_MIN_BIN_SAMPLES = 10;
const BIN_SAMPLE_MARGIN = 1.5;
/** Wait past a wave's nominal duration before the next stdin line (s). */
const WAVE_END_SLACK_SEC = 0.5;
/** Trace target match tolerance for hold-at steps (the trace prints 6 decimals). */
const TRACE_TARGET_TOL_RAD = 5e-5;
const PREFLIGHT_TIMEOUT_MS = 20_000;
const TAU_GUARD_TIMEOUT_MS = 60_000;
const JOINT_NAME = /^[A-Za-z0-9_]+$/;
const TAU_G_LINE = /^([A-Za-z0-9_]+): tau_g = (-?\d+(?:\.\d+)?) Nm$/;
const POSE_MARKER = "@@gravcal_pose ";

// ---------------------------------------------------------------------------
// Plan (contract: plan.json version 2 steps)

export interface HoldStep extends SweepHold {
  kind: "hold";
}

export interface FixedStep {
  kind: "fixed";
  joint: string;
  target_rad: number;
}

export interface WaveStep {
  kind: "wave";
  joint: string;
  min_rad: number;
  max_rad: number;
  cycles: number;
  half_period_s: number;
  /** Wave method: the pose (index into poses_rad) this local wave is centred on. */
  pose_index?: number;
}

export type JointCalStep = HoldStep | FixedStep | WaveStep;

export interface VelocityPasses {
  min_rad: number;
  max_rad: number;
  half_periods_s: number[];
  cycles: number;
}

/** Per-joint limits read from the Pi's live config (control/motors/robot.yaml + URDF). */
export interface JointLimits {
  window: JointWindow;
  /** min(URDF effort, motors.yaml torque_limit_nm, robot.yaml max_joint_torque_nm, motor-type tau_ff_max_nm). */
  tauFfCapNm: number;
  /** control.yaml joint > actuator group > motor-type velocity_max_rad_s (ADR 0010). */
  velocityCapRadS: number;
  /** control.yaml position_trajectory_velocity_rad_s, when set (marengo-pi wave admission). */
  trajectoryVelocityRadS?: number;
  /** control.yaml position_trajectory_accel_rad_s2, when set (marengo-pi wave admission). */
  trajectoryAccelRadS2?: number;
  /** control.yaml position_slew_rad_s, when set (planner speed of moves ≤ the threshold). */
  positionSlewRadS?: number;
  /** control.yaml position_trajectory_threshold_rad: moves up to this run at the slew speed. */
  trajectoryThresholdRad?: number;
  /** control.yaml impedance.kp: the P gain that drives a local wave through stiction. */
  kp?: number;
  /** control.yaml friction breakaway `fs`, else Coulomb `fc` (Nm). */
  breakawayNm?: number;
  /**
   * Tightest `control.danger_zones` `clamp_velocity` rule naming the joint: the lowest
   * threshold and the lowest speed over all of them (marengo-pi `DescentCap::from_zones`).
   */
  descentCap?: DescentCap;
}

/** Davout clamps the MIT velocity of a joint descending above `aboveRad` to `maxVelocityRadS`. */
export interface DescentCap {
  zones: string[];
  aboveRad: number;
  maxVelocityRadS: number;
}

/** Local waves of the wave method: amplitude and one (half period, cycles) per speed. */
export interface LocalWaves {
  amplitudeRad: number;
  passes: { half_period_s: number; cycles: number }[];
}

export interface JointCalPlan {
  method: CalibrationMethod;
  sweepJoint: string;
  /** Profile joints in chain order (proximal first). */
  chain: string[];
  /** Non-zero fixed poses (a zero entry means "stay at the reference" and is dropped). */
  fixedRad: Record<string, number>;
  posesRad: number[];
  approachOffsetRad: number;
  /** Wave method: the local-wave amplitude (rad). */
  waveAmplitudeRad?: number;
  steps: JointCalStep[];
}

const ceilMrad = (x: number) => Math.ceil(x * 1000 - 1e-9) / 1000;
const floorMrad = (x: number) => Math.floor(x * 1000 + 1e-9) / 1000;

/**
 * `count` evenly spaced poses across `fraction` of the joint's soft ∩ hard window, centred on the
 * reference (0, the hanging rest, lowest gravity) and shifted to stay inside the window shrunk by
 * the 0.05 rad inset plus the approach overshoot δ, so the overshoots clear the inset too.
 */
export function defaultPoses(
  window: JointWindow,
  fraction: number,
  approachOffsetRad: number,
  count = DEFAULT_POSE_COUNT,
): { ok: true; poses: number[] } | Refusal {
  const lo = window.lower + LIMIT_INSET_RAD + approachOffsetRad;
  const hi = window.upper - LIMIT_INSET_RAD - approachOffsetRad;
  const width = Math.min(fraction * (window.upper - window.lower), hi - lo);
  if (!(width > 0)) {
    return {
      ok: false,
      message: `Refused: ${window.joint} window [${window.lower}, ${window.upper}] rad is too narrow for poses ${LIMIT_INSET_RAD} + ${approachOffsetRad} rad inside each limit; no motion was run.`,
    };
  }
  const start = Math.min(Math.max(-width / 2, lo), hi - width);
  const a = ceilMrad(start);
  const b = floorMrad(start + width);
  return {
    ok: true,
    poses: Array.from({ length: count }, (_, i) => Math.round((a + ((b - a) * i) / (count - 1)) * 1000) / 1000),
  };
}

/** Peak speed of a raised-cosine wave between min and max with half period T: π·(max−min)/(2T). */
export function wavePeakSpeed(minRad: number, maxRad: number, halfPeriodS: number): number {
  return (Math.PI * (maxRad - minRad)) / (2 * halfPeriodS);
}

/**
 * Share of a raised-cosine wave's peak speed spent descending while q is above `aboveRad`:
 * q = c + A·cos θ descends at A·ω·sin θ, so above max(aboveRad, c) the speed is at most
 * peak·√(1 − u²), u = (max(aboveRad, c) − c)/A; 0 when the band stays at or below the
 * threshold. Mirrors marengo-pi `DescentCap::wave_descent_speed`.
 */
export function waveDescentFactor(minRad: number, maxRad: number, aboveRad: number): number {
  if (!(maxRad > aboveRad)) return 0;
  const center = (minRad + maxRad) / 2;
  const amplitude = (maxRad - minRad) / 2;
  const u = Math.min(1, Math.max(0, (Math.max(aboveRad, center) - center) / amplitude));
  return Math.sqrt(1 - u * u);
}

/**
 * Highest wave peak speed marengo-pi admits for a wave over [minRad, maxRad]: within 80 % of
 * the velocity cap, the trajectory speed and acceleration, and the joint's descent cap above
 * its danger-zone threshold (no extra share: marengo-pi admits up to the cap itself).
 */
export function admissibleWaveSpeed(limits: JointLimits, minRad: number, maxRad: number): number {
  const span = maxRad - minRad;
  const cap = limits.descentCap;
  const factor = cap === undefined ? 0 : waveDescentFactor(minRad, maxRad, cap.aboveRad);
  return Math.min(
    SPEED_CAP_SHARE * limits.velocityCapRadS,
    limits.trajectoryVelocityRadS ?? Number.POSITIVE_INFINITY,
    // Peak acceleration (span/2)·ω² with ω = v/(span/2): v² ≤ accel · span/2.
    limits.trajectoryAccelRadS2 === undefined ? Number.POSITIVE_INFINITY : Math.sqrt((limits.trajectoryAccelRadS2 * span) / 2),
    cap === undefined || factor === 0 ? Number.POSITIVE_INFINITY : cap.maxVelocityRadS / factor,
  );
}

/**
 * Default velocity passes: a band up to 0.3 rad wide centred in the hold-pose band, at 25/50/75 %
 * of the admissible peak speed, half periods rounded up to 0.05 s (rounding only slows a pass).
 */
export function defaultVelocityPasses(poses: readonly number[], limits: JointLimits): VelocityPasses {
  const a = poses[0];
  const b = poses[poses.length - 1];
  const center = (a + b) / 2;
  const half = Math.min(DEFAULT_WAVE_SPAN_RAD, b - a) / 2;
  const minRad = ceilMrad(center - half);
  const maxRad = floorMrad(center + half);
  const vMax = admissibleWaveSpeed(limits, minRad, maxRad);
  const halfPeriods = DEFAULT_WAVE_SPEED_SHARES.map(
    (share) => Math.ceil((Math.PI * (maxRad - minRad) * 20) / (2 * share * vMax)) / 20,
  );
  return { min_rad: minRad, max_rad: maxRad, half_periods_s: [...new Set(halfPeriods)], cycles: DEFAULT_WAVE_CYCLES };
}

/**
 * The session's step list: fixed poses (chain order), then
 * - method `wave`: per pose (ascending) a park hold at pose − a, then one local `wave`
 *   [pose − a, pose + a] per speed (`pose_index` = the pose);
 * - method `static`: the static sweep (up pass from below, down pass from above), then a hold
 *   at the wave band's lower edge and one velocity-pass `wave` per half period.
 */
export function planJointCalibration(input: {
  sweepJoint: string;
  chain: readonly string[];
  limits: Readonly<Record<string, JointLimits>>;
  fixedRad: Readonly<Record<string, number>>;
  posesRad?: readonly number[];
  amplitudeFraction: number;
  approachOffsetRad: number;
  method: CalibrationMethod;
  /** Method `wave`: amplitude and passes (see {@link localWaves}). */
  waves?: LocalWaves;
  velocityPasses?: VelocityPasses;
}): ({ ok: true } & JointCalPlan) | Refusal {
  const { sweepJoint, chain, approachOffsetRad: delta, method } = input;
  const refuse = (message: string): Refusal => ({ ok: false, message: `Refused: ${message}` });
  if (!chain.includes(sweepJoint)) return refuse(`sweep_joint ${sweepJoint} is not referenced by the bench profile`);
  if (!Number.isFinite(delta) || delta <= 0) return refuse("approach_offset_rad must be positive");
  for (const [joint, rad] of Object.entries(input.fixedRad)) {
    if (joint === sweepJoint) return refuse(`fixed_rad names the sweep joint ${joint}; the sweep moves it`);
    if (!chain.includes(joint)) return refuse(`fixed_rad joint ${joint} is not referenced by the bench profile`);
    if (!Number.isFinite(rad)) return refuse(`fixed_rad ${joint} must be finite`);
  }
  const fixedRad = Object.fromEntries(
    chain.filter((j) => (input.fixedRad[j] ?? 0) !== 0).map((j) => [j, input.fixedRad[j]]),
  );
  const sweepLimits = input.limits[sweepJoint];
  if (sweepLimits === undefined) return refuse(`no limits for ${sweepJoint}`);

  let poses: number[];
  if (input.posesRad === undefined) {
    // Wave poses keep room for the largest admissible amplitude; static ones for δ.
    const inset = method === "wave" ? MAX_WAVE_AMPLITUDE_RAD : delta;
    const defaults = defaultPoses(sweepLimits.window, input.amplitudeFraction, inset);
    if (!defaults.ok) return defaults;
    poses = defaults.poses;
  } else {
    poses = input.posesRad.slice();
  }
  const valid = validatePoses(poses, MIN_POSES, MAX_POSES);
  if (!valid.ok) return valid;
  poses = valid.poses;

  const steps: JointCalStep[] = Object.entries(fixedRad).map(([joint, rad]) => ({
    kind: "fixed",
    joint,
    target_rad: rad,
  }));
  if (method === "wave") {
    if (input.velocityPasses !== undefined) {
      return refuse("velocity_passes is for method static; the wave method's local waves already run at several speeds");
    }
    const waves = input.waves;
    if (waves === undefined) return refuse("method wave needs local waves (amplitude and speeds)");
    const a = waves.amplitudeRad;
    if (!(a >= MIN_WAVE_AMPLITUDE_RAD && a <= MAX_WAVE_AMPLITUDE_RAD)) {
      return refuse(`wave amplitude ${a} rad outside [${MIN_WAVE_AMPLITUDE_RAD}, ${MAX_WAVE_AMPLITUDE_RAD}]`);
    }
    if (waves.passes.length === 0) return refuse("method wave needs at least one wave speed");
    for (const p of waves.passes) {
      if (!Number.isInteger(p.cycles) || p.cycles < 1) return refuse("wave cycles must be a positive integer");
      if (!Number.isFinite(p.half_period_s) || p.half_period_s <= 0) return refuse("wave half periods must be positive");
    }
    poses.forEach((pose, i) => {
      const minRad = roundRad(pose - a);
      const maxRad = roundRad(pose + a);
      steps.push({ kind: "hold", joint: sweepJoint, target_rad: minRad, measure: false });
      for (const p of waves.passes) {
        steps.push({ kind: "wave", joint: sweepJoint, min_rad: minRad, max_rad: maxRad, ...p, pose_index: i });
      }
    });
  } else {
    const holds = holdSweepSteps(sweepJoint, poses, delta);
    steps.push(...holds.map((h): HoldStep => ({ kind: "hold", ...h })));
    const passes = input.velocityPasses ?? defaultVelocityPasses(poses, sweepLimits);
    if (passes.half_periods_s.length > 0) {
      const { min_rad: minRad, max_rad: maxRad, cycles } = passes;
      if (!Number.isFinite(minRad) || !Number.isFinite(maxRad) || maxRad - minRad < MIN_WAVE_SPAN_RAD) {
        return refuse(`velocity_passes needs max_rad − min_rad ≥ ${MIN_WAVE_SPAN_RAD} rad (got [${minRad}, ${maxRad}])`);
      }
      if (!Number.isInteger(cycles) || cycles < 1) return refuse("velocity_passes cycles must be a positive integer");
      if (passes.half_periods_s.some((t) => !Number.isFinite(t) || t <= 0)) {
        return refuse("velocity_passes half_periods_s entries must be positive");
      }
      if (holds[holds.length - 1].target_rad !== minRad) {
        steps.push({ kind: "hold", joint: sweepJoint, target_rad: minRad, measure: false });
      }
      for (const halfPeriod of passes.half_periods_s) {
        steps.push({ kind: "wave", joint: sweepJoint, min_rad: minRad, max_rad: maxRad, cycles, half_period_s: halfPeriod });
      }
    }
  }
  for (let i = 1; i < steps.length; i += 1) {
    const [prev, cur] = [steps[i - 1], steps[i]];
    if (prev.kind !== "wave" && cur.kind !== "wave" && prev.joint === cur.joint && prev.target_rad === cur.target_rad) {
      return refuse(`plan repeats hold-at ${cur.joint} ${cur.target_rad} on consecutive steps`);
    }
  }
  return {
    ok: true,
    method,
    sweepJoint,
    chain: [...chain],
    fixedRad,
    posesRad: poses,
    approachOffsetRad: delta,
    ...(method === "wave" ? { waveAmplitudeRad: input.waves?.amplitudeRad } : {}),
    steps,
  };
}

/**
 * Local-wave amplitude a = (F_s + (UNVERIFIED_TAU_FACTOR − 1)·max|τ_g|)/kp, rounded up to 1 mrad
 * and at least {@link MIN_WAVE_AMPLITUDE_RAD}: the P torque kp·a alone must break stiction
 * (F_s = control.yaml `fs`, else `fc`) plus the unverified model error, even for a calibrated
 * joint (a calibration session re-measures the model it would otherwise trust)
 * (`max|τ_g|` = the sweep joint's model gravity over the poses). Refuses without kp/F_s or
 * above {@link MAX_WAVE_AMPLITUDE_RAD}.
 */
export function deriveWaveAmplitude(
  joint: string,
  limits: JointLimits,
  maxAbsTauGNm: number,
): { ok: true; amplitudeRad: number; basis: string } | Refusal {
  const { kp, breakawayNm } = limits;
  if (kp === undefined || !(kp > 0) || breakawayNm === undefined || !(breakawayNm >= 0)) {
    return {
      ok: false,
      message: `Refused: ${joint} has no control.yaml impedance.kp > 0 or friction fc/fs on the Pi; the wave amplitude cannot be derived (pass wave_amplitude_rad). No motion was run.`,
    };
  }
  const modelErrorNm = (UNVERIFIED_TAU_FACTOR - 1) * maxAbsTauGNm;
  const raw = (breakawayNm + modelErrorNm) / kp;
  const amplitudeRad = Math.max(MIN_WAVE_AMPLITUDE_RAD, ceilMrad(raw));
  const basis =
    `wave amplitude ${amplitudeRad} rad = max(${MIN_WAVE_AMPLITUDE_RAD}, (F_s ${breakawayNm} + ` +
    `${(UNVERIFIED_TAU_FACTOR - 1).toFixed(1)} × max|τ_g| ${maxAbsTauGNm.toFixed(3)} Nm) / kp ${kp})`;
  if (amplitudeRad > MAX_WAVE_AMPLITUDE_RAD) {
    return {
      ok: false,
      message: `Refused: ${basis} exceeds ${MAX_WAVE_AMPLITUDE_RAD} rad: friction or model error too large for a local wave; fix the model first or pass a smaller wave_amplitude_rad. No motion was run.`,
    };
  }
  return { ok: true, amplitudeRad, basis };
}

/**
 * Local-wave passes for amplitude `a`: peak speeds (default 3, evenly spaced from
 * {@link MIN_WAVE_PEAK_SPEED_RAD_S} to what marengo-pi admits within 80 % of the velocity cap),
 * half period π·a/v to 0.01 s (the slowest rounded down, the others up, so every speed stays in
 * that band), and per speed enough cycles for gravity-fit's centre bin to collect
 * 1.5 × 10 samples per direction at `loopHz`. The admissible speed is the lowest over the waves
 * centred on `centersRad` (the poses; a danger-zone descent cap depends on where a wave runs).
 */
export function localWaves(
  joint: string,
  limits: JointLimits,
  amplitudeRad: number,
  loopHz: number,
  speedsRadS?: readonly number[],
  centersRad: readonly number[] = [0],
): { ok: true; waves: LocalWaves } | Refusal {
  const vMax = Math.min(...centersRad.map((c) => admissibleWaveSpeed(limits, c - amplitudeRad, c + amplitudeRad)));
  let speeds: number[];
  if (speedsRadS === undefined) {
    if (!(vMax > MIN_WAVE_PEAK_SPEED_RAD_S)) {
      return {
        ok: false,
        message: `Refused: ${joint} admits at most ${vMax.toFixed(3)} rad/s for a ±${amplitudeRad} rad wave, not above the ${MIN_WAVE_PEAK_SPEED_RAD_S} rad/s floor gravity-fit needs; increase wave_amplitude_rad. No motion was run.`,
      };
    }
    speeds = [0, 0.5, 1].map((s) => MIN_WAVE_PEAK_SPEED_RAD_S + s * (vMax - MIN_WAVE_PEAK_SPEED_RAD_S));
  } else {
    speeds = [...speedsRadS].sort((x, y) => x - y);
    if (speeds[0] < MIN_WAVE_PEAK_SPEED_RAD_S) {
      return {
        ok: false,
        message: `Refused: wave_speeds_rad_s ${speeds[0]} is below ${MIN_WAVE_PEAK_SPEED_RAD_S} rad/s: gravity-fit's centre bin would sit in its moving deadband. No motion was run.`,
      };
    }
  }
  const binRad = FIT_BIN_SHARE * amplitudeRad;
  const passes = speeds.map((v, i) => {
    const t = (Math.PI * amplitudeRad) / v;
    const halfPeriod = i === 0 ? Math.floor(t * 100 + 1e-9) / 100 : Math.ceil(t * 100 - 1e-9) / 100;
    const peak = (Math.PI * amplitudeRad) / halfPeriod;
    const perPass = (binRad * loopHz) / peak;
    const cycles = Math.max(DEFAULT_WAVE_CYCLES, Math.ceil((BIN_SAMPLE_MARGIN * FIT_MIN_BIN_SAMPLES) / perPass));
    return { half_period_s: halfPeriod, cycles };
  });
  const unique = passes.filter((p, i) => passes.findIndex((q) => q.half_period_s === p.half_period_s) === i);
  if (unique.length < 2 || unique.some((p) => !(p.half_period_s > 0))) {
    return {
      ok: false,
      message: `Refused: local waves need ≥ 2 distinct speeds (got half periods ${passes.map((p) => p.half_period_s).join(", ")} s); gravity-fit separates inertia from gravity by speed. No motion was run.`,
    };
  }
  return { ok: true, waves: { amplitudeRad, passes: unique } };
}

/**
 * Wave method: adjacent poses more than 2a apart, so each park target (pose − a) lies outside
 * the previous wave and the trace segments every step unambiguously.
 */
export function checkWavePoseSpacing(plan: JointCalPlan): { ok: true } | Refusal {
  const a = plan.waveAmplitudeRad;
  if (plan.method !== "wave" || a === undefined) return { ok: true };
  for (let i = 1; i < plan.posesRad.length; i += 1) {
    const gap = plan.posesRad[i] - plan.posesRad[i - 1];
    if (!(gap > 2 * a + 2 * TRACE_TARGET_TOL_RAD)) {
      return {
        ok: false,
        message: `Refused: poses ${plan.posesRad[i - 1]} and ${plan.posesRad[i]} are ${roundRad(gap)} rad apart, not more than 2 × wave amplitude ${a} rad; use fewer poses or a wider amplitude_fraction. No motion was run.`,
      };
    }
  }
  return { ok: true };
}

/**
 * marengo-pi stdin script: reference every profile joint (awaited), home, enable, the steps,
 * then `hold-at <joint> 0` for every profile joint distal first (waiting return_home_sec after
 * each moved joint), status, disable, quit. Static holds dwell settle + measure; in the wave
 * method, fixed poses and wave parks only settle.
 */
export function jointCalibrationScript(
  plan: JointCalPlan,
  opts: { operator: string; settleSec: number; measureSec: number; returnHomeSec: number },
): string[] {
  const dwell = plan.method === "wave" ? opts.settleSec : roundRad(opts.settleSec + opts.measureSec);
  const script = [referenceAcquireLine(plan.chain), "home", `enable ${opts.operator}`];
  for (const step of plan.steps) {
    if (step.kind === "wave") {
      // Nominal duration (cycles full periods) plus slack, to 0.1 s.
      const waitSec = Math.ceil((step.cycles * 2 * step.half_period_s + WAVE_END_SLACK_SEC) * 10) / 10;
      script.push(
        `wave ${step.joint} ${String(step.min_rad)} ${String(step.max_rad)} ${step.cycles} ${String(step.half_period_s)}`,
        `sleep ${waitSec}`,
      );
    } else {
      script.push(`hold-at ${step.joint} ${String(step.target_rad)}`, `sleep ${dwell}`);
    }
  }
  // Joints moved away from 0: the sweep joint and every fixed pose.
  const moved = new Set([plan.sweepJoint, ...Object.keys(plan.fixedRad)]);
  for (const joint of [...plan.chain].reverse()) {
    script.push(`hold-at ${joint} 0`);
    if (moved.has(joint)) script.push(`sleep ${opts.returnHomeSec}`);
  }
  script.push("status", "disable", "quit");
  return script;
}

// ---------------------------------------------------------------------------
// Pre-flight guard

/**
 * Per-joint τ_ff cap, velocity cap, pose window, impedance kp and friction breakaway from the
 * Pi's config, plus the control loop rate, or a refusal.
 */
export function readJointLimits(
  preflight: Pick<PreflightFiles, "robotYaml" | "controlYaml" | "motorsYaml" | "urdf">,
  joints: readonly string[],
): { ok: true; limits: Record<string, JointLimits>; robotJoints: string[]; loopHz?: number } | Refusal {
  const windows = jointWindows(preflight.controlYaml, preflight.motorsYaml, joints);
  if (!windows.ok) return windows;
  let robot: YamlNode;
  let control: YamlNode;
  let motors: YamlNode;
  try {
    robot = parseYamlLite(preflight.robotYaml);
    control = parseYamlLite(preflight.controlYaml);
    motors = parseYamlLite(preflight.motorsYaml);
  } catch (err) {
    return { ok: false, message: `Refused: cannot parse Pi robot/control/motors.yaml: ${String(err)}; no motion was run.` };
  }
  const robotJoints = yamlStringList(yamlGet(robot, "robot", "joints"));
  const robotMaxNm = yamlNumber(yamlGet(robot, "robot", "bench", "max_joint_torque_nm"));
  if (robotJoints === undefined || robotJoints.length === 0 || robotJoints.some((j) => !JOINT_NAME.test(j))) {
    return { ok: false, message: "Refused: Pi robot.yaml robot.joints missing or invalid; no motion was run." };
  }
  const groups = yamlGet(control, "control", "actuator_groups");
  const zoneList = yamlGet(control, "control", "danger_zones");
  const zones = Array.isArray(zoneList) ? zoneList : [];
  const limits: Record<string, JointLimits> = {};
  for (const joint of joints) {
    if (!robotJoints.includes(joint)) {
      return { ok: false, message: `Refused: ${joint} is not in the Pi robot.yaml robot.joints; no motion was run.` };
    }
    const motor = motorEntry(motors, joint);
    const motorType = yamlGet(motor, "motor_type");
    const typeDefaults =
      typeof motorType === "string" ? yamlGet(control, "control", "motor_type_defaults", motorType) : undefined;
    const typeTauNm = yamlNumber(yamlGet(typeDefaults, "tau_ff_max_nm"));
    const torqueLimitNm = yamlNumber(yamlGet(motor, "bench", "torque_limit_nm"));
    const urdfEffortNm = urdfJointEffort(preflight.urdf, joint);
    const entry = yamlGet(control, "control", "joints", joint);
    const group =
      groups !== null && typeof groups === "object" && !Array.isArray(groups)
        ? Object.values(groups).find((g) => yamlStringList(yamlGet(g, "joints"))?.includes(joint))
        : undefined;
    const velocityCap =
      yamlNumber(yamlGet(entry, "velocity_max_rad_s")) ??
      yamlNumber(yamlGet(group, "velocity_max_rad_s")) ??
      yamlNumber(yamlGet(typeDefaults, "velocity_max_rad_s"));
    const missing = [
      typeof motorType !== "string" && `motors.yaml ${joint} motor_type`,
      typeTauNm === undefined && `control.yaml motor_type_defaults.${String(motorType)}.tau_ff_max_nm`,
      torqueLimitNm === undefined && `motors.yaml ${joint} bench.torque_limit_nm`,
      robotMaxNm === undefined && "robot.yaml robot.bench.max_joint_torque_nm",
      urdfEffortNm === undefined && `URDF joint ${joint} <limit effort>`,
      velocityCap === undefined && `control.yaml velocity cap for ${joint} (joint, actuator group or motor type)`,
    ].filter((m): m is string => typeof m === "string");
    if (
      typeTauNm === undefined ||
      torqueLimitNm === undefined ||
      robotMaxNm === undefined ||
      urdfEffortNm === undefined ||
      velocityCap === undefined ||
      !(velocityCap > 0)
    ) {
      return {
        ok: false,
        message: `Refused: ${joint} torque/velocity caps missing or unparseable on the Pi: ${missing.join(", ") || "velocity cap ≤ 0"}; no motion was run.`,
      };
    }
    const friction = yamlGet(entry, "friction");
    limits[joint] = {
      window: windows.windows[joint],
      tauFfCapNm: Math.min(urdfEffortNm, torqueLimitNm, robotMaxNm, typeTauNm),
      velocityCapRadS: velocityCap,
      trajectoryVelocityRadS: yamlNumber(yamlGet(entry, "position_trajectory_velocity_rad_s")),
      trajectoryAccelRadS2: yamlNumber(yamlGet(entry, "position_trajectory_accel_rad_s2")),
      positionSlewRadS: yamlNumber(yamlGet(entry, "position_slew_rad_s")),
      trajectoryThresholdRad: yamlNumber(yamlGet(entry, "position_trajectory_threshold_rad")),
      kp: yamlNumber(yamlGet(entry, "impedance", "kp")),
      breakawayNm: yamlNumber(yamlGet(friction, "fs")) ?? yamlNumber(yamlGet(friction, "fc")),
      descentCap: descentCapFor(zones, joint),
    };
  }
  return { ok: true, limits, robotJoints, loopHz: yamlNumber(yamlGet(control, "control", "loop_hz")) };
}

/** `<limit effort="…">` of URDF joint `joint`, or undefined. */
function urdfJointEffort(urdf: string, joint: string): number | undefined {
  const start = urdf.search(new RegExp(`<joint\\s+name="${joint}"`));
  if (start < 0) return undefined;
  const end = urdf.indexOf("</joint>", start);
  const body = urdf.slice(start, end < 0 ? undefined : end);
  const effort = /<limit\b[^>]*\beffort="([^"]+)"/.exec(body)?.[1];
  return effort === undefined ? undefined : yamlNumber(effort);
}

/**
 * Every hold target (overshoots included), fixed pose and wave extreme at least 0.05 rad inside
 * its joint's soft ∩ hard window; the return pose 0 of every profile joint inside the window.
 */
export function checkJointCalLimits(
  plan: JointCalPlan,
  limits: Readonly<Record<string, JointLimits>>,
): { ok: true } | Refusal {
  const targets: LimitTarget[] = [];
  plan.steps.forEach((s, i) => {
    if (s.kind === "wave") {
      targets.push(
        { joint: s.joint, rad: s.min_rad, what: `step ${i} wave min`, inset: LIMIT_INSET_RAD },
        { joint: s.joint, rad: s.max_rad, what: `step ${i} wave max`, inset: LIMIT_INSET_RAD },
      );
    } else {
      const what = s.kind === "fixed" ? `step ${i} fixed pose` : `step ${i} target`;
      targets.push({ joint: s.joint, rad: s.target_rad, what, inset: LIMIT_INSET_RAD });
    }
  });
  for (const joint of plan.chain) targets.push({ joint, rad: 0, what: "return pose", inset: 0 });
  const windows = Object.fromEntries(Object.entries(limits).map(([j, l]) => [j, l.window]));
  return checkTargetsInWindows(targets, windows);
}

/**
 * Each wave's peak speed π·(max−min)/(2·half_period) ≤ 80 % of the joint's velocity cap, and
 * within what marengo-pi admits for a wave (trajectory velocity and acceleration), so it is not
 * refused mid-session.
 */
export function checkWaveSpeeds(
  plan: JointCalPlan,
  limits: Readonly<Record<string, JointLimits>>,
): { ok: true } | Refusal {
  for (const s of plan.steps) {
    if (s.kind !== "wave") continue;
    const l = limits[s.joint];
    const peak = wavePeakSpeed(s.min_rad, s.max_rad, s.half_period_s);
    const capShare = SPEED_CAP_SHARE * l.velocityCapRadS;
    const fmt = (x: number) => x.toFixed(3);
    const wave = `wave ${s.joint} [${s.min_rad}, ${s.max_rad}] half period ${s.half_period_s} s`;
    if (peak > capShare) {
      return {
        ok: false,
        message:
          `Refused: ${wave} peaks at ${fmt(peak)} rad/s, above 80 % of its velocity cap ` +
          `${l.velocityCapRadS} rad/s (${fmt(capShare)} rad/s); lengthen half_periods_s. No motion was run.`,
      };
    }
    if (l.trajectoryVelocityRadS !== undefined && peak > l.trajectoryVelocityRadS) {
      return {
        ok: false,
        message: `Refused: ${wave} peaks at ${fmt(peak)} rad/s, above position_trajectory_velocity_rad_s ${l.trajectoryVelocityRadS}; marengo-pi would refuse it. No motion was run.`,
      };
    }
    const amplitude = (s.max_rad - s.min_rad) / 2;
    const accel = amplitude * (Math.PI / s.half_period_s) ** 2;
    if (l.trajectoryAccelRadS2 !== undefined && accel > l.trajectoryAccelRadS2) {
      return {
        ok: false,
        message: `Refused: ${wave} peaks at ${fmt(accel)} rad/s², above position_trajectory_accel_rad_s2 ${l.trajectoryAccelRadS2}; marengo-pi would refuse it. No motion was run.`,
      };
    }
    const cap = l.descentCap;
    if (cap !== undefined) {
      const descent = peak * waveDescentFactor(s.min_rad, s.max_rad, cap.aboveRad);
      if (descent > cap.maxVelocityRadS) {
        return {
          ok: false,
          message:
            `Refused: ${wave} descends at up to ${fmt(descent)} rad/s above ${cap.aboveRad} rad, where danger zone ` +
            `${cap.zones.join(", ")} clamps descent to ${cap.maxVelocityRadS} rad/s; marengo-pi would refuse it. No motion was run.`,
        };
      }
    }
  }
  return { ok: true };
}

/** {@link DescentCap} over the `clamp_velocity` zones naming `joint`, or undefined. */
function descentCapFor(zones: readonly YamlNode[], joint: string): DescentCap | undefined {
  let cap: DescentCap | undefined;
  for (const z of zones) {
    if (yamlGet(z, "joint") !== joint || yamlGet(z, "action") !== "clamp_velocity") continue;
    const above = yamlNumber(yamlGet(z, "position_above_rad"));
    const speed = yamlNumber(yamlGet(z, "max_velocity_rad_s"));
    const name = yamlGet(z, "name");
    // marengo-pi refuses a zone without these at load, so a planner never sees one.
    if (above === undefined || speed === undefined) continue;
    cap = {
      zones: [...(cap?.zones ?? []), typeof name === "string" ? name : "unnamed"],
      aboveRad: Math.min(cap?.aboveRad ?? above, above),
      maxVelocityRadS: Math.min(cap?.maxVelocityRadS ?? speed, speed),
    };
  }
  return cap;
}

/** Points from `a` to `b` (both included) at most TAU_SAMPLE_STEP_RAD apart. */
function samplePath(a: number, b: number): number[] {
  const n = Math.max(1, Math.ceil(Math.abs(b - a) / TAU_SAMPLE_STEP_RAD));
  return Array.from({ length: n + 1 }, (_, k) => roundRad(a + ((b - a) * k) / n));
}

/**
 * Distinct joint configurations along every commanded path (other joints 0): each fixed joint
 * from 0 to its pose in chain order with the earlier ones already fixed (the distal-first return
 * retraces these), then the sweep joint over [min, max] of its targets, wave extremes, poses and
 * 0 with every fixed pose held; samples ≤ 0.05 rad apart plus every exact target and pose.
 */
export function guardConfigurations(plan: JointCalPlan): Record<string, number>[] {
  const out = new Map<string, Record<string, number>>();
  const add = (config: Record<string, number>) => {
    const key = plan.chain.map((j) => (config[j] ?? 0).toFixed(6)).join(" ");
    if (!out.has(key)) out.set(key, config);
  };
  const base: Record<string, number> = {};
  for (const [joint, rad] of Object.entries(plan.fixedRad)) {
    for (const q of samplePath(0, rad)) add({ ...base, [joint]: q });
    base[joint] = rad;
  }
  const sweepTargets = [0, ...plan.posesRad];
  for (const s of plan.steps) {
    if (s.joint !== plan.sweepJoint) continue;
    if (s.kind === "wave") sweepTargets.push(s.min_rad, s.max_rad);
    else sweepTargets.push(s.target_rad);
  }
  const lo = Math.min(...sweepTargets);
  const hi = Math.max(...sweepTargets);
  for (const q of [...samplePath(lo, hi), ...sweepTargets]) add({ ...base, [plan.sweepJoint]: q });
  return [...out.values()];
}

/**
 * Largest |τ_g| of the sweep joint at the plan's poses (fixed poses held), from the τ guard's
 * batch over {@link guardConfigurations} (which includes every pose); undefined when one is
 * missing.
 */
export function maxSweepTauAtPoses(
  plan: JointCalPlan,
  configs: readonly Record<string, number>[],
  tau: ReadonlyMap<number, Record<string, number>>,
): number | undefined {
  const key = (c: Record<string, number>) => plan.chain.map((j) => (c[j] ?? 0).toFixed(6)).join(" ");
  const index = new Map(configs.map((c, i) => [key(c), i]));
  let worst = 0;
  for (const pose of plan.posesRad) {
    const i = index.get(key({ ...plan.fixedRad, [plan.sweepJoint]: pose }));
    const t = i === undefined ? undefined : tau.get(i)?.[plan.sweepJoint];
    if (t === undefined) return undefined;
    worst = Math.max(worst, Math.abs(t));
  }
  return worst;
}

/**
 * Remote shell (sole CAN owner, like the gravity gate's preview): one `motor-repl gravity-preview`
 * per configuration, each a full robot.yaml-order vector, every block headed `@@gravcal_pose <i>`.
 */
export function gravityBatchShell(configs: readonly Record<string, number>[], robotJoints: readonly string[]): string {
  const rows = configs.map((c, i) => `'${[i, ...robotJoints.map((j) => c[j] ?? 0)].map(String).join(" ")}'`);
  return [
    `printf '%s\\n' ${rows.join(" ")} | while read -r GB_I GB_Q; do`,
    `  printf '${POSE_MARKER}%s\\n' "$GB_I"`,
    `  bin/motor-repl gravity-preview $GB_Q || printf 'gravity-preview failed for pose %s\\n' "$GB_I"`,
    "done",
  ].join("\n");
}

/** τ_g per configuration index from {@link gravityBatchShell} output. */
export function parseGravityBatch(output: string): Map<number, Record<string, number>> {
  const out = new Map<number, Record<string, number>>();
  let current: Record<string, number> | undefined;
  for (const line of output.split("\n")) {
    if (line.startsWith(POSE_MARKER)) {
      current = {};
      out.set(Number(line.slice(POSE_MARKER.length)), current);
      continue;
    }
    const m = TAU_G_LINE.exec(line.trim());
    if (m && current) current[m[1]] = Number(m[2]);
  }
  return out;
}

function describeConfig(config: Record<string, number>): string {
  const set = Object.entries(config).filter(([, q]) => q !== 0);
  return set.length === 0 ? "all joints at 0" : set.map(([j, q]) => `${j}=${q}`).join(" ");
}

/**
 * For every configuration and guarded joint: factor × |τ_g| ≤ 0.8 × τ_ff cap, the factor from
 * {@link TauFactors} (per joint and configuration). Fails closed on any missing τ_g. On success,
 * the factor report then one summary line per joint (worst case).
 */
export function checkTauGuard(
  configs: readonly Record<string, number>[],
  tau: ReadonlyMap<number, Record<string, number>>,
  joints: readonly string[],
  limits: Readonly<Record<string, JointLimits>>,
  factors: TauFactors,
): { ok: true; report: string[] } | Refusal {
  const report = [
    `τ guard: ${configs.length} configurations along the commanded paths; factor × |τ_g| must stay ≤ ${TAU_CAP_SHARE} × τ_ff cap`,
    ...factors.report,
  ];
  for (const joint of joints) {
    const allowed = TAU_CAP_SHARE * limits[joint].tauFfCapNm;
    let worst = { tau: 0, factor: factors.at(joint, configs[0] ?? {}), index: 0 };
    for (let i = 0; i < configs.length; i += 1) {
      const t = tau.get(i)?.[joint];
      if (t === undefined) {
        return {
          ok: false,
          message: `Refused: τ guard unavailable: gravity-preview gave no τ_g for ${joint} at ${describeConfig(configs[i])}; no motion was run.`,
        };
      }
      const factor = factors.at(joint, configs[i]);
      if (factor * Math.abs(t) > worst.factor * Math.abs(worst.tau)) worst = { tau: t, factor, index: i };
    }
    const factored = worst.factor * Math.abs(worst.tau);
    const line =
      `${joint}: max |τ_g| ${Math.abs(worst.tau).toFixed(3)} Nm at ${describeConfig(configs[worst.index])}; ` +
      `× ${worst.factor} = ${factored.toFixed(3)} Nm vs ${TAU_CAP_SHARE} × cap ${limits[joint].tauFfCapNm} Nm = ${allowed.toFixed(3)} Nm`;
    if (factored > allowed) {
      return {
        ok: false,
        message:
          `Refused: τ guard: ${line}. The factored model torque exceeds the feed-forward budget; ` +
          "reduce amplitude_fraction, poses_rad or the fixed poses. No motion was run.",
      };
    }
    report.push(`  ${line} ok`);
  }
  return { ok: true, report };
}

// ---------------------------------------------------------------------------
// Session outcome

/** Split one CSV line, honouring double-quoted fields. */
function splitCsvLine(line: string): string[] {
  const out: string[] = [];
  let field = "";
  let quoted = false;
  for (let i = 0; i < line.length; i += 1) {
    const c = line[i];
    if (quoted) {
      if (c === '"' && line[i + 1] === '"') {
        field += '"';
        i += 1;
      } else if (c === '"') {
        quoted = false;
      } else {
        field += c;
      }
    } else if (c === '"') {
      quoted = true;
    } else if (c === ",") {
      out.push(field);
      field = "";
    } else {
      field += c;
    }
  }
  out.push(field);
  return out;
}

/**
 * How many plan steps, in order, the position trace shows reached: a hold or fixed step when the
 * joint's `target_raw` changes to its target; a wave when `target_raw` reaches the band's top
 * `cycles` times and then comes back to its bottom.
 */
export function stepsSeenInTrace(steps: readonly JointCalStep[], traceCsv: string): number {
  const [header, ...lines] = traceCsv.split(/\r?\n/).filter((l) => l !== "");
  if (header === undefined) return 0;
  const cols = splitCsvLine(header);
  const jointCol = cols.indexOf("joint");
  const targetCol = cols.indexOf("target_raw");
  if (jointCol < 0 || targetCol < 0) return 0;
  const rows = lines.map(splitCsvLine).map((f) => ({ joint: f[jointCol], target: Number(f[targetCol]) }));
  const last: Record<string, number> = {};
  let row = 0;
  let seen = 0;
  for (const step of steps) {
    let matched = false;
    let peaks = 0;
    let inPeak = false;
    for (; row < rows.length && !matched; row += 1) {
      const { joint, target } = rows[row];
      if (!Number.isFinite(target)) continue;
      const prev = last[joint];
      last[joint] = target;
      if (joint !== step.joint) continue;
      if (step.kind !== "wave") {
        const changed = prev === undefined || Math.abs(prev - target) > TRACE_TARGET_TOL_RAD;
        matched = changed && Math.abs(target - step.target_rad) <= TRACE_TARGET_TOL_RAD;
        continue;
      }
      const tol = Math.max(0.005, 0.05 * (step.max_rad - step.min_rad));
      const atTop = target >= step.max_rad - tol;
      if (atTop && !inPeak) peaks += 1;
      inPeak = atTop;
      matched = peaks >= step.cycles && target <= step.min_rad + tol;
    }
    if (!matched) break;
    seen += 1;
  }
  return seen;
}

// ---------------------------------------------------------------------------
// Tool

const OPERATOR = /^[A-Za-z0-9_-]+$/;

export const jointCalibrateSchema = motionConfirmSchema.extend({
  profile: z.enum(BENCH_PROFILES).default("arm_attached"),
  ...referenceOptInShape,
  skip_hanging_rest_gravity_check: z
    .boolean()
    .default(false)
    .describe(
      "Required true: skip only the gate's hanging-rest |τ_g| refusal (residuals still reported); " +
        "the calibration exists to fix that model",
    ),
  sweep_joint: z.enum(MASTER_JOINTS).describe("The one joint this session moves"),
  method: z
    .enum(CALIBRATION_METHODS)
    .default("wave")
    .describe(
      "wave (default): at each pose a local wave through stiction at 2–3 speeds; gravity-fit takes up/down " +
        "bin means. static: holds approached from below and above, plus velocity_passes",
    ),
  fixed_rad: z
    .record(z.string().regex(JOINT_NAME), z.number())
    .default({})
    .describe("Poses (rad) other profile joints hold during the sweep, set in chain order first; 0 = stay at the reference"),
  poses_rad: z
    .array(z.number())
    .min(MIN_POSES)
    .max(MAX_POSES)
    .optional()
    .describe(
      "Poses (rad); default 5 evenly spaced across amplitude_fraction of the window, kept 0.05 rad plus δ " +
        `(static) or ${MAX_WAVE_AMPLITUDE_RAD} rad (wave) inside the limits`,
    ),
  amplitude_fraction: z
    .union([z.literal(0.25), z.literal(0.5), z.literal(0.9)])
    .default(0.25)
    .describe("Default poses span this share of the soft ∩ hard window, centred on 0, ≥ 0.05 rad inside the limits"),
  wave_amplitude_rad: z
    .number()
    .min(MIN_WAVE_AMPLITUDE_RAD)
    .max(MAX_WAVE_AMPLITUDE_RAD)
    .optional()
    .describe(
      "Wave method: local-wave amplitude a (rad). Default (F_s + 0.6 × max|τ_g| at the poses) / kp from the Pi's " +
        `control.yaml friction fs|fc and impedance kp, ≥ ${MIN_WAVE_AMPLITUDE_RAD}, refused above ${MAX_WAVE_AMPLITUDE_RAD}`,
    ),
  wave_speeds_rad_s: z
    .array(z.number().positive())
    .min(2)
    .max(3)
    .optional()
    .describe(
      `Wave method: 2–3 local-wave peak speeds (rad/s), each ≥ ${MIN_WAVE_PEAK_SPEED_RAD_S}. Default 3 evenly spaced ` +
        `from ${MIN_WAVE_PEAK_SPEED_RAD_S} to the admissible speed (80 % velocity cap, trajectory velocity/accel)`,
    ),
  wave_cycles: z
    .number()
    .int()
    .min(1)
    .max(5)
    .optional()
    .describe(
      `Wave method: cycles per local wave. Default per speed: ≥ ${DEFAULT_WAVE_CYCLES}, enough for gravity-fit's ` +
        `centre bin to hold ${BIN_SAMPLE_MARGIN} × ${FIT_MIN_BIN_SAMPLES} samples per direction`,
    ),
  approach_offset_rad: z
    .number()
    .min(0.02)
    .max(0.15)
    .default(DEFAULT_APPROACH_OFFSET_RAD)
    .describe("Static method: overshoot δ before each pass"),
  settle_sec: z
    .number()
    .min(1)
    .max(10)
    .default(DEFAULT_SETTLE_SEC)
    .describe("Settle after each hold-at (static holds, wave parks, fixed poses)"),
  measure_sec: z.number().min(1).max(5).default(DEFAULT_MEASURE_SEC).describe("Static method: measured hold tail"),
  velocity_passes: z
    .object({
      min_rad: z.number(),
      max_rad: z.number(),
      half_periods_s: z.array(z.number().positive()).max(6).describe("One wave per entry; [] = no velocity passes"),
      cycles: z.number().int().min(1).max(5),
    })
    .optional()
    .describe(
      "Static method only: in-loop `wave` passes for friction vs speed; default ≤ 0.3 rad mid-band at 25/50/75 % of " +
        "the admissible speed, 2 cycles",
    ),
  return_home_sec: z.number().int().min(5).max(120).default(DEFAULT_RETURN_HOME_SEC),
  config_dir: z
    .string()
    .optional()
    .describe("MARENGO_CONFIG_DIR override (default: master /opt/marengo/config)"),
  operator: z.string().regex(OPERATOR).default("bench"),
  run_fit: z.boolean().default(true).describe("Run marengo-log-cli gravity-fit over the session dir afterwards"),
});

export type JointCalibrateArgs = Partial<z.infer<typeof jointCalibrateSchema>> & { confirm: true };

export function registerJointCalibrateTools(
  cfg: MarengoPiConfig,
  runRemote: RunRemote,
  auditMotion: AuditMotion,
  deps: CalibrationDeps = defaultCalibrationDeps,
) {
  return {
    pi_joint_calibrate: {
      description:
        "Right-arm calibration session (docs/commissioning/right-arm-calibration-suite.md) in ONE marengo-pi " +
        "session: `home <profile joints> sign-tested` (awaited), home, enable (awaited), hold-at each fixed_rad pose " +
        "(chain order), then sweep_joint (any right-arm joint) per method, then every profile joint back to 0 " +
        "distal first, disable, quit. method wave (default): at each pose a park hold-at pose−a, then a local " +
        "`wave` [pose−a, pose+a] at 2–3 peak speeds; a = (F_s + 0.6 × max|τ_g|)/kp from the Pi's control.yaml " +
        "unless wave_amplitude_rad. method static: holds approached from below (after a min−δ overshoot) and " +
        "from above (after a max+δ overshoot), then `wave` velocity passes. Default poses: 5 across " +
        "amplitude_fraction (0.25|0.5|0.9) of the soft ∩ hard window, centred on 0. Needs confirm + " +
        "confirm_weighted_motion (default profile arm_attached), set_zero + at_mechanical_reference, " +
        "skip_hanging_rest_gravity_check: true. Pre-flight guard, before any motion, on the Pi's live config: " +
        "every target, overshoot, fixed pose and wave extreme ≥ 0.05 rad inside [max(soft, hard) lower, " +
        "min(soft, hard) upper]; model |τ_g| × factor ≤ 0.8 × τ_ff cap on every joint along every commanded path " +
        "(factor 1.6, or 1 + max(3σ_A/A, 0.15) for a joint whose fit docs/commissioning/calibrations/" +
        "applied-gravity.json pins to the Pi URDF; τ_g from motor-repl gravity-preview, batched; wave method: " +
        "±0.1 rad around each pose); wave peak speed " +
        "π·(max−min)/(2·half_period) ≤ 0.8 × velocity cap; total sleep budget ≤ 300 s. Writes " +
        "var/gravity-calibration/<TS>/ (plan.json version 2 with method and session_complete, " +
        "position-trace.csv, pi-marengo.urdf, config/*.yaml, bench-session.txt) and, with run_fit, runs " +
        "`marengo-log-cli gravity-fit` (completed steps only). Never applies anything (ADR 0017). " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: jointCalibrateSchema,
      handler: async (args: JointCalibrateArgs): Promise<string> => {
        const profile = args.profile ?? "arm_attached";
        const check = validateMotionConfirm({ ...args, profile }, cfg.benchProfile);
        if (!check.ok) return check.message;
        if (args.set_zero !== true || args.at_mechanical_reference !== true) return REFERENCE_OPT_IN_REQUIRED;
        if (args.skip_hanging_rest_gravity_check !== true) return skipHangingRestRequired(TOOL);
        const sweepJoint = args.sweep_joint;
        if (sweepJoint === undefined) return "Refused: sweep_joint is required.";
        const operator = args.operator ?? "bench";
        if (!OPERATOR.test(operator)) return "Refused: operator must match ^[A-Za-z0-9_-]+$.";
        const chain = profileMeta(profile).setZeroJoints;
        const fixedInput = args.fixed_rad ?? {};
        const unreferenced = [sweepJoint, ...Object.keys(fixedInput)].filter((j) => !chain.includes(j));
        if (unreferenced.length > 0) {
          return `Refused: bench profile ${profile} does not reference ${unreferenced.join(", ")}; pick a profile whose joints include the sweep and fixed joints.`;
        }

        const configDir = benchConfigDirForJoint(cfg, sweepJoint, args.config_dir) ?? BENCH_CONFIG_MASTER;
        const refused = (message: string) => {
          auditMotion(TOOL, args, message, 1);
          return message;
        };
        const preflight = parsePreflight(
          await runRemote(wrapRemoteWithConfig(cfg, preflightReadShell(), configDir), PREFLIGHT_TIMEOUT_MS),
          cfg.piRoot,
        );
        if (!preflight.ok) return refused(preflight.message);
        const read = readJointLimits(preflight, chain);
        if (!read.ok) return refused(read.message);
        const factors = await resolveTauFactors({
          urdf: preflight.urdf,
          joints: chain,
          readLocal: (rel) => deps.readFile(path.join(cfg.localRoot, rel)),
        });

        const method = args.method ?? "wave";
        const approachOffsetRad = args.approach_offset_rad ?? DEFAULT_APPROACH_OFFSET_RAD;
        const settleSec = args.settle_sec ?? DEFAULT_SETTLE_SEC;
        const measureSec = args.measure_sec ?? DEFAULT_MEASURE_SEC;
        const planInput = {
          sweepJoint,
          chain,
          limits: read.limits,
          fixedRad: fixedInput,
          posesRad: args.poses_rad,
          amplitudeFraction: args.amplitude_fraction ?? AMPLITUDE_FRACTIONS[0],
          approachOffsetRad,
          method,
          velocityPasses: args.velocity_passes,
        };
        const scriptWithin = (p: JointCalPlan): { ok: true; script: string[]; budgetSec: number } | Refusal => {
          const script = jointCalibrationScript(p, {
            operator,
            settleSec,
            measureSec,
            returnHomeSec: args.return_home_sec ?? DEFAULT_RETURN_HOME_SEC,
          });
          const budgetSec = scriptSleepTotalSec(script);
          if (budgetSec <= MAX_SESSION_SLEEP_SEC) return { ok: true, script, budgetSec };
          return {
            ok: false,
            message:
              `Refused: session budget ${budgetSec} s (sleeps + reference acquisition) exceeds ${MAX_SESSION_SLEEP_SEC} s. ` +
              "Split it into several pi_joint_calibrate sessions (e.g. half the poses each, or holds in one and " +
              "velocity_passes in another), or use fewer poses, speeds or cycles, or shorter settle_sec/measure_sec.",
          };
        };
        const tauBatch = async (configs: Record<string, number>[]) =>
          parseGravityBatch(
            await runRemote(
              wrapRemoteWithConfig(cfg, soleCanOwnerShell(gravityBatchShell(configs, read.robotJoints)), configDir),
              TAU_GUARD_TIMEOUT_MS + CAN_SESSION_SLACK_MS,
            ),
          );

        let plan: JointCalPlan;
        let session: { script: string[]; budgetSec: number };
        let preamble: string[];
        if (method === "static") {
          const planned = planJointCalibration(planInput);
          if (!planned.ok) return refused(planned.message);
          plan = planned;
          const limitsOk = checkJointCalLimits(plan, read.limits);
          if (!limitsOk.ok) return refused(limitsOk.message);
          const speedsOk = checkWaveSpeeds(plan, read.limits);
          if (!speedsOk.ok) return refused(speedsOk.message);
          const within = scriptWithin(plan);
          if (!within.ok) return refused(within.message);
          session = within;
          const configs = guardConfigurations(plan);
          const tauGuard = checkTauGuard(configs, await tauBatch(configs), chain, read.limits, factors);
          if (!tauGuard.ok) return refused(tauGuard.message);
          preamble = tauGuard.report;
        } else {
          // The τ guard covers the largest admissible amplitude around every pose (a superset of
          // the final paths) and supplies τ_g at the poses for the derived amplitude.
          const envelope = planJointCalibration({
            ...planInput,
            waves: { amplitudeRad: MAX_WAVE_AMPLITUDE_RAD, passes: [{ half_period_s: 1, cycles: 1 }] },
          });
          if (!envelope.ok) return refused(envelope.message);
          const configs = guardConfigurations(envelope);
          const tau = await tauBatch(configs);
          const tauGuard = checkTauGuard(configs, tau, chain, read.limits, factors);
          if (!tauGuard.ok) return refused(tauGuard.message);
          const sweepLimits = read.limits[sweepJoint];
          let amplitudeRad: number;
          let basis: string;
          if (args.wave_amplitude_rad !== undefined) {
            amplitudeRad = args.wave_amplitude_rad;
            basis = `wave amplitude ${amplitudeRad} rad (wave_amplitude_rad)`;
          } else {
            const maxTau = maxSweepTauAtPoses(envelope, configs, tau);
            if (maxTau === undefined) {
              return refused(`Refused: no τ_g for ${sweepJoint} at every pose; the wave amplitude cannot be derived. No motion was run.`);
            }
            const derived = deriveWaveAmplitude(sweepJoint, sweepLimits, maxTau);
            if (!derived.ok) return refused(derived.message);
            ({ amplitudeRad, basis } = derived);
          }
          if (read.loopHz === undefined || !(read.loopHz > 0)) {
            return refused("Refused: Pi control.yaml control.loop_hz missing; wave cycles cannot be sized. No motion was run.");
          }
          const waves = localWaves(
            sweepJoint,
            sweepLimits,
            amplitudeRad,
            read.loopHz,
            args.wave_speeds_rad_s,
            envelope.posesRad,
          );
          if (!waves.ok) return refused(waves.message);
          if (args.wave_cycles !== undefined) {
            for (const p of waves.waves.passes) p.cycles = args.wave_cycles;
          }
          const planned = planJointCalibration({ ...planInput, waves: waves.waves });
          if (!planned.ok) return refused(planned.message);
          plan = planned;
          for (const check of [
            checkWavePoseSpacing(plan),
            checkJointCalLimits(plan, read.limits),
            checkWaveSpeeds(plan, read.limits),
          ]) {
            if (!check.ok) return refused(check.message);
          }
          const within = scriptWithin(plan);
          if (!within.ok) return refused(within.message);
          session = within;
          const speeds = waves.waves.passes.map(
            (p) => `${wavePeakSpeed(-amplitudeRad, amplitudeRad, p.half_period_s).toFixed(3)} rad/s × ${p.cycles}`,
          );
          preamble = [...tauGuard.report, basis, `local waves per pose: ${speeds.join(", ")} cycles`];
        }
        const { script, budgetSec } = session;

        return runCalibrationSession(cfg, runRemote, auditMotion, deps, {
          tool: TOOL,
          label: "joint-calibrate",
          args,
          profile,
          gateJoints: chain,
          configDir,
          script,
          budgetSec,
          preflight,
          preamble,
          runFit: args.run_fit !== false,
          fitParams: [],
          fitIncomplete: true,
          fullRateJoints: [sweepJoint],
          plan: ({ sessionTs, gateReport, sessionExit, trace }) => {
            const complete =
              sessionExit === 0 && trace !== undefined && stepsSeenInTrace(plan.steps, trace) === plan.steps.length;
            return {
              complete,
              json: {
                version: 2,
                created_utc: deps.now().toISOString(),
                session_ts: sessionTs,
                profile,
                method: plan.method,
                sweep_joint: plan.sweepJoint,
                fixed_rad: plan.fixedRad,
                poses_rad: plan.posesRad,
                ...(plan.method === "wave"
                  ? { wave_amplitude_rad: plan.waveAmplitudeRad }
                  : { approach_offset_rad: approachOffsetRad }),
                settle_sec: settleSec,
                ...(plan.method === "static" ? { measure_sec: measureSec } : {}),
                steps: plan.steps,
                gravity_gate_report: gateReport,
                session_complete: complete,
              },
            };
          },
        });
      },
    },
  };
}
