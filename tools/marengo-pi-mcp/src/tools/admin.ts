import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { sudoCanUpCommand, sudoInstallCommand, sudoStagingInstallCommand } from "../config.js";
import { wrapRemote } from "../env.js";
import { runSyncMain } from "./deploy.js";
import { waitForDeployReady } from "./deploy-wait.js";
import { cleanTreeSchema, runCleanTree } from "./clean-tree.js";
import {
  runSyncBenchConfig,
  runSyncBenchUrdfAssets,
  syncBenchConfigSchema,
  syncBenchUrdfSchema,
} from "./sync-config.js";
import { syncTreeSchema, runSyncTree } from "./sync-tree.js";

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

    pi_sync_tree: {
      description:
        "Sync the Marengo Pi working tree with origin/main: fetch, checkout main, pull --ff-only. " +
        "Fails if the Pi working tree is dirty. Does not build or install.",
      inputSchema: syncTreeSchema,
      handler: async () => {
        return runSyncTree(cfg, runRemote);
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
          profile: args.profile ?? "arm_4dof_right",
          install_to_opt: args.install_to_opt ?? true,
        });
      },
    },

    pi_sync_bench_urdf: {
      description:
        "Rsync selected bench URDF assets from local assets/urdf to the Pi. " +
        "Use after editing bench URDF COM/mass assets; sets ~/marengo and optionally /opt/marengo.",
      inputSchema: syncBenchUrdfSchema,
      handler: async (args: z.infer<typeof syncBenchUrdfSchema>) => {
        return runSyncBenchUrdfAssets(cfg, runRemote, {
          assets: args.assets,
          install_to_opt: args.install_to_opt,
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

    pi_clean_tree: {
      description:
        "Clean the Marengo Pi working tree so pi_sync_main / pi_git_pull can run. " +
        "Default mode stashes changes; use reset-hard or clean-untracked to discard. " +
        "Requires confirm: true.",
      inputSchema: cleanTreeSchema,
      handler: async (args: { confirm: true; mode: "stash" | "reset-hard" | "clean-untracked" }) => {
        return runCleanTree(cfg, runRemote, args);
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
          "sudo systemctl stop marengo-pi.service 2>/dev/null || true",
          "sudo systemctl disable marengo-pi.service 2>/dev/null || true",
          "sudo pkill -f /opt/marengo/bin/marengo-pi 2>/dev/null || true",
          "sudo git config --global --add safe.directory \"$(pwd)\" 2>/dev/null || true",
          "if [[ -x ./scripts/pi-native-build.sh ]]; then ./scripts/pi-native-build.sh; else",
          '  if [[ -f "${HOME}/.cargo/env" ]]; then set -a; source "${HOME}/.cargo/env"; set +a; fi',
          '  export PATH="${HOME}/.cargo/bin:/usr/local/cargo/bin:${PATH:-}"',
          "  command -v cargo >/dev/null || { echo 'error: cargo not on PATH'; exit 127; }",
          "  cargo build -p marengo-pi -p marengo-gateway -p marengo-log-cli -p motor-repl -p imu-probe --features socketcan,linux-i2c --release",
          "  if command -v npm >/dev/null && [[ -f consul/package-lock.json ]]; then",
          "    (cd consul && npm ci && env -u VITE_CHAPPE_HTTP_URL -u VITE_CHAPPE_WEBTRANSPORT_URL npm run build)",
          "  fi",
          "fi",
          sudoInstallCommand(cfg),
          'SHA="$(git rev-parse HEAD)"; TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"; printf "%s %s\\n" "$SHA" "$TS" | sudo tee /opt/marengo/.deploy-rev >/dev/null',
          "if [[ -f consul/dist/index.html ]]; then sudo rsync -a --delete consul/dist/ /opt/marengo/www/; fi",
          "sudo systemctl restart marengo-gateway.service 2>/dev/null || true",
        ].join("\n"),
        );
        return runRemote(body, 900_000);
      },
    },
  };
}

export type AdminTools = ReturnType<typeof registerAdminTools>;
