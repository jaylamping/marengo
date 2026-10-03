import { z } from "zod";
import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { BENCH_PROFILES, profileMeta } from "../bench-profiles.js";
import { appendAudit } from "../audit.js";
import { shellQuote, wrapRemote, wrapRemoteWithConfig } from "../env.js";
import { effectiveProfile, validateMotionConfirm } from "../safety.js";
import { homingStatusShell } from "../homing-preflight.js";
import { renderRobotStateHoming } from "../robot-state.js";
import {
  REFUSE_UNSETTLED_MARENGO_PI,
  canOwnedSkipLine,
  canOwnerBranch,
  canSettleShell,
  soleCanOwnerShell,
  waitCanReleasedShell,
} from "../can-owner.js";
import { gravityGateSnapshotShell, runGravityGate } from "../gravity-gate.js";

/** Explicit operator opt-in for in-process reference acquisition (SetZero at the current pose). */
export const referenceOptInShape = {
  set_zero: z
    .boolean()
    .default(false)
    .describe(
      "Acquire references in this marengo-pi session (`home <joints> sign-tested`; runs SetZero at the current pose)",
    ),
  at_mechanical_reference: z
    .boolean()
    .default(false)
    .describe("Operator statement that every referenced joint is at its mechanical reference now"),
};

export const REFERENCE_OPT_IN_REQUIRED =
  "Refused: enabling needs a current reference, which marengo-pi grants only inside the session that " +
  "acquires it (`home <joints> sign-tested`), and acquisition runs SetZero at the current pose. " +
  "Place the joints at their mechanical reference, then retry with set_zero: true and " +
  "at_mechanical_reference: true.";

const benchProfileZod = z.enum(BENCH_PROFILES);

export const motionConfirmSchema = z.object({
  confirm: z.literal(true),
  confirm_weighted_motion: z.literal(true).optional(),
  profile: benchProfileZod.optional(),
});

export const BENCH_CONFIG_MASTER = "/opt/marengo/config";

/** Keep newest N timestamped bench artifacts; symlinks (bench-latest.*) untouched. */
export const BENCH_LOG_KEEP_COUNT = 50;

export function benchLogArchiveShell(
  piRoot: string,
  keep = BENCH_LOG_KEEP_COUNT,
): string {
  const root = shellQuote(piRoot);
  return [
    `# archive hot bench logs (keep ${keep})`,
    `if command -v marengo-log-cli >/dev/null 2>&1; then`,
    `  MARENGO_ROOT=${root} marengo-log-cli session register \\`,
    `    --id "$TS" --label "$LABEL" \\`,
    `    --bench "$LOG" \\`,
    `    --candump "\${CANDUMP:-}" \\`,
    `    --trace "$TRACE" || true`,
    `  MARENGO_ROOT=${root} marengo-log-cli session finalize --id "$TS" || true`,
    `  MARENGO_ROOT=${root} marengo-log-cli archive --keep ${keep} || true`,
    `else`,
    ...benchLogPruneShell("$LOGDIR", keep).split("\n"),
    `fi`,
  ].join("\n");
}

export function benchLogPruneShell(logDirVar = "$LOGDIR", keep = BENCH_LOG_KEEP_COUNT): string {
  return [
    `# prune old bench logs/traces (keep ${keep} newest each)`,
    // ls exits 2 for a pattern with no files (no bench-*.json today); under the session's
    // `set -euo pipefail` that ended the wrapper before its session JSON line.
    `for _pat in bench-*.log position-trace-*.csv bench-*.json candump-*.log; do`,
    `  { ls -1t ${logDirVar}/$_pat 2>/dev/null || true; } | tail -n +${keep + 1} | while IFS= read -r _f; do rm -f "$_f"; done`,
    `done`,
  ].join("\n");
}

/** Snapshot CAN kernel RX/TX packet counters for UP interfaces. */
export function benchCanKernelSnapshotShell(kind: "start" | "end"): string {
  const varName = kind === "start" ? "CAN_KERNEL_START" : "CAN_KERNEL_END";
  return [
    `: > "$${varName}"`,
    "for _if in can0 can1 can2; do",
    '  ip link show "$_if" 2>/dev/null | grep -q " state UP " || continue',
    '  _rx=$(ip -statistics link show dev "$_if" 2>/dev/null | awk \'/^[[:space:]]+RX:/ {getline; print $2}\')',
    '  _tx=$(ip -statistics link show dev "$_if" 2>/dev/null | awk \'/^[[:space:]]+TX:/ {getline; print $2}\')',
    `  printf '%s %s %s\\n' "$_if" "$_rx" "$_tx" >> "$${varName}"`,
    "done",
    `echo "can kernel ${kind}: $(tr '\\n' ' ' < "$${varName}" 2>/dev/null || true)" | tee -a "$LOG"`,
  ].join("\n");
}

/** Log per-interface kernel packet rates using start/end snapshots. */
export function benchCanKernelDeltaShell(): string {
  return [
    'if [ -f "$CAN_KERNEL_START" ] && [ -f "$CAN_KERNEL_END" ]; then',
    '  _dur="${CANDUMP_DURATION_SEC:-}"',
    '  if [ -z "$_dur" ] && [ -f "${CANDUMP:-}" ]; then',
    '    _first=$(grep -m1 -E "^[[:space:]]*\\(" "$CANDUMP" 2>/dev/null | sed -n \'s/^[[:space:]]*(\\([0-9.]*\\)).*/\\1/p\' || true)',
    '    _last=$(grep -E "^[[:space:]]*\\(" "$CANDUMP" 2>/dev/null | tail -n 1 | sed -n \'s/^[[:space:]]*(\\([0-9.]*\\)).*/\\1/p\' || true)',
    '    _dur=$(awk -v a="$_first" -v b="$_last" \'BEGIN { if (a != "" && b != "" && b>a) printf "%.3f", b-a; else print "0" }\')',
    '  fi',
    '  while read -r _if _rx0 _tx0; do',
    '    [ -n "$_if" ] || continue',
    '    _line=$(grep "^$_if " "$CAN_KERNEL_END" || true)',
    '    [ -n "$_line" ] || continue',
    '    _rx1=$(printf "%s" "$_line" | awk \'{print $2}\')',
    '    _tx1=$(printf "%s" "$_line" | awk \'{print $3}\')',
    '    _rx0=${_rx0:-0}; _tx0=${_tx0:-0}; _rx1=${_rx1:-0}; _tx1=${_tx1:-0}',
    '    _drx=$((_rx1 - _rx0))',
    '    _dtx=$((_tx1 - _tx0))',
    '    if [ -n "$_dur" ] && [ "$_dur" != "0" ]; then',
    '      _hz=$(awk -v t="$_drx + $_dtx" -v s="$_dur" \'BEGIN { printf "%.1f", t/s }\')',
    '      echo "can kernel delta $_if: drx=$_drx dtx=$_dtx total=${_hz}/s (${_dur}s window)" | tee -a "$LOG"',
    '    else',
    '      echo "can kernel delta $_if: drx=$_drx dtx=$_dtx" | tee -a "$LOG"',
    '    fi',
    '  done < "$CAN_KERNEL_START"',
    "fi",
  ].join("\n");
}

/** Start background candump on all UP CAN interfaces for the bench session. */
export function benchCandumpStartShell(): string {
  return [
    'CAN_KERNEL_START="$LOGDIR/can-kernel-$TS.start"',
    'CAN_KERNEL_END="$LOGDIR/can-kernel-$TS.end"',
    benchCanKernelSnapshotShell("start"),
    'CANDUMP="$LOGDIR/candump-$TS.log"',
    'CANDUMP_ARGS=""',
    "for _if in can0 can1 can2; do",
    '  ip link show "$_if" 2>/dev/null | grep -q " state UP " && CANDUMP_ARGS="$CANDUMP_ARGS $_if"',
    "done",
    'if [ -n "$CANDUMP_ARGS" ]; then',
    '  candump -t z $CANDUMP_ARGS > "$CANDUMP" 2>&1 &',
    "  CANDUMP_PID=$!",
    '  echo "candump recording:$CANDUMP_ARGS -> $CANDUMP" | tee -a "$LOG"',
    "else",
    '  echo "candump skipped (no UP CAN interfaces)" | tee -a "$LOG"',
    "  CANDUMP_PID=",
    "fi",
  ].join("\n");
}

/** Stop session candump and link candump-latest.log. */
export function benchCandumpStopShell(): string {
  return [
    'if [ -n "${CANDUMP_PID:-}" ]; then',
    '  kill "$CANDUMP_PID" 2>/dev/null || true',
    '  wait "$CANDUMP_PID" 2>/dev/null || true',
    "fi",
    benchCanKernelSnapshotShell("end"),
    benchCanKernelDeltaShell(),
    'if [ -f "${CANDUMP:-}" ]; then',
    '  _lines=$(wc -l < "$CANDUMP" | tr -d " ")',
    '  echo "candump ${_lines} frames -> $CANDUMP" | tee -a "$LOG"',
    '  ln -sf "$CANDUMP" "$LOGDIR/candump-latest.log"',
    "fi",
  ].join("\n");
}

function normalizeBenchConfigDir(cfg: MarengoPiConfig, configDir: string): string {
  if (configDir.startsWith("/") || configDir.startsWith("~/")) {
    return configDir;
  }
  return cfg.configDir;
}

/** Master config when config_dir omitted (legacy bringup slugs ignored). */
export function benchConfigDirForJoint(
  cfg: MarengoPiConfig,
  _joint?: string,
  configDir?: string,
): string | undefined {
  if (configDir) {
    return normalizeBenchConfigDir(cfg, configDir);
  }
  return BENCH_CONFIG_MASTER;
}

export const benchLogWrapper = (
  cfg: MarengoPiConfig,
  pipeCmd: string,
  label: string,
  configDir?: string,
) => {
  const logDir = `${cfg.piRoot}/var/log`;
  const body = soleCanOwnerShell([
    `LOGDIR=${shellQuote(logDir)}`,
    'mkdir -p "$LOGDIR"',
    'TS=$(date -u +"%Y%m%dT%H%M%SZ")',
    'LOG="$LOGDIR/bench-$TS.log"',
    'TRACE="$LOGDIR/position-trace-$TS.csv"',
    'export MARENGO_POSITION_TRACE="$TRACE"',
    'export MARENGO_POSITION_TRACE_HZ="${MARENGO_POSITION_TRACE_HZ:-50}"',
    'export MARENGO_LOG_SESSION_ID="$TS"',
    `LABEL=${shellQuote(label)}`,
    // The one pre-session disable: stops every drive, including joints this marengo-pi session
    // will not reference. marengo-pi launches only after canSettleShell (marengoPiLaunchShell).
    "bin/motor-repl disable 2>/dev/null || true",
    'echo "=== bench session $TS ($LABEL) ===" | tee "$LOG"',
    benchCandumpStartShell(),
    "set +e",
    "{",
    pipeCmd,
    '} 2>&1 | tee -a "$LOG"',
    "PIPE_STATUS=${PIPESTATUS[0]}",
    "set -e",
    benchCandumpStopShell(),
    'ln -sf "$LOG" "$LOGDIR/bench-latest.log"',
    'ln -sf "$TRACE" "$LOGDIR/position-trace-latest.csv"',
    benchLogArchiveShell(cfg.piRoot),
    'echo "{\"log\":\"$LOG\",\"trace\":\"$TRACE\",\"candump\":\"${CANDUMP:-}\",\"ts\":\"$TS\",\"label\":\"$LABEL\"}"',
    'exit "$PIPE_STATUS"',
  ].join("\n"));
  return configDir
    ? wrapRemoteWithConfig(cfg, body, configDir)
    : wrapRemote(cfg, body, false);
};

/**
 * Installed binary first, staging build only when it is missing. Selection never runs
 * marengo-pi: stdin commands (even `help`) are read only after it has opened SocketCAN.
 */
function marengoPiBinarySelector(cfg: MarengoPiConfig): string {
  const fallback = cfg.piStagingRoot.startsWith("~/")
    ? `"${"$HOME"}${cfg.piStagingRoot.slice(1)}/target/release/marengo-pi"`
    : shellQuote(`${cfg.piStagingRoot}/target/release/marengo-pi`);
  return [
    'PI_BIN=bin/marengo-pi',
    `PI_FALLBACK=${fallback}`,
    'if ! test -x "$PI_BIN" && test -x "$PI_FALLBACK"; then',
    '  PI_BIN="$PI_FALLBACK"',
    "fi",
  ].join("\n");
}

/**
 * Last step before every marengo-pi pipe: pick the binary, then wait for the bus to settle
 * (canSettleShell). Refuses with exit 1 rather than start marengo-pi into a CAN error frame.
 */
export function marengoPiLaunchShell(cfg: MarengoPiConfig): string {
  return [marengoPiBinarySelector(cfg), canSettleShell(REFUSE_UNSETTLED_MARENGO_PI)].join("\n");
}

/** Default dwell at target before hold-at 0 (Layer 2 0.1 rad gate: brief settle). */
const DEFAULT_HOLD_DWELL_SEC = 5;

/** Default wait for return to home after hold-at 0 (≤5 s motion budget + slack). */
const DEFAULT_RETURN_HOME_SEC = 6;

/** Default total marengo-pi pipe timeout (includes script `sleep N` lines). */
const DEFAULT_MOTION_TIMEOUT_SEC = DEFAULT_HOLD_DWELL_SEC;

/** SSH wrapper slack beyond pipe timeout. */
const REMOTE_SSH_SLACK_MS = 10_000;

/**
 * soleCanOwnerShell overhead: helper stop + owner wait, the pre-launch CAN settle, the
 * post-session release wait and the settle before restoring marengo-pi.service.
 */
export const CAN_SESSION_SLACK_MS = 20_000;

export const SOLE_CAN_OWNER_NOTE =
  "Runs as sole CAN owner: stops marengo-pi.service via the pi_restart_marengo_pi helper, " +
  "refuses if any marengo-pi/motor-repl remains, restarts the unit afterwards if it was active.";

const GRAVITY_GATE_NOTE =
  "Before any enable, a gravity-model gate runs motor-repl gravity-preview (as sole CAN owner) and refuses " +
  "with `FAIL gravity_model_mismatch` when |τ_meas − τ_g| ≥ 0.20 Nm on any joint (gateway drive torque " +
  "while marengo-pi holds the arm), or else when |τ_g| ≥ 0.20 Nm at the profile's hanging rest pose.";

/**
 * Wait budget per joint for marengo-pi `home <joints> sign-tested`; the backend
 * acquires one joint at a time (stop, identity, SetZero, ack, mechPos readback).
 */
export const REFERENCE_ACQUIRE_SEC_PER_JOINT = 10;

/** Wait for marengo-pi's answer to an awaited `home` readiness check or `enable` line. */
export const ADMISSION_REPLY_SEC = 10;

/**
 * Joints of a marengo-pi `home <joints> sign-tested` line (`sign-tested` / `--sign-tested`
 * anywhere, as marengo-pi accepts), or null when the line is not a reference acquisition.
 */
function referenceAcquireJoints(line: string): string[] | null {
  const [command, ...tokens] = line.trim().split(/\s+/);
  const isAttestation = (t: string) => t === "sign-tested" || t === "--sign-tested";
  const joints = tokens.filter((t) => !isAttestation(t));
  if (command !== "home" || !tokens.some(isAttestation) || joints.length === 0) return null;
  return joints.every((j) => /^[A-Za-z0-9_]+$/.test(j)) ? joints : null;
}

/**
 * marengo-pi stdin line acquiring a qualified current reference for `joints`.
 * The grant lives only in that marengo-pi process, and acquisition runs SetZero
 * at the current pose: the arm must be at the mechanical reference.
 */
export function referenceAcquireLine(joints: readonly string[]): string {
  if (joints.length === 0 || joints.some((j) => !/^[A-Za-z0-9_]+$/.test(j))) {
    throw new Error(`invalid reference joints: ${JSON.stringify(joints)}`);
  }
  return `home ${joints.join(" ")} sign-tested`;
}

/** Sum `sleep N` dwell lines plus the wait budget of reference acquisition lines. */
export function scriptSleepTotalSec(script: string[]): number {
  let total = 0;
  for (const line of script) {
    const sleepMatch = /^sleep (\d+(?:\.\d+)?)$/i.exec(line.trim());
    if (sleepMatch) {
      total += Number(sleepMatch[1]);
    }
    const joints = referenceAcquireJoints(line);
    if (joints) {
      total += joints.length * REFERENCE_ACQUIRE_SEC_PER_JOINT;
    }
  }
  return total;
}

/** After each `wave` line, insert a sleep for in-loop wave duration (+ slack). */
export function expandScriptWithWaveWaits(script: string[]): string[] {
  const out: string[] = [];
  for (const line of script) {
    out.push(line);
    const waveMatch =
      /^wave\s+\S+\s+[\d.]+\s+[\d.]+\s+(\d+)(?:\s+([\d.]+))?\s*$/i.exec(line.trim());
    if (waveMatch) {
      const cycles = Number(waveMatch[1]);
      const halfPeriod =
        waveMatch[2] !== undefined ? Number(waveMatch[2]) : 0.4;
      const waitSec = Math.ceil((cycles * 2 * halfPeriod + 0.15) * 10) / 10;
      out.push(`sleep ${waitSec}`);
    }
  }
  return out;
}

/** Total pipe timeout passed to `timeout(1) marengo-pi`. */
export function marengoPiPipeTimeoutSec(
  _script: string[],
  totalTimeoutSec: number,
): number {
  return Math.ceil(totalTimeoutSec);
}

/** End stdin session so marengo-pi exits instead of running until timeout. */
function ensureScriptQuit(script: string[]): string[] {
  const out = [...script];
  if (out[out.length - 1]?.trim().toLowerCase() !== "quit") {
    out.push("quit");
  }
  return out;
}

/**
 * Feeder shell for an awaited stdin line: send it, then poll marengo-pi's output in "$LOG"
 * until `ok` (a shell test on "$_ref_out", the output since the send) holds, so later lines
 * (and their sleeps) start only once marengo-pi accepted this one. When a line matching
 * `fail` (grep -E) appears or `waitSec` passes it sends `disable` + `quit` and exits 1:
 * no later motion line reaches marengo-pi. Runs inside the stdin feeder: anything but
 * marengo-pi commands goes to stderr.
 */
function awaitReplyShell(opts: {
  line: string;
  label: string;
  subject: string;
  ok: string;
  fail: string;
  waitSec: number;
}): string {
  return [
    '_ref_from=$(wc -l < "$LOG")',
    `printf '%s\\n' ${JSON.stringify(opts.line)}`,
    "_ref_state=timeout",
    `for _ in $(seq ${opts.waitSec * 5}); do`,
    '  _ref_out=$(tail -n "+$((_ref_from + 1))" "$LOG")',
    `  if grep -Eq '${opts.fail}' <<<"$_ref_out"; then _ref_state=failed; break; fi`,
    `  if ${opts.ok}; then _ref_state=ok; break; fi`,
    "  sleep 0.2",
    "done",
    'if [[ "$_ref_state" != ok ]]; then',
    `  echo "${opts.label} $_ref_state (${opts.subject}); sending disable/quit" >&2`,
    "  printf '%s\\n' disable quit",
    "  exit 1",
    "fi",
  ].join("\n");
}

/**
 * `home <joints> sign-tested`: awaited until every joint printed `reference <joint> current`.
 * A failed or skipped joint, `home failed:` or a timeout ends the session.
 */
function referenceAcquireShell(line: string, joints: string[]): string {
  return awaitReplyShell({
    line,
    label: "reference acquisition",
    subject: joints.join(" "),
    ok: joints.map((j) => `grep -q '^reference ${j} current ' <<<"$_ref_out"`).join(" && "),
    fail: "^(reference [^ ]+ (failed|skipped):|home failed:)",
    waitSec: joints.length * REFERENCE_ACQUIRE_SEC_PER_JOINT,
  });
}

/**
 * Awaited `home` readiness check or `enable [...]` line, or null for any other line. marengo-pi
 * answers `homing verified → Ready` / `home failed:` and `enabled (operator=…)` /
 * `enable failed:` / `enable blocked:` / `enable refused:`.
 */
function admissionGateShell(line: string): string | null {
  const trimmed = line.trim();
  if (trimmed === "home") {
    return awaitReplyShell({
      line: trimmed,
      label: "home",
      subject: "readiness check",
      ok: `grep -q '^homing verified ' <<<"$_ref_out"`,
      fail: "^home failed:",
      waitSec: ADMISSION_REPLY_SEC,
    });
  }
  if (trimmed.split(/\s+/)[0] === "enable") {
    return awaitReplyShell({
      line: trimmed,
      label: "enable",
      subject: trimmed,
      ok: `grep -q '^enabled (operator=' <<<"$_ref_out"`,
      fail: "^enable (failed|blocked|refused):",
      waitSec: ADMISSION_REPLY_SEC,
    });
  }
  return null;
}

/**
 * First line of a session's output saying marengo-pi refused the reference, readiness check or
 * enable (or the awaited-feeder line reporting that it ended the session), or undefined.
 */
export function sessionRefusal(output: string): string | undefined {
  return /^(?:reference \S+ (?:failed|skipped):|home failed:|enable (?:failed|blocked|refused):|.* (?:failed|timeout) \(.*\); sending disable\/quit$).*$/m.exec(
    output,
  )?.[0];
}

/** One feeder entry: shell sleep, awaited reference acquisition, or a printf'd stdin line. */
export function marengoPiPipeLine(line: string): string {
  const sleepMatch = /^sleep (\d+(?:\.\d+)?)$/i.exec(line.trim());
  if (sleepMatch) {
    return `sleep ${sleepMatch[1]}`;
  }
  const joints = referenceAcquireJoints(line);
  if (joints) {
    return referenceAcquireShell(line.trim(), joints);
  }
  return `printf '%s\\n' ${JSON.stringify(line)}`;
}

export function marengoPiPipe(script: string[], pipeTimeoutSec: number, binary = "$PI_BIN"): string {
  const pipeLines = script.map(marengoPiPipeLine).join(";\n");
  return `{\n${pipeLines};\n} | timeout ${pipeTimeoutSec} ${binary}`;
}

/**
 * {@link marengoPiPipe} whose `home` readiness check and `enable` lines are awaited too: a
 * refused or unanswered one ends the session (disable + quit) before any later motion line.
 */
export function marengoPiAdmittedPipe(script: string[], pipeTimeoutSec: number): string {
  const pipeLines = script
    .map((line) => admissionGateShell(line) ?? marengoPiPipeLine(line))
    .join(";\n");
  return `{\n${pipeLines};\n} | timeout ${pipeTimeoutSec} $PI_BIN`;
}

function marengoPiTimedPipe(
  script: string[],
  dwellSec: number,
  returnHomeSec: number,
  returnJoint?: string,
): string {
  const returnHold =
    returnJoint !== undefined ? `hold-at ${returnJoint} 0` : "hold-at 0";
  const lines = [
    ...script,
    `sleep ${dwellSec}`,
    returnHold,
    `sleep ${returnHomeSec}`,
    "status",
    "disable",
    "quit",
  ];
  return marengoPiAdmittedPipe(lines, scriptSleepTotalSec(lines) + 10);
}

/**
 * marengo-pi session body (inside benchLogWrapper, after its pre-session disable). Every drive
 * is already disabled and marengo-pi starts Disabled, so no motor-repl runs between the settle
 * and marengo-pi (`pipeCmd`, a {@link marengoPiPipe}). The trailing motor-repl disable runs only
 * once marengo-pi has released can0; it never becomes a second writer beside a marengo-pi still
 * shutting down.
 */
export function marengoPiSessionBody(cfg: MarengoPiConfig, pipeCmd: string): string {
  return [
    marengoPiLaunchShell(cfg),
    "set +e",
    pipeCmd,
    "PIPE_STATUS=$?",
    'echo "can errors after marengo-pi: $(can_error_counters)"',
    waitCanReleasedShell(25),
    "set -e",
    canOwnerBranch(
      "bin/motor-repl disable",
      [canOwnedSkipLine("post-session bin/motor-repl disable"), "PIPE_STATUS=1"].join("\n"),
    ),
    'exit "$PIPE_STATUS"',
  ].join("\n");
}

/** Hold session body: reference, home, enable, hold, return to 0, disable ({@link marengoPiSessionBody}). */
export function holdSessionRemoteBody(
  cfg: MarengoPiConfig,
  args: {
    joint: string;
    referenceJoints: string[];
    operator: string;
    positionRad?: number;
    timeoutSec: number;
    returnHomeSec: number;
  },
): string {
  const holdLine =
    args.positionRad !== undefined
      ? `hold-at ${args.joint} ${args.positionRad}`
      : "hold-on";
  return marengoPiSessionBody(
    cfg,
    marengoPiTimedPipe(
      [referenceAcquireLine(args.referenceJoints), "home", `enable ${args.operator}`, holdLine],
      args.timeoutSec,
      args.returnHomeSec,
      args.joint,
    ),
  );
}

/**
 * Disable drives (clears most Robstride faults), then read fault= from marengo-pi
 * `status` while Disabled (type-24 reporting). No reference, no enable: after a
 * fault the arm is not attested at the mechanical reference.
 */
function motorRecoverRemoteBody(cfg: MarengoPiConfig): string {
  return [
    "bin/motor-repl disable 2>/dev/null || true",
    marengoPiLaunchShell(cfg),
    '{ sleep 1; echo status; echo disable; echo quit; } | timeout 10 "$PI_BIN"',
  ].join("\n");
}

/** Parse bench log for fault= lines; printed after tee to $LOG. */
const motorRecoverSummaryShell = [
  'echo "--- RECOVER_SUMMARY ---"',
  'grep -E "fault=0x|operational:|enabled|disabled" "$LOG" 2>/dev/null | tail -15 || true',
  'if grep -qE "fault=0x[0-9a-fA-F]*[1-9a-fA-F]" "$LOG" 2>/dev/null; then',
  '  echo "RECOVER_FAIL: non-zero motor fault — power-cycle arm drive, then run pi_motor_recover again"',
  'elif grep -q "fault=0x0000" "$LOG" 2>/dev/null; then',
  '  echo "RECOVER_OK: fault=0x0000 — safe to re-enable / hold test"',
  'else',
  '  echo "RECOVER_UNKNOWN: no fault= feedback — check CAN/power; see log path in JSON below"',
  'fi',
].join("\n");

/** motor-repl set-zero (with verify) + homing-status readback. */
function zeroActuatorRemoteBody(joint: string, verify: boolean): string {
  const lines = [
    `bin/motor-repl set-zero ${shellQuote(joint)} --sign-tested`,
    "bin/motor-repl homing-status",
  ];
  lines.push("bin/motor-repl disable 2>/dev/null || true");
  if (verify) {
    lines.push("sleep 0.3", "bin/motor-repl homing-status");
  }
  return lines.join("\n");
}

export function registerMotionTools(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  auditMotion: (
    tool: string,
    args: Record<string, unknown>,
    result: string,
    exitCode: number,
  ) => void,
) {
  function gate(args: z.infer<typeof motionConfirmSchema>) {
    return validateMotionConfirm(args, cfg.benchProfile);
  }

  return {
    pi_motor_enable: {
      description:
        "motor-repl enable bench (short probe only — use marengo-pi for sustained control). " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: motionConfirmSchema.extend({
        operator: z.string().default("bench"),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        operator?: string;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const body = wrapRemote(
          cfg,
          soleCanOwnerShell(`bin/motor-repl enable ${args.operator ?? "bench"}`),
        );
        const out = await runRemote(body, 30_000 + CAN_SESSION_SLACK_MS);
        auditMotion("pi_motor_enable", args, out, 0);
        return out;
      },
    },

    pi_motor_disable: {
      description:
        "motor-repl disable all joints (Robstride DISABLE frame — primary fault clear; no Motor Studio). " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: motionConfirmSchema.extend({
        config_dir: z
          .string()
          .optional()
          .describe(
            "MARENGO_CONFIG_DIR override (default: master /opt/marengo/config)",
          ),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        config_dir?: string;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const configDir = benchConfigDirForJoint(cfg, undefined, args.config_dir);
        const body = wrapRemoteWithConfig(
          cfg,
          soleCanOwnerShell("bin/motor-repl disable"),
          configDir,
        );
        const out = await runRemote(body, 30_000 + CAN_SESSION_SLACK_MS);
        auditMotion("pi_motor_disable", args, out, 0);
        return out;
      },
    },

    pi_motor_recover: {
      description:
        "Recover after drive fault (replaces Motor Studio clear + manual SSH). " +
        "stop marengo-pi → motor-repl disable → marengo-pi status while Disabled → prints RECOVER_OK or RECOVER_FAIL. " +
        "Never acquires a reference or enables (the arm is not attested at the mechanical reference after a fault). " +
        "Logs to var/log/bench-latest.log. Args: confirm:true; optional config_dir " +
        "(default master /opt/marengo/config). " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: motionConfirmSchema.extend({
        config_dir: z
          .string()
          .optional()
          .describe(
            "MARENGO_CONFIG_DIR (default /opt/marengo/config)",
          ),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        config_dir?: string;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const configDir = benchConfigDirForJoint(
          cfg,
          undefined,
          args.config_dir ?? BENCH_CONFIG_MASTER,
        );
        const pipeCmd = [
          motorRecoverRemoteBody(cfg),
          motorRecoverSummaryShell,
        ].join("\n");
        const body = benchLogWrapper(cfg, pipeCmd, "motor-recover", configDir);
        const out = await runRemote(body, 45_000 + CAN_SESSION_SLACK_MS);
        auditMotion("pi_motor_recover", args, out, 0);
        return out;
      },
    },

    pi_homing_status: {
      description:
        "Read-only homing state per joint (motor-repl homing-status). No motion. " +
        "While marengo-pi owns CAN, reports marengo-pi's RobotState homing via the gateway instead of opening CAN.",
      inputSchema: z.object({
        config_dir: z.string().optional(),
      }),
      handler: async (args: { config_dir?: string }) => {
        const configDir =
          benchConfigDirForJoint(cfg, undefined, args.config_dir) ??
          BENCH_CONFIG_MASTER;
        const body = wrapRemoteWithConfig(cfg, homingStatusShell(), configDir);
        return renderRobotStateHoming(await runRemote(body, 20_000));
      },
    },

    pi_set_zero: {
      description:
        "Zero encoder at mechanical reference via motor-repl `set-zero <joint> --sign-tested` (CAN SetZero, " +
        "qualified readback) and print homing-status. The reference grant ends when motor-repl exits: enabling " +
        "still requires marengo-pi `home <joints> sign-tested` in the controlling session (pi_hold_on set_zero). " +
        "Position arm first; confirm: true. " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: motionConfirmSchema.extend({
        joint: z
          .string()
          .default("right_shoulder_pitch")
          .describe("Joint name from motors.yaml"),
        config_dir: z
          .string()
          .optional()
          .describe(
            "Override MARENGO_CONFIG_DIR (e.g. /opt/marengo/config)",
          ),
        verify: z
          .boolean()
          .default(true)
          .describe("Enable briefly and print status so feedback pos ≈ 0 is visible"),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        joint?: string;
        config_dir?: string;
        verify?: boolean;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const joint = args.joint ?? "right_shoulder_pitch";
        const verify = args.verify ?? true;
        const configDir =
          benchConfigDirForJoint(cfg, joint, args.config_dir) ?? BENCH_CONFIG_MASTER;
        const body = wrapRemoteWithConfig(
          cfg,
          soleCanOwnerShell(zeroActuatorRemoteBody(joint, verify)),
          configDir,
        );
        const out = await runRemote(body, (verify ? 45_000 : 30_000) + CAN_SESSION_SLACK_MS);
        auditMotion("pi_set_zero", { ...args, joint, verify }, out, 0);
        // Consul soft-invalidate is browser-local until a Chappe/gateway signal exists.
        return (
          `${out}\n\n` +
          "NOTE: Consul teach overlays do not auto-bump on pi_set_zero. " +
          "If a taught Wave overlay is applied, open Teach Record → click " +
          '"I set-zero\'d" (or Reset overlay). Home alone does not invalidate.'
        );
      },
    },

    pi_jog: {
      description: `motor-repl jog joint to position_rad. ${SOLE_CAN_OWNER_NOTE}`,
      inputSchema: motionConfirmSchema.extend({
        joint: z.string(),
        position_rad: z.number(),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        joint: string;
        position_rad: number;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const body = wrapRemote(
          cfg,
          soleCanOwnerShell(`bin/motor-repl jog ${args.joint} ${args.position_rad}`),
        );
        const out = await runRemote(body, 30_000 + CAN_SESSION_SLACK_MS);
        auditMotion("pi_jog", args, out, 0);
        return out;
      },
    },

    pi_hold_on: {
      description:
        "Compliant position hold in one marengo-pi session: `home <joints> sign-tested`, home and enable " +
        "(each awaited; a refusal ends the session with disable/quit), hold-on or hold-at, return to 0, disable. References exist only inside that process, so " +
        "every hold acquires them; acquisition runs SetZero at the current pose and needs set_zero: true " +
        "plus at_mechanical_reference: true (otherwise refused before touching the Pi). " +
        "Uses kp/kd/slew/trim from master /opt/marengo/config/control.yaml. Logs to var/log. " +
        "Call pi_sync_bench_config first if control.yaml was edited locally. " +
        GRAVITY_GATE_NOTE +
        " " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: motionConfirmSchema.extend({
        config_dir: z
          .string()
          .optional()
          .describe(
            "MARENGO_CONFIG_DIR override (default: MCP env or /opt/marengo/config)",
          ),
        joint: z
          .string()
          .regex(/^[A-Za-z0-9_]+$/)
          .optional()
          .describe(
            "Joint to reference and hold. Omit to reference every joint of the bench profile " +
            "and hold right_shoulder_pitch.",
          ),
        timeout_sec: z
          .number()
          .int()
          .min(5)
          .max(120)
          .default(DEFAULT_MOTION_TIMEOUT_SEC),
        ...referenceOptInShape,
        position_rad: z
          .number()
          .optional()
          .describe("If set, use hold-at instead of latching current pose"),
        return_home_sec: z
          .number()
          .int()
          .min(5)
          .max(120)
          .default(DEFAULT_RETURN_HOME_SEC)
          .describe(
            "After dwell at target, hold-at 0 and wait this long before disable/quit",
          ),
        operator: z.string().default("bench"),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        config_dir?: string;
        joint?: string;
        timeout_sec?: number;
        set_zero?: boolean;
        at_mechanical_reference?: boolean;
        position_rad?: number;
        operator?: string;
        return_home_sec?: number;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        if (args.set_zero !== true || args.at_mechanical_reference !== true) {
          return REFERENCE_OPT_IN_REQUIRED;
        }
        const timeoutSec = args.timeout_sec ?? DEFAULT_MOTION_TIMEOUT_SEC;
        const returnHomeSec = args.return_home_sec ?? DEFAULT_RETURN_HOME_SEC;
        const joint = args.joint ?? "right_shoulder_pitch";
        const profile = effectiveProfile(cfg.benchProfile, args.profile);
        const referenceJoints =
          args.joint !== undefined ? [args.joint] : profileMeta(profile).setZeroJoints;
        const configDir =
          benchConfigDirForJoint(cfg, joint, args.config_dir) ?? BENCH_CONFIG_MASTER;
        const gravity = await runGravityGate({
          profile,
          joints: [...new Set([...referenceJoints, joint])],
          snapshotOutput: await runRemote(
            wrapRemoteWithConfig(cfg, gravityGateSnapshotShell(), configDir),
            15_000,
          ),
          runPreview: (shell) =>
            runRemote(
              wrapRemoteWithConfig(cfg, soleCanOwnerShell(shell), configDir),
              30_000 + CAN_SESSION_SLACK_MS,
            ),
        });
        if (!gravity.ok) {
          auditMotion("pi_hold_on", args, gravity.report, 1);
          return gravity.report;
        }
        const pipeCmd = holdSessionRemoteBody(cfg, {
          joint,
          referenceJoints,
          operator: args.operator ?? "bench",
          positionRad: args.position_rad,
          timeoutSec,
          returnHomeSec,
        });
        const body = benchLogWrapper(cfg, pipeCmd, "hold-on", configDir);
        const session = `${gravity.report}\n${await runRemote(
          body,
          (timeoutSec + returnHomeSec + referenceJoints.length * REFERENCE_ACQUIRE_SEC_PER_JOINT) * 1000 +
            20_000 +
            CAN_SESSION_SLACK_MS,
        )}`;
        const refusal = sessionRefusal(session);
        const out =
          refusal === undefined
            ? session
            : `${session}\n\n--- hold ---\nmarengo-pi refused the session before the hold: ${refusal}\n` +
              "The feeder sent disable/quit instead of the hold: no pose was held.";
        auditMotion("pi_hold_on", args, out, refusal === undefined ? 0 : 1);
        return out;
      },
    },

    pi_hold_off: {
      description:
        "Stop hold: stop marengo-pi (service via the pi_restart_marengo_pi helper) and motor-repl disable. " +
        "Omitted config_dir uses master /opt/marengo/config. " +
        "A marengo-pi.service that was active is restarted afterwards and comes up Disabled; " +
        "use pi_restart_marengo_pi mode=stop to keep control off.",
      inputSchema: motionConfirmSchema.extend({
        joint: z.string().optional(),
        config_dir: z.string().optional(),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        joint?: string;
        config_dir?: string;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const configDir =
          benchConfigDirForJoint(cfg, args.joint, args.config_dir) ??
          BENCH_CONFIG_MASTER;
        const body = wrapRemoteWithConfig(
          cfg,
          soleCanOwnerShell("bin/motor-repl disable"),
          configDir,
        );
        const out = await runRemote(body, 20_000 + CAN_SESSION_SLACK_MS);
        auditMotion("pi_hold_off", args, out, 0);
        return out;
      },
    },

    pi_marengo_pi_script: {
      description:
        "Pipe stdin script to marengo-pi (enable/gravity-on/hold-on/status/disable/quit); logs to var/log. " +
        SOLE_CAN_OWNER_NOTE,
      inputSchema: motionConfirmSchema.extend({
        joint: z
          .string()
          .optional()
          .describe(
            "When config_dir omitted, uses master /opt/marengo/config",
          ),
        config_dir: z
          .string()
          .optional()
          .describe("MARENGO_CONFIG_DIR override"),
        script: z
          .array(z.string())
          .min(1)
          .describe("Lines to pipe to marengo-pi stdin"),
        timeout_sec: z
          .number()
          .int()
          .min(5)
          .max(120)
          .default(DEFAULT_MOTION_TIMEOUT_SEC)
          .describe(
            "Total marengo-pi pipe timeout; script sleep lines count against this budget",
          ),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        joint?: string;
        config_dir?: string;
        script: string[];
        timeout_sec?: number;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const expanded = expandScriptWithWaveWaits(args.script);
        const sleepBudget = scriptSleepTotalSec(expanded);
        const controlTimeoutSec =
          args.timeout_sec !== undefined
            ? args.timeout_sec
            : Math.max(
                DEFAULT_MOTION_TIMEOUT_SEC,
                Math.ceil(sleepBudget + 10),
              );
        const script = ensureScriptQuit(expanded);
        const pipeTimeoutSec = marengoPiPipeTimeoutSec(script, controlTimeoutSec);
        const pipeCmd = [
          marengoPiLaunchShell(cfg),
          marengoPiPipe(script, pipeTimeoutSec),
        ].join("\n");
        const configDir =
          benchConfigDirForJoint(cfg, args.joint, args.config_dir) ??
          args.config_dir;
        const body = benchLogWrapper(cfg, pipeCmd, "marengo-pi-script", configDir);
        const out = await runRemote(
          body,
          pipeTimeoutSec * 1000 + REMOTE_SSH_SLACK_MS + CAN_SESSION_SLACK_MS,
        );
        auditMotion("pi_marengo_pi_script", args, out, 0);
        return out;
      },
    },

    pi_bench_harness: {
      description:
        "Profile-aware bench test matrix (bare_motor, weighted, roll_attached, arm_2dof_smoke, yaw_attached). " +
        "Enable-requiring suites run in ONE marengo-pi session after a single awaited " +
        "`home <joints> sign-tested` (grants are in-process only); they need set_zero: true and " +
        "at_mechanical_reference: true, otherwise the harness refuses them up front. " +
        GRAVITY_GATE_NOTE,
      inputSchema: motionConfirmSchema.extend({
        profile: benchProfileZod.optional(),
        config_dir: z.string().optional(),
        joints: z
          .array(z.string().regex(/^[A-Za-z0-9_]+$/))
          .min(1)
          .optional()
          .describe("Joints to reference; default every joint of the profile"),
        loaded_joint: z.string().optional(),
        gravity_angles: z.array(z.number()).optional(),
        ...referenceOptInShape,
        debug: z.boolean().default(false),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        config_dir?: string;
        joints?: string[];
        loaded_joint?: string;
        gravity_angles?: number[];
        set_zero?: boolean;
        at_mechanical_reference?: boolean;
        debug?: boolean;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;

        const { runBenchHarness } = await import("../harness/index.js");
        const out = await runBenchHarness(cfg, runRemote, args);
        auditMotion("pi_bench_harness", args, out, 0);
        return out;
      },
    },
  };
}

export type MotionTools = ReturnType<typeof registerMotionTools>;

export function makeAuditMotion(cfg: MarengoPiConfig) {
  return (
    tool: string,
    args: Record<string, unknown>,
    stdout: string,
    exitCode: number,
  ) => {
    appendAudit({
      tool,
      args,
      bench_profile: cfg.benchProfile,
      confirm_weighted_motion: args.confirm_weighted_motion === true,
      stdout: stdout.slice(0, 8000),
      exitCode,
    });
  };
}
