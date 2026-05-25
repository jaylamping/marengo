import { execFile } from "node:child_process";
import { promisify } from "node:util";
import type { MarengoPiConfig } from "./config.js";
import { sshTarget } from "./config.js";

const execFileAsync = promisify(execFile);

export interface RemoteExecResult {
  stdout: string;
  stderr: string;
  exitCode: number;
}

export interface RemoteExecOptions {
  timeoutMs?: number;
  cwd?: string;
}

export async function execRemote(
  cfg: MarengoPiConfig,
  remoteScript: string,
  opts: RemoteExecOptions = {},
): Promise<RemoteExecResult> {
  const timeoutMs = opts.timeoutMs ?? 120_000;
  const args = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=15"];
  if (cfg.sshIdentityFile) {
    args.push("-i", cfg.sshIdentityFile);
  }
  args.push(sshTarget(cfg), "bash", "-lc", remoteScript);

  try {
    const { stdout, stderr } = await execFileAsync("ssh", args, {
      maxBuffer: 16 * 1024 * 1024,
      timeout: timeoutMs,
    });
    return { stdout: stdout ?? "", stderr: stderr ?? "", exitCode: 0 };
  } catch (err: unknown) {
    const e = err as {
      stdout?: string;
      stderr?: string;
      code?: number;
      message?: string;
    };
    const stdout = e.stdout ?? "";
    const stderr = e.stderr ?? e.message ?? String(err);
    const exitCode = typeof e.code === "number" ? e.code : 1;
    return { stdout, stderr, exitCode };
  }
}

export async function execLocal(
  command: string,
  args: string[],
  opts: RemoteExecOptions = {},
): Promise<RemoteExecResult> {
  const timeoutMs = opts.timeoutMs ?? 600_000;
  try {
    const { stdout, stderr } = await execFileAsync(command, args, {
      cwd: opts.cwd,
      maxBuffer: 16 * 1024 * 1024,
      timeout: timeoutMs,
    });
    return { stdout: stdout ?? "", stderr: stderr ?? "", exitCode: 0 };
  } catch (err: unknown) {
    const e = err as {
      stdout?: string;
      stderr?: string;
      code?: number;
      message?: string;
    };
    return {
      stdout: e.stdout ?? "",
      stderr: e.stderr ?? e.message ?? String(err),
      exitCode: typeof e.code === "number" ? e.code : 1,
    };
  }
}

export function formatRemoteResult(r: RemoteExecResult): string {
  const parts: string[] = [];
  if (r.stdout.trim()) parts.push(r.stdout.trimEnd());
  if (r.stderr.trim()) parts.push(`[stderr]\n${r.stderr.trimEnd()}`);
  if (r.exitCode !== 0) parts.push(`[exit ${r.exitCode}]`);
  return parts.join("\n\n") || "(no output)";
}
