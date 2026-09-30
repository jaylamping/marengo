import { z } from "zod";
import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { BENCH_PROFILES } from "../bench-profiles.js";
import { appendAudit } from "../audit.js";
import { shellQuote, wrapRemote, wrapRemoteWithConfig } from "../env.js";
import { validateMotionConfirm } from "../safety.js";

const benchProfileZod = z.enum(BENCH_PROFILES);

const motionConfirmSchema = z.object({
  confirm: z.literal(true),
  confirm_weighted_motion: z.literal(true).optional(),
  profile: benchProfileZod.optional(),
});

const BENCH_CONFIG_MASTER = "/opt/marengo/config";

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
    `for _pat in bench-*.log position-trace-*.csv bench-*.json candump-*.log; do`,
    `  ls -1t ${logDirVar}/$_pat 2>/dev/null | tail -n +${keep + 1} | while IFS= read -r _f; do rm -f "$_f"; done`,
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
function benchConfigDirForJoint(
  cfg: MarengoPiConfig,
  _joint?: string,
  configDir?: string,
): string | undefined {
  if (configDir) {
    return normalizeBenchConfigDir(cfg, configDir);
  }
  return BENCH_CONFIG_MASTER;
}

const benchLogWrapper = (
  cfg: MarengoPiConfig,
  pipeCmd: string,
  label: string,
  configDir?: string,
) => {
  const logDir = `${cfg.piRoot}/var/log`;
  const body = [
    `LOGDIR=${shellQuote(logDir)}`,
    'mkdir -p "$LOGDIR"',
    'TS=$(date -u +"%Y%m%dT%H%M%SZ")',
    'LOG="$LOGDIR/bench-$TS.log"',
    'TRACE="$LOGDIR/position-trace-$TS.csv"',
    'export MARENGO_POSITION_TRACE="$TRACE"',
    'export MARENGO_POSITION_TRACE_HZ="${MARENGO_POSITION_TRACE_HZ:-50}"',
    'export MARENGO_LOG_SESSION_ID="$TS"',
    `LABEL=${shellQuote(label)}`,
    marengoPiPkillLine(cfg),
    "sleep 0.3",
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
  ].join("\n");
  return configDir
    ? wrapRemoteWithConfig(cfg, body, configDir)
    : wrapRemote(cfg, body, false);
};

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
    'if test -x "$PI_BIN"; then',
    '  PI_HELP=$(printf \'%s\\n\' help | timeout 2 "$PI_BIN" 2>&1 || true)',
    '  if ! printf \'%s\\n\' "$PI_HELP" | grep -q hold-on && test -x "$PI_FALLBACK"; then',
    '    PI_BIN="$PI_FALLBACK"',
    "  fi",
    "fi",
  ].join("\n");
}

/** Default dwell at target before hold-at 0 (Layer 2 0.1 rad gate: brief settle). */
const DEFAULT_HOLD_DWELL_SEC = 5;

/** Default wait for return to home after hold-at 0 (≤5 s motion budget + slack). */
const DEFAULT_RETURN_HOME_SEC = 6;

/** Fair Layer 2 start: dwell after hold-at 0; analyzer `--require-home-start` enforces |q|<5 mrad. */
export const LAYER2_HOME_SETTLE_SEC = 5;

export const LAYER2_HOLD_ROUND_TRIP_SCRIPT = [
  "home",
  "enable bench",
  "hold-at 0",
  `sleep ${LAYER2_HOME_SETTLE_SEC}`,
  "hold-at 0.1",
  "sleep 5",
  "hold-at 0",
  "sleep 5",
  "status",
  "disable",
] as const;

/** Pipe timeout for [`LAYER2_HOLD_ROUND_TRIP_SCRIPT`] (sleep sum + startup slack). */
export const LAYER2_HOLD_ROUND_TRIP_TIMEOUT_SEC = 21;

/** Default total marengo-pi pipe timeout (includes script `sleep N` lines). */
const DEFAULT_MOTION_TIMEOUT_SEC = DEFAULT_HOLD_DWELL_SEC;

/** SSH wrapper slack beyond pipe timeout. */
const REMOTE_SSH_SLACK_MS = 10_000;

/** Sum `sleep N` dwell lines (for docs / harness helpers). */
export function scriptSleepTotalSec(script: string[]): number {
  let total = 0;
  for (const line of script) {
    const sleepMatch = /^sleep (\d+(?:\.\d+)?)$/i.exec(line.trim());
    if (sleepMatch) {
      total += Number(sleepMatch[1]);
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

/** Shell sleep between marengo-pi stdin lines (e.g. dwell after hold-at). */
function marengoPiPipeLine(line: string): string {
  const sleepMatch = /^sleep (\d+(?:\.\d+)?)$/i.exec(line.trim());
  if (sleepMatch) {
    return `sleep ${sleepMatch[1]}`;
  }
  return `printf '%s\\n' ${JSON.stringify(line)}`;
}

function marengoPiPipe(script: string[], pipeTimeoutSec: number, binary = "$PI_BIN"): string {
  const pipeLines = script.map(marengoPiPipeLine).join(";\n");
  return `{\n${pipeLines};\n} | timeout ${pipeTimeoutSec} ${binary}`;
}

function marengoPiTimedPipe(
  script: string[],
  dwellSec: number,
  returnHomeSec: number,
  returnJoint?: string,
  binary = "$PI_BIN",
): string {
  const returnHold =
    returnJoint !== undefined ? `hold-at ${returnJoint} 0` : "hold-at 0";
  const commandLines = [
    ...script.map((l) => `printf '%s\\n' ${JSON.stringify(l)};`),
    `sleep ${dwellSec};`,
    `printf '%s\\n' ${JSON.stringify(returnHold)};`,
    `sleep ${returnHomeSec};`,
    `printf '%s\\n' "status";`,
    `printf '%s\\n' "disable";`,
    `printf '%s\\n' "quit";`,
  ];
  const pipeTimeoutSec = dwellSec + returnHomeSec + 10;
  return `{\n${commandLines.join("\n")}\n} | timeout ${pipeTimeoutSec} ${binary}`;
}

function marengoPiPkillLine(cfg: MarengoPiConfig): string {
  const bin = `${cfg.piRoot}/bin/marengo-pi`;
  return `pkill -f ${shellQuote(bin)} 2>/dev/null || true`;
}

function holdSessionRemoteBody(
  cfg: MarengoPiConfig,
  args: {
    joint: string;
    setZero: boolean;
    killStale: boolean;
    operator: string;
    positionRad?: number;
    timeoutSec: number;
    returnHomeSec: number;
  },
): string {
  const lines: string[] = [];
  if (args.killStale) {
    lines.push(marengoPiPkillLine(cfg), "sleep 0.3");
  }
  lines.push("bin/motor-repl disable 2>/dev/null || true");
  if (args.setZero) {
    lines.push(`bin/motor-repl set-zero ${shellQuote(args.joint)}`);
  }
  lines.push(marengoPiBinarySelector(cfg));
  const holdLine =
    args.positionRad !== undefined
      ? `hold-at ${args.joint} ${args.positionRad}`
      : "hold-on";
  lines.push(
    "set +e",
    marengoPiTimedPipe(
      ["home", `enable ${args.operator}`, holdLine],
      args.timeoutSec,
      args.returnHomeSec,
      args.joint,
    ),
    "PIPE_STATUS=$?",
    "set -e",
    "bin/motor-repl disable",
    'exit "$PIPE_STATUS"',
  );
  return lines.join("\n");
}

/** Stop control, disable drives (clears most Robstride faults), brief enable to read fault= line. */
function motorRecoverRemoteBody(cfg: MarengoPiConfig): string {
  return [
    marengoPiPkillLine(cfg),
    "sleep 0.3",
    "bin/motor-repl disable 2>/dev/null || true",
    "sleep 0.5",
    marengoPiBinarySelector(cfg),
    [
      "{",
      "echo home;",
      "echo 'enable bench';",
      "sleep 1;",
      "echo status;",
      "echo disable;",
      "echo quit;",
      "} | timeout 10 \"$PI_BIN\"",
    ].join(" "),
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
        "motor-repl enable bench (short probe only — use marengo-pi for sustained control)",
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
          `bin/motor-repl enable ${args.operator ?? "bench"}`,
        );
        const out = await runRemote(body, 30_000);
        auditMotion("pi_motor_enable", args, out, 0);
        return out;
      },
    },

    pi_motor_disable: {
      description:
        "motor-repl disable all joints (Robstride DISABLE frame — primary fault clear; no Motor Studio)",
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
          "bin/motor-repl disable",
          configDir,
        );
        const out = await runRemote(body, 30_000);
        auditMotion("pi_motor_disable", args, out, 0);
        return out;
      },
    },

    pi_motor_recover: {
      description:
        "Recover after drive fault (replaces Motor Studio clear + manual SSH). " +
        "pkill marengo-pi → motor-repl disable → brief enable/status → prints RECOVER_OK or RECOVER_FAIL. " +
        "Logs to var/log/bench-latest.log. Args: confirm:true; optional config_dir " +
        "(default master /opt/marengo/config).",
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
        const out = await runRemote(body, 45_000);
        auditMotion("pi_motor_recover", args, out, 0);
        return out;
      },
    },

    pi_homing_status: {
      description:
        "Read-only homing state per joint (motor-repl homing-status). No motion.",
      inputSchema: z.object({
        config_dir: z.string().optional(),
      }),
      handler: async (args: { config_dir?: string }) => {
        const configDir =
          benchConfigDirForJoint(cfg, undefined, args.config_dir) ??
          BENCH_CONFIG_MASTER;
        const body = wrapRemoteWithConfig(
          cfg,
          "bin/motor-repl homing-status",
          configDir,
        );
        return runRemote(body, 20_000);
      },
    },

    pi_set_zero: {
      description:
        "Zero encoder at mechanical reference via CAN SetZero. Verifies |pos| < tolerance, " +
        "writes calibration record, and prints homing-status. Position arm first; confirm: true.",
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
          zeroActuatorRemoteBody(joint, verify),
          configDir,
        );
        const out = await runRemote(body, verify ? 45_000 : 30_000);
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
      description: "motor-repl jog joint to position_rad",
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
          `bin/motor-repl jog ${args.joint} ${args.position_rad}`,
        );
        const out = await runRemote(body, 30_000);
        auditMotion("pi_jog", args, out, 0);
        return out;
      },
    },

    pi_hold_on: {
      description:
        "Compliant position hold: set-zero (optional), home, enable, hold-on or hold-at. " +
        "Uses kp/kd/slew/trim from master /opt/marengo/config/control.yaml. Logs to var/log. " +
        "Call pi_sync_bench_config first if control.yaml was edited locally.",
      inputSchema: motionConfirmSchema.extend({
        config_dir: z
          .string()
          .optional()
          .describe(
            "MARENGO_CONFIG_DIR override (default: MCP env or /opt/marengo/config)",
          ),
        joint: z.string().default("right_shoulder_pitch"),
        timeout_sec: z
          .number()
          .int()
          .min(5)
          .max(120)
          .default(DEFAULT_MOTION_TIMEOUT_SEC),
        set_zero: z.boolean().default(false),
        kill_stale: z
          .boolean()
          .default(true)
          .describe("pkill stale marengo-pi before session"),
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
        kill_stale?: boolean;
        position_rad?: number;
        operator?: string;
        return_home_sec?: number;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const timeoutSec = args.timeout_sec ?? DEFAULT_MOTION_TIMEOUT_SEC;
        const returnHomeSec = args.return_home_sec ?? DEFAULT_RETURN_HOME_SEC;
        const joint = args.joint ?? "right_shoulder_pitch";
        const configDir =
          benchConfigDirForJoint(cfg, joint, args.config_dir) ?? BENCH_CONFIG_MASTER;
        const pipeCmd = holdSessionRemoteBody(cfg, {
          joint,
          setZero: args.set_zero ?? true,
          killStale: args.kill_stale ?? true,
          operator: args.operator ?? "bench",
          positionRad: args.position_rad,
          timeoutSec,
          returnHomeSec,
        });
        const body = benchLogWrapper(cfg, pipeCmd, "hold-on", configDir);
        const out = await runRemote(
          body,
          (timeoutSec + returnHomeSec) * 1000 + 20_000,
        );
        auditMotion("pi_hold_on", args, out, 0);
        return out;
      },
    },

    pi_hold_off: {
      description:
        "Stop hold: pkill marengo-pi and motor-repl disable. " +
        "Omitted config_dir uses master /opt/marengo/config.",
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
          [marengoPiPkillLine(cfg), "bin/motor-repl disable"].join(
            "\n",
          ),
          configDir,
        );
        const out = await runRemote(body, 20_000);
        auditMotion("pi_hold_off", args, out, 0);
        return out;
      },
    },

    pi_marengo_pi_script: {
      description:
        "Pipe stdin script to marengo-pi (enable/gravity-on/hold-on/status/disable/quit); logs to var/log",
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
          marengoPiBinarySelector(cfg),
          marengoPiPipe(script, pipeTimeoutSec),
        ].join("\n");
        const configDir =
          benchConfigDirForJoint(cfg, args.joint, args.config_dir) ??
          args.config_dir;
        const body = benchLogWrapper(cfg, pipeCmd, "marengo-pi-script", configDir);
        const out = await runRemote(
          body,
          pipeTimeoutSec * 1000 + REMOTE_SSH_SLACK_MS,
        );
        auditMotion("pi_marengo_pi_script", args, out, 0);
        return out;
      },
    },

    pi_bench_harness: {
      description:
        "Profile-aware bench test matrix (bare_motor, weighted, roll_attached, arm_2dof_smoke, yaw_attached)",
      inputSchema: motionConfirmSchema.extend({
        profile: benchProfileZod.optional(),
        config_dir: z.string().optional(),
        joints: z.array(z.string()).optional(),
        loaded_joint: z.string().optional(),
        gravity_angles: z.array(z.number()).optional(),
        skip_set_zero: z
          .boolean()
          .default(true)
          .describe("Skip set-zero by default (requires mechanical zero)"),
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
        skip_set_zero?: boolean;
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
