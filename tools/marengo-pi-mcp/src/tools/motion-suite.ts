/**
 * pi_motion_suite: a thorough single-joint motion test (ADR 0039 bench scoring), split into
 * marengo-pi sessions that each fit the 300 s session budget. Every session references the
 * profile, homes, enables, runs its motion blocks, returns every joint to 0 and disables; the
 * position trace (swept joint every tick) is then scored per move by
 * scripts/analyze-position-trace.py --score-bench.
 *
 * hold-at always runs at the configured planner speed, so a move at a chosen speed is a
 * single-cycle `wave` a→b→a (the move plus its return) whose peak speed is a share of what
 * marengo-pi admits for that span.
 *
 * Guards (all sessions, before any motion): every target ≥ 0.05 rad inside soft ∩ hard, wave
 * speeds and accelerations within the admission limits, model |τ_g| × 1.6 ≤ 0.8 × τ_ff cap
 * along every commanded path, session budget ≤ 300 s; the gravity gate before each enable keeps
 * its hanging-rest refusal.
 */

import path from "node:path";
import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { BENCH_PROFILES, MASTER_JOINTS, profileMeta } from "../bench-profiles.js";
import { wrapRemoteWithConfig } from "../env.js";
import { soleCanOwnerShell } from "../can-owner.js";
import { validateMotionConfirm } from "../safety.js";
import {
  type AuditMotion,
  type CalibrationDeps,
  type JointWindow,
  MAX_SESSION_SLEEP_SEC,
  type Refusal,
  type RunRemote,
  defaultCalibrationDeps,
  parsePreflight,
  preflightReadShell,
  roundRad,
  runCalibrationSession,
} from "./calibration-session.js";
import {
  type JointCalPlan,
  type JointCalStep,
  type JointLimits,
  LIMIT_INSET_RAD,
  TAU_CAP_SHARE,
  TAU_DISTRUST_FACTOR,
  TAU_SAMPLE_STEP_RAD,
  admissibleWaveSpeed,
  checkJointCalLimits,
  checkTauGuard,
  checkWaveSpeeds,
  gravityBatchShell,
  guardConfigurations,
  parseGravityBatch,
  readJointLimits,
  stepsSeenInTrace,
  wavePeakSpeed,
} from "./joint-calibrate.js";
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

const TOOL = "pi_motion_suite";

export const SUITE_SESSIONS = [
  "long_moves",
  "sweeps_and_reversals",
  "short_moves",
  "gravity_extremes",
  "repeatability",
] as const;
export type SuiteSession = (typeof SUITE_SESSIONS)[number];

/** Narrowest usable window the suite runs in (rad). */
export const MIN_USABLE_WIDTH_RAD = 0.2;
/** A gravity extreme closer than this to the rest pose is no extreme (rad). */
export const MIN_EXTREME_OFFSET_RAD = 0.15;
const WAVE_END_SLACK_SEC = 0.5;
/** Extra wait after the estimated travel of a hold-at, for the governor and catch-up. */
const MOVE_SLACK_SEC = 0.5;
const DEFAULT_SPAN_FRACTIONS = [0.25, 0.5, 0.9];
const DEFAULT_SPEED_FRACTIONS = [0.25, 0.5, 0.9];
const DEFAULT_SHORT_MOVES_RAD = [0.02, 0.05, 0.1];
const DEFAULT_HOLD_SEC = 10;
const DEFAULT_REPEAT_COUNT = 5;
const DEFAULT_SETTLE_SEC = 2.5;
const DEFAULT_RETURN_HOME_SEC = 6;
const PREFLIGHT_TIMEOUT_MS = 20_000;
const TAU_GUARD_TIMEOUT_MS = 60_000;
const SCORER_TIMEOUT_MS = 120_000;
const SCORER = "scripts/analyze-position-trace.py";
const SCORE_HEADER = "=== ADR 0039 bench score:";
const OPERATOR = /^[A-Za-z0-9_-]+$/;

const ceilTenth = (x: number) => Math.ceil(x * 10 - 1e-9) / 10;
const ceilMrad = (x: number) => Math.ceil(x * 1000 - 1e-9) / 1000;
const floorMrad = (x: number) => Math.floor(x * 1000 + 1e-9) / 1000;

// ---------------------------------------------------------------------------
// Geometry

export interface SuiteGeometry {
  joint: string;
  /** Usable window U: soft ∩ hard inset by 0.05 rad, narrowed where the τ guard would refuse. */
  lower: number;
  upper: number;
  lowerBound: "limit" | "tau";
  upperBound: "limit" | "tau";
  /** Largest |τ_g| pose on each side of 0 inside U, when ≥ 0.15 rad from 0. */
  gLo?: number;
  gHi?: number;
}

/** The sweep joint's τ grid: every 0.05 rad of the inset window plus its edges and 0. */
export function tauGridSamples(window: JointWindow): number[] {
  const lo = window.lower + LIMIT_INSET_RAD;
  const hi = window.upper - LIMIT_INSET_RAD;
  const out = new Set<number>([roundRad(lo), roundRad(hi)]);
  if (lo <= 0 && 0 <= hi) out.add(0);
  for (let k = Math.ceil(lo / TAU_SAMPLE_STEP_RAD); k * TAU_SAMPLE_STEP_RAD <= hi; k += 1) {
    out.add(roundRad(k * TAU_SAMPLE_STEP_RAD));
  }
  return [...out].sort((a, b) => a - b);
}

/**
 * U = the run of grid samples around 0 where every chain joint passes the τ guard
 * (1.6 · |τ_g| ≤ 0.8 · τ_ff cap), and the gravity extremes inside it. Fails closed on a missing τ.
 */
export function suiteGeometry(
  joint: string,
  samples: readonly number[],
  tau: readonly (Record<string, number> | undefined)[],
  chain: readonly string[],
  limits: Readonly<Record<string, JointLimits>>,
): ({ ok: true } & SuiteGeometry) | Refusal {
  const zero = samples.indexOf(0);
  if (zero < 0) {
    return { ok: false, message: `Refused: ${joint}'s rest pose 0 is not ${LIMIT_INSET_RAD} rad inside its window; no motion was run.` };
  }
  const passes = (i: number): boolean | undefined => {
    const t = tau[i];
    if (t === undefined) return undefined;
    for (const j of chain) {
      if (t[j] === undefined) return undefined;
      if (TAU_DISTRUST_FACTOR * Math.abs(t[j]) > TAU_CAP_SHARE * limits[j].tauFfCapNm) return false;
    }
    return true;
  };
  for (let i = 0; i < samples.length; i += 1) {
    if (passes(i) === undefined) {
      return { ok: false, message: `Refused: τ guard unavailable: gravity-preview gave no τ_g at ${joint}=${samples[i]}; no motion was run.` };
    }
  }
  if (!passes(zero)) {
    return { ok: false, message: `Refused: τ guard fails at the rest pose ${joint}=0; no motion was run.` };
  }
  let lo = zero;
  while (lo > 0 && passes(lo - 1)) lo -= 1;
  let hi = zero;
  while (hi < samples.length - 1 && passes(hi + 1)) hi += 1;
  // To the mrad, inward: stdin targets stay short and never leave the guarded samples.
  const lower = ceilMrad(samples[lo]);
  const upper = floorMrad(samples[hi]);
  if (upper - lower < MIN_USABLE_WIDTH_RAD) {
    return {
      ok: false,
      message: `Refused: ${joint} usable window [${lower}, ${upper}] rad is narrower than ${MIN_USABLE_WIDTH_RAD} rad (limits and τ guard); no motion was run.`,
    };
  }
  const extreme = (from: number, to: number): number | undefined => {
    let best: number | undefined;
    for (let i = from; i <= to; i += 1) {
      if (best === undefined || Math.abs(tau[i]?.[joint] ?? 0) > Math.abs(tau[best]?.[joint] ?? 0)) best = i;
    }
    if (best === undefined || Math.abs(samples[best]) < MIN_EXTREME_OFFSET_RAD) return undefined;
    return samples[best] < 0 ? ceilMrad(samples[best]) : floorMrad(samples[best]);
  };
  return {
    ok: true,
    joint,
    lower,
    upper,
    lowerBound: lo === 0 ? "limit" : "tau",
    upperBound: hi === samples.length - 1 ? "limit" : "tau",
    gLo: zero > lo ? extreme(lo, zero - 1) : undefined,
    gHi: hi > zero ? extreme(zero + 1, hi) : undefined,
  };
}

/** Band of width `fraction` × |U| centred on 0, shifted to fit U, to the mrad inside. */
export function suiteBand(g: SuiteGeometry, fraction: number): [number, number] {
  const width = fraction * (g.upper - g.lower);
  const start = Math.min(Math.max(-width / 2, g.lower), g.upper - width);
  return [ceilMrad(start), floorMrad(start + width)];
}

/** Half period (0.01 s, rounded up) of a single-cycle wave over `span` at `share` × v_adm(span). */
export function suiteWaveHalfPeriod(limits: JointLimits, span: number, share: number): number {
  const v = share * admissibleWaveSpeed(limits, span);
  return Math.ceil(((Math.PI * span) / (2 * v)) * 100 - 1e-9) / 100;
}

/** Time a rest-to-rest trapezoid at speed v and accel a needs for distance d. */
export function trapezoidTime(d: number, v: number, a: number): number {
  return d >= (v * v) / a ? d / v + v / a : 2 * Math.sqrt(d / a);
}

/** Time from rest to cover d (the first half of a reversal), to the nearest 0.05 s (≥ 0.05). */
export function reversalSleep(d: number, v: number, a: number): number {
  const ramp = (v * v) / (2 * a);
  const t = d <= ramp ? Math.sqrt((2 * d) / a) : (d - ramp) / v + v / a;
  return Math.max(0.05, Math.round(t * 20) / 20);
}

interface Motion {
  slew: number;
  threshold: number;
  v: number;
  a: number;
}

/** Planner travel estimate for a hold-at over d (slew speed up to the threshold). */
function travelSec(m: Motion, d: number): number {
  if (d <= 1e-9) return 0;
  return d <= m.threshold ? d / m.slew + m.slew / m.a : trapezoidTime(d, m.v, m.a);
}

// ---------------------------------------------------------------------------
// Plan

/** One commanded motion: a hold-at dwelling after its travel, a single-cycle wave, or a retarget
 * issued mid-move (no travel wait: the reversal). */
type Op =
  | { kind: "hold"; target: number; dwell: number }
  | { kind: "wave"; a: number; b: number; share: number }
  | { kind: "retarget"; target: number };

/** Atomic run of motions: a session part never splits one. */
type Block = Op[];

interface Rendered {
  lines: string[];
  steps: JointCalStep[];
  end: number;
}

export interface SuitePart {
  /** Session name, `<session>_<n>` when a session was split. */
  name: string;
  session: SuiteSession;
  steps: JointCalStep[];
  script: string[];
  budgetSec: number;
}

export interface SuiteOptions {
  spanFractions: readonly number[];
  speedFractions: readonly number[];
  shortMovesRad: readonly number[];
  holdSec: number;
  repeatCount: number;
  settleSec: number;
  returnHomeSec: number;
  operator: string;
}

/** The motion blocks of one session, before any start pose is known. */
function sessionBlocks(session: SuiteSession, g: SuiteGeometry, o: SuiteOptions): Block[] {
  const hold = (target: number, dwell = o.settleSec): Op => ({ kind: "hold", target: roundRad(target), dwell });
  const sweep = ([a, b]: [number, number]): Block => [hold(a), ...o.speedFractions.map((share): Op => ({ kind: "wave", a, b, share }))];
  switch (session) {
    case "long_moves":
      return o.spanFractions.map((f) => sweep(suiteBand(g, f)));
    case "sweeps_and_reversals":
      return [
        sweep([ceilMrad(g.lower), floorMrad(g.upper)]),
        ...o.spanFractions.map((f): Block => {
          const [a, b] = suiteBand(g, f);
          // Retarget back to a halfway through the move to b: the planner reverses mid-move.
          return [hold(a), { kind: "retarget", target: b }, hold(a)];
        }),
      ];
    case "short_moves":
      return [g.gLo, 0, g.gHi]
        .filter((site): site is number => site !== undefined)
        .map((site) => {
          const sigma = site > 0 ? -1 : 1;
          return [hold(site), ...o.shortMovesRad.flatMap((d) => [hold(site + sigma * d), hold(site)])];
        });
    case "gravity_extremes": {
      const ends = [g.gLo, g.gHi].filter((x): x is number => x !== undefined);
      if (ends.length === 2) return [[hold(ends[0], o.holdSec), hold(ends[1], o.holdSec), hold(ends[0])]];
      if (ends.length === 1) return [[hold(ends[0], o.holdSec), hold(0), hold(ends[0])]];
      return [];
    }
    case "repeatability": {
      const [a, b] = suiteBand(g, o.spanFractions[0]);
      const target = b >= MIN_EXTREME_OFFSET_RAD ? b : a;
      return Array.from({ length: o.repeatCount }, () => [hold(target), hold(0)]);
    }
  }
}

/**
 * stdin lines and plan steps of `block` starting at `start`. A hold-at to the current target is
 * dropped: the trace would show no target change for it.
 */
function renderBlock(block: Block, start: number, g: SuiteGeometry, limits: JointLimits, m: Motion): Rendered {
  const J = g.joint;
  const r: Rendered = { lines: [], steps: [], end: start };
  for (const op of block) {
    if (op.kind === "wave") {
      const half = suiteWaveHalfPeriod(limits, op.b - op.a, op.share);
      r.lines.push(`wave ${J} ${String(op.a)} ${String(op.b)} 1 ${String(half)}`, `sleep ${ceilTenth(2 * half + WAVE_END_SLACK_SEC)}`);
      r.steps.push({ kind: "wave", joint: J, min_rad: op.a, max_rad: op.b, cycles: 1, half_period_s: half });
      r.end = op.a;
      continue;
    }
    if (op.target === r.end) continue;
    const sleep =
      op.kind === "retarget"
        ? reversalSleep(Math.abs(op.target - r.end) / 2, m.v, m.a)
        : ceilTenth(travelSec(m, Math.abs(op.target - r.end)) + MOVE_SLACK_SEC + op.dwell);
    r.lines.push(`hold-at ${J} ${String(op.target)}`, `sleep ${sleep}`);
    r.steps.push({ kind: "hold", joint: J, target_rad: op.target, measure: false });
    r.end = op.target;
  }
  return r;
}

function partScript(chain: readonly string[], joint: string, blocks: readonly Rendered[], o: SuiteOptions): string[] {
  const script = [referenceAcquireLine(chain), "home", `enable ${o.operator}`];
  for (const b of blocks) script.push(...b.lines);
  for (const j of [...chain].reverse()) {
    script.push(`hold-at ${j} 0`);
    if (j === joint) script.push(`sleep ${o.returnHomeSec}`);
  }
  script.push("status", "disable", "quit");
  return script;
}

/**
 * Every requested session as one or more parts within MAX_SESSION_SLEEP_SEC. Blocks are packed
 * greedily and never split; a new part starts from rest (0), so its first block is re-rendered
 * from there.
 */
export function planSuite(input: {
  geometry: SuiteGeometry;
  chain: readonly string[];
  limits: Readonly<Record<string, JointLimits>>;
  sessions: readonly SuiteSession[];
  options: SuiteOptions;
}): { ok: true; parts: SuitePart[]; skipped: string[] } | Refusal {
  const { geometry: g, chain, limits, options: o } = input;
  const l = limits[g.joint];
  if (l.trajectoryVelocityRadS === undefined || l.trajectoryAccelRadS2 === undefined || l.positionSlewRadS === undefined) {
    return {
      ok: false,
      message: `Refused: ${g.joint} needs control.yaml position_trajectory_velocity_rad_s, position_trajectory_accel_rad_s2 and position_slew_rad_s on the Pi to time its moves; no motion was run.`,
    };
  }
  const m: Motion = {
    slew: l.positionSlewRadS,
    threshold: l.trajectoryThresholdRad ?? 0,
    v: Math.min(l.trajectoryVelocityRadS, l.velocityCapRadS),
    a: l.trajectoryAccelRadS2,
  };
  const parts: SuitePart[] = [];
  const skipped: string[] = [];
  const fits = (rs: Rendered[]) => scriptSleepTotalSec(partScript(chain, g.joint, rs, o)) <= MAX_SESSION_SLEEP_SEC;
  for (const session of input.sessions) {
    const blocks = sessionBlocks(session, g, o);
    if (blocks.length === 0) {
      skipped.push(`${session}: no gravity extreme ≥ ${MIN_EXTREME_OFFSET_RAD} rad from 0 inside the usable window`);
      continue;
    }
    const groups: Rendered[][] = [];
    let group: Rendered[] = [];
    for (const block of blocks) {
      const next = renderBlock(block, group.at(-1)?.end ?? 0, g, l, m);
      if (fits([...group, next])) {
        group.push(next);
        continue;
      }
      const fresh = renderBlock(block, 0, g, l, m);
      if (group.length === 0 || !fits([fresh])) {
        return {
          ok: false,
          message: `Refused: a ${session} block needs more than the ${MAX_SESSION_SLEEP_SEC} s session budget (with reference acquisition and return); shorten hold_sec/settle_sec or the speed fractions. No motion was run.`,
        };
      }
      groups.push(group);
      group = [fresh];
    }
    groups.push(group);
    groups.forEach((rs, i) => {
      const script = partScript(chain, g.joint, rs, o);
      parts.push({
        name: groups.length > 1 ? `${session}_${i + 1}` : session,
        session,
        steps: rs.flatMap((r) => r.steps),
        script,
        budgetSec: Math.round(scriptSleepTotalSec(script) * 100) / 100,
      });
    });
  }
  return { ok: true, parts, skipped };
}

/** A pseudo calibration plan of one part, for the shared limit / speed / τ guards. */
export function partGuardPlan(part: SuitePart, g: SuiteGeometry, chain: readonly string[]): JointCalPlan {
  return {
    method: "static",
    sweepJoint: g.joint,
    chain: [...chain],
    fixedRad: {},
    posesRad: [g.lower, g.upper],
    approachOffsetRad: LIMIT_INSET_RAD,
    steps: part.steps,
  };
}

/** One line per part: name, budget, what it commands. */
export function describeParts(parts: readonly SuitePart[]): string[] {
  return parts.map((p) => {
    const waves = p.steps.filter((s) => s.kind === "wave");
    const holds = p.steps.length - waves.length;
    const speeds = waves
      .map((s) => (s.kind === "wave" ? wavePeakSpeed(s.min_rad, s.max_rad, s.half_period_s) : 0))
      .map((v) => v.toFixed(2));
    const vText = speeds.length > 0 ? `; wave peaks ${speeds.join(", ")} rad/s` : "";
    return `  ${p.name}: budget ${p.budgetSec} s, ${holds} hold-at, ${waves.length} single-cycle waves${vText}`;
  });
}

// ---------------------------------------------------------------------------
// Tool

export const motionSuiteSchema = motionConfirmSchema.extend({
  profile: z.enum(BENCH_PROFILES).default("arm_attached"),
  ...referenceOptInShape,
  sweep_joint: z.enum(MASTER_JOINTS).default("right_shoulder_pitch").describe("The one joint the suite moves"),
  sessions: z
    .array(z.enum(SUITE_SESSIONS))
    .min(1)
    .optional()
    .describe(`Sessions to run, in order (default all: ${SUITE_SESSIONS.join(", ")})`),
  dry_run: z
    .boolean()
    .default(false)
    .describe("Read config, run the τ batches and every guard, return the session plan; no motion, no enable"),
  span_fractions: z
    .array(z.number().gt(0).max(1))
    .min(1)
    .max(4)
    .default(DEFAULT_SPAN_FRACTIONS)
    .describe("Long-move / reversal spans as shares of the usable window (the first sizes the repeated move)"),
  speed_fractions: z
    .array(z.number().gt(0).max(0.9))
    .min(1)
    .max(4)
    .default(DEFAULT_SPEED_FRACTIONS)
    .describe("Wave peak speeds as shares of the admissible speed for the span (≤ 0.9)"),
  short_moves_rad: z.array(z.number().gt(0).max(0.15)).min(1).max(5).default(DEFAULT_SHORT_MOVES_RAD),
  hold_sec: z.number().min(10).max(60).default(DEFAULT_HOLD_SEC).describe("Drift hold at each gravity extreme"),
  repeat_count: z.number().int().min(3).max(10).default(DEFAULT_REPEAT_COUNT),
  settle_sec: z.number().min(1).max(10).default(DEFAULT_SETTLE_SEC),
  return_home_sec: z.number().int().min(5).max(120).default(DEFAULT_RETURN_HOME_SEC),
  config_dir: z.string().optional().describe("MARENGO_CONFIG_DIR override (default: master /opt/marengo/config)"),
  operator: z.string().regex(OPERATOR).default("bench"),
});

export type MotionSuiteArgs = Partial<z.infer<typeof motionSuiteSchema>> & { confirm: true };

/** Lines from the scorer's bench-score header to the end. */
export function scoreBlock(stdout: string): string[] {
  const lines = stdout.trimEnd().split("\n");
  const start = lines.findIndex((l) => l.startsWith(SCORE_HEADER));
  return start < 0 ? lines : lines.slice(start);
}

interface PartOutcome {
  name: string;
  ts: string;
  budgetSec: number;
  verdict: string;
  complete: boolean;
  pass: boolean;
}

export function registerMotionSuiteTools(
  cfg: MarengoPiConfig,
  runRemote: RunRemote,
  auditMotion: AuditMotion,
  deps: CalibrationDeps = defaultCalibrationDeps,
) {
  return {
    pi_motion_suite: {
      description:
        "Single-joint motion suite in several marengo-pi sessions (each ≤ 300 s with reference acquisition): " +
        "long_moves (single-cycle `wave` a→b→a over span_fractions 25/50/90 % of the usable window at " +
        "speed_fractions 25/50/90 % of the admissible speed), sweeps_and_reversals (full-width waves at the same " +
        "speeds; hold-at a→b retargeted back to a halfway), short_moves (0.02/0.05/0.1 rad out and back at each " +
        "gravity extreme and at 0), gravity_extremes (≥ hold_sec drift holds at both extremes, moves between " +
        "them), repeatability (the same hold-at move × repeat_count). Usable window: soft ∩ hard inset 0.05 rad, " +
        "narrowed where model |τ_g| × 1.6 > 0.8 × τ_ff cap (motor-repl gravity-preview grid). Each session: " +
        "`home <profile joints> sign-tested`, home, enable, motion, every joint back to 0 distal first, disable, " +
        "quit; the sweep joint is traced every tick (MARENGO_POSITION_TRACE_FULL_RATE_JOINTS). Guards before any " +
        "motion of any session: targets ≥ 0.05 rad inside the window, wave speed/accel within the admission " +
        "limits, τ guard along every commanded path, budget ≤ 300 s; the gravity gate before each enable keeps " +
        "its hanging-rest refusal. Each session writes var/motion-suite/<TS>/ (plan.json, position-trace.csv, " +
        "bench-session.txt, config, score.txt) and is scored per move by scripts/analyze-position-trace.py " +
        "--score-bench; the result ends with a per-session PASS/FAIL table. Stops at the first session that " +
        "refuses, fails or is incomplete. dry_run: plan + guards only. Needs confirm (+ confirm_weighted_motion " +
        "on weighted profiles), set_zero + at_mechanical_reference. " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: motionSuiteSchema,
      handler: async (args: MotionSuiteArgs): Promise<string> => {
        const profile = args.profile ?? "arm_attached";
        const check = validateMotionConfirm({ ...args, profile }, cfg.benchProfile);
        if (!check.ok) return check.message;
        if (args.set_zero !== true || args.at_mechanical_reference !== true) return REFERENCE_OPT_IN_REQUIRED;
        const joint = args.sweep_joint ?? "right_shoulder_pitch";
        const operator = args.operator ?? "bench";
        if (!OPERATOR.test(operator)) return "Refused: operator must match ^[A-Za-z0-9_-]+$.";
        const chain = profileMeta(profile).setZeroJoints;
        if (!chain.includes(joint)) {
          return `Refused: bench profile ${profile} does not reference ${joint}; pick a profile whose joints include it.`;
        }
        const refused = (message: string) => {
          auditMotion(TOOL, args, message, 1);
          return message;
        };
        const configDir = benchConfigDirForJoint(cfg, joint, args.config_dir) ?? BENCH_CONFIG_MASTER;
        const preflight = parsePreflight(
          await runRemote(wrapRemoteWithConfig(cfg, preflightReadShell(), configDir), PREFLIGHT_TIMEOUT_MS),
          cfg.piRoot,
        );
        if (!preflight.ok) return refused(preflight.message);
        const read = readJointLimits(preflight, chain);
        if (!read.ok) return refused(read.message);
        const tauBatch = async (configs: Record<string, number>[]) =>
          parseGravityBatch(
            await runRemote(
              wrapRemoteWithConfig(cfg, soleCanOwnerShell(gravityBatchShell(configs, read.robotJoints)), configDir),
              TAU_GUARD_TIMEOUT_MS + CAN_SESSION_SLACK_MS,
            ),
          );

        const samples = tauGridSamples(read.limits[joint].window);
        const gridTau = await tauBatch(samples.map((q) => ({ [joint]: q })));
        const geometry = suiteGeometry(joint, samples, samples.map((_, i) => gridTau.get(i)), chain, read.limits);
        if (!geometry.ok) return refused(geometry.message);

        const options: SuiteOptions = {
          spanFractions: args.span_fractions ?? DEFAULT_SPAN_FRACTIONS,
          speedFractions: args.speed_fractions ?? DEFAULT_SPEED_FRACTIONS,
          shortMovesRad: args.short_moves_rad ?? DEFAULT_SHORT_MOVES_RAD,
          holdSec: args.hold_sec ?? DEFAULT_HOLD_SEC,
          repeatCount: args.repeat_count ?? DEFAULT_REPEAT_COUNT,
          settleSec: args.settle_sec ?? DEFAULT_SETTLE_SEC,
          returnHomeSec: args.return_home_sec ?? DEFAULT_RETURN_HOME_SEC,
          operator,
        };
        const planned = planSuite({
          geometry,
          chain,
          limits: read.limits,
          sessions: args.sessions ?? SUITE_SESSIONS,
          options,
        });
        if (!planned.ok) return refused(planned.message);

        const fmt = (x: number | undefined) => (x === undefined ? "none" : x.toFixed(3));
        const header = [
          `${TOOL}: ${joint} (profile ${profile})`,
          `usable window [${geometry.lower}, ${geometry.upper}] rad (lower bound: ${geometry.lowerBound}, upper: ${geometry.upperBound}); ` +
            `gravity extremes g_lo ${fmt(geometry.gLo)} g_hi ${fmt(geometry.gHi)} rad`,
        ];
        for (const part of planned.parts) {
          const guardPlan = partGuardPlan(part, geometry, chain);
          for (const guard of [checkJointCalLimits(guardPlan, read.limits), checkWaveSpeeds(guardPlan, read.limits)]) {
            if (!guard.ok) return refused(`${part.name}: ${guard.message}`);
          }
          const configs = guardConfigurations(guardPlan);
          const tauGuard = checkTauGuard(configs, await tauBatch(configs), chain, read.limits);
          if (!tauGuard.ok) return refused(`${part.name}: ${tauGuard.message}`);
          if (part === planned.parts[0]) header.push(...tauGuard.report);
        }
        header.push("sessions:", ...describeParts(planned.parts), ...planned.skipped.map((s) => `  skipped ${s}`));
        if (args.dry_run === true) {
          const text = [...header, "", "dry_run: no session was run."].join("\n");
          auditMotion(TOOL, args, text, 0);
          return text;
        }

        const out = [...header];
        const outcomes: PartOutcome[] = [];
        for (const part of planned.parts) {
          let outcome: PartOutcome = {
            name: part.name,
            ts: "-",
            budgetSec: part.budgetSec,
            verdict: "not run (refused or failed)",
            complete: false,
            pass: false,
          };
          const text = await runCalibrationSession(cfg, runRemote, () => {}, deps, {
            tool: TOOL,
            label: `motion-suite-${part.name}`,
            args,
            profile,
            gateJoints: chain,
            configDir,
            script: part.script,
            budgetSec: part.budgetSec,
            preflight,
            preamble: [],
            runFit: false,
            fitParams: [],
            fitIncomplete: false,
            fullRateJoints: [joint],
            allowHangingRestMismatch: false,
            outputSubdir: "motion-suite",
            sectionHeader: `--- motion suite: ${part.name} ---`,
            plan: ({ sessionTs, gateReport, sessionExit, trace }) => {
              const complete =
                sessionExit === 0 && trace !== undefined && stepsSeenInTrace(part.steps, trace) === part.steps.length;
              return {
                complete,
                json: {
                  version: 1,
                  tool: TOOL,
                  created_utc: deps.now().toISOString(),
                  session_ts: sessionTs,
                  session: part.name,
                  profile,
                  sweep_joint: joint,
                  usable_window_rad: [geometry.lower, geometry.upper],
                  gravity_extremes_rad: { lo: geometry.gLo ?? null, hi: geometry.gHi ?? null },
                  budget_sec: part.budgetSec,
                  steps: part.steps,
                  gravity_gate_report: gateReport,
                  session_complete: complete,
                },
              };
            },
            afterSession: async ({ dir, complete, sessionExit }) => {
              const trace = path.join(dir, "position-trace.csv");
              const log = path.join(dir, "bench-session.txt");
              const scored = await deps.execLocal(
                "python3",
                [SCORER, trace, "--score-bench", "--joint", joint, "--bench-log", log],
                { cwd: cfg.localRoot, timeoutMs: SCORER_TIMEOUT_MS },
              );
              await deps.writeFile(path.join(dir, "score.txt"), `${scored.stdout}${scored.stderr ? `\n[stderr]\n${scored.stderr}` : ""}`);
              const verdict =
                scored.exitCode === 0 ? "PASS" : scored.exitCode === 2 ? "FAIL" : `scorer error (exit ${scored.exitCode})`;
              const pass = complete && scored.exitCode === 0;
              outcome = {
                ...outcome,
                ts: path.basename(dir),
                verdict: complete ? verdict : `incomplete (marengo-pi exit ${sessionExit}); scored ${verdict}`,
                complete,
                pass,
              };
              const lines = [`score (${SCORER}): ${verdict}`, ...scoreBlock(scored.stdout)];
              if (scored.exitCode !== 0 && scored.exitCode !== 2) lines.push(scored.stderr.trimEnd());
              return { lines, exitCode: pass ? 0 : complete ? 2 : sessionExit || 1 };
            },
          });
          outcomes.push(outcome);
          out.push("", `=== session ${part.name} ===`, text);
          if (!outcome.complete) break;
        }
        const ran = new Set(outcomes.map((o) => o.name));
        const rows = [
          ...outcomes,
          ...planned.parts
            .filter((p) => !ran.has(p.name))
            .map((p) => ({ name: p.name, ts: "-", budgetSec: p.budgetSec, verdict: "not run (suite stopped)", pass: false })),
        ];
        const overall = rows.length > 0 && rows.every((r) => r.pass);
        out.push(
          "",
          "=== motion suite summary ===",
          "session | ts | budget s | verdict",
          ...rows.map((r) => `${r.name} | ${r.ts} | ${r.budgetSec} | ${r.verdict}`),
          `overall: ${overall ? "PASS" : "FAIL"}`,
        );
        const text = out.join("\n");
        auditMotion(TOOL, args, text, overall ? 0 : 2);
        return text;
      },
    },
  };
}
