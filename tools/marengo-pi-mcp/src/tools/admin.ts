import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { sudoCanUpCommand, sudoInstallCommand } from "../config.js";
import { wrapRemote } from "../env.js";
import { runSyncMain } from "./deploy.js";

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
        "Sync local main, cross-build, deploy to Pi, install-pi, write .deploy-rev",
      inputSchema: z.object({
        strategy: z.enum(["cross", "pi_native"]).default("cross"),
      }),
      handler: async (args: { strategy: "cross" | "pi_native" }) => {
        return runSyncMain(cfg, runRemote, args.strategy);
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
            "cargo build -p marengo-pi -p motor-repl --features socketcan --release",
            sudoInstallCommand(cfg),
          ].join("\n"),
        );
        return runRemote(body, 900_000);
      },
    },
  };
}

export type AdminTools = ReturnType<typeof registerAdminTools>;
