#!/usr/bin/env node
/**
 * Cross-platform research MCP entry (avoids `sh` missing from Cursor PATH on Windows).
 *
 * Run: node --experimental-strip-types launch.ts
 */
import { spawn, spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(root, "..", "..");
const home = homedir();

if (!process.env.MARENGO_RESEARCH_CACHE_DIR?.trim()) {
  process.env.MARENGO_RESEARCH_CACHE_DIR = path.join(repoRoot, ".marengo-research");
}
if (!process.env.MARENGO_WORKSPACE?.trim()) {
  process.env.MARENGO_WORKSPACE = repoRoot;
}

const extra = [
  path.join(home, ".local", "share", "mise", "shims"),
  path.join(home, "AppData", "Local", "mise", "shims"),
  path.join(home, ".cargo", "bin"),
  path.join(home, ".local", "bin"),
  "C:\\Program Files\\Git\\bin",
  "C:\\Program Files\\Git\\cmd",
].filter((p) => existsSync(p));
if (extra.length) {
  process.env.PATH = `${extra.join(path.delimiter)}${path.delimiter}${process.env.PATH || ""}`;
}

if (!process.env.GITHUB_TOKEN?.trim()) {
  const tok = spawnSync("gh", ["auth", "token"], {
    encoding: "utf8",
    shell: process.platform === "win32",
    windowsHide: true,
  });
  if (tok.status === 0 && tok.stdout?.trim()) {
    process.env.GITHUB_TOKEN = tok.stdout.trim();
  }
}

const child = spawn(
  "uv",
  ["run", "python", "-m", "marengo_research_mcp.server"],
  {
    cwd: root,
    env: process.env,
    stdio: "inherit",
    shell: process.platform === "win32",
    windowsHide: true,
  },
);

child.on("error", (err: Error) => {
  console.error(
    `launch: failed to start uv (${err.message}). Run: just research-mcp-setup`,
  );
  process.exit(127);
});

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
    return;
  }
  process.exit(code ?? 1);
});
