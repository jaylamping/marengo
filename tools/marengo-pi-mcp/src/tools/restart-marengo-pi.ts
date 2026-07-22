import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { wrapRemote } from "../env.js";

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

/** Remote shell body (no preamble). Exported for unit tests. */
export function restartMarengoPiShell(mode: "restart" | "stop"): string {
  const lines = [
    "echo '=== before ==='",
    "systemctl is-active marengo-pi.service 2>/dev/null || echo inactive",
    "pgrep -af '/opt/marengo/bin/marengo-pi|/bin/marengo-pi' || echo '(no marengo-pi process)'",
    "echo",
    "echo '=== stop ==='",
    "sudo systemctl stop marengo-pi.service 2>/dev/null || true",
    "sudo pkill -f '/opt/marengo/bin/marengo-pi' 2>/dev/null || true",
    "pkill -f '/opt/marengo/bin/marengo-pi' 2>/dev/null || true",
    "for i in 1 2 3 4 5 6 7 8 9 10; do",
    "  pgrep -f '/opt/marengo/bin/marengo-pi' >/dev/null 2>&1 || break",
    "  sleep 0.2",
    "done",
  ];

  if (mode === "restart") {
    lines.push(
      "echo",
      "echo '=== start ==='",
      "if systemctl cat marengo-pi.service >/dev/null 2>&1; then",
      "  sudo systemctl start marengo-pi.service",
      "  sleep 1",
      "  systemctl is-active marengo-pi.service || true",
      "else",
      "  echo 'error: marengo-pi.service unit not found — process stopped; start manually' >&2",
      "  exit 1",
      "fi",
    );
  } else {
    lines.push("echo", "echo '=== stop-only (not starting systemd unit) ==='");
  }

  lines.push(
    "echo",
    "echo '=== after ==='",
    "systemctl is-active marengo-pi.service 2>/dev/null || echo inactive",
    "pgrep -af '/opt/marengo/bin/marengo-pi|/bin/marengo-pi' || echo '(no marengo-pi process)'",
    "echo",
    "echo 'Hard limits / motors.yaml reload on marengo-pi process start.'",
  );

  return lines.join("\n");
}

export async function runRestartMarengoPi(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
  args: RestartMarengoPiArgs,
): Promise<string> {
  return runRemote(wrapRemote(cfg, restartMarengoPiShell(args.mode)), 60_000);
}
