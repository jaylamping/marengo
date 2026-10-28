import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import { type MarengoPiConfig } from "../config.js";
import { shellQuote, wrapRemote } from "../env.js";
import { checkManagementGate } from "../management-gate.js";

// A stop is always allowed and never waits for a confirm (TOOL-01); starting the unit does.
export const restartMarengoPiSchema = z
  .object({
    confirm: z
      .literal(true)
      .optional()
      .describe(
        "Must be true for mode=restart: the control loop stops (motors go limp) and restarts. " +
          "Support the arm if elevated. mode=stop needs no confirm.",
      ),
    mode: z
      .enum(["restart", "stop"])
      .default("restart")
      .describe(
        "restart = systemctl stop/pkill then start marengo-pi.service (boot-reload YAML/URDF; not needed after Durable Set Limits Apply); " +
          "stop = stop/pkill only (do not start systemd unit)",
      ),
  })
  .superRefine((args, ctx) => {
    if (args.mode !== "stop" && args.confirm !== true) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        path: ["confirm"],
        message: "confirm: true is required for mode=restart",
      });
    }
  });

export type RestartMarengoPiArgs = z.infer<typeof restartMarengoPiSchema>;

/** Path to the canonical shell (repo scripts/). */
export function restartMarengoPiScriptPath(localRoot: string): string {
  return path.join(localRoot, "scripts", "pi-restart-marengo-pi.sh");
}

/** Load canonical script text from the local Marengo checkout. */
export function loadRestartMarengoPiScript(localRoot: string): string {
  return fs.readFileSync(restartMarengoPiScriptPath(localRoot), "utf8");
}

/** Root-owned helper install-pi.sh grants the deploy user via NOPASSWD sudoers. */
export const INSTALLED_RESTART_HELPER = "/usr/local/libexec/marengo/pi-restart-marengo-pi.sh";

/**
 * Remote shell body: prefer the immutable libexec helper via passwordless sudo.
 * (deploy-user sudoers). Fall back to embedding the local checkout copy.
 */
export function restartMarengoPiShell(
  mode: "restart" | "stop",
  scriptSource: string,
  installedScriptPath = INSTALLED_RESTART_HELPER,
): string {
  const body = scriptSource.replace(/\r\n/g, "\n").replace(/^#![^\n]*\n/, "");
  return [
    `INSTALLED=${shellQuote(installedScriptPath)}`,
    `if [[ -x "$INSTALLED" ]] && sudo -n "$INSTALLED" ${shellQuote(mode)}; then`,
    "  exit 0",
    "fi",
    `echo "warning: passwordless sudo $INSTALLED failed; falling back to embedded script" >&2`,
    `set -- ${shellQuote(mode)}`,
    body,
  ].join("\n");
}

export async function runRestartMarengoPi(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  args: RestartMarengoPiArgs,
): Promise<string> {
  // A stop is never gated; a restart is a management change like the gateway's (P2B-05).
  if (args.mode === "restart") {
    const refusal = await checkManagementGate(cfg, runRemote, "pi_restart_marengo_pi restart");
    if (refusal) return refusal;
  }
  const script = loadRestartMarengoPiScript(cfg.localRoot);
  return runRemote(
    wrapRemote(cfg, restartMarengoPiShell(args.mode, script)),
    60_000,
  );
}
