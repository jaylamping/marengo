import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { sudoCanUpCommand, sudoInstallCommand, sudoStagingInstallCommand } from "../config.js";
import { wrapRemote } from "../env.js";
import { runSyncMain } from "./deploy.js";
import { waitForDeployReady } from "./deploy-wait.js";
import {
  runSyncBenchConfig,
  syncBenchConfigSchema,
} from "./sync-config.js";

export function registerAdminTools(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
) {
  return {
    pi_can_up: {
      description: "Bring up CAN interfaces can0 and can1",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(cfg, sudoCanUpCommand(cfg));
        return runRemote(body, 60_000);
      },
    },

    pi_sync_main: {
      description:
        "Sync local main, cross-build, deploy to Pi, install-pi (writes /opt/marengo/.deploy-rev), " +
        "then poll until gateway /health OK and deploy rev matches (default wait 180s)",
      inputSchema: z.object({
        strategy: z.enum(["cross", "pi_native"]).default("cross"),
        wait_for_ready: z
          .boolean()
          .default(true)
          .describe("Poll Pi until gateway health + deploy rev match"),
        wait_timeout_sec: z.number().int().min(30).max(600).default(180),
      }),
      handler: async (args: {
        strategy: "cross" | "pi_native";
        wait_for_ready: boolean;
        wait_timeout_sec: number;
      }) => {
        return runSyncMain(cfg, runRemote, args.strategy, {
          waitForReady: args.wait_for_ready,
          waitTimeoutSec: args.wait_timeout_sec,
        });
      },
    },

    pi_wait_deploy: {
      description:
        "Poll Pi until .deploy-rev matches expected git SHA prefix and marengo-gateway /health OK",
      inputSchema: z.object({
        expected_rev: z
          .string()
          .min(7)
          .describe("Full or short git SHA written to .deploy-rev"),
        wait_timeout_sec: z.number().int().min(10).max(600).default(180),
      }),
      handler: async (args: { expected_rev: string; wait_timeout_sec: number }) => {
        const wait = await waitForDeployReady(cfg, args.expected_rev, {
          timeoutMs: args.wait_timeout_sec * 1000,
        });
        if (!wait.ready) {
          return `[timeout after ${args.wait_timeout_sec}s]\n\n${wait.log}`;
        }
        return `[ready]\n\n${wait.log}`;
      },
    },

    pi_sync_bench_config: {
      description:
        "Rsync a bringup profile (control.yaml, motors.yaml, robot.yaml, homing.yaml) from local repo to Pi. " +
        "Use after editing config on Mac; sets ~/marengo and optionally /opt/marengo.",
      inputSchema: syncBenchConfigSchema,
      handler: async (args: {
        profile?: string;
        install_to_opt?: boolean;
      }) => {
        return runSyncBenchConfig(cfg, runRemote, {
          profile: args.profile ?? "shoulder_pitch_right_only",
          install_to_opt: args.install_to_opt ?? true,
        });
      },
    },

    pi_install_staging: {
      description:
        "Install ~/marengo staging tree into /opt/marengo via passwordless sudo (install-pi.sh). " +
        "Run after scp/manual staging or when /opt lags ~/marengo.",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(cfg, sudoStagingInstallCommand(cfg));
        return runRemote(body, 120_000);
      },
    },

    pi_git_pull: {
      description: "git pull in MARENGO_PI_ROOT on Pi (fails if dirty)",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(
          cfg,
          [
            "if ! git diff --quiet || ! git diff --cached --quiet; then",
            "  echo 'dirty working tree — commit or stash first' >&2",
            "  git status --short",
            "  exit 1",
            "fi",
            "git pull --ff-only",
          ].join("\n"),
        );
        return runRemote(body, 120_000);
      },
    },

    pi_build: {
      description: "Native cargo build on Pi + install-pi.sh (slow fallback)",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(
          cfg,
        [
          'if [[ -f "${HOME}/.cargo/env" ]]; then set -a; source "${HOME}/.cargo/env"; set +a; fi',
          'export PATH="${HOME}/.cargo/bin:/usr/local/cargo/bin:${PATH:-}"',
          "command -v cargo >/dev/null || { echo 'error: cargo not on PATH'; exit 127; }",
          "cargo build -p marengo-pi -p marengo-gateway -p motor-repl -p imu-probe --features socketcan,linux-i2c --release",
          sudoInstallCommand(cfg),
        ].join("\n"),
        );
        return runRemote(body, 900_000);
      },
    },
  };
}

export type AdminTools = ReturnType<typeof registerAdminTools>;
