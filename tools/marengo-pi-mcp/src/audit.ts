import { appendFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { auditLogPath } from "./config.js";

export interface AuditEntry {
  ts: string;
  tool: string;
  args: Record<string, unknown>;
  bench_profile?: string;
  confirm_weighted_motion?: boolean;
  stdout?: string;
  stderr?: string;
  exitCode?: number;
}

export function appendAudit(entry: Omit<AuditEntry, "ts">): void {
  const logPath = auditLogPath();
  mkdirSync(path.dirname(logPath), { recursive: true });
  const line: AuditEntry = { ts: new Date().toISOString(), ...entry };
  appendFileSync(logPath, `${JSON.stringify(line)}\n`, "utf8");
}
