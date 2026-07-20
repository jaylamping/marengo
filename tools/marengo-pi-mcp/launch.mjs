#!/usr/bin/env node
/**
 * Cross-platform MCP entry for Cursor.
 * Prefer this over bare `sh`/`bash`: Cursor's spawn PATH often lacks Git shims
 * on Windows (and sometimes mise `node`). Defaults stay HERE — not in mcp.json.
 */
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = path.dirname(fileURLToPath(import.meta.url));
const home = homedir();

function setDefault(key, value) {
  if (!process.env[key]?.trim()) process.env[key] = value;
}

setDefault("MARENGO_PI_HOST", "joey-robot.tail0b414.ts.net");
setDefault("MARENGO_PI_USER", "joey");
setDefault("MARENGO_PI_ROOT", "/opt/marengo");
setDefault(
  "MARENGO_CONFIG_DIR",
  "/opt/marengo/config/bringup/arm_4dof_right",
);
setDefault("MARENGO_BENCH_PROFILE", "elbow_attached");

if (!process.env.SSH_IDENTITY_FILE?.trim()) {
  for (const candidate of [
    path.join(home, ".ssh", "id_ed25519_marengo"),
    path.join(home, ".ssh", "id_ed25519"),
  ]) {
    if (existsSync(candidate)) {
      process.env.SSH_IDENTITY_FILE = candidate;
      break;
    }
  }
}

// Help child `ssh` / tools see mise when Cursor's PATH is minimal.
const extra = [
  path.join(home, ".local", "share", "mise", "shims"),
  path.join(home, "AppData", "Local", "mise", "shims"),
  path.join(home, "AppData", "Local", "mise", "installs", "node", "24.16.0"),
  "C:\\Program Files\\Git\\bin",
  "C:\\Program Files\\Git\\cmd",
].filter((p) => existsSync(p));
if (extra.length) {
  process.env.PATH = `${extra.join(path.delimiter)}${path.delimiter}${process.env.PATH || ""}`;
}

const entry = path.join(root, "dist", "index.js");
if (!existsSync(entry)) {
  console.error(
    `launch.mjs: missing ${entry} — run \`just mcp-build\` (or npm run build in tools/marengo-pi-mcp).`,
  );
  process.exit(1);
}

await import(pathToFileURL(entry).href);
