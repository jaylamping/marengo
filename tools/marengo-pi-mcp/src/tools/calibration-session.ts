/**
 * Shared machinery of the right-arm calibration tools (pi_gravity_calibrate, pi_joint_calibrate):
 * the read-only Pi pre-flight (config + URDF capture, ADR 0017), the soft ∩ hard pose windows,
 * the static hold sweep, and the one-session runner (gravity gate, marengo-pi session, trace
 * fetch, local `var/gravity-calibration/<TS>/` directory, workstation `gravity-fit`).
 * Nothing here ever applies a model or config change to the Pi.
 */

import { mkdir as fsMkdir, readFile as fsReadFile, writeFile as fsWriteFile } from "node:fs/promises";
import path from "node:path";
import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { shellQuote, wrapRemote, wrapRemoteWithConfig } from "../env.js";
import { soleCanOwnerShell } from "../can-owner.js";
import { gravityGateSnapshotShell, runGravityGate } from "../gravity-gate.js";
import { execLocal as sshExecLocal, type RemoteExecResult } from "../ssh.js";
import {
  CAN_SESSION_SLACK_MS,
  benchLogWrapper,
  marengoPiAdmittedPipe,
  marengoPiSessionBody,
  sessionRefusal,
} from "./motion.js";

/** Upper bound on a calibration session's sleep + reference budget (scriptSleepTotalSec). */
export const MAX_SESSION_SLEEP_SEC = 300;
const FIT_TIMEOUT_MS = 900_000;
const TRACE_FETCH_TIMEOUT_MS = 60_000;

export type Refusal = { ok: false; message: string };

export const skipHangingRestRequired = (tool: string) =>
  `Refused: ${tool} needs skip_hanging_rest_gravity_check: true. The gravity gate's ` +
  "hanging-rest |τ_g| check is skipped only for this tool, because the calibration exists to fix " +
  "the model that check would refuse; its residuals are still reported, and an unavailable " +
  "gravity preview or a measured-torque mismatch still refuses.";

const NEVER_APPLIED_NOTE =
  "Nothing was applied to the Pi. Applying a proposed inertial patch is a separate explicit step: " +
  "review the patch, apply it to assets/urdf/marengo.urdf, then run pi_sync_bench_urdf (ADR 0017). " +
  "Never automatic.";

/** Drop float noise (e.g. 1.2 + 0.05) so hold-at values read cleanly and round-trip. */
export function roundRad(x: number): number {
  return Math.round(x * 1e9) / 1e9;
}

/** One static `hold-at` of the sweep (v1 plan step; v2 adds `kind: "hold"`). */
export interface SweepHold {
  joint: string;
  target_rad: number;
  measure: boolean;
  pose_index?: number;
  approach?: "below" | "above";
}

/**
 * The static sweep over sorted, distinct `poses`: an up pass (min−δ overshoot, then every pose
 * ascending, approached from below) and a down pass (max+δ overshoot, then every pose
 * descending, approached from above). ½Δ between the passes is Coulomb friction, the mean gravity.
 */
export function holdSweepSteps(joint: string, poses: readonly number[], delta: number): SweepHold[] {
  const steps: SweepHold[] = [];
  const last = poses.length - 1;
  steps.push({ joint, target_rad: roundRad(poses[0] - delta), measure: false });
  poses.forEach((q, i) => steps.push({ joint, target_rad: q, measure: true, pose_index: i, approach: "below" }));
  steps.push({ joint, target_rad: roundRad(poses[last] + delta), measure: false });
  for (let i = last; i >= 0; i -= 1) {
    steps.push({ joint, target_rad: poses[i], measure: true, pose_index: i, approach: "above" });
  }
  return steps;
}

/** Poses validated (count, finite, distinct) and sorted ascending. */
export function validatePoses(
  raw: readonly number[],
  minPoses: number,
  maxPoses: number,
): { ok: true; poses: number[] } | Refusal {
  const refuse = (message: string): Refusal => ({ ok: false, message: `Refused: ${message}` });
  if (raw.length < minPoses || raw.length > maxPoses) {
    return refuse(`poses_rad needs ${minPoses}–${maxPoses} entries (got ${raw.length})`);
  }
  if (raw.some((q) => !Number.isFinite(q))) return refuse("poses_rad entries must be finite numbers");
  const poses = [...raw].sort((a, b) => a - b);
  if (poses.some((q, i) => i > 0 && q === poses[i - 1])) {
    return refuse(`poses_rad entries must be distinct (got ${JSON.stringify(raw)})`);
  }
  return { ok: true, poses };
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

export function yamlGet(node: YamlNode | undefined, ...keys: string[]): YamlNode | undefined {
  let cur: YamlNode | undefined = node;
  for (const key of keys) {
    if (cur === null || cur === undefined || typeof cur !== "object" || Array.isArray(cur)) return undefined;
    cur = cur[key];
  }
  return cur;
}

const YAML_NUMBER = /^[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/;

export function yamlNumber(node: YamlNode | undefined): number | undefined {
  return typeof node === "string" && YAML_NUMBER.test(node) ? Number(node) : undefined;
}

/** A block sequence of scalars, or a one-line flow sequence (`[a, b]`), as strings. */
export function yamlStringList(node: YamlNode | undefined): string[] | undefined {
  if (Array.isArray(node)) {
    return node.every((n): n is string => typeof n === "string") ? node : undefined;
  }
  if (typeof node === "string" && /^\[.*\]$/.test(node)) {
    return node
      .slice(1, -1)
      .split(",")
      .map((s) => unquote(s))
      .filter((s) => s !== "");
  }
  return undefined;
}

/** motors.yaml `motors` item for `joint`, or undefined. */
export function motorEntry(motors: YamlNode, joint: string): YamlNode | undefined {
  const list = yamlGet(motors, "motors");
  return Array.isArray(list) ? list.find((m) => yamlGet(m, "joint") === joint) : undefined;
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
  const windows: Record<string, JointWindow> = {};
  for (const joint of joints) {
    const softLo = yamlNumber(yamlGet(control, "control", "joints", joint, "position_soft_lower_rad"));
    const softHi = yamlNumber(yamlGet(control, "control", "joints", joint, "position_soft_upper_rad"));
    const motor = motorEntry(motors, joint);
    const hardLo = yamlNumber(yamlGet(motor, "bench", "position_lower_rad"));
    const hardHi = yamlNumber(yamlGet(motor, "bench", "position_upper_rad"));
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

/** One commanded position and what it is, for limit checks. */
export interface LimitTarget {
  joint: string;
  rad: number;
  what: string;
  /** Required clearance from each window edge (rad). */
  inset: number;
}

/** Every target inside its joint's window shrunk by its inset, else the first refusal. */
export function checkTargetsInWindows(
  targets: readonly LimitTarget[],
  windows: Readonly<Record<string, JointWindow>>,
): { ok: true } | Refusal {
  for (const t of targets) {
    const w = windows[t.joint];
    if (w === undefined) {
      return { ok: false, message: `Refused: no pose window for ${t.joint}; no motion was run.` };
    }
    const lo = roundRad(w.lower + t.inset);
    const hi = roundRad(w.upper - t.inset);
    if (t.rad < lo || t.rad > hi) {
      const insetNote = t.inset > 0 ? ` shrunk by ${t.inset} rad on each side` : "";
      return {
        ok: false,
        message:
          `Refused: ${t.joint} ${t.what} ${t.rad} rad is outside its allowed window [${lo}, ${hi}] rad ` +
          `(control.yaml soft [${w.soft[0]}, ${w.soft[1]}] ∩ motors.yaml hard [${w.hard[0]}, ${w.hard[1]}]${insetNote}); ` +
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
export function markedCatShell(name: string, pathExpr: string): string {
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
// Session runner

export interface CalibrationDeps {
  execLocal: (command: string, args: string[], opts: { cwd?: string; timeoutMs?: number }) => Promise<RemoteExecResult>;
  writeFile: (file: string, data: string) => Promise<void>;
  /** Workstation file as UTF-8 (rejects with ENOENT when missing). */
  readFile: (file: string) => Promise<string>;
  mkdir: (dir: string) => Promise<void>;
  now: () => Date;
}

export const defaultCalibrationDeps: CalibrationDeps = {
  execLocal: sshExecLocal,
  writeFile: (file, data) => fsWriteFile(file, data, "utf8"),
  readFile: (file) => fsReadFile(file, "utf8"),
  mkdir: async (dir) => {
    await fsMkdir(dir, { recursive: true });
  },
  now: () => new Date(),
};

export type RunRemote = (body: string, timeoutMs?: number) => Promise<string>;
export type AuditMotion = (tool: string, args: Record<string, unknown>, result: string, exitCode: number) => void;

export interface CalibrationRun {
  /** MCP tool name (audit, messages). */
  tool: string;
  /** benchLogWrapper session label. */
  label: string;
  args: Record<string, unknown>;
  profile: BenchProfile;
  /** Joints the gravity gate checks. */
  gateJoints: readonly string[];
  configDir: string;
  /** marengo-pi stdin script (reference, home, enable, motion, return, disable, quit). */
  script: string[];
  /** scriptSleepTotalSec(script), already within MAX_SESSION_SLEEP_SEC. */
  budgetSec: number;
  preflight: PreflightFiles;
  /** Report lines from checks run before the session (prepended to the result). */
  preamble: string[];
  runFit: boolean;
  fitParams: readonly string[];
  /** Run the fit on an incomplete session too (the fitter then uses completed steps only). */
  fitIncomplete: boolean;
  /** plan.json content and whether the session completed, from the session outcome. */
  plan: (ctx: { sessionTs: string; gateReport: string; sessionExit: number; trace: string | undefined }) => {
    json: Record<string, unknown>;
    complete: boolean;
  };
  /** Joints traced every control tick (MARENGO_POSITION_TRACE_FULL_RATE_JOINTS). */
  fullRateJoints?: readonly string[];
  /**
   * Let the gravity gate's hanging-rest |τ_g| mismatch through (default true: calibration exists
   * to fix that model). A motion test keeps the refusal.
   */
  allowHangingRestMismatch?: boolean;
  /** Local output directory under var/ (default `gravity-calibration`). */
  outputSubdir?: string;
  /** Result section header (default `--- gravity calibration ---`). */
  sectionHeader?: string;
  /**
   * Replaces the gravity fit: called once the session's files are written (trace present).
   * Its lines are appended to the result and its exit code is the tool's.
   */
  afterSession?: (ctx: { dir: string; complete: boolean; sessionExit: number }) => Promise<{
    lines: string[];
    exitCode: number;
  }>;
}

/**
 * Gravity gate (hanging-rest mismatch let through, residuals reported), then ONE marengo-pi
 * session, then the trace fetch and the local calibration directory, then `gravity-fit`.
 * A session refused before any motion writes nothing.
 */
export async function runCalibrationSession(
  cfg: MarengoPiConfig,
  runRemote: RunRemote,
  auditMotion: AuditMotion,
  deps: CalibrationDeps,
  run: CalibrationRun,
): Promise<string> {
  const { tool, args, configDir } = run;
  const gravity = await runGravityGate({
    profile: run.profile,
    joints: run.gateJoints,
    snapshotOutput: await runRemote(wrapRemoteWithConfig(cfg, gravityGateSnapshotShell(), configDir), 15_000),
    runPreview: (shell) =>
      runRemote(wrapRemoteWithConfig(cfg, soleCanOwnerShell(shell), configDir), 30_000 + CAN_SESSION_SLACK_MS),
    allowHangingRestMismatch: run.allowHangingRestMismatch ?? true,
  });
  const preamble = run.preamble.length > 0 ? [...run.preamble, ""] : [];
  if (!gravity.ok) {
    const text = [...preamble, gravity.report].join("\n");
    auditMotion(tool, args, text, 1);
    return text;
  }

  const pipeCmd = marengoPiSessionBody(cfg, marengoPiAdmittedPipe(run.script, run.budgetSec + 10));
  const sessionOut = await runRemote(
    benchLogWrapper(cfg, pipeCmd, run.label, configDir, run.fullRateJoints),
    run.budgetSec * 1000 + 30_000 + CAN_SESSION_SLACK_MS,
  );
  const sessionText = [...preamble, gravity.report, sessionOut].join("\n");
  const sessionExit = Number(/\[exit (\d+)\]\s*$/.exec(sessionOut)?.[1] ?? 0);
  const out: string[] = [sessionText, "", run.sectionHeader ?? "--- gravity calibration ---"];
  const finish = (exitCode: number) => {
    if (run.afterSession === undefined) out.push("", NEVER_APPLIED_NOTE);
    const text = out.join("\n");
    auditMotion(tool, args, text, exitCode);
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
  const traceOk = trace !== undefined && trace.startsWith("tick,");

  const dir = path.join(cfg.localRoot, "var", run.outputSubdir ?? "gravity-calibration", session.ts);
  const plan = run.plan({
    sessionTs: session.ts,
    gateReport: gravity.report,
    sessionExit,
    trace: traceOk ? trace : undefined,
  });
  await deps.mkdir(path.join(dir, "config"));
  const files: [string, string][] = [
    ["plan.json", `${JSON.stringify(plan.json, null, 2)}\n`],
    ["pi-marengo.urdf", run.preflight.urdf],
    ["config/robot.yaml", run.preflight.robotYaml],
    ["config/control.yaml", run.preflight.controlYaml],
    ["config/motors.yaml", run.preflight.motorsYaml],
    ["bench-session.txt", `${sessionText}\n`],
  ];
  if (traceOk) files.splice(1, 0, ["position-trace.csv", trace]);
  for (const [name, data] of files) await deps.writeFile(path.join(dir, name), data);
  out.push(`calibration dir: ${dir}`, `wrote: ${files.map(([n]) => n).join(", ")}`);

  if (!traceOk) {
    out.push(`position trace ${session.trace} missing or without a header: no fit was run.`, traceOut);
    return finish(sessionExit || 1);
  }
  if (run.afterSession !== undefined) {
    const after = await run.afterSession({ dir, complete: plan.complete, sessionExit });
    out.push(...after.lines);
    return finish(after.exitCode);
  }
  const failExit = plan.complete ? 0 : sessionExit || 1;
  if (!plan.complete) {
    if (!run.fitIncomplete) {
      out.push(`marengo-pi session exited ${sessionExit}: the sweep may be incomplete, so no fit was run.`);
      return finish(sessionExit);
    }
    out.push(
      `session incomplete (marengo-pi exit ${sessionExit}, or a planned step never reached the trace): ` +
        "plan.json session_complete is false; the fitter uses completed steps only.",
    );
  }
  const fitArgs = [
    "run", "--release", "-q", "-p", "marengo-log-cli", "--", "gravity-fit", "--dir", dir,
    "--out-dir", path.join(cfg.localRoot, "docs/commissioning/calibrations"),
    "--repo-urdf", path.join(cfg.localRoot, "assets/urdf/marengo.urdf"),
    ...run.fitParams.flatMap((param) => ["--fit", param]),
  ];
  const fitCommand = `cd ${shellQuote(cfg.localRoot)} && cargo ${fitArgs
    .map((a) => (/^[A-Za-z0-9_./:=-]+$/.test(a) ? a : shellQuote(a)))
    .join(" ")}`;
  if (!run.runFit) {
    out.push(`run_fit: false — fit later with: ${fitCommand}`);
    return finish(failExit);
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
  return finish(failExit);
}
