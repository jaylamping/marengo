import fs from "node:fs";
import path from "node:path";
import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { shellQuote, wrapRemote } from "../env.js";

export const restartMarengoPiSchema = z.object({
  confirm: z
    .literal(true)
    .describe(
      "Must be true — stops the control loop (motors go limp). Support the arm if elevated.",
    ),
  mode: z
    .enum(["restart", "stop"])
    .default("restart")
    .describe(
      "restart = systemctl stop/pkill then start marengo-pi.service (reloads motors.yaml hard limits); " +
        "stop = stop/pkill only (do not start systemd unit)",
    ),
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

/**
 * Remote shell body: run the canonical script with mode as $1.
 * Embeds the local file so the tool works before the Pi has the new script installed.
 */
export function restartMarengoPiShell(
  mode: "restart" | "stop",
  scriptSource: string,
): string {
  const body = scriptSource.replace(/\r\n/g, "\n").replace(/^#![^\n]*\n/, "");
  return [`set -- ${shellQuote(mode)}`, body].join("\n");
}

export async function runRestartMarengoPi(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  args: RestartMarengoPiArgs,
): Promise<string> {
  const script = loadRestartMarengoPiScript(cfg.localRoot);
  return runRemote(wrapRemote(cfg, restartMarengoPiShell(args.mode, script)), 60_000);
}
