import { z } from "zod";
import type { BenchProfile, MarengoPiConfig } from "../config.js";
import { appendAudit } from "../audit.js";
import { shellQuote, wrapRemote } from "../env.js";
import { validateMotionConfirm } from "../safety.js";

const motionConfirmSchema = z.object({
  confirm: z.literal(true),
  confirm_weighted_motion: z.literal(true).optional(),
  profile: z
    .enum(["bare_motor", "weighted_single_arm", "arm_attached"])
    .optional(),
});

const benchLogWrapper = (cfg: MarengoPiConfig, pipeCmd: string, label: string) => {
  const logDir = `${cfg.piRoot}/var/log`;
  return wrapRemote(
    cfg,
    [
      `LOGDIR=${shellQuote(logDir)}`,
      "mkdir -p \"$LOGDIR\"",
      'TS=$(date -u +"%Y%m%dT%H%M%SZ")',
      `LOG="$LOGDIR/bench-$TS.log"`,
      `LABEL=${shellQuote(label)}`,
      "echo \"=== bench session $TS ($LABEL) ===\" | tee \"$LOG\"",
      `${pipeCmd} 2>&1 | tee -a "$LOG"`,
      "ln -sf \"$LOG\" \"$LOGDIR/bench-latest.log\"",
      'echo "{\"log\":\"$LOG\",\"ts\":\"$TS\",\"label\":\"$LABEL\"}"',
    ].join("\n"),
    false,
  );
};

function marengoPiPipe(script: string[], timeoutSec: number): string {
  const printfLines = script
    .map((l) => `printf '%s\\n' ${JSON.stringify(l)}`)
    .join("\n");
  return `${printfLines} | timeout ${timeoutSec} bin/marengo-pi`;
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
      description: "motor-repl disable all joints",
      inputSchema: motionConfirmSchema,
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const body = wrapRemote(cfg, "bin/motor-repl disable");
        const out = await runRemote(body, 30_000);
        auditMotion("pi_motor_disable", args, out, 0);
        return out;
      },
    },

    pi_set_zero: {
      description: "motor-repl set-zero at current angle (hardware encoder zero)",
      inputSchema: motionConfirmSchema.extend({
        joint: z.string(),
      }),
      handler: async (args: {
        confirm: true;
        confirm_weighted_motion?: true;
        profile?: BenchProfile;
        joint: string;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const body = wrapRemote(cfg, `bin/motor-repl set-zero ${args.joint}`);
        const out = await runRemote(body, 30_000);
        auditMotion("pi_set_zero", args, out, 0);
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

    pi_marengo_pi_script: {
      description:
        "Pipe stdin script to marengo-pi (enable/gravity-on/status/disable/quit); logs to var/log",
      inputSchema: motionConfirmSchema.extend({
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
        script: string[];
        timeout_sec?: number;
      }) => {
        const check = gate(args);
        if (!check.ok) return check.message;
        const pipeCmd = marengoPiPipe(args.script, args.timeout_sec ?? 30);
        const body = benchLogWrapper(cfg, pipeCmd, "marengo-pi-script");
        const out = await runRemote(body, (args.timeout_sec ?? 30) * 1000 + 15_000);
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
