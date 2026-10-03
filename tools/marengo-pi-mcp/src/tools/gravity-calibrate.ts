/**
 * pi_gravity_calibrate: one marengo-pi session sweeping a right-arm joint through static
 * poses (approached from below, then from above) while the position trace records τ_meas.
 * The workstation fitter (`marengo-log-cli gravity-fit`) turns the captured directory into a
 * proposed URDF inertial patch. Nothing is ever applied to the Pi here (ADR 0017).
 */

import { mkdir as fsMkdir, writeFile as fsWriteFile } from "node:fs/promises";
import path from "node:path";
import { z } from "zod";
import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { profileMeta } from "../bench-profiles.js";
import { shellQuote, wrapRemote, wrapRemoteWithConfig } from "../env.js";
import { effectiveProfile, validateMotionConfirm } from "../safety.js";
import { soleCanOwnerShell } from "../can-owner.js";
import { gravityGateSnapshotShell, runGravityGate } from "../gravity-gate.js";
import { execLocal as sshExecLocal, type RemoteExecResult } from "../ssh.js";
import {
  BENCH_CONFIG_MASTER,
  CAN_SESSION_SLACK_MS,
  REFERENCE_OPT_IN_REQUIRED,
  SOLE_CAN_OWNER_NOTE,
  benchConfigDirForJoint,
  benchLogWrapper,
  marengoPiAdmittedPipe,
  marengoPiSessionBody,
  motionConfirmSchema,
  referenceAcquireLine,
  referenceOptInShape,
  scriptSleepTotalSec,
  sessionRefusal,
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
/** Upper bound on the session's sleep + reference budget (scriptSleepTotalSec). */
export const MAX_SESSION_SLEEP_SEC = 300;
const MIN_POSES = 2;
const MAX_POSES = 12;
const MIN_PITCH_POSES = 3;
const FIT_TIMEOUT_MS = 900_000;
const PREFLIGHT_TIMEOUT_MS = 20_000;
const TRACE_FETCH_TIMEOUT_MS = 60_000;

export const SKIP_HANGING_REST_REQUIRED =
  "Refused: pi_gravity_calibrate needs skip_hanging_rest_gravity_check: true. The gravity gate's " +
  "hanging-rest |τ_g| check is skipped only for this tool, because the calibration exists to fix " +
  "the model that check would refuse; its residuals are still reported, and an unavailable " +
  "gravity preview or a measured-torque mismatch still refuses.";

const NEVER_APPLIED_NOTE =
  "Nothing was applied to the Pi. Applying a proposed inertial patch is a separate explicit step: " +
  "review the patch, apply it to assets/urdf/marengo.urdf, then run pi_sync_bench_urdf (ADR 0017). " +
  "Never automatic.";

export interface CalibrationStep {
  joint: string;
  target_rad: number;
  measure: boolean;
  pose_index?: number;
  approach?: "below" | "above";
}

export interface SweepPlan {
  sweepJoint: SweepJoint;
  posesRad: number[];
  fixedRad: Record<string, number>;
  steps: CalibrationStep[];
}

type Refusal = { ok: false; message: string };

/** Drop float noise (e.g. 1.2 + 0.05) so hold-at values read cleanly and round-trip. */
function roundRad(x: number): number {
  return Math.round(x * 1e9) / 1e9;
}

/**
 * Poses validated and sorted, then the step plan: an up pass (min−δ overshoot, then every pose
 * ascending, approached from below) and a down pass (max+δ overshoot, then every pose descending,
 * approached from above). An elbow sweep with a non-zero fixed pitch first moves the pitch there.
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
  const raw = input.posesRad;
  if (raw.length < MIN_POSES || raw.length > MAX_POSES) {
    return refuse(`poses_rad needs ${MIN_POSES}–${MAX_POSES} entries (got ${raw.length})`);
  }
  if (raw.some((q) => !Number.isFinite(q))) return refuse("poses_rad entries must be finite numbers");
  const poses = [...raw].sort((a, b) => a - b);
  if (poses.some((q, i) => i > 0 && q === poses[i - 1])) {
    return refuse(`poses_rad entries must be distinct (got ${JSON.stringify(raw)})`);
  }
  if (sweepJoint === PITCH && poses.length < MIN_PITCH_POSES) {
    return refuse(`a right_shoulder_pitch sweep needs at least ${MIN_PITCH_POSES} distinct poses`);
  }

  const steps: CalibrationStep[] = [];
  if (sweepJoint === ELBOW && fixedPitchRad !== 0) {
    steps.push({ joint: PITCH, target_rad: fixedPitchRad, measure: false });
  }
  const last = poses.length - 1;
  steps.push({ joint: sweepJoint, target_rad: roundRad(poses[0] - delta), measure: false });
  poses.forEach((q, i) =>
    steps.push({ joint: sweepJoint, target_rad: q, measure: true, pose_index: i, approach: "below" }),
  );
  steps.push({ joint: sweepJoint, target_rad: roundRad(poses[last] + delta), measure: false });
  for (let i = last; i >= 0; i -= 1) {
    steps.push({ joint: sweepJoint, target_rad: poses[i], measure: true, pose_index: i, approach: "above" });
  }
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

// ---------------------------------------------------------------------------
// Minimal indentation-aware YAML reader (block mappings, block sequences, scalars as strings).

export type YamlNode = string | null | YamlNode[] | { [key: string]: YamlNode };

interface YamlLine {
  indent: number;
  text: string;
}

const YAML_KEY = /^("[^"]*"|'[^']*'|[^"'\s-][^:]*?|-[^\s:][^:]*?)\s*:(?:\s+(.*))?$/;

function stripYamlComment(line: string): string {
  let quote: string | null = null;
  for (let i = 0; i < line.length; i += 1) {
    const c = line[i];
    if (quote) {
      if (c === quote) quote = null;
    } else if (c === '"' || c === "'") {
      quote = c;
    } else if (c === "#" && (i === 0 || /\s/.test(line[i - 1]))) {
      return line.slice(0, i).trimEnd();
    }
  }
  return line.trimEnd();
}

function unquote(s: string): string {
  const t = s.trim();
  if (t.length >= 2 && ((t[0] === '"' && t.endsWith('"')) || (t[0] === "'" && t.endsWith("'")))) {
    return t.slice(1, -1);
  }
  return t;
}

const isSeqItem = (text: string) => text === "-" || text.startsWith("- ");

function parseYamlBlock(lines: YamlLine[], pos: { i: number }, indent: number): YamlNode {
  return isSeqItem(lines[pos.i].text)
    ? parseYamlSeq(lines, pos, indent)
    : parseYamlMap(lines, pos, indent);
}

function parseYamlMap(lines: YamlLine[], pos: { i: number }, indent: number): YamlNode {
  const out: { [key: string]: YamlNode } = {};
  while (pos.i < lines.length) {
    const line = lines[pos.i];
    if (line.indent < indent || (line.indent === indent && isSeqItem(line.text))) break;
    if (line.indent > indent) throw new Error(`unexpected indentation at "${line.text}"`);
    const m = YAML_KEY.exec(line.text);
    if (!m) throw new Error(`not a mapping entry: "${line.text}"`);
    pos.i += 1;
    const key = unquote(m[1]);
    const rest = (m[2] ?? "").trim();
    const next = lines[pos.i];
    if (/^[|>]/.test(rest)) {
      const body: string[] = [];
      while (pos.i < lines.length && lines[pos.i].indent > indent) body.push(lines[pos.i++].text);
      out[key] = body.join("\n");
    } else if (rest !== "") {
      out[key] = unquote(rest);
    } else if (next && next.indent > indent) {
      out[key] = parseYamlBlock(lines, pos, next.indent);
    } else if (next && next.indent === indent && isSeqItem(next.text)) {
      out[key] = parseYamlSeq(lines, pos, indent);
    } else {
      out[key] = null;
    }
  }
  return out;
}

function parseYamlSeq(lines: YamlLine[], pos: { i: number }, indent: number): YamlNode {
  const out: YamlNode[] = [];
  while (pos.i < lines.length) {
    const line = lines[pos.i];
    if (line.indent < indent || !isSeqItem(line.text)) break;
    if (line.indent > indent) throw new Error(`unexpected indentation at "${line.text}"`);
    const item = line.text.slice(1).trimStart();
    if (item === "") {
      pos.i += 1;
      const next = lines[pos.i];
      out.push(next && next.indent > indent ? parseYamlBlock(lines, pos, next.indent) : null);
    } else if (YAML_KEY.test(item)) {
      // `- key: v` opens a mapping whose entries align with `key`.
      const childIndent = line.indent + (line.text.length - item.length);
      lines[pos.i] = { indent: childIndent, text: item };
      out.push(parseYamlMap(lines, pos, childIndent));
    } else {
      pos.i += 1;
      out.push(unquote(item));
    }
  }
  return out;
}

/** Parse the YAML subset Marengo config uses; throws on anything it does not understand. */
export function parseYamlLite(text: string): YamlNode {
  const lines: YamlLine[] = [];
  for (const raw of text.split(/\r?\n/)) {
    const stripped = stripYamlComment(raw);
    const trimmed = stripped.trim();
    if (trimmed === "" || trimmed === "---") continue;
    lines.push({ indent: stripped.length - stripped.trimStart().length, text: trimmed });
  }
  if (lines.length === 0) return null;
  const pos = { i: 0 };
  const root = parseYamlBlock(lines, pos, lines[0].indent);
  if (pos.i < lines.length) throw new Error(`unexpected indentation at "${lines[pos.i].text}"`);
  return root;
}

function yamlGet(node: YamlNode, ...keys: string[]): YamlNode | undefined {
  let cur: YamlNode | undefined = node;
  for (const key of keys) {
    if (cur === null || cur === undefined || typeof cur !== "object" || Array.isArray(cur)) return undefined;
    cur = cur[key];
  }
  return cur;
}

const YAML_NUMBER = /^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/;

function yamlNumber(node: YamlNode | undefined): number | undefined {
  return typeof node === "string" && YAML_NUMBER.test(node) ? Number(node) : undefined;
}

// ---------------------------------------------------------------------------
// Pose limits

export interface JointWindow {
  joint: string;
  soft: [number, number];
  hard: [number, number];
  lower: number;
  upper: number;
}

/**
 * Allowed window per joint = [max(soft_lo, hard_lo), min(soft_hi, hard_hi)], soft from
 * control.yaml `control.joints.<joint>.position_soft_{lower,upper}_rad`, hard from the
 * motors.yaml `motors` item with `joint: <joint>` → `bench.position_{lower,upper}_rad`.
 */
export function jointWindows(
  controlYaml: string,
  motorsYaml: string,
  joints: readonly string[],
): { ok: true; windows: Record<string, JointWindow> } | Refusal {
  let control: YamlNode;
  let motors: YamlNode;
  try {
    control = parseYamlLite(controlYaml);
    motors = parseYamlLite(motorsYaml);
  } catch (err) {
    return { ok: false, message: `Refused: cannot parse Pi control.yaml/motors.yaml: ${String(err)}` };
  }
  const motorList = yamlGet(motors, "motors");
  const windows: Record<string, JointWindow> = {};
  for (const joint of joints) {
    const softLo = yamlNumber(yamlGet(control, "control", "joints", joint, "position_soft_lower_rad"));
    const softHi = yamlNumber(yamlGet(control, "control", "joints", joint, "position_soft_upper_rad"));
    const motor = Array.isArray(motorList)
      ? motorList.find((m) => yamlGet(m, "joint") === joint)
      : undefined;
    const hardLo = motor === undefined ? undefined : yamlNumber(yamlGet(motor, "bench", "position_lower_rad"));
    const hardHi = motor === undefined ? undefined : yamlNumber(yamlGet(motor, "bench", "position_upper_rad"));
    const missing = [
      softLo === undefined && `control.yaml control.joints.${joint}.position_soft_lower_rad`,
      softHi === undefined && `control.yaml control.joints.${joint}.position_soft_upper_rad`,
      hardLo === undefined && `motors.yaml ${joint} bench.position_lower_rad`,
      hardHi === undefined && `motors.yaml ${joint} bench.position_upper_rad`,
    ].filter((m): m is string => typeof m === "string");
    if (softLo === undefined || softHi === undefined || hardLo === undefined || hardHi === undefined) {
      return {
        ok: false,
        message: `Refused: ${joint} pose limits missing or unparseable on the Pi: ${missing.join(", ")}; no motion was run.`,
      };
    }
    const lower = Math.max(softLo, hardLo);
    const upper = Math.min(softHi, hardHi);
    if (!(lower <= upper)) {
      return {
        ok: false,
        message: `Refused: ${joint} has an empty pose window (soft [${softLo}, ${softHi}] ∩ hard [${hardLo}, ${hardHi}]); no motion was run.`,
      };
    }
    windows[joint] = { joint, soft: [softLo, softHi], hard: [hardLo, hardHi], lower, upper };
  }
  return { ok: true, windows };
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
  const targets: { joint: string; rad: number; what: string }[] = plan.steps.map((s, i) => ({
    joint: s.joint,
    rad: s.target_rad,
    what: `step ${i} target`,
  }));
  for (const [joint, rad] of Object.entries(plan.fixedRad)) {
    targets.push({ joint, rad, what: "fixed pose" });
  }
  for (const joint of planJoints(plan)) targets.push({ joint, rad: 0, what: "return pose" });
  for (const t of targets) {
    const w = windows[t.joint];
    if (w === undefined) {
      return { ok: false, message: `Refused: no pose window for ${t.joint}; no motion was run.` };
    }
    if (t.rad < w.lower || t.rad > w.upper) {
      return {
        ok: false,
        message:
          `Refused: ${t.joint} ${t.what} ${t.rad} rad is outside its allowed window [${w.lower}, ${w.upper}] rad ` +
          `(control.yaml soft [${w.soft[0]}, ${w.soft[1]}] ∩ motors.yaml hard [${w.hard[0]}, ${w.hard[1]}]); ` +
          "no motion was run.",
      };
    }
  }
  return { ok: true };
}

// ---------------------------------------------------------------------------
// Remote reads (no CAN)

const MARKER = "=====MARENGO_GRAVCAL";
const beginMarker = (name: string) => `${MARKER}_${name}_BEGIN=====`;
const endMarker = (name: string) => `${MARKER}_${name}_END=====`;
const URDF_PATH_PREFIX = "MARENGO_GRAVCAL_URDF_PATH=";

/** Shell printing `path` between markers (content verbatim; `\n` + END marker follows it). */
function markedCatShell(name: string, pathExpr: string): string {
  return [
    `printf '%s\\n' ${shellQuote(beginMarker(name))}`,
    `cat ${pathExpr}`,
    `printf '\\n%s\\n' ${shellQuote(endMarker(name))}`,
  ].join("\n");
}

/** Content between the markers of `name`, verbatim, or undefined when absent. */
export function extractMarked(output: string, name: string): string | undefined {
  const begin = `${beginMarker(name)}\n`;
  const start = output.indexOf(begin);
  if (start < 0) return undefined;
  const from = start + begin.length;
  const end = output.indexOf(`\n${endMarker(name)}`, from);
  return end < 0 ? undefined : output.slice(from, end);
}

const URDF_AWK =
  "/^robot:/{r=1;next} /^[^[:space:]#]/{r=0} " +
  'r && /^[[:space:]]+urdf:/{sub(/^[[:space:]]+urdf:[[:space:]]*/,""); sub(/[[:space:]]+#.*$/,""); gsub(/["\']/,""); print; exit}';

/** Read-only pre-flight: config_dir robot/control/motors.yaml and the URDF robot.yaml names. */
export function preflightReadShell(): string {
  return [
    'GC_CFG="$MARENGO_CONFIG_DIR"',
    ...["robot", "control", "motors"].map((f) => markedCatShell(f, `"$GC_CFG/${f}.yaml"`)),
    `GC_URDF_REL="$(awk ${shellQuote(URDF_AWK)} "$GC_CFG/robot.yaml")"`,
    'test -n "$GC_URDF_REL" || { echo "robot.yaml has no robot.urdf" >&2; exit 1; }',
    'case "$GC_URDF_REL" in /*) GC_URDF="$GC_URDF_REL" ;; *) GC_URDF="$MARENGO_ROOT/$GC_URDF_REL" ;; esac',
    `printf '${URDF_PATH_PREFIX}%s\\n' "$GC_URDF"`,
    markedCatShell("urdf", '"$GC_URDF"'),
  ].join("\n");
}

export interface PreflightFiles {
  robotYaml: string;
  controlYaml: string;
  motorsYaml: string;
  urdf: string;
  urdfPath: string;
}

export function parsePreflight(output: string, piRoot: string): ({ ok: true } & PreflightFiles) | Refusal {
  const robotYaml = extractMarked(output, "robot");
  const controlYaml = extractMarked(output, "control");
  const motorsYaml = extractMarked(output, "motors");
  const urdf = extractMarked(output, "urdf");
  const urdfPath = new RegExp(`^${URDF_PATH_PREFIX}(.*)$`, "m").exec(output)?.[1];
  if (
    robotYaml === undefined ||
    controlYaml === undefined ||
    motorsYaml === undefined ||
    urdf === undefined ||
    urdfPath === undefined ||
    urdf.trim() === ""
  ) {
    return {
      ok: false,
      message: `Refused: pre-flight read of the Pi config/URDF was incomplete; no motion was run.\n${output}`,
    };
  }
  let named: string | undefined;
  try {
    const v = yamlGet(parseYamlLite(robotYaml), "robot", "urdf");
    named = typeof v === "string" && v !== "" ? v : undefined;
  } catch (err) {
    return { ok: false, message: `Refused: cannot parse Pi robot.yaml: ${String(err)}; no motion was run.` };
  }
  const expected = named === undefined ? undefined : named.startsWith("/") ? named : `${piRoot}/${named}`;
  if (expected !== urdfPath) {
    return {
      ok: false,
      message: `Refused: robot.yaml robot.urdf (${named ?? "missing"}) does not match the URDF read (${urdfPath}); no motion was run.`,
    };
  }
  return { ok: true, robotYaml, controlYaml, motorsYaml, urdf, urdfPath };
}

/** `{"log":..,"trace":..,"ts":..}` line benchLogWrapper echoes after the session. */
export function parseSessionJson(output: string): { trace: string; ts: string } | undefined {
  const lines = output.split("\n").filter((l) => /^\{"log":.*\}\s*$/.test(l));
  const last = lines.at(-1);
  if (last === undefined) return undefined;
  try {
    const v = JSON.parse(last) as { trace?: unknown; ts?: unknown };
    if (
      typeof v.trace === "string" &&
      typeof v.ts === "string" &&
      /^\d{8}T\d{6}Z$/.test(v.ts) &&
      /^\/[A-Za-z0-9_./-]+$/.test(v.trace)
    ) {
      return { trace: v.trace, ts: v.ts };
    }
  } catch {
    return undefined;
  }
  return undefined;
}

// ---------------------------------------------------------------------------
// Tool

export interface GravityCalibrateDeps {
  execLocal: (command: string, args: string[], opts: { cwd?: string; timeoutMs?: number }) => Promise<RemoteExecResult>;
  writeFile: (file: string, data: string) => Promise<void>;
  mkdir: (dir: string) => Promise<void>;
  now: () => Date;
}

const defaultDeps: GravityCalibrateDeps = {
  execLocal: sshExecLocal,
  writeFile: (file, data) => fsWriteFile(file, data, "utf8"),
  mkdir: async (dir) => {
    await fsMkdir(dir, { recursive: true });
  },
  now: () => new Date(),
};

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
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  auditMotion: (tool: string, args: Record<string, unknown>, result: string, exitCode: number) => void,
  deps: GravityCalibrateDeps = defaultDeps,
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

        const gravity = await runGravityGate({
          profile,
          joints: [...new Set([...referenceJoints, ...planJoints(plan)])],
          snapshotOutput: await runRemote(
            wrapRemoteWithConfig(cfg, gravityGateSnapshotShell(), configDir),
            15_000,
          ),
          runPreview: (shell) =>
            runRemote(wrapRemoteWithConfig(cfg, soleCanOwnerShell(shell), configDir), 30_000 + CAN_SESSION_SLACK_MS),
          allowHangingRestMismatch: true,
        });
        if (!gravity.ok) {
          auditMotion("pi_gravity_calibrate", args, gravity.report, 1);
          return gravity.report;
        }

        const pipeCmd = marengoPiSessionBody(cfg, marengoPiAdmittedPipe(script, budgetSec + 10));
        const sessionOut = await runRemote(
          benchLogWrapper(cfg, pipeCmd, "gravity-calibrate", configDir),
          budgetSec * 1000 + 30_000 + CAN_SESSION_SLACK_MS,
        );
        const sessionText = `${gravity.report}\n${sessionOut}`;
        const sessionExit = Number(/\[exit (\d+)\]\s*$/.exec(sessionOut)?.[1] ?? 0);
        const out: string[] = [sessionText, "", "--- gravity calibration ---"];
        const finish = (exitCode: number) => {
          out.push("", NEVER_APPLIED_NOTE);
          const text = out.join("\n");
          auditMotion("pi_gravity_calibrate", args, text, exitCode);
          return text;
        };

        const refusal = sessionRefusal(sessionOut);
        if (refusal !== undefined) {
          out.push(
            `marengo-pi refused the session before the sweep: ${refusal}`,
            "The feeder sent disable/quit instead of any hold-at: no calibration files were written and no fit was run.",
          );
          return finish(sessionExit || 1);
        }
        const session = parseSessionJson(sessionOut);
        if (session === undefined) {
          out.push("The session reported no bench log/trace line: no calibration files were written and no fit was run.");
          return finish(sessionExit || 1);
        }
        const traceQuoted = shellQuote(session.trace);
        const traceOut = await runRemote(
          wrapRemote(cfg, [`test -s ${traceQuoted}`, markedCatShell("trace", traceQuoted)].join("\n")),
          TRACE_FETCH_TIMEOUT_MS,
        );
        const trace = extractMarked(traceOut, "trace");

        const dir = path.join(cfg.localRoot, "var", "gravity-calibration", session.ts);
        const planJson = {
          version: 1,
          created_utc: deps.now().toISOString(),
          session_ts: session.ts,
          profile,
          sweep_joint: plan.sweepJoint,
          fixed_rad: plan.fixedRad,
          poses_rad: plan.posesRad,
          approach_offset_rad: args.approach_offset_rad ?? DEFAULT_APPROACH_OFFSET_RAD,
          settle_sec: settleSec,
          measure_sec: measureSec,
          steps: plan.steps,
          gravity_gate_report: gravity.report,
        };
        await deps.mkdir(path.join(dir, "config"));
        const files: [string, string][] = [
          ["plan.json", `${JSON.stringify(planJson, null, 2)}\n`],
          ["pi-marengo.urdf", preflight.urdf],
          ["config/robot.yaml", preflight.robotYaml],
          ["config/control.yaml", preflight.controlYaml],
          ["config/motors.yaml", preflight.motorsYaml],
          ["bench-session.txt", `${sessionText}\n`],
        ];
        const traceOk = trace !== undefined && trace.startsWith("tick,");
        if (traceOk) files.splice(1, 0, ["position-trace.csv", trace]);
        for (const [name, data] of files) await deps.writeFile(path.join(dir, name), data);
        out.push(`calibration dir: ${dir}`, `wrote: ${files.map(([n]) => n).join(", ")}`);

        if (!traceOk) {
          out.push(`position trace ${session.trace} missing or without a header: no fit was run.`, traceOut);
          return finish(sessionExit || 1);
        }
        if (sessionExit !== 0) {
          out.push(`marengo-pi session exited ${sessionExit}: the sweep may be incomplete, so no fit was run.`);
          return finish(sessionExit);
        }
        const fitArgs = [
          "run", "--release", "-q", "-p", "marengo-log-cli", "--", "gravity-fit", "--dir", dir,
          ...(args.fit_params ?? []).flatMap((p) => ["--fit", p]),
        ];
        const fitCommand = `cd ${shellQuote(cfg.localRoot)} && cargo ${fitArgs
          .map((a) => (/^[A-Za-z0-9_./:=-]+$/.test(a) ? a : shellQuote(a)))
          .join(" ")}`;
        if (args.run_fit === false) {
          out.push(`run_fit: false — fit later with: ${fitCommand}`);
          return finish(0);
        }
        const fit = await deps.execLocal("cargo", fitArgs, { cwd: cfg.localRoot, timeoutMs: FIT_TIMEOUT_MS });
        const fitText = [fit.stdout.trimEnd(), fit.stderr.trim() ? `[stderr]\n${fit.stderr.trimEnd()}` : ""]
          .filter(Boolean)
          .join("\n");
        if (fit.exitCode === 0) {
          out.push("gravity-fit: proposal written (exit 0)", fitText);
        } else if (fit.exitCode === 2) {
          out.push("gravity-fit refused the fit (exit 2: ill-conditioned, residual too high, or limits)", fitText);
        } else {
          out.push(
            `gravity-fit did not run (exit ${fit.exitCode}${/ENOENT/.test(fit.stderr) ? ", cargo not found" : ""}).`,
            fitText,
            `Run it on the workstation: ${fitCommand}`,
          );
        }
        return finish(0);
      },
    },
  };
}
