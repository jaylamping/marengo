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
      description:
        "List recent bench sessions (gateway SQL when reachable, else hot files)",
      inputSchema: z.object({
        limit: z.number().int().min(1).max(50).default(15),
      }),
      handler: async (args: { limit: number }) => {
        const gw = `http://${cfg.host}:8080`;
        const body = wrapRemote(
          cfg,
          [
            `if curl -sf "${gw}/logs/sessions?limit=${args.limit}" >/tmp/m-sessions.json 2>/dev/null; then`,
            '  echo "=== gateway sessions ==="',
            '  cat /tmp/m-sessions.json',
            '  echo ""',
            "fi",
            `LOGDIR=${JSON.stringify(logDir)}`,
            'du -sh "$LOGDIR" 2>/dev/null || true',
            'echo "=== hot files ==="',
            'ls -lt "$LOGDIR"/bench-*.log "$LOGDIR"/bench-*.json "$LOGDIR"/position-trace-*.csv "$LOGDIR"/candump-*.log 2>/dev/null',
            `  | head -n ${args.limit} || echo '(no bench logs yet)'`,
          ].join("\n"),
        );
        return runRemote(body, 20_000);
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

    pi_logs_archive_list: {
      description: "List archived bench sessions via marengo-log-cli / gateway SQL store",
      inputSchema: z.object({
        limit: z.number().int().min(1).max(50).default(15),
      }),
      handler: async (args: { limit: number }) => {
        const gw = `http://${cfg.host}:8080`;
        const body = wrapRemote(
          cfg,
          [
            `if curl -sf "${gw}/logs/sessions?limit=${args.limit}" >/tmp/m-sessions.json 2>/dev/null; then`,
            '  cat /tmp/m-sessions.json',
            "else",
            `  marengo-log-cli archive --keep 50 2>/dev/null || true`,
            `  ls -lt ${logDir}/blobs/*/* 2>/dev/null | head -n ${args.limit} || echo '(no archived sessions)'`,
            "fi",
          ].join("\n"),
        );
        return runRemote(body, 20_000);
      },
    },

    pi_candump_summary: {
      description:
        "Summarize candump-latest.log: frame count, duration, approximate Hz (after pi_hold_on/harness)",
      inputSchema: z.object({}),
      handler: async () => {
        const body = wrapRemote(
          cfg,
          [
            `F=${JSON.stringify(`${logDir}/candump-latest.log`)}`,
            'if ! test -f "$F"; then',
            '  echo "(no candump-latest.log — run pi_hold_on or pi_bench_harness first)"',
            "  exit 0",
            "fi",
            'lines=$(wc -l < "$F" | tr -d " ")',
            'bytes=$(wc -c < "$F" | tr -d " ")',
            'echo "file=$F lines=$lines bytes=$bytes"',
            'first=$(grep -m1 -E "^[[:space:]]*\\(" "$F" 2>/dev/null || true)',
            'last=$(grep -E "^[[:space:]]*\\(" "$F" | tail -n 1 || true)',
            'echo "first=$first"',
            'echo "last=$last"',
            "if [ -n \"$first\" ] && [ -n \"$last\" ]; then",
            '  t0=$(echo "$first" | sed -n "s/^[[:space:]]*(\\([0-9.]*\\)).*/\\1/p")',
            '  t1=$(echo "$last" | sed -n "s/^[[:space:]]*(\\([0-9.]*\\)).*/\\1/p")',
            '  if [ -n "$t0" ] && [ -n "$t1" ]; then',
            '    dur=$(awk -v t0="$t0" -v t1="$t1" \'BEGIN { d=t1-t0; if (d>0) printf "%.3f", d; else print "0" }\')',
            '    hz=$(awk -v n="$lines" -v t0="$t0" -v t1="$t1" \'BEGIN { d=t1-t0; if (d>0) printf "%.1f", n/d; else print "n/a" }\')',
            '    echo "duration_sec=$dur approx_hz=$hz"',
            '  fi',
            "fi",
            'echo "--- per-interface ---"',
            'for _if in can0 can1 can2; do',
            '  c=$(grep -c " ${_if} " "$F" 2>/dev/null || true)',
            '  c=${c:-0}',
            '  if [ "$c" -gt 0 ] 2>/dev/null; then echo "${_if}_frames=$c"; fi',
            "done",
            'echo "--- top CAN IDs ---"',
            "awk '/^[[:space:]]*\\(/ { ids[$3]++ } END { for (id in ids) print ids[id], id }' \"$F\" | sort -nr | head -n 10",
          ].join("\n"),
        );
        return runRemote(body, 15_000);
      },
    },
  };
}

export type LogTools = ReturnType<typeof registerLogTools>;
