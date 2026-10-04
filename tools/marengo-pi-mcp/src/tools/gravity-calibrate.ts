/**
 * pi_gravity_calibrate: one marengo-pi session sweeping a right-arm joint through static
 * poses (approached from below, then from above) while the position trace records τ_meas.
 * The workstation fitter (`marengo-log-cli gravity-fit`) turns the captured directory into a
 * proposed URDF inertial patch. Nothing is ever applied to the Pi here (ADR 0017).
 */

import { z } from "zod";
import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { profileMeta } from "../bench-profiles.js";
import { wrapRemoteWithConfig } from "../env.js";
import { effectiveProfile, validateMotionConfirm } from "../safety.js";
import {
  type AuditMotion,
  type CalibrationDeps,
  type JointWindow,
  MAX_SESSION_SLEEP_SEC,
  type Refusal,
  type RunRemote,
  type SweepHold,
  checkTargetsInWindows,
  defaultCalibrationDeps,
  holdSweepSteps,
  jointWindows,
  parsePreflight,
  preflightReadShell,
  roundRad,
  runCalibrationSession,
  skipHangingRestRequired,
  validatePoses,
} from "./calibration-session.js";
import {
  BENCH_CONFIG_MASTER,
  REFERENCE_OPT_IN_REQUIRED,
  SOLE_CAN_OWNER_NOTE,
  benchConfigDirForJoint,
  motionConfirmSchema,
  referenceAcquireLine,
  referenceOptInShape,
  scriptSleepTotalSec,
} from "./motion.js";

const PITCH = "right_shoulder_pitch";
const ELBOW = "right_elbow_pitch";

export const SWEEP_JOINTS = [PITCH, ELBOW] as const;
export type SweepJoint = (typeof SWEEP_JOINTS)[number];

export const DEFAULT_POSES_RAD: Record<SweepJoint, readonly number[]> = {
  [PITCH]: [0, 0.25, 0.48, 0.8, 1.2],
  [ELBOW]: [0, 0.25, 0.5, 0.75],
};

const DEFAULT_APPROACH_OFFSET_RAD = 0.05;
const DEFAULT_SETTLE_SEC = 2.5;
const DEFAULT_MEASURE_SEC = 1.5;
const DEFAULT_RETURN_HOME_SEC = 6;
/** Dwell between returning the swept elbow to 0 and returning a non-zero fixed pitch. */
const ELBOW_RETURN_DWELL_SEC = 2;
const MIN_POSES = 2;
const MAX_POSES = 12;
const MIN_PITCH_POSES = 3;
const PREFLIGHT_TIMEOUT_MS = 20_000;

export const SKIP_HANGING_REST_REQUIRED = skipHangingRestRequired("pi_gravity_calibrate");

export type CalibrationStep = SweepHold;

export interface SweepPlan {
  sweepJoint: SweepJoint;
  posesRad: number[];
  fixedRad: Record<string, number>;
  steps: CalibrationStep[];
}

/**
 * Poses validated and sorted, then the step plan ({@link holdSweepSteps}). An elbow sweep with a
 * non-zero fixed pitch first moves the pitch there.
 */
export function planCalibrationSweep(input: {
  sweepJoint: SweepJoint;
  posesRad: readonly number[];
  fixedPitchRad: number;
  approachOffsetRad: number;
}): ({ ok: true } & SweepPlan) | Refusal {
  const { sweepJoint, fixedPitchRad, approachOffsetRad: delta } = input;
  const refuse = (message: string): Refusal => ({ ok: false, message: `Refused: ${message}` });
  if (sweepJoint === PITCH && fixedPitchRad !== 0) {
    return refuse(
      `fixed_pitch_rad (${fixedPitchRad}) applies only to a right_elbow_pitch sweep; a right_shoulder_pitch sweep moves the pitch itself`,
    );
  }
  if (!Number.isFinite(fixedPitchRad)) return refuse("fixed_pitch_rad must be finite");
  if (!Number.isFinite(delta) || delta <= 0) return refuse("approach_offset_rad must be positive");
  const valid = validatePoses(input.posesRad, MIN_POSES, MAX_POSES);
  if (!valid.ok) return valid;
  const poses = valid.poses;
  if (sweepJoint === PITCH && poses.length < MIN_PITCH_POSES) {
    return refuse(`a right_shoulder_pitch sweep needs at least ${MIN_PITCH_POSES} distinct poses`);
  }

  const steps: CalibrationStep[] = [];
  if (sweepJoint === ELBOW && fixedPitchRad !== 0) {
    steps.push({ joint: PITCH, target_rad: fixedPitchRad, measure: false });
  }
  steps.push(...holdSweepSteps(sweepJoint, poses, delta));
  for (let i = 1; i < steps.length; i += 1) {
    if (steps[i].joint === steps[i - 1].joint && steps[i].target_rad === steps[i - 1].target_rad) {
      return refuse(`plan repeats hold-at ${steps[i].joint} ${steps[i].target_rad} on consecutive steps`);
    }
  }
  return {
    ok: true,
    sweepJoint,
    posesRad: poses,
    fixedRad: sweepJoint === ELBOW ? { [PITCH]: fixedPitchRad } : {},
    steps,
  };
}

/** marengo-pi stdin script for the whole calibration session (one process). */
export function calibrationSessionScript(
  plan: SweepPlan,
  opts: {
    referenceJoints: readonly string[];
    operator: string;
    settleSec: number;
    measureSec: number;
    returnHomeSec: number;
  },
): string[] {
  const dwell = roundRad(opts.settleSec + opts.measureSec);
  const script = [referenceAcquireLine(opts.referenceJoints), "home", `enable ${opts.operator}`];
  for (const step of plan.steps) {
    script.push(`hold-at ${step.joint} ${String(step.target_rad)}`, `sleep ${dwell}`);
  }
  script.push(`hold-at ${plan.sweepJoint} 0`);
  if (plan.sweepJoint === ELBOW && (plan.fixedRad[PITCH] ?? 0) !== 0) {
    script.push(`sleep ${ELBOW_RETURN_DWELL_SEC}`, `hold-at ${PITCH} 0`);
  }
  script.push(`sleep ${opts.returnHomeSec}`, "status", "disable", "quit");
  return script;
}

/** Joints whose pose window the plan needs (sweep joint, plus the pitch for an elbow sweep). */
export function planJoints(plan: SweepPlan): string[] {
  return plan.sweepJoint === ELBOW ? [PITCH, ELBOW] : [PITCH];
}

/** Every commanded target (steps, fixed pitch, return pose 0) inside its joint's window. */
export function checkPlanLimits(
  plan: SweepPlan,
  windows: Readonly<Record<string, JointWindow>>,
): { ok: true } | Refusal {
  return checkTargetsInWindows(
    [
      ...plan.steps.map((s, i) => ({ joint: s.joint, rad: s.target_rad, what: `step ${i} target`, inset: 0 })),
      ...Object.entries(plan.fixedRad).map(([joint, rad]) => ({ joint, rad, what: "fixed pose", inset: 0 })),
      ...planJoints(plan).map((joint) => ({ joint, rad: 0, what: "return pose", inset: 0 })),
    ],
    windows,
  );
}

// ---------------------------------------------------------------------------
// Tool

const FIT_PARAM = /^(mass|com):[A-Za-z0-9_]+$/;

export const gravityCalibrateSchema = motionConfirmSchema.extend({
  ...referenceOptInShape,
  skip_hanging_rest_gravity_check: z
    .boolean()
    .default(false)
    .describe(
      "Required true: skip only the gate's hanging-rest |τ_g| refusal (residuals still reported); " +
        "the calibration exists to fix that model",
    ),
  sweep_joint: z.enum(SWEEP_JOINTS).default(PITCH),
  poses_rad: z
    .array(z.number())
    .optional()
    .describe("Static poses (rad); default pitch [0,0.25,0.48,0.8,1.2], elbow [0,0.25,0.5,0.75]"),
  fixed_pitch_rad: z
    .number()
    .default(0)
    .describe("right_shoulder_pitch pose held during an elbow sweep (elbow sweep only)"),
  approach_offset_rad: z.number().min(0.02).max(0.15).default(DEFAULT_APPROACH_OFFSET_RAD),
  settle_sec: z.number().min(1).max(10).default(DEFAULT_SETTLE_SEC),
  measure_sec: z.number().min(1).max(5).default(DEFAULT_MEASURE_SEC),
  return_home_sec: z.number().int().min(5).max(120).default(DEFAULT_RETURN_HOME_SEC),
  config_dir: z
    .string()
    .optional()
    .describe("MARENGO_CONFIG_DIR override (default: master /opt/marengo/config)"),
  operator: z.string().default("bench"),
  run_fit: z.boolean().default(true).describe("Run marengo-log-cli gravity-fit on the workstation afterwards"),
  fit_params: z
    .array(z.string().regex(FIT_PARAM))
    .optional()
    .describe("Fitter parameters `mass:<link>` / `com:<link>` (default: fitter's choice)"),
});

export type GravityCalibrateArgs = Partial<z.infer<typeof gravityCalibrateSchema>> & {
  confirm: true;
};

export function registerGravityCalibrateTools(
  cfg: MarengoPiConfig,
  runRemote: RunRemote,
  auditMotion: AuditMotion,
  deps: CalibrationDeps = defaultCalibrationDeps,
) {
  return {
    pi_gravity_calibrate: {
      description:
        "Right-arm gravity-model calibration sweep in ONE marengo-pi session: `home <profile joints> sign-tested` " +
        "(awaited), home and enable (each awaited; a refusal ends the session with disable/quit), then hold-at each static pose of sweep_joint (right_shoulder_pitch, or " +
        "right_elbow_pitch with the pitch at fixed_pitch_rad) approached from below (up pass after a min−δ " +
        "overshoot) and from above (down pass after a max+δ overshoot), dwelling settle_sec + measure_sec at each, " +
        "then return to 0 and disable. Needs confirm (+ confirm_weighted_motion on weighted profiles), " +
        "set_zero + at_mechanical_reference (SetZero at the current pose), and skip_hanging_rest_gravity_check: " +
        "true — the gravity gate's hanging-rest |τ_g| refusal is skipped only for this tool because the " +
        "calibration exists to fix that model; residuals are still reported, and an unavailable preview or a " +
        "measured-torque mismatch still refuses. Before any motion a read-only pre-flight reads the Pi " +
        "robot/control/motors.yaml and URDF and refuses if any target (overshoots included), the fixed pitch or " +
        "the return pose 0 lies outside [max(soft, hard) lower, min(soft, hard) upper]; total sleep budget ≤ 300 s. " +
        "Writes var/gravity-calibration/<TS>/ locally (plan.json, position-trace.csv, pi-marengo.urdf, " +
        "config/*.yaml, bench-session.txt) and, with run_fit, runs `marengo-log-cli gravity-fit` to propose a URDF " +
        "inertial patch. Never applies anything: pi_sync_bench_urdf is a separate explicit step after review " +
        "(ADR 0017). " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: gravityCalibrateSchema,
      handler: async (args: GravityCalibrateArgs): Promise<string> => {
        const check = validateMotionConfirm(args, cfg.benchProfile);
        if (!check.ok) return check.message;
        if (args.set_zero !== true || args.at_mechanical_reference !== true) {
          return REFERENCE_OPT_IN_REQUIRED;
        }
        if (args.skip_hanging_rest_gravity_check !== true) return SKIP_HANGING_REST_REQUIRED;

        const profile: BenchProfile = effectiveProfile(cfg.benchProfile, args.profile);
        const referenceJoints = profileMeta(profile).setZeroJoints;
        const sweepJoint = args.sweep_joint ?? PITCH;
        const plan = planCalibrationSweep({
          sweepJoint,
          posesRad: args.poses_rad ?? DEFAULT_POSES_RAD[sweepJoint],
          fixedPitchRad: args.fixed_pitch_rad ?? 0,
          approachOffsetRad: args.approach_offset_rad ?? DEFAULT_APPROACH_OFFSET_RAD,
        });
        if (!plan.ok) return plan.message;
        const unreferenced = planJoints(plan).filter((j) => !referenceJoints.includes(j));
        if (unreferenced.length > 0) {
          return `Refused: bench profile ${profile} does not reference ${unreferenced.join(", ")}; pick a profile whose joints include the sweep.`;
        }
        const settleSec = args.settle_sec ?? DEFAULT_SETTLE_SEC;
        const measureSec = args.measure_sec ?? DEFAULT_MEASURE_SEC;
        const script = calibrationSessionScript(plan, {
          referenceJoints,
          operator: args.operator ?? "bench",
          settleSec,
          measureSec,
          returnHomeSec: args.return_home_sec ?? DEFAULT_RETURN_HOME_SEC,
        });
        const budgetSec = scriptSleepTotalSec(script);
        if (budgetSec > MAX_SESSION_SLEEP_SEC) {
          return (
            `Refused: session budget ${budgetSec} s (sleeps + reference acquisition) exceeds ${MAX_SESSION_SLEEP_SEC} s; ` +
            "use fewer poses or shorter settle_sec/measure_sec."
          );
        }

        const configDir = benchConfigDirForJoint(cfg, sweepJoint, args.config_dir) ?? BENCH_CONFIG_MASTER;
        const preflight = parsePreflight(
          await runRemote(wrapRemoteWithConfig(cfg, preflightReadShell(), configDir), PREFLIGHT_TIMEOUT_MS),
          cfg.piRoot,
        );
        if (!preflight.ok) {
          auditMotion("pi_gravity_calibrate", args, preflight.message, 1);
          return preflight.message;
        }
        const windows = jointWindows(preflight.controlYaml, preflight.motorsYaml, planJoints(plan));
        const limits = windows.ok ? checkPlanLimits(plan, windows.windows) : windows;
        if (!limits.ok) {
          auditMotion("pi_gravity_calibrate", args, limits.message, 1);
          return limits.message;
        }

        return runCalibrationSession(cfg, runRemote, auditMotion, deps, {
          tool: "pi_gravity_calibrate",
          label: "gravity-calibrate",
          args,
          profile,
          gateJoints: [...new Set([...referenceJoints, ...planJoints(plan)])],
          configDir,
          script,
          budgetSec,
          preflight,
          preamble: [],
          runFit: args.run_fit !== false,
          fitParams: args.fit_params ?? [],
          fitIncomplete: false,
          plan: ({ sessionTs, gateReport, sessionExit }) => ({
            complete: sessionExit === 0,
            json: {
              version: 1,
              created_utc: deps.now().toISOString(),
              session_ts: sessionTs,
              profile,
              sweep_joint: plan.sweepJoint,
              fixed_rad: plan.fixedRad,
              poses_rad: plan.posesRad,
              approach_offset_rad: args.approach_offset_rad ?? DEFAULT_APPROACH_OFFSET_RAD,
              settle_sec: settleSec,
              measure_sec: measureSec,
              steps: plan.steps,
              gravity_gate_report: gateReport,
            },
          }),
        });
      },
    },
  };
}
