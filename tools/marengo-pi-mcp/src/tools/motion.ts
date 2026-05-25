import { z } from "zod";
import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { appendAudit } from "../audit.js";
import { shellQuote, wrapRemote, wrapRemoteWithConfig } from "../env.js";
import { validateMotionConfirm } from "../safety.js";

const motionConfirmSchema = z.object({
  confirm: z.literal(true),
  confirm_weighted_motion: z.literal(true).optional(),
  profile: z
    .enum(["bare_motor", "weighted_single_arm", "arm_attached"])
    .optional(),
});

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
    `LABEL=${shellQuote(label)}`,
    'echo "=== bench session $TS ($LABEL) ===" | tee "$LOG"',
    `${pipeCmd} 2>&1 | tee -a "$LOG"`,
    'ln -sf "$LOG" "$LOGDIR/bench-latest.log"',
    'echo "{\"log\":\"$LOG\",\"ts\":\"$TS\",\"label\":\"$LABEL\"}"',
  ].join("\n");
  return configDir
    ? wrapRemoteWithConfig(cfg, body, configDir)
    : wrapRemote(cfg, body, false);
};

function marengoPiBinarySelector(cfg: MarengoPiConfig): string {
  const fallback = `${cfg.piStagingRoot.replace(/^~/, "$HOME")}/target/release/marengo-pi`;
  return [
    'PI_BIN=bin/marengo-pi',
    'if ! printf \'%s\\n\' help | timeout 2 "$PI_BIN" 2>&1 | grep -q hold-on; then',
    `  PI_BIN=${shellQuote(fallback)}`,
    "fi",
  ].join("\n");
}

function marengoPiPipe(script: string[], timeoutSec: number, binary = "$PI_BIN"): string {
  const printfLines = script
    .map((l) => `printf '%s\\n' ${JSON.stringify(l)}`)
    .join("\n");
  return `${printfLines} | timeout ${timeoutSec} ${binary}`;
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
  },
): string {
  const lines: string[] = [];
  if (args.killStale) {
    lines.push("pkill -f 'marengo-pi' 2>/dev/null || true", "sleep 0.3");
  }
  lines.push("bin/motor-repl disable 2>/dev/null || true");
  if (args.setZero) {
    lines.push(`bin/motor-repl set-zero ${shellQuote(args.joint)}`);
  }
  lines.push(marengoPiBinarySelector(cfg));
  const holdLine =
    args.positionRad !== undefined ? `hold-at ${args.positionRad}` : "hold-on";
  lines.push(
    marengoPiPipe(
      ["home", `enable ${args.operator}`, holdLine],
      args.timeoutSec,
    ),
  );
  lines.push("bin/motor-repl disable");
  return lines.join("\n");
}

/** Stop control, disable drives (clears most Robstride faults), brief enable to read fault= line. */
function motorRecoverRemoteBody(cfg: MarengoPiConfig): string {
  return [
    "pkill -f 'marengo-pi' 2>/dev/null || true",
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

/** motor-repl set-zero + optional position readback (replaces Motor Studio Set Zero). */
function zeroActuatorRemoteBody(joint: string, verify: boolean): string {
  const lines = [`bin/motor-repl set-zero ${shellQuote(joint)}`];
  lines.push("bin/motor-repl disable 2>/dev/null || true");
  if (verify) {
    lines.push(
      "sleep 0.3",
      "{ echo home; echo 'enable bench'; sleep 2; echo status; echo disable; echo quit; } | timeout 8 bin/marengo-pi",
    );
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
            "MARENGO_CONFIG_DIR override (default: right-only bench for bare_motor)",
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
        const configDir =
          args.config_dir ??
          (args.profile === "bare_motor" || cfg.benchProfile === "bare_motor"
            ? "/opt/marengo/config/bringup/shoulder_pitch_right_only"
            : undefined);
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
        "Logs to var/log/bench-latest.log. Args: confirm:true; optional config_dir (default right-only bench).",
      inputSchema: motionConfirmSchema.extend({
        config_dir: z
          .string()
          .optional()
          .describe(
            "MARENGO_CONFIG_DIR (default /opt/marengo/config/bringup/shoulder_pitch_right_only)",
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
        const configDir =
          args.config_dir ??
          "/opt/marengo/config/bringup/shoulder_pitch_right_only";
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

    pi_set_zero: {
      description:
        "Zero encoder at the current shaft angle via CAN SetZero (no Motor Studio). " +
        "Position the arm at mechanical zero first, then call with confirm: true. " +
        "Sends motor-repl set-zero, disables, and reads back position when verify=true.",
      inputSchema: motionConfirmSchema.extend({
        joint: z
          .string()
          .default("right_shoulder_pitch")
          .describe("Joint name from motors.yaml"),
        config_dir: z
          .string()
          .optional()
          .describe(
            "Override MARENGO_CONFIG_DIR (e.g. /opt/marengo/config/bringup/shoulder_pitch_right_only)",
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
        const body = wrapRemoteWithConfig(
          cfg,
          zeroActuatorRemoteBody(joint, verify),
          args.config_dir,
        );
        const out = await runRemote(body, verify ? 45_000 : 30_000);
        auditMotion("pi_set_zero", { ...args, joint, verify }, out, 0);
        return out;
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
        "Uses kp/kd/slew/trim from control.yaml (right-only bench: kp=12 kd=0.75). Logs to var/log. " +
        "Call pi_sync_bench_config first if control.yaml was edited locally.",
      inputSchema: motionConfirmSchema.extend({
        config_dir: z
          .string()
          .optional()
          .describe(
            "MARENGO_CONFIG_DIR override (default: MCP env or shoulder_pitch_right_only)",
          ),
        joint: z.string().default("right_shoulder_pitch"),
        timeout_sec: z.number().int().min(5).max(120).default(30),
        set_zero: z.boolean().default(true),
        kill_stale: z
          .boolean()
          .default(true)
          .describe("pkill stale marengo-pi before session"),
        position_rad: z
          .number()
          .optional()
          .describe("If set, use hold-at instead of latching current pose"),
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
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const timeoutSec = args.timeout_sec ?? 30;
        const pipeCmd = holdSessionRemoteBody(cfg, {
          joint: args.joint ?? "right_shoulder_pitch",
          setZero: args.set_zero ?? true,
          killStale: args.kill_stale ?? true,
          operator: args.operator ?? "bench",
          positionRad: args.position_rad,
          timeoutSec,
        });
        const body = benchLogWrapper(cfg, pipeCmd, "hold-on", args.config_dir);
        const out = await runRemote(body, timeoutSec * 1000 + 20_000);
        auditMotion("pi_hold_on", args, out, 0);
        return out;
      },
    },

    pi_hold_off: {
      description: "Stop hold: pkill marengo-pi and motor-repl disable",
      inputSchema: motionConfirmSchema.extend({
        config_dir: z.string().optional(),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        config_dir?: string;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const body = wrapRemoteWithConfig(
          cfg,
          ["pkill -f 'marengo-pi' 2>/dev/null || true", "bin/motor-repl disable"].join(
            "\n",
          ),
          args.config_dir,
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
        config_dir: z
          .string()
          .optional()
          .describe("MARENGO_CONFIG_DIR override"),
        script: z
          .array(z.string())
          .min(1)
          .describe("Lines to pipe to marengo-pi stdin"),
        timeout_sec: z.number().int().min(5).max(120).default(30),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        config_dir?: string;
        script: string[];
        timeout_sec?: number;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const timeoutSec = args.timeout_sec ?? 30;
        const pipeCmd = [
          marengoPiBinarySelector(cfg),
          marengoPiPipe(args.script, timeoutSec),
        ].join("\n");
        const body = benchLogWrapper(cfg, pipeCmd, "marengo-pi-script", args.config_dir);
        const out = await runRemote(body, timeoutSec * 1000 + 15_000);
        auditMotion("pi_marengo_pi_script", args, out, 0);
        return out;
      },
    },

    pi_bench_harness: {
      description:
        "Profile-aware bench test matrix (bare_motor or weighted_single_arm)",
      inputSchema: motionConfirmSchema.extend({
        profile: z
          .enum(["bare_motor", "weighted_single_arm", "arm_attached"])
          .optional(),
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
