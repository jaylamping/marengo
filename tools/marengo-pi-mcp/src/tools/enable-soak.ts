/**
 * pi_enable_soak: no-motion enable reliability check. Each cycle is a fresh marengo-pi
 * process (startup type-24 sync, inherited streams, SetZero blackout, staggered Enable):
 * `home <joints> sign-tested`, `home`, `enable`, dwell, `status`, `disable`, `quit`.
 * One candump covers the whole session; the workstation then runs `marengo-log-cli
 * firmware-timing` on it, whose non_neutral_mit count is the wire-level proof that no
 * cycle ever commanded torque.
 */

import { mkdir as fsMkdir, writeFile as fsWriteFile } from "node:fs/promises";
import path from "node:path";
import { z } from "zod";
import { type MarengoPiConfig, sshTarget } from "../config.js";
import { BENCH_PROFILES, profileMeta } from "../bench-profiles.js";
import { waitCanReleasedShell } from "../can-owner.js";
import { execLocal as sshExecLocal, type RemoteExecResult } from "../ssh.js";
import {
  ADMISSION_REPLY_SEC,
  BENCH_CONFIG_MASTER,
  REFERENCE_OPT_IN_REQUIRED,
  SOLE_CAN_OWNER_NOTE,
  admissionGateShell,
  benchLogWrapper,
  marengoPiLaunchShell,
  marengoPiPipeLine,
  referenceAcquireLine,
  referenceOptInShape,
  scriptSleepTotalSec,
} from "./motion.js";

export const SOAK_TOOL = "pi_enable_soak";

export const DEFAULT_CYCLES = 20;
export const MIN_CYCLES = 1;
export const MAX_CYCLES = 50;
export const DEFAULT_DWELL_SEC = 2;
export const MIN_DWELL_SEC = 0.5;
export const MAX_DWELL_SEC = 10;
/** Refuse plans whose estimated duration exceeds this. */
export const MAX_SOAK_SEC = 480;
/**
 * Typical non-dwell time of one cycle: CAN settle (0.5–2 s), marengo-pi startup, reference
 * acquisition (~0.1 s per joint on the bench), the 650 ms post-SetZero Enable hold and Enable
 * completion, status/disable/quit and process exit.
 */
export const CYCLE_OVERHEAD_SEC = 6;
/** Owner take/restore, pre-session disable, candump start/stop and log archive. */
export const SESSION_OVERHEAD_SEC = 20;
/** "$LOG" poll step of the awaited `enable` (enable → enabled resolution). */
export const ENABLE_POLL_SEC = 0.02;

/** The only stdin verbs a soak cycle may send. */
export const SOAK_VERBS = ["home", "enable", "sleep", "status", "disable", "quit"] as const;
/** marengo-pi verbs that change the control mode away from Disabled (never sent here). */
export const MOTION_VERBS = [
  "hold-on",
  "hold_on",
  "hold-at",
  "hold_at",
  "hold-off",
  "hold_off",
  "gravity-on",
  "gravity_on",
  "gravity-off",
  "gravity_off",
  "torque-cmd",
  "torque_cmd",
  "impedance-on",
  "impedance_on",
  "impedance-off",
  "impedance_off",
  "wave",
] as const;

/**
 * Fault/refusal lines in one cycle's marengo-pi output (grep -E and JS RegExp alike):
 * reference/readiness/enable refusals, unknown stdin, tracing ERROR lines, WARN lines naming a
 * Transport/DriveState/watchdog/homing-verify/fault cause, and any non-zero drive fault word.
 */
export const CYCLE_FAULT_ERE =
  "^(reference [^ ]+ (failed|skipped):|home failed:|enable (failed|blocked|refused):|disable failed:|unknown command:)" +
  "| ERROR " +
  "| WARN .*(Transport|DriveState|[Ww]atchdog|homing verify|[Ff]ault)" +
  "|fault=0x[0-9A-Fa-f]*[1-9A-Fa-f]";

/** Shell-side lines (feeder, CAN settle, owner checks) that also mark a cycle as faulted. */
const SHELL_FAULT_RE =
  /(?:failed|timeout) \(.*\); sending disable\/quit$|can settle: FAIL|refusing to start marengo-pi|still owns CAN/;
const CYCLE_FAULT_RE = new RegExp(CYCLE_FAULT_ERE);

export const CHAPPE_ISOLATION_NOTE =
  "Every soak marengo-pi runs under `env -u MARENGO_CHAPPE_SOCKET`: /etc/marengo/env points it at the " +
  "gateway IPC socket, and the gateway forwards Consul `robot/testing/mit_command_batch` (Testing hold / " +
  "wave) into marengo-pi, which enters Position control and sends non-neutral MIT. Without the socket, " +
  "stdin is the only command source (gateway RobotState is not published during the soak).";

export const NEUTRAL_MIT_NOTE =
  "Wire content while operational=Active and control=Disabled (code-proved): Berthier's tick " +
  "(crates/berthier/src/loop.rs, ControlMode::Disabled branch) sends only kp=0, kd=0, velocity=0, " +
  "torque_ff=0 MIT per active joint, position = current q clamped to the hard limits (a status " +
  "solicit, zero torque); Davout's filter keeps it neutral (keepalive skips the envelope clamp, " +
  "tau_ff rate limit slews from 0 to 0, danger zones only cap) and its stop path sends neutral MIT. " +
  "Only stdin hold/gravity/impedance/torque/wave or a Chappe testing command leave ControlMode::Disabled; " +
  "the soak sends none and has no Chappe socket. The session candump's firmware-timing non_neutral_mit " +
  "count must be 0 (wire-level proof); any non-neutral frame, or no analyzer result, FAILs the soak.";

export interface SoakPlan {
  profile: (typeof BENCH_PROFILES)[number];
  joints: string[];
  operator: string;
  cycles: number;
  dwellSec: number;
  stopOnFault: boolean;
  script: string[];
  estimatedSec: number;
  worstCaseSec: number;
}

type Refusal = { ok: false; message: string };

/** marengo-pi stdin of one soak cycle; throws if a line is not a soak verb. */
export function soakCycleScript(joints: readonly string[], operator: string, dwellSec: number): string[] {
  const script = [referenceAcquireLine(joints), "home", `enable ${operator}`, `sleep ${dwellSec}`, "status", "disable", "quit"];
  for (const line of script) {
    const tokens = line.trim().split(/\s+/);
    if (
      !(SOAK_VERBS as readonly string[]).includes(tokens[0] ?? "") ||
      tokens.some((t) => (MOTION_VERBS as readonly string[]).includes(t))
    ) {
      throw new Error(`soak script line is not a no-motion verb: ${line}`);
    }
  }
  return script;
}

/** Wall budget of one marengo-pi process: sleeps + reference waits + awaited home/enable + slack. */
function cyclePipeTimeoutSec(script: string[]): number {
  return Math.ceil(scriptSleepTotalSec(script) + 2 * ADMISSION_REPLY_SEC + 10);
}

export function estimateSoakSec(cycles: number, dwellSec: number): number {
  return Math.ceil(SESSION_OVERHEAD_SEC + cycles * (CYCLE_OVERHEAD_SEC + dwellSec));
}

export function planSoak(input: {
  profile?: (typeof BENCH_PROFILES)[number];
  cycles?: number;
  dwellSec?: number;
  operator?: string;
  stopOnFault?: boolean;
}): ({ ok: true } & SoakPlan) | Refusal {
  const profile = input.profile ?? "arm_attached";
  const cycles = input.cycles ?? DEFAULT_CYCLES;
  const dwellSec = input.dwellSec ?? DEFAULT_DWELL_SEC;
  const operator = input.operator ?? "bench";
  if (!Number.isInteger(cycles) || cycles < MIN_CYCLES || cycles > MAX_CYCLES) {
    return { ok: false, message: `Refused: cycles must be an integer in [${MIN_CYCLES}, ${MAX_CYCLES}] (got ${cycles}).` };
  }
  if (!Number.isFinite(dwellSec) || dwellSec < MIN_DWELL_SEC || dwellSec > MAX_DWELL_SEC) {
    return { ok: false, message: `Refused: dwell_sec must be in [${MIN_DWELL_SEC}, ${MAX_DWELL_SEC}] (got ${dwellSec}).` };
  }
  if (!/^[A-Za-z0-9_-]+$/.test(operator) || (MOTION_VERBS as readonly string[]).includes(operator)) {
    return {
      ok: false,
      message: `Refused: operator must match [A-Za-z0-9_-]+ and not be a motion verb (got ${JSON.stringify(operator)}).`,
    };
  }
  const estimatedSec = estimateSoakSec(cycles, dwellSec);
  if (estimatedSec > MAX_SOAK_SEC) {
    return {
      ok: false,
      message:
        `Refused: estimated soak duration ${estimatedSec} s (${SESSION_OVERHEAD_SEC} s + ${cycles} × (${CYCLE_OVERHEAD_SEC} s + ` +
        `${dwellSec} s dwell)) exceeds ${MAX_SOAK_SEC} s; use fewer cycles or a shorter dwell_sec.`,
    };
  }
  const joints = [...profileMeta(profile).setZeroJoints];
  const script = soakCycleScript(joints, operator, dwellSec);
  // CAN settle (≤ 4 windows + owner wait) + process budget + release wait + counter reads.
  const worstCaseSec = SESSION_OVERHEAD_SEC + cycles * (5 + cyclePipeTimeoutSec(script) + 5 + 2);
  return {
    ok: true,
    profile,
    joints,
    operator,
    cycles,
    dwellSec,
    stopOnFault: input.stopOnFault ?? false,
    script,
    estimatedSec,
    worstCaseSec,
  };
}

/** Feeder for one cycle: reference wait, awaited home, timed awaited enable, dwell, then the rest. */
function soakFeederShell(script: string[]): string {
  return script
    .map((line) => {
      const verb = line.trim().split(/\s+/)[0];
      if (verb === "enable") {
        return [
          "_soak_t0=$EPOCHREALTIME",
          admissionGateShell(line, ENABLE_POLL_SEC),
          "_soak_t1=$EPOCHREALTIME",
          'echo "soak enable_to_enabled_ms=$(( (${_soak_t1//[!0-9]/} - ${_soak_t0//[!0-9]/}) / 1000 ))" >&2',
        ].join("\n");
      }
      return admissionGateShell(line) ?? marengoPiPipeLine(line);
    })
    .join(";\n");
}

/**
 * Session body (inside benchLogWrapper, which owns CAN, records the candump and tees "$LOG"):
 * `plan.cycles` fresh marengo-pi processes, each after its own CAN settle, each with Chappe
 * IPC removed. Each cycle's output is also tee'd to a private cycle log the feeder polls.
 */
export function soakSessionBody(cfg: MarengoPiConfig, plan: SoakPlan): string {
  const subset = profileMeta(plan.profile).jointSubset;
  const pipeTimeout = cyclePipeTimeoutSec(plan.script);
  return [
    ...(subset ? [`export MARENGO_JOINT_SUBSET=${subset.join(",")}`] : []),
    "SOAK_FAILED=0",
    "soak_can0() {",
    '  local _s="${MARENGO_CAN_SYSFS:-/sys/class/net}/can0/statistics"',
    `  printf 'soak cycle %s can0 %s rx_over_errors=%s rx_errors=%s\\n' "$1" "$2" "$(cat "$_s/rx_over_errors" 2>/dev/null || echo NA)" "$(cat "$_s/rx_errors" 2>/dev/null || echo NA)"`,
    "}",
    `for _soak_i in $(seq 1 ${plan.cycles}); do`,
    `echo "=== soak cycle $_soak_i/${plan.cycles} begin ==="`,
    'soak_can0 "$_soak_i" before',
    "(",
    'LOG="$(mktemp "${TMPDIR:-/tmp}/enable-soak-cycle.XXXXXX")"',
    "trap 'rm -f \"$LOG\"' EXIT",
    marengoPiLaunchShell(cfg),
    "set +e",
    `{\n${soakFeederShell(plan.script)};\n} | timeout ${pipeTimeout} env -u MARENGO_CHAPPE_SOCKET "$PI_BIN" 2>&1 | tee -a "$LOG"`,
    '_soak_status=("${PIPESTATUS[@]}")',
    'echo "can errors after marengo-pi: $(can_error_counters)"',
    waitCanReleasedShell(25),
    'echo "soak cycle $_soak_i feeder=${_soak_status[0]} marengo_pi=${_soak_status[1]}"',
    'if [[ "${_soak_status[0]}" != 0 || "${_soak_status[1]}" != 0 ]]; then exit 1; fi',
    `if grep -Eq '${CYCLE_FAULT_ERE}' "$LOG"; then exit 3; fi`,
    "exit 0",
    ")",
    "_soak_rc=$?",
    'soak_can0 "$_soak_i" after',
    `echo "=== soak cycle $_soak_i/${plan.cycles} end exit=$_soak_rc ==="`,
    'if [[ "$_soak_rc" != 0 ]]; then',
    "  SOAK_FAILED=$((SOAK_FAILED + 1))",
    ...(plan.stopOnFault
      ? ['  echo "soak stop_on_fault: stopping after cycle $_soak_i"', "  break"]
      : []),
    "fi",
    "done",
    'echo "soak failed cycles: $SOAK_FAILED"',
    'if [[ "$SOAK_FAILED" != 0 ]]; then exit 1; fi',
    "exit 0",
  ].join("\n");
}

// ---------------------------------------------------------------------------
// Parsing

export interface CanCounters {
  rxOver: number;
  rxErrors: number;
}

export type FaultKind =
  | "Transport"
  | "DriveState"
  | "homing_verify"
  | "watchdog"
  | "can_settle"
  | "can_owner"
  | "reference"
  | "enable_refused"
  | "drive_fault"
  | "script"
  | "error";

export interface SoakCycle {
  index: number;
  referencesAcquired: number;
  referencesExpected: number;
  enableLine?: string;
  enabled: boolean;
  enableMs?: number;
  faults: string[];
  faultKinds: FaultKind[];
  can0Before?: CanCounters;
  can0After?: CanCounters;
  feederStatus?: number;
  marengoPiStatus?: number;
  exit?: number;
  clean: boolean;
}

export function classifyFault(line: string): FaultKind {
  if (/Transport/.test(line)) return "Transport";
  if (/DriveState/.test(line)) return "DriveState";
  if (/homing verify|home failed:/.test(line)) return "homing_verify";
  if (/[Ww]atchdog/.test(line)) return "watchdog";
  if (/can settle: FAIL|refusing to start marengo-pi/.test(line)) return "can_settle";
  if (/still owns CAN/.test(line)) return "can_owner";
  if (/^reference |^reference acquisition /.test(line)) return "reference";
  if (/^enable (failed|blocked|refused):|^enable (failed|timeout) \(/.test(line)) return "enable_refused";
  if (/fault=0x/.test(line)) return "drive_fault";
  if (/^unknown command:/.test(line)) return "script";
  return "error";
}

function parseCounters(text: string, index: number, when: "before" | "after"): CanCounters | undefined {
  const m = new RegExp(`^soak cycle ${index} can0 ${when} rx_over_errors=(\\d+) rx_errors=(\\d+)$`, "m").exec(text);
  return m ? { rxOver: Number(m[1]), rxErrors: Number(m[2]) } : undefined;
}

/** Per-cycle records from the session output (cycle markers emitted by soakSessionBody). */
export function parseSoakCycles(output: string, referencesExpected: number): SoakCycle[] {
  const begin = /^=== soak cycle (\d+)\/\d+ begin ===$/gm;
  const starts: { index: number; at: number }[] = [];
  for (let m = begin.exec(output); m !== null; m = begin.exec(output)) {
    starts.push({ index: Number(m[1]), at: m.index });
  }
  return starts.map(({ index, at }, i) => {
    const text = output.slice(at, starts[i + 1]?.at ?? output.length);
    const lines = text.split("\n").map((l) => l.replace(/\r$/, ""));
    const refs = new Set<string>();
    for (const line of lines) {
      const r = /^reference (\S+) current /.exec(line);
      if (r) refs.add(r[1]);
    }
    const enableLine = lines.find((l) =>
      /^enabled \(operator=|^enable (?:failed|blocked|refused):|^enable (?:failed|timeout) \(.*\); sending disable\/quit$/.test(l),
    );
    const enabled = enableLine?.startsWith("enabled (operator=") ?? false;
    const ms = /^soak enable_to_enabled_ms=(\d+)$/m.exec(text);
    const faults = [...new Set(lines.filter((l) => CYCLE_FAULT_RE.test(l) || SHELL_FAULT_RE.test(l)))];
    const statuses = new RegExp(`^soak cycle ${index} feeder=(\\d+) marengo_pi=(\\d+)$`, "m").exec(text);
    const end = new RegExp(`^=== soak cycle ${index}/\\d+ end exit=(\\d+) ===$`, "m").exec(text);
    const exit = end ? Number(end[1]) : undefined;
    const cycle: SoakCycle = {
      index,
      referencesAcquired: refs.size,
      referencesExpected,
      enableLine,
      enabled,
      enableMs: ms ? Number(ms[1]) : undefined,
      faults,
      faultKinds: faults.map(classifyFault),
      can0Before: parseCounters(text, index, "before"),
      can0After: parseCounters(text, index, "after"),
      feederStatus: statuses ? Number(statuses[1]) : undefined,
      marengoPiStatus: statuses ? Number(statuses[2]) : undefined,
      exit,
      clean: false,
    };
    cycle.clean =
      exit === 0 &&
      enabled &&
      refs.size === referencesExpected &&
      faults.length === 0;
    return cycle;
  });
}

// ---------------------------------------------------------------------------
// firmware-timing analyzer (marengo-log-cli firmware-timing --json)

const statsSchema = z.object({
  n: z.number().int(),
  min: z.number(),
  p50: z.number(),
  p95: z.number(),
  max: z.number(),
});
export type Stats = z.infer<typeof statsSchema>;

const driveSchema = z.object({
  enable_to_run_ms: statsSchema,
  enable_never_run: z.number().int(),
  reset_after_enable: z.number().int(),
  set_zero_silence_start_ms: statsSchema,
  set_zero_silence_ms: statsSchema,
  identity_reply_ms: statsSchema,
  identity_unanswered: z.number().int(),
  param_read_reply_ms: statsSchema,
  report_period_ms: statsSchema,
  report_off_to_last_ms: statsSchema,
  mit_reply_ms: statsSchema,
  mit_unanswered: z.number().int(),
  disable_to_reset_ms: statsSchema,
});

export const firmwareTimingSchema = z.object({
  files: z.array(z.string()),
  frames: z.number().int(),
  span_s: z.number(),
  drives: z.record(driveSchema),
  non_neutral_mit: z.object({
    count: z.number().int(),
    first_s: z.number().nullable(),
    last_s: z.number().nullable(),
  }),
  bus: z.object({
    max_frames_per_10ms: z.number().int(),
    gaps_over_5ms: z.number().int(),
  }),
});
export type FirmwareTiming = z.infer<typeof firmwareTimingSchema>;

export function parseFirmwareTiming(stdout: string): { ok: true; timing: FirmwareTiming } | Refusal {
  let raw: unknown;
  try {
    raw = JSON.parse(stdout.trim());
  } catch (err) {
    return { ok: false, message: `firmware-timing output is not JSON: ${String(err)}` };
  }
  const parsed = firmwareTimingSchema.safeParse(raw);
  if (!parsed.success) {
    const issue = parsed.error.issues[0];
    return {
      ok: false,
      message: `firmware-timing JSON does not match the contract: ${issue?.path.join(".") ?? ""} ${issue?.message ?? ""}`.trim(),
    };
  }
  return { ok: true, timing: parsed.data };
}

const fmt = (x: number) => (Number.isInteger(x) ? String(x) : x.toFixed(1));
const statsLine = (s: Stats) => (s.n === 0 ? "n=0" : `n=${s.n} p50=${fmt(s.p50)} p95=${fmt(s.p95)} max=${fmt(s.max)}`);

export function formatFirmwareTiming(t: FirmwareTiming): string[] {
  const out = [
    `frames=${t.frames} span=${t.span_s.toFixed(1)} s non_neutral_mit=${t.non_neutral_mit.count}` +
      (t.non_neutral_mit.count > 0 ? ` (first ${t.non_neutral_mit.first_s ?? "?"} s, last ${t.non_neutral_mit.last_s ?? "?"} s)` : "") +
      ` bus max_frames_per_10ms=${t.bus.max_frames_per_10ms} gaps_over_5ms=${t.bus.gaps_over_5ms}`,
  ];
  for (const [drive, d] of Object.entries(t.drives).sort(([a], [b]) => a.localeCompare(b))) {
    out.push(
      `  ${drive}: enable→run ms ${statsLine(d.enable_to_run_ms)}; never_run=${d.enable_never_run} ` +
        `reset_after_enable=${d.reset_after_enable}; set_zero silence ms ${statsLine(d.set_zero_silence_ms)} ` +
        `(starts ${statsLine(d.set_zero_silence_start_ms)}); identity_unanswered=${d.identity_unanswered} ` +
        `mit_unanswered=${d.mit_unanswered}`,
    );
  }
  return out;
}

// ---------------------------------------------------------------------------
// Verdict and report

export type AnalyzerResult =
  | { status: "ok"; timing: FirmwareTiming }
  | { status: "unavailable"; reason: string };

export interface SoakVerdict {
  pass: boolean;
  reasons: string[];
  cleanCycles: number;
  faultKinds: Record<string, number>;
  rxOverDelta?: number;
  rxErrorsDelta?: number;
}

export function evaluateSoak(cycles: SoakCycle[], requested: number, analyzer: AnalyzerResult): SoakVerdict {
  const reasons: string[] = [];
  const cleanCycles = cycles.filter((c) => c.clean).length;
  const faultKinds: Record<string, number> = {};
  for (const c of cycles) for (const k of c.faultKinds) faultKinds[k] = (faultKinds[k] ?? 0) + 1;
  if (cycles.length < requested) reasons.push(`${cycles.length}/${requested} cycles ran`);
  if (cleanCycles < cycles.length) reasons.push(`${cycles.length - cleanCycles} unclean cycle(s)`);
  const first = cycles[0]?.can0Before;
  const last = cycles.at(-1)?.can0After;
  const rxOverDelta = first && last ? last.rxOver - first.rxOver : undefined;
  const rxErrorsDelta = first && last ? last.rxErrors - first.rxErrors : undefined;
  if (rxOverDelta === undefined) reasons.push("can0 rx_over_errors not read before and after");
  else if (rxOverDelta > 0) reasons.push(`can0 rx_over_errors +${rxOverDelta}`);
  if (analyzer.status !== "ok") {
    reasons.push(`wire-level neutral-MIT check unavailable (${analyzer.reason})`);
  } else if (analyzer.timing.non_neutral_mit.count > 0) {
    reasons.push(`non_neutral_mit=${analyzer.timing.non_neutral_mit.count} on the wire`);
  }
  return { pass: reasons.length === 0 && cycles.length > 0, reasons, cleanCycles, faultKinds, rxOverDelta, rxErrorsDelta };
}

const clip = (s: string, n = 200) => (s.length > n ? `${s.slice(0, n - 1)}…` : s);

export function formatSoakReport(cycles: SoakCycle[], verdict: SoakVerdict): string[] {
  const out = [
    "| cycle | refs | enable | enable→enabled ms | faults | can0 rx_over Δ | rx_errors Δ | exit |",
    "|---:|---:|---|---:|---|---:|---:|---:|",
  ];
  for (const c of cycles) {
    const enable = c.enabled ? "enabled" : clip(c.enableLine ?? "no enable reply", 80);
    const [overDelta, errorsDelta] =
      c.can0Before && c.can0After
        ? [c.can0After.rxOver - c.can0Before.rxOver, c.can0After.rxErrors - c.can0Before.rxErrors]
        : ["?", "?"];
    out.push(
      `| ${c.index} | ${c.referencesAcquired}/${c.referencesExpected} | ${enable} | ${c.enableMs ?? "-"} | ` +
        `${c.faultKinds.length ? [...new Set(c.faultKinds)].join(",") : "-"} | ${overDelta} | ${errorsDelta} | ${c.exit ?? "?"} |`,
    );
  }
  const ms = cycles.flatMap((c) => (c.enableMs === undefined ? [] : [c.enableMs])).sort((a, b) => a - b);
  out.push(
    "",
    `clean cycles: ${verdict.cleanCycles}/${cycles.length}`,
    `fault kinds: ${Object.keys(verdict.faultKinds).length ? Object.entries(verdict.faultKinds).map(([k, n]) => `${k}×${n}`).join(", ") : "none"}`,
    `can0 overrun delta (rx_over_errors): ${verdict.rxOverDelta ?? "unknown"}; rx_errors delta: ${verdict.rxErrorsDelta ?? "unknown"}`,
    ms.length
      ? `enable→enabled ms (±${ENABLE_POLL_SEC * 1000} ms poll): min ${ms[0]} p50 ${ms[Math.floor((ms.length - 1) / 2)]} max ${ms.at(-1)}`
      : "enable→enabled ms: none",
  );
  const faulted = cycles.filter((c) => c.faults.length > 0);
  if (faulted.length) {
    out.push("", "fault / refusal lines:");
    for (const c of faulted) for (const f of c.faults) out.push(`  cycle ${c.index}: ${clip(f)}`);
  }
  return out;
}

const REMOTE_PATH = /^\/[A-Za-z0-9_./-]+$/;
const sessionJsonSchema = z.object({
  log: z.string().regex(REMOTE_PATH),
  candump: z.string().optional(),
  ts: z.string().regex(/^\d{8}T\d{6}Z$/),
});

/** `{"log":..,"candump":..,"ts":..}` line benchLogWrapper echoes after the session. */
export function parseSoakSessionJson(output: string): { log: string; candump?: string; ts: string } | undefined {
  const last = output.split("\n").filter((l) => /^\{"log":.*\}\s*$/.test(l)).at(-1);
  if (last === undefined) return undefined;
  let raw: unknown;
  try {
    raw = JSON.parse(last);
  } catch {
    return undefined;
  }
  const parsed = sessionJsonSchema.safeParse(raw);
  if (!parsed.success) return undefined;
  const { log, candump, ts } = parsed.data;
  return { log, candump: candump !== undefined && REMOTE_PATH.test(candump) ? candump : undefined, ts };
}

// ---------------------------------------------------------------------------
// Tool

export interface EnableSoakDeps {
  execLocal: (command: string, args: string[], opts: { cwd?: string; timeoutMs?: number }) => Promise<RemoteExecResult>;
  writeFile: (file: string, data: string) => Promise<void>;
  mkdir: (dir: string) => Promise<void>;
  now: () => Date;
}

const defaultDeps: EnableSoakDeps = {
  execLocal: sshExecLocal,
  writeFile: (file, data) => fsWriteFile(file, data, "utf8"),
  mkdir: async (dir) => {
    await fsMkdir(dir, { recursive: true });
  },
  now: () => new Date(),
};

const SCP_TIMEOUT_MS = 180_000;
const ANALYZER_TIMEOUT_MS = 900_000;

export const enableSoakSchema = z.object({
  confirm: z.literal(true),
  ...referenceOptInShape,
  profile: z.enum(BENCH_PROFILES).default("arm_attached").describe("Bench profile; references its setZeroJoints"),
  cycles: z.number().int().min(MIN_CYCLES).max(MAX_CYCLES).default(DEFAULT_CYCLES),
  dwell_sec: z.number().min(MIN_DWELL_SEC).max(MAX_DWELL_SEC).default(DEFAULT_DWELL_SEC).describe("Enabled dwell per cycle"),
  stop_on_fault: z.boolean().default(false).describe("Stop after the first unclean cycle"),
  operator: z.string().regex(/^[A-Za-z0-9_-]+$/).default("bench"),
});

export type EnableSoakArgs = Partial<z.infer<typeof enableSoakSchema>> & { confirm: true };

export function registerEnableSoakTools(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  auditMotion: (tool: string, args: Record<string, unknown>, result: string, exitCode: number) => void,
  deps: EnableSoakDeps = defaultDeps,
) {
  const scpArgs = (remote: string, local: string) => [
    "-o", "BatchMode=yes", "-o", "ConnectTimeout=15",
    ...(cfg.sshIdentityFile ? ["-i", cfg.sshIdentityFile] : []),
    `${sshTarget(cfg)}:${remote}`, local,
  ];

  return {
    [SOAK_TOOL]: {
      description:
        "No-motion enable reliability soak (run after firmware, wiring or software changes). Each of `cycles` " +
        "(1–50, default 20) runs a FRESH marengo-pi process after its own CAN settle, with stdin exactly " +
        "`home <profile joints> sign-tested` (awaited), `home` (awaited), `enable <operator>` (awaited, timed), " +
        "sleep dwell_sec (0.5–10, default 2), `status`, `disable`, `quit` — never hold/gravity/torque/impedance/wave. " +
        "Every cycle re-zeroes at the current pose: the arm must hang limp at its mechanical reference. Needs " +
        "confirm, set_zero and at_mechanical_reference. Refuses plans estimated over 480 s. " +
        NEUTRAL_MIT_NOTE + " " + CHAPPE_ISOLATION_NOTE + " " +
        "Per cycle: references acquired, enable outcome, fault/refusal lines (Transport, DriveState, homing verify, " +
        "watchdog), enable→enabled ms, can0 rx_over_errors/rx_errors before and after, exit status. One candump " +
        "covers the session; candump and session log are copied to var/enable-soak/<TS>/ and fed to " +
        "`marengo-log-cli firmware-timing --json`. PASS only when every cycle is clean, can0 rx_over_errors did " +
        "not increase and firmware-timing ran and reported non_neutral_mit 0; a missing, failing or " +
        "unparseable analyzer result is FAIL. " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: enableSoakSchema,
      handler: async (args: EnableSoakArgs): Promise<string> => {
        if (args.confirm !== true) return "Motion blocked — user must approve; retry with confirm: true";
        if (args.set_zero !== true || args.at_mechanical_reference !== true) return REFERENCE_OPT_IN_REQUIRED;
        const plan = planSoak({
          profile: args.profile,
          cycles: args.cycles,
          dwellSec: args.dwell_sec,
          operator: args.operator,
          stopOnFault: args.stop_on_fault,
        });
        if (!plan.ok) return plan.message;

        const sessionOut = await runRemote(
          benchLogWrapper(cfg, soakSessionBody(cfg, plan), "enable-soak", BENCH_CONFIG_MASTER),
          plan.worstCaseSec * 1000 + 60_000,
        );
        const cycles = parseSoakCycles(sessionOut, plan.joints.length);
        const out: string[] = [
          `--- ${SOAK_TOOL}: profile ${plan.profile}, ${plan.cycles} cycle(s), dwell ${plan.dwellSec} s, ` +
            `joints ${plan.joints.join(",")}, estimated ${plan.estimatedSec} s ---`,
        ];
        if (cycles.length === 0) {
          out.push("No soak cycle ran. Session output tail:", ...sessionOut.split("\n").slice(-40));
        }

        const session = parseSoakSessionJson(sessionOut);
        const ts = session?.ts ?? deps.now().toISOString().replace(/[-:]/g, "").replace(/\.\d+Z$/, "Z");
        const dir = path.join(cfg.localRoot, "var", "enable-soak", ts);
        await deps.mkdir(dir);
        const notes: string[] = [];
        let analyzer: AnalyzerResult = { status: "unavailable", reason: "no session candump" };

        const logLocal = path.join(dir, "bench-session.log");
        const logCopy = session ? await deps.execLocal("scp", scpArgs(session.log, logLocal), { timeoutMs: SCP_TIMEOUT_MS }) : undefined;
        if (logCopy?.exitCode !== 0) {
          await deps.writeFile(logLocal, sessionOut);
          notes.push(`session log: Pi copy unavailable (${logCopy?.stderr.trim() || "no session JSON line"}); wrote MCP-captured output instead`);
        }
        const candumpLocal = path.join(dir, "candump.log");
        if (session?.candump) {
          const copy = await deps.execLocal("scp", scpArgs(session.candump, candumpLocal), { timeoutMs: SCP_TIMEOUT_MS });
          if (copy.exitCode !== 0) {
            analyzer = { status: "unavailable", reason: `candump copy failed: ${copy.stderr.trim()}` };
          } else {
            const run = await deps.execLocal(
              "cargo",
              ["run", "--release", "-q", "-p", "marengo-log-cli", "--", "firmware-timing", "--json", candumpLocal],
              { cwd: cfg.localRoot, timeoutMs: ANALYZER_TIMEOUT_MS },
            );
            if (run.exitCode !== 0) {
              analyzer = {
                status: "unavailable",
                reason: `firmware-timing exit ${run.exitCode}: ${clip(run.stderr.trim().split("\n").slice(-3).join(" "), 300)}`,
              };
            } else {
              const parsed = parseFirmwareTiming(run.stdout);
              if (parsed.ok) {
                analyzer = { status: "ok", timing: parsed.timing };
                await deps.writeFile(path.join(dir, "firmware-timing.json"), `${JSON.stringify(parsed.timing, null, 2)}\n`);
              } else {
                analyzer = { status: "unavailable", reason: parsed.message };
              }
            }
          }
        }

        const verdict = evaluateSoak(cycles, plan.cycles, analyzer);
        out.push(...formatSoakReport(cycles, verdict), "", "--- firmware-timing ---");
        if (analyzer.status === "ok") out.push(...formatFirmwareTiming(analyzer.timing));
        else out.push(`not run: ${analyzer.reason} (wire-level neutral-MIT check unavailable: FAIL)`);
        out.push(...notes, `soak dir: ${dir}`, "", verdict.pass ? "VERDICT: PASS" : `VERDICT: FAIL — ${verdict.reasons.join("; ")}`);
        const text = out.join("\n");
        await deps.writeFile(path.join(dir, "soak-summary.txt"), `${text}\n`);
        auditMotion(SOAK_TOOL, args, text, verdict.pass ? 0 : 1);
        return text;
      },
    },
  };
}
