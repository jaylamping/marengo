import { z } from "zod";
import type { MarengoPiConfig } from "../config.js";
import { wrapRemote } from "../env.js";

export function registerLogTools(
  cfg: MarengoPiConfig,
  runRemote: (body: string, timeoutMs?: number) => Promise<string>,
) {
  const logDir = `${cfg.piRoot}/var/log`;

  return {
    pi_logs_tail: {
      description: "Last N lines of bench-latest.log on Pi",
      inputSchema: z.object({
        lines: z.number().int().min(1).max(2000).default(100),
      }),
      handler: async (args: { lines: number }) => {
        const body = wrapRemote(
          cfg,
          `tail -n ${args.lines} ${logDir}/bench-latest.log 2>&1 || echo '(no bench-latest.log — run harness or pi_marengo_pi_script first)'`,
        );
        return runRemote(body, 15_000);
      },
    },

    pi_logs_grep: {
      description: "Regex grep over bench-latest.log or last K bench logs",
      inputSchema: z.object({
        pattern: z.string().describe("Extended regex (-E)"),
        last_files: z.number().int().min(1).max(20).default(1),
      }),
      handler: async (args: { pattern: string; last_files: number }) => {
        const pat = args.pattern.replace(/'/g, `'\\''`);
        const body = wrapRemote(
          cfg,
          [
            `LOGDIR=${JSON.stringify(logDir)}`,
            `if [[ ${args.last_files} -eq 1 ]]; then`,
            `  grep -E '${pat}' "$LOGDIR/bench-latest.log" 2>/dev/null || echo '(no matches or no log)'`,
            "else",
            `  ls -t "$LOGDIR"/bench-*.log 2>/dev/null | head -n ${args.last_files} | while read -r f; do`,
            `    echo "=== $f ==="`,
            `    grep -E '${pat}' "$f" || true`,
            "  done",
            "fi",
          ].join("\n"),
        );
        return runRemote(body, 30_000);
      },
    },

    pi_logs_list: {
      description: "List recent bench-*.log and bench-*.json with mtimes",
      inputSchema: z.object({
        limit: z.number().int().min(1).max(50).default(15),
      }),
      handler: async (args: { limit: number }) => {
        const body = wrapRemote(
          cfg,
          `ls -lt ${logDir}/bench-*.log ${logDir}/bench-*.json 2>/dev/null | head -n ${args.limit} || echo '(no bench logs yet)'`,
        );
        return runRemote(body, 15_000);
      },
    },

    pi_logs_last_fault: {
      description:
        "ERROR/WARN/fault/watchdog/limit lines from latest bench session",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(
          cfg,
          `grep -E 'ERROR|WARN|fault|watchdog|outside|limit' ${logDir}/bench-latest.log 2>/dev/null | tail -n 80 || echo '(no fault lines or no bench-latest.log)'`,
        );
        return runRemote(body, 15_000);
      },
    },
  };
}

export type LogTools = ReturnType<typeof registerLogTools>;
