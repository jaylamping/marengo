import { execFile, spawn } from "node:child_process";
import { delimiter } from "node:path";
import { promisify } from "node:util";
import type { MarengoPiConfig } from "./config.js";
import { sshTarget } from "./config.js";

const execFileAsync = promisify(execFile);
const LOCAL_TOOL_PATHS = [
  "/opt/homebrew/opt/rustup/bin",
  "/opt/homebrew/bin",
  "/usr/local/bin",
];

export interface RemoteExecResult {
  stdout: string;
  stderr: string;
  exitCode: number;
  /** The signal fired while ssh ran and the local ssh was killed. */
  cancelled?: boolean;
}

export interface RemoteExecOptions {
  timeoutMs?: number;
  cwd?: string;
  env?: NodeJS.ProcessEnv;
  /** Abort: refuse to start, or kill the local ssh (remote cleanup is the caller's). */
  signal?: AbortSignal;
  /** Passed as `bash -s <tag>` so the remote process can be found by argv. */
  sessionTag?: string;
}

function sshArgs(cfg: MarengoPiConfig, sessionTag?: string): string[] {
  const args = ["-o", "BatchMode=yes", "-o", "ConnectTimeout=15"];
  if (cfg.sshIdentityFile) {
    args.push("-i", cfg.sshIdentityFile);
  }
  args.push(sshTarget(cfg), "bash", "-s");
  if (sessionTag) args.push(sessionTag);
  return args;
}

/** Run a script on the Pi via `ssh … bash -s` (stdin). Avoids OpenSSH splitting `-lc` words. */
export function execRemote(
  cfg: MarengoPiConfig,
  remoteScript: string,
  opts: RemoteExecOptions = {},
): Promise<RemoteExecResult> {
  const timeoutMs = opts.timeoutMs ?? 120_000;
  if (opts.signal?.aborted) {
    return Promise.resolve({ stdout: "", stderr: "cancelled before start", exitCode: 130 });
  }

  return new Promise((resolve) => {
    const child = spawn("ssh", sshArgs(cfg, opts.sessionTag), { stdio: ["pipe", "pipe", "pipe"] });
    let stdout = "";
    let stderr = "";
    let timedOut = false;
    let cancelled = false;

    const timer = setTimeout(() => {
      timedOut = true;
      child.kill("SIGTERM");
    }, timeoutMs);
    const onAbort = () => {
      cancelled = true;
      child.kill("SIGTERM");
    };
    opts.signal?.addEventListener("abort", onAbort, { once: true });

    child.stdout.on("data", (chunk: Buffer) => {
      stdout += chunk.toString();
    });
    child.stderr.on("data", (chunk: Buffer) => {
      stderr += chunk.toString();
    });

    child.on("error", (err) => {
      clearTimeout(timer);
      opts.signal?.removeEventListener("abort", onAbort);
      resolve({
        stdout,
        stderr: stderr || err.message,
        exitCode: 1,
      });
    });

    child.on("close", (code) => {
      clearTimeout(timer);
      opts.signal?.removeEventListener("abort", onAbort);
      if (cancelled) {
        resolve({ stdout, stderr: `${stderr}cancelled`, exitCode: 130, cancelled: true });
        return;
      }
      if (timedOut) {
        resolve({
          stdout,
          stderr: stderr || `ssh timed out after ${timeoutMs}ms`,
          exitCode: 124,
        });
        return;
      }
      resolve({
        stdout,
        stderr,
        exitCode: code ?? 1,
      });
    });

    child.stdin.write(remoteScript);
    child.stdin.end();
  });
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
      env: opts.env ?? localExecEnv(),
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

export function localExecEnv(): NodeJS.ProcessEnv {
  const home = process.env.HOME ?? process.env.USERPROFILE ?? "";
  const homeCargoBin = home
    ? [`${home}/.cargo/bin`, `${home}\\.cargo\\bin`]
    : [];
  const pathEntries = [
    ...LOCAL_TOOL_PATHS,
    ...homeCargoBin,
    process.env.PATH ?? "",
  ].filter((entry) => entry.length > 0);

  return {
    ...process.env,
    PATH: pathEntries.join(delimiter),
  };
}

export function formatRemoteResult(r: RemoteExecResult): string {
  const parts: string[] = [];
  if (r.stdout.trim()) parts.push(r.stdout.trimEnd());
  if (r.stderr.trim()) parts.push(`[stderr]\n${r.stderr.trimEnd()}`);
  if (r.exitCode !== 0) parts.push(`[exit ${r.exitCode}]`);
  return parts.join("\n\n") || "(no output)";
}

/** Exit code carried by {@link formatRemoteResult}'s trailing `[exit N]` marker (0 when absent). */
export function exitCodeOfRemoteOutput(output: string): number {
  const match = /\[exit (-?\d+)\]\s*$/.exec(output);
  return match ? Number(match[1]) : 0;
}
